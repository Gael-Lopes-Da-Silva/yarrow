use std::path::{Path, PathBuf};

use crate::compiler::{CompileError, Compiler, RunResult};
use crate::diagnostics::{ColorChoice, Diagnostic, DiagnosticBatch, SourceFile, Span, render};
use crate::interpreter::EvalContext;
use crate::parser::Parser;
use crate::parser::ast::{Program, StmtKind};
use crate::tokenizer::{Token, Tokenizer};

/// How a session turns a checked program into code or executes it.
///
/// `Check` / `Jit` (13a), `Interpret` (13b), and `Object` emit (13c) are landed.
/// Default is [`ExecutionMode::Object`] (Stage 29); set [`ExecutionMode::Jit`]
/// explicitly for in-process JIT / [`Session::compile_source`] / `run_main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// Full type / ownership / stack / region checks; no JIT / object product.
    Check,
    /// Cranelift in-process machine code. Opt in for `compile_source` / `run_main`.
    Jit,
    /// Native relocatable object (AOT). Default for [`CompileOptions::new`].
    /// Use [`Session::compile_object_source`] / [`Session::compile_executable_source`].
    #[default]
    Object,
    /// Stack VM / AST interpreter (`Session::interpret_source`).
    Interpret,
}

/// Cranelift optimization tier for JIT / object / executable backends (Stage 25).
///
/// Maps onto Cranelift `opt_level` settings. Default is [`OptLevel::None`]
/// (debug-friendly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptLevel {
    /// No Cranelift mid-end opts (`opt_level=none`).
    #[default]
    None,
    /// Optimize for speed (`opt_level=speed`).
    Speed,
    /// Optimize for speed and size (`opt_level=speed_and_size`).
    Size,
}

impl OptLevel {
    /// Cranelift settings string for `opt_level`.
    pub fn as_cranelift(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Speed => "speed",
            Self::Size => "speed_and_size",
        }
    }
}

/// Options for one compile/check session.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Source path shown in diagnostics and used for module-relative imports.
    pub source_path: String,
    /// Extra module lookup roots (`"a.b"` => `a/b.yar`).
    pub module_search_paths: Vec<PathBuf>,
    /// Whether this session should require a top-level entry function.
    pub require_main: bool,
    /// Top-level entry function name (default [`crate::DEFAULT_ENTRY_NAME`]).
    ///
    /// Used by require-entry (E360), JIT / interpret `run_main`, and object
    /// emit (CRT trampoline target). CLI `--main` maps here; core does not
    /// parse argv.
    pub entry_name: String,
    /// Maximum number of diagnostics to collect before aborting.
    pub error_limit: usize,
    /// Backend / check mode for this session.
    pub mode: ExecutionMode,
    /// Cranelift optimization tier (JIT, object, and executable).
    pub opt_level: OptLevel,
    /// Emit DWARF into object / executable products (ignored for JIT / check).
    ///
    /// Default `true`: AOT artifacts include compilation units and function
    /// names / line mappings when spans exist.
    pub debug_info: bool,
    /// Object / executable target triple (Stage 26 / 33 / 34).
    ///
    /// `None` means the host. Set to e.g. `aarch64-unknown-linux-gnu`,
    /// `x86_64-unknown-linux-musl`, `x86_64-pc-windows-gnu`, or
    /// `x86_64-apple-darwin` for a non-host object. JIT rejects non-host
    /// triples (`E397`). Mach-O / Windows are object-emit only on linux hosts.
    pub target: Option<crate::target::TargetTriple>,
}

impl CompileOptions {
    pub fn new(source_path: impl Into<String>) -> Self {
        Self {
            source_path: source_path.into(),
            module_search_paths: Vec::new(),
            require_main: true,
            entry_name: crate::DEFAULT_ENTRY_NAME.to_string(),
            error_limit: crate::diagnostics::DEFAULT_ERROR_LIMIT,
            mode: ExecutionMode::Object,
            opt_level: OptLevel::None,
            debug_info: true,
            target: None,
        }
    }

    /// Effective AOT / check triple (`target` or host).
    pub fn effective_target(&self) -> crate::target::TargetTriple {
        self.target
            .clone()
            .unwrap_or_else(crate::target::TargetTriple::host)
    }
}

/// Stateful frontend entry point: tokenize -> parse -> check / compile.
#[derive(Debug, Clone)]
pub struct Session {
    pub options: CompileOptions,
}

/// Program that passed semantic analysis (Stage 13a handoff for later backends).
///
/// Warnings (Stage 20) may be non-empty while this remains `Ok` from
/// [`Session::check_source`]. [`CheckedProgram::type_at`] answers typed hover
/// probes (Stage 30) without re-running the checker.
#[derive(Debug, Clone)]
pub struct CheckedProgram {
    pub file: SourceFile,
    pub program: Program,
    pub warnings: DiagnosticBatch,
    /// Typed sites in the root file (bindings / function signatures).
    pub type_index: crate::TypeIndex,
}

impl CheckedProgram {
    /// Probe the binding / function type at a byte offset in the root file.
    ///
    /// Returns `None` when the offset is outside any recorded site (whitespace,
    /// comments, or code that was not indexed).
    pub fn type_at(&self, offset: usize) -> Option<crate::TypeProbe> {
        self.type_index.type_at(offset)
    }
}

/// Successful JIT session artifact ready for run / IR dump.
pub struct SessionArtifact {
    pub file: SourceFile,
    pub compiler: Compiler,
}

/// Relocatable native object produced by [`Session::compile_object_source`].
///
/// Bytes are ELF / Mach-O / COFF for [`ObjectArtifact::target`]. Host runtime
/// symbols (`print_str`, …) remain unresolved imports until linked with
/// [`crate::linkable_archive_for`]. Process entry is exported as
/// [`crate::PROCESS_MAIN_SYMBOL`] (`main`).
pub struct ObjectArtifact {
    pub file: SourceFile,
    /// Object file bytes (non-empty on success).
    pub bytes: Vec<u8>,
    /// Cranelift IR captured during the same lower pass (debug / dump).
    pub ir: String,
    /// Yarrow entry name that process `main` calls in this object.
    pub entry_name: String,
    /// Triple this object was lowered for.
    pub target: crate::target::TargetTriple,
}

/// Host executable produced by [`Session::compile_executable_source`].
///
/// Linked from the program object (with process `main`) and
/// [`crate::linkable_archive_for`] via a system linker (`ld` / `lld`), not `cc`.
pub struct ExecutableArtifact {
    pub file: SourceFile,
    /// Executable file bytes (non-empty on success).
    pub bytes: Vec<u8>,
    /// Cranelift IR from the object lower pass (dump parity).
    pub ir: String,
    /// Yarrow entry name that process `main` calls.
    pub entry_name: String,
    /// Triple this executable was linked for.
    pub target: crate::target::TargetTriple,
}

/// Diagnostics emitted while tokenizing/parsing/compiling one source file.
#[derive(Debug)]
pub struct SessionDiagnostics {
    pub file: SourceFile,
    pub batch: DiagnosticBatch,
}

impl Session {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    /// Tokenize source text without parsing or compiling.
    pub fn tokenize_source(
        &self,
        source: String,
    ) -> Result<(SourceFile, Vec<Token>), SessionDiagnostics> {
        let path = self.options.source_path.clone();
        let file = SourceFile::new(path.clone(), source.clone());
        match Tokenizer::new(source).tokenize() {
            Ok(tokens) => Ok((file, tokens)),
            Err(e) => Err(SessionDiagnostics {
                file,
                batch: tokenize_batch(e, &path, self.options.error_limit),
            }),
        }
    }

    /// Parse source text without compiling.
    pub fn parse_source(
        &self,
        source: String,
    ) -> Result<(SourceFile, Program), SessionDiagnostics> {
        let (file, program, batch) = self.parse_source_recovering(source)?;
        if !batch.is_empty() {
            return Err(SessionDiagnostics { file, batch });
        }
        Ok((file, program))
    }

    /// Tokenize and parse with recovery: returns the recovered program plus any
    /// syntax diagnostics. Tokenize failure is still `Err`.
    ///
    /// The program may be incomplete when `batch` is non-empty; see
    /// [`Parser::parse_recovering`].
    pub fn parse_source_recovering(
        &self,
        source: String,
    ) -> Result<(SourceFile, Program, DiagnosticBatch), SessionDiagnostics> {
        let (file, tokens) = self.tokenize_source(source)?;
        let (program, batch) =
            Parser::with_error_limit(tokens, self.options.error_limit).parse_recovering();
        Ok((file, program, batch))
    }

    /// Type-check / ownership-check source without a JIT or object product.
    ///
    /// Same semantic pipeline as JIT compile (CLIF lowering for analysis), but
    /// uses an object ISA module with no `define_function` / `define_data` /
    /// finalize (`ExecutionMode::Check`, Stage 24).
    /// On success, [`CheckedProgram::warnings`] may contain Stage 20 / 31 warnings;
    /// they do not fail the check.
    /// they do not turn the result into `Err`.
    pub fn check_source(&self, source: String) -> Result<CheckedProgram, SessionDiagnostics> {
        let (file, program) = self.parse_source(source)?;
        self.require_main_if_needed(&file, &program)?;
        let mut compiler = self.lower(&file, &program, LowerKind::Check)?;
        let warnings = compiler.take_warnings();
        let type_index = compiler.take_type_index();
        Ok(CheckedProgram {
            file,
            program,
            warnings,
            type_index,
        })
    }

    /// Type-check a multi-root project (Stage 28).
    ///
    /// See [`crate::project::check_project`]. Single-file [`Self::check_source`]
    /// remains the API for one root.
    pub fn check_project(
        options: &crate::project::ProjectOptions,
    ) -> Result<crate::project::CheckedProject, SessionDiagnostics> {
        crate::project::check_project(options)
    }

    /// Compile source to a JIT [`SessionArtifact`] (does not run `main`).
    ///
    /// Requires [`ExecutionMode::Jit`] or [`ExecutionMode::Check`]. The library
    /// default is [`ExecutionMode::Object`]; set `mode` to `Jit` before calling,
    /// or use [`Self::compile_object_source`] / [`Self::compile_executable_source`].
    ///
    /// - [`ExecutionMode::Jit`]: full check + JIT install.
    /// - [`ExecutionMode::Check`]: same as [`Self::check_source`] but returns an
    ///   artifact whose compiler is check-only (`run_main` will fail).
    /// - [`ExecutionMode::Object`]: clear error; use [`Self::compile_object_source`].
    /// - [`ExecutionMode::Interpret`]: clear error; use [`Self::interpret_source`].
    pub fn compile_source(&self, source: String) -> Result<SessionArtifact, SessionDiagnostics> {
        match self.options.mode {
            ExecutionMode::Object => {
                return Err(self.backend_not_ready(
                    source,
                    "E391",
                    "ExecutionMode::Object does not produce a JIT artifact",
                    "call Session::compile_object_source to emit a relocatable object",
                ));
            }
            ExecutionMode::Interpret => {
                return Err(self.backend_not_ready(
                    source,
                    "E392",
                    "ExecutionMode::Interpret does not produce a JIT artifact",
                    "call Session::interpret_source to check and run on the interpreter",
                ));
            }
            ExecutionMode::Check | ExecutionMode::Jit => {}
        }

        let (file, program) = self.parse_source(source)?;
        self.require_main_if_needed(&file, &program)?;
        let kind = if matches!(self.options.mode, ExecutionMode::Check) {
            LowerKind::Check
        } else {
            LowerKind::Jit
        };
        let compiler = self.lower(&file, &program, kind)?;
        Ok(SessionArtifact { file, compiler })
    }

    /// Check + lower to a relocatable native object (Stage 13c).
    ///
    /// Ignores [`CompileOptions::mode`] other than using the same search paths /
    /// `require_main` / error limit. Prefer setting `mode` to
    /// [`ExecutionMode::Object`] for clarity.
    pub fn compile_object_source(
        &self,
        source: String,
    ) -> Result<ObjectArtifact, SessionDiagnostics> {
        let (file, program) = self.parse_source(source)?;
        self.require_main_if_needed(&file, &program)?;
        let module_name = object_module_name(&self.options.source_path);
        let compiler = self.lower(&file, &program, LowerKind::Object { module_name })?;
        let ir = compiler.emit_ir();
        let bytes = match compiler.emit_object() {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(SessionDiagnostics {
                    file,
                    batch: one_compile_error(e, self.options.error_limit),
                });
            }
        };
        if bytes.is_empty() {
            return Err(SessionDiagnostics {
                file,
                batch: one_compile_error(
                    CompileError::new(
                        "object emit produced an empty artifact",
                        Span::default(),
                        "E391",
                    ),
                    self.options.error_limit,
                ),
            });
        }
        Ok(ObjectArtifact {
            file,
            bytes,
            ir,
            entry_name: self.options.entry_name.clone(),
            target: self.options.effective_target(),
        })
    }

    /// Check, emit a program object (with process `main`), and link with the
    /// runtime archive for [`CompileOptions::target`] into a runnable
    /// executable (Stage 19 / 26).
    ///
    /// Uses a system linker (`ld` / `lld`). Does not invoke `cc` / `gcc` /
    /// `clang` as a compile or link driver, and never falls back to JIT.
    pub fn compile_executable_source(
        &self,
        source: String,
    ) -> Result<ExecutableArtifact, SessionDiagnostics> {
        let object = self.compile_object_source(source)?;
        let target = object.target.clone();
        if !target.supports_executable_link() {
            return Err(SessionDiagnostics {
                file: object.file,
                batch: crate::link::LinkError::new(
                    "E397",
                    format!(
                        "AOT executable link is not available for '{}' on this host (Stage 34 is object emit only)",
                        target.as_str()
                    ),
                )
                .with_help(
                    "use Session::compile_object_source for Mach-O / COFF; link executables on the target OS or stick to linux-gnu / linux-musl",
                )
                .into_batch(self.options.error_limit),
            });
        }
        let archive = match crate::linkable_archive_for(&target) {
            Ok(archive) => archive,
            Err(msg) => {
                return Err(SessionDiagnostics {
                    file: object.file,
                    batch: crate::link::LinkError::new(
                        "E396",
                        format!("runtime archive unavailable for '{}': {msg}", target.as_str()),
                    )
                    .with_help(
                        "rebuild yarrow-core with the target installed, or set YARROW_RUNTIME_AOT_ARCHIVE_<triple> to libyarrow_runtime_aot.a (see docs/RUNTIME.md)",
                    )
                    .into_batch(self.options.error_limit),
                });
            }
        };
        let bytes = match crate::link::link_executable(&object.bytes, &archive.bytes, &target) {
            Ok(bytes) => bytes,
            Err(err) => {
                return Err(SessionDiagnostics {
                    file: object.file,
                    batch: err.into_batch(self.options.error_limit),
                });
            }
        };
        Ok(ExecutableArtifact {
            file: object.file,
            bytes,
            ir: object.ir,
            entry_name: object.entry_name,
            target,
        })
    }

    /// Check source, then execute the configured entry on the AST interpreter
    /// (Stage 13b).
    ///
    /// Returns the same [`RunResult`] shape as JIT `run_main` when supported.
    pub fn interpret_source(&self, source: String) -> Result<RunResult, SessionDiagnostics> {
        let checked = self.check_source(source)?;
        let mut ctx = EvalContext::new();
        if let Some(dir) = Path::new(&self.options.source_path).parent()
            && !dir.as_os_str().is_empty()
        {
            ctx.add_module_search_path(dir);
        }
        for p in &self.options.module_search_paths {
            ctx.add_module_search_path(p.clone());
        }
        if let Err(e) = ctx.load_program(&checked.program) {
            return Err(SessionDiagnostics {
                file: checked.file,
                batch: one_compile_error(e.into_compile_error(), self.options.error_limit),
            });
        }
        match ctx.run_entry(&self.options.entry_name) {
            Ok(result) => Ok(result),
            Err(e) => Err(SessionDiagnostics {
                file: checked.file,
                batch: one_compile_error(e.into_compile_error(), self.options.error_limit),
            }),
        }
    }

    fn backend_not_ready(
        &self,
        source: String,
        code: &str,
        message: &str,
        help: &str,
    ) -> SessionDiagnostics {
        let path = self.options.source_path.clone();
        let file = SourceFile::new(path.clone(), source);
        let mut batch = DiagnosticBatch::with_limit(self.options.error_limit);
        let diag = Diagnostic::error(code, message)
            .with_path(path)
            .with_primary(Span::default(), "")
            .with_help(help);
        batch.push(diag);
        SessionDiagnostics { file, batch }
    }

    fn require_main_if_needed(
        &self,
        file: &SourceFile,
        program: &Program,
    ) -> Result<(), SessionDiagnostics> {
        let entry = self.options.entry_name.as_str();
        if !self.options.require_main || program.has_entry(entry) {
            return Ok(());
        }
        let path = self.options.source_path.clone();
        let mut batch = DiagnosticBatch::with_limit(self.options.error_limit);
        let span = program
            .entry_function(entry)
            .map(|(_, span)| span)
            .or_else(|| {
                program.items.iter().find_map(|item| match &item.kind {
                    StmtKind::Function(_) => Some(item.span),
                    _ => None,
                })
            })
            .or_else(|| program.items.first().map(|item| item.span))
            .unwrap_or_default();
        let diag = Diagnostic::error("E360", format!("program has no '{entry}' function"))
            .with_path(path)
            .with_primary(span, "")
            .with_note(format!(
                "running a `.yar` file requires a top-level `{entry}` entry point"
            ))
            .with_help(format!(
                "add `{entry} function do ... end`, optionally `with T` for a printable result"
            ));
        batch.push(diag);
        Err(SessionDiagnostics {
            file: file.clone(),
            batch,
        })
    }

    fn lower(
        &self,
        file: &SourceFile,
        program: &Program,
        kind: LowerKind,
    ) -> Result<Compiler, SessionDiagnostics> {
        let path = self.options.source_path.clone();
        let backend = match &kind {
            LowerKind::Jit => crate::compiler::CompilerBackend::Jit,
            LowerKind::Check => crate::compiler::CompilerBackend::Check,
            LowerKind::Object { module_name } => crate::compiler::CompilerBackend::Object {
                module_name: module_name.clone(),
            },
        };
        let mut compiler = Compiler::with_options(
            backend,
            self.options.opt_level,
            self.options.debug_info,
            &self.options.effective_target(),
        )
        .map_err(|e| SessionDiagnostics {
            file: file.clone(),
            batch: one_compile_error(e, self.options.error_limit),
        })?;
        compiler.set_error_limit(self.options.error_limit);
        compiler.set_source_path(path);
        compiler.set_source_text(file.source.clone());
        compiler.set_entry_name(self.options.entry_name.clone());
        if let Some(dir) = Path::new(&self.options.source_path).parent()
            && !dir.as_os_str().is_empty()
        {
            compiler.add_module_search_path(dir);
        }
        for p in &self.options.module_search_paths {
            compiler.add_module_search_path(p.clone());
        }

        if let Err(batch) = compiler.compile(program) {
            return Err(SessionDiagnostics {
                file: file.clone(),
                batch,
            });
        }

        Ok(compiler)
    }
}

enum LowerKind {
    Jit,
    /// Semantic analysis via Cranelift without JIT install or object emit.
    Check,
    Object {
        module_name: String,
    },
}

fn object_module_name(source_path: &str) -> String {
    Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("yarrow")
        .to_string()
}

impl SessionArtifact {
    pub fn run_main(&mut self) -> Result<RunResult, CompileError> {
        self.compiler.run_main()
    }

    /// Cranelift IR captured during compile (see `Compiler::emit_ir`).
    pub fn emit_ir(&self) -> String {
        self.compiler.emit_ir()
    }
}

fn tokenize_batch(
    err: crate::tokenizer::TokenizeError,
    path: &str,
    error_limit: usize,
) -> DiagnosticBatch {
    let mut batch = DiagnosticBatch::with_limit(error_limit);
    let diag = Diagnostic::error(err.code, err.message)
        .with_path(path)
        .with_primary(Span::from_location(err.location), "");
    batch.push(diag);
    batch
}

fn one_compile_error(err: CompileError, error_limit: usize) -> DiagnosticBatch {
    let mut batch = DiagnosticBatch::with_limit(error_limit);
    batch.push((*err.diagnostic).clone());
    batch
}

/// Render all diagnostics from one batch for one source file.
pub fn render_batch(batch: &DiagnosticBatch, file: &SourceFile, color: ColorChoice) -> String {
    let mut out = String::new();
    for diag in batch.iter() {
        let mut diag = diag.clone();
        if diag.path.is_empty() {
            diag.path = file.path.clone();
        }
        out.push_str(&render(&diag, file, color));
    }
    if batch.is_at_limit() {
        out.push_str(&format!(
            "error: aborting due to {} previous errors (limit {})\n",
            batch.error_count(),
            batch.limit()
        ));
    }
    out
}
