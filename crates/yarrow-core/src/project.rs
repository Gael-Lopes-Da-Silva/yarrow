//! Multi-root project check beyond a single file’s nested `require` (Stage 28).
//!
//! A project is an explicit set of root sources that share module search paths.
//! Each root is still a normal compilation unit (`require` closure + optional
//! entry). There is no package-manager or lifetime syntax here; see
//! [`docs/RUNTIME.md`](../../../docs/RUNTIME.md) (Projects).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::compiler::modules::ModuleLoader;
use crate::diagnostics::{DEFAULT_ERROR_LIMIT, Diagnostic, DiagnosticBatch, SourceFile, Span};
use crate::parser::ast::{Program, Stmt, StmtKind};
use crate::parser::parse;
use crate::session::{CheckedProgram, CompileOptions, ExecutionMode, Session, SessionDiagnostics};
use crate::tokenizer::Tokenizer;

/// One root source in a multi-file project.
#[derive(Debug, Clone)]
pub struct ProjectRoot {
    /// Path shown in diagnostics and used for module-relative imports.
    pub path: String,
    /// File contents.
    pub source: String,
}

/// Options for [`check_project`].
///
/// Roots share [`Self::module_search_paths`]. Each root’s directory is also
/// added as a search path (same rule as single-file [`Session::check_source`]).
#[derive(Debug, Clone)]
pub struct ProjectOptions {
    pub roots: Vec<ProjectRoot>,
    pub module_search_paths: Vec<PathBuf>,
    /// When true, every root must define [`Self::entry_name`] (`E360`).
    pub require_main: bool,
    pub entry_name: String,
    pub error_limit: usize,
}

impl ProjectOptions {
    /// Build options from on-disk root paths (read into memory).
    ///
    /// Missing or unreadable paths fail with `E383`.
    pub fn from_root_paths(
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<Self, SessionDiagnostics> {
        let mut roots = Vec::new();
        for p in paths {
            let path = p.as_ref();
            let display = path.display().to_string();
            let source = std::fs::read_to_string(path).map_err(|e| {
                project_diag(
                    &display,
                    String::new(),
                    "E383",
                    format!("project root not found or unreadable: {display} ({e})"),
                    "pass an existing `.yar` path, or supply `ProjectRoot::source` in memory",
                )
            })?;
            roots.push(ProjectRoot {
                path: display,
                source,
            });
        }
        Ok(Self {
            roots,
            module_search_paths: Vec::new(),
            require_main: true,
            entry_name: crate::DEFAULT_ENTRY_NAME.to_string(),
            error_limit: DEFAULT_ERROR_LIMIT,
        })
    }
}

/// One `require` edge in the project module graph.
#[derive(Debug, Clone)]
pub struct ModuleGraphEdge {
    /// Root file path or dotted module path that issued the `require`.
    pub from: String,
    /// Resolved module path that was loaded.
    pub to: String,
    /// Path string as written in source.
    pub require_path: String,
    pub span: Span,
}

/// Require closure across all project roots (shared modules appear once).
#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    /// Module paths reached from any root (dependency order, deps first).
    pub modules: Vec<String>,
    pub edges: Vec<ModuleGraphEdge>,
}

/// Successful multi-root check: each root’s [`CheckedProgram`] plus the union graph.
#[derive(Debug, Clone)]
pub struct CheckedProject {
    pub roots: Vec<CheckedProgram>,
    pub graph: ModuleGraph,
}

/// Type-check every project root, sharing search paths and deduplicating the
/// require graph metadata.
///
/// Each root is checked with the same pipeline as [`Session::check_source`].
/// Shared helpers reached by `require` from more than one root are re-checked
/// per root (safe isolation); [`ModuleGraph`] records the union of edges.
///
/// Failures:
/// - empty `roots` → `E383`
/// - missing on-disk root via [`ProjectOptions::from_root_paths`] → `E383`
/// - require cycle → `E382` (from the compiler)
/// - unknown module → `E380`
pub fn check_project(options: &ProjectOptions) -> Result<CheckedProject, SessionDiagnostics> {
    if options.roots.is_empty() {
        return Err(project_diag(
            "<project>",
            String::new(),
            "E383",
            "project has no roots",
            "add at least one root `.yar` path or `ProjectRoot`",
        ));
    }

    let mut checked_roots = Vec::with_capacity(options.roots.len());
    for root in &options.roots {
        let mut opts = CompileOptions::new(root.path.clone());
        opts.module_search_paths = options.module_search_paths.clone();
        opts.require_main = options.require_main;
        opts.entry_name = options.entry_name.clone();
        opts.error_limit = options.error_limit;
        opts.mode = ExecutionMode::Check;
        let session = Session::new(opts);
        checked_roots.push(session.check_source(root.source.clone())?);
    }

    let graph = build_module_graph(options)?;
    Ok(CheckedProject {
        roots: checked_roots,
        graph,
    })
}

fn project_diag(
    path: &str,
    source: String,
    code: &str,
    message: impl Into<String>,
    help: &str,
) -> SessionDiagnostics {
    let file = SourceFile::new(path.to_string(), source);
    let mut batch = DiagnosticBatch::with_limit(DEFAULT_ERROR_LIMIT);
    let diag = Diagnostic::error(code, message)
        .with_path(path.to_string())
        .with_primary(Span::default(), "")
        .with_help(help);
    batch.push(diag);
    SessionDiagnostics { file, batch }
}

/// Walk every root’s `require` closure with the same loader rules as the compiler.
fn build_module_graph(options: &ProjectOptions) -> Result<ModuleGraph, SessionDiagnostics> {
    let mut graph = ModuleGraph::default();
    let mut loaded: HashMap<String, ()> = HashMap::new();
    let mut loading: HashSet<String> = HashSet::new();

    for root in &options.roots {
        let (file, program) = {
            let mut opts = CompileOptions::new(root.path.clone());
            opts.error_limit = options.error_limit;
            let session = Session::new(opts);
            session.parse_source(root.source.clone())?
        };

        let mut loader = ModuleLoader::new();
        if let Some(dir) = Path::new(&root.path).parent()
            && !dir.as_os_str().is_empty()
        {
            loader.add_search_path(dir);
        }
        for p in &options.module_search_paths {
            loader.add_search_path(p.clone());
        }

        if let Err(e) = load_graph_requires(
            &mut loader,
            &program,
            &root.path,
            &mut loaded,
            &mut loading,
            &mut graph,
        ) {
            return Err(SessionDiagnostics {
                file,
                batch: {
                    let mut batch = DiagnosticBatch::with_limit(options.error_limit);
                    batch.push((*e.diagnostic).clone());
                    batch
                },
            });
        }
    }

    Ok(graph)
}

fn load_graph_requires(
    loader: &mut ModuleLoader,
    program: &Program,
    from: &str,
    loaded: &mut HashMap<String, ()>,
    loading: &mut HashSet<String>,
    graph: &mut ModuleGraph,
) -> Result<(), crate::compiler::CompileError> {
    load_graph_stmts(loader, &program.items, from, loaded, loading, graph)
}

fn load_graph_stmts(
    loader: &mut ModuleLoader,
    stmts: &[Stmt],
    from: &str,
    loaded: &mut HashMap<String, ()>,
    loading: &mut HashSet<String>,
    graph: &mut ModuleGraph,
) -> Result<(), crate::compiler::CompileError> {
    for s in stmts {
        match &s.kind {
            StmtKind::Require { path, .. } => {
                load_graph_one(loader, path, s.span, from, loaded, loading, graph)?;
            }
            StmtKind::Function(f) => {
                load_graph_stmts(loader, &f.body, from, loaded, loading, graph)?;
            }
            StmtKind::Implement(imp) => {
                for f in &imp.functions {
                    load_graph_stmts(loader, &f.body, from, loaded, loading, graph)?;
                }
            }
            StmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                load_graph_stmts(loader, then_branch, from, loaded, loading, graph)?;
                load_graph_stmts(loader, else_branch, from, loaded, loading, graph)?;
            }
            StmtKind::For { body, .. }
            | StmtKind::Defer { body }
            | StmtKind::Handle { body, .. }
            | StmtKind::Unsafe { body } => {
                load_graph_stmts(loader, body, from, loaded, loading, graph)?;
            }
            StmtKind::Match {
                cases, else_branch, ..
            } => {
                for c in cases {
                    load_graph_stmts(loader, &c.body, from, loaded, loading, graph)?;
                }
                load_graph_stmts(loader, else_branch, from, loaded, loading, graph)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn load_graph_one(
    loader: &mut ModuleLoader,
    require_path: &str,
    span: Span,
    from: &str,
    loaded: &mut HashMap<String, ()>,
    loading: &mut HashSet<String>,
    graph: &mut ModuleGraph,
) -> Result<(), crate::compiler::CompileError> {
    let (module_path, _) = resolve_require(loader, require_path)?;
    graph.edges.push(ModuleGraphEdge {
        from: from.to_string(),
        to: module_path.clone(),
        require_path: require_path.to_string(),
        span,
    });

    if loaded.contains_key(&module_path) {
        return Ok(());
    }
    if !loading.insert(module_path.clone()) {
        return Err(crate::compiler::CompileError::new(
            format!("module dependency cycle involving '{module_path}'"),
            span,
            "E382",
        )
        .with_help("break the cycle by removing or restructuring one `require`"));
    }

    let source = loader.load_at(&module_path, span)?;
    let tokens = Tokenizer::new(source).tokenize()?;
    let sub = parse(tokens)?;
    load_graph_requires(loader, &sub, &module_path, loaded, loading, graph)?;
    loading.remove(&module_path);
    loaded.insert(module_path.clone(), ());
    if !graph.modules.iter().any(|m| m == &module_path) {
        graph.modules.push(module_path);
    }
    Ok(())
}

fn resolve_require(
    loader: &ModuleLoader,
    path: &str,
) -> Result<(String, Option<String>), crate::compiler::CompileError> {
    if let Some((parent, last)) = path.rsplit_once('.')
        && let Some(parent_source) = loader.try_load(parent)
    {
        let tokens = Tokenizer::new(parent_source).tokenize()?;
        let parent_prog = parse(tokens)?;
        if parent_prog
            .items
            .iter()
            .any(|i| matches!(&i.kind, StmtKind::Function(f) if f.name == last))
        {
            return Ok((parent.to_string(), Some(last.to_string())));
        }
    }
    Ok((path.to_string(), None))
}
