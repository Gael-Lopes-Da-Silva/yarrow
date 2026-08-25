//! A Cranelift JIT compiler for Yarrow programs.
//!
//! The compiler mirrors the parser's operand-stack model: statements are balanced
//! against a compile-time value stack (`Vec<Slot>`), and binary operators the
//! parser left as runtime `ApplyBin`/`ApplyUn`/`StackOp` ops are lowered by
//! popping operands off that same stack.

mod backend;
mod errors;
pub(crate) mod modules;
mod types;

use std::collections::HashMap;

use backend::CodeModule;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    AbiParam, Block, BlockArg, FuncRef, GlobalValue, InstBuilder as _, StackSlotData,
    StackSlotKind, TrapCode, Type as CLType, Value, types as irtypes,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};

use crate::diagnostics::{DEFAULT_ERROR_LIMIT, DiagnosticBatch, Span};
use crate::parser::ast::{
    BinOp, Expr, Function, MatchCase, MatchCaseKind, ParamModifier, Primitive, Program, StackOp,
    Stmt, StmtKind, UnOp, Visibility,
};
use crate::parser::literals::{
    FloatLiteralKind, decode_float_literal, decode_int_literal, decode_rune_literal,
    decode_string_literal, float_literal_kind,
};
use crate::parser::parse;
use crate::tokenizer::Tokenizer;
use modules::{ModuleLoader, RequiredModule};

pub use errors::CompileError;
use types::CResult;
pub use types::Ty;
use types::{
    StructLayout, coerce, coercible, common_type, elem_code, elem_ty, error_return, kind_code,
    layout, primitive_ty, resolve, scalar_ty,
};

/// A variable binding that a `for` loop clobbered, so it can be restored at
/// loop end: the name plus the previous binding (if any).
/// The result of running a program's `main`, in a driver-displayable form.
#[derive(Debug, Clone, PartialEq)]
pub enum RunResult {
    /// `main` returns nothing (no `with` clause).
    Void,
    /// An integer (or rune) result.
    Int(i64),
    /// A boolean result.
    Bool(bool),
    /// A float result.
    Float(f64),
    /// A string result, decoded from its heap handle.
    Str(String),
}

/// Compile-time ownership of a value on the operand stack (or in a variable).
///
/// Yarrow owns heap values (strings, lists, hashmaps) on the stack or in a
/// variable; popping, overwriting or leaving scope drops them. Borrows are
/// tracked so a value cannot be dropped while a reference to it is live, and
/// moved values cannot be used again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Own {
    /// Owns its heap storage; dropping emits `yarrow_free_value`.
    Owned,
    /// A borrow/reference into something else; dropping releases the borrow.
    Borrow,
    /// Scalars, struct/array frame pointers, regions and other non-owning
    /// values: dropping is a no-op.
    Trivial,
}

impl Own {
    fn is_owned(self) -> bool {
        matches!(self, Own::Owned)
    }
}

/// A value on the compile-time operand stack: a Cranelift SSA value plus the
/// physical `Ty` it carries and its ownership state.
#[derive(Debug, Clone, Copy)]
struct Slot {
    value: Value,
    ty: Ty,
    own: Own,
}

/// Per-function lowering state.
struct FnState {
    vars: HashMap<String, (Variable, Ty, Own)>,
    loops: Vec<LoopCtx>,
    returns: Vec<Ty>,
    frefs: HashMap<String, FuncRef>,
    /// Imported host runtime functions (see `crate::runtime`).
    rt: HashMap<String, FuncRef>,
    /// Module path prefix of the function being compiled (e.g. `"std.io"`),
    /// or `None` for top-level functions. Used to resolve intra-module calls.
    module: Option<String>,
    /// Handles currently owned by this function, keyed by SSA value. Only heap
    /// values (strings/lists/hashmaps) ever appear here.
    owns: std::collections::HashSet<Value>,
    /// Values with an active borrow (from `borrow` or a heap-value `dup`).
    borrowed: std::collections::HashSet<Value>,
    /// Values moved away with `move` / region put; dropping them is skipped.
    moved: std::collections::HashSet<Value>,
    /// Variable names whose owned value was transferred by `move`.
    moved_vars: std::collections::HashSet<String>,
    /// Heap values attached to a region: value → region handle.
    region_attached: HashMap<Value, Value>,
    /// Values freed with their region; further use is a compile error.
    region_freed: std::collections::HashSet<Value>,
    /// `defer` bodies, run in reverse order at scope exit.
    deferred: Vec<Vec<Stmt>>,
    /// Struct layout ids whose field descriptors were already registered in
    /// the runtime this function.
    registered_descs: std::collections::HashSet<u32>,
    /// Union ids whose member-kind tables were already registered in the
    /// runtime this function.
    registered_unions: std::collections::HashSet<u32>,
    /// Payload type of this function's fallible `|T Err|` return envelope, if
    /// it returns an error. `None` means the function cannot error.
    error_value: Option<Ty>,
    /// Number of active `unsafe ... end` blocks around the current statement.
    /// `> 0` (or an unsafe function body) permits pointer access, raw memory
    /// words and unsafe host functions.
    unsafe_depth: u32,
    /// Whether the current block flow already ended with an explicit `return`
    /// (so the implicit fallthrough return must not be emitted on an empty
    /// compile-time stack). Reset after each compound statement whose merge
    /// block stays live.
    terminated: bool,
    /// A statement in this body already failed; remaining statements are still
    /// type-checked for more diagnostics, but IR is abandoned.
    had_error: bool,
    /// Nested functions declared in this body: short name → fully-qualified
    /// name (`demo::add`). Only callable from the enclosing function.
    local_funcs: HashMap<String, String>,
    /// Span of the statement currently being lowered.
    current_span: Span,
    /// Variable name → span where it was moved from (for secondary labels).
    move_sites: HashMap<String, Span>,
    /// Borrowed SSA value → span of the `borrow` / region-put that created it.
    borrow_sites: HashMap<Value, Span>,
}

struct LoopCtx {
    break_to: Block,
    continue_to: Block,
    /// Current iterable element for `std.loop` `value` (set each iteration).
    value: Option<(Value, Ty)>,
    /// Current iterable index for `std.loop` `index`.
    index: Option<Value>,
}

/// A program-wide enum declaration: each member name bound to its value
/// (an implicit ordinal or an explicit one).
#[derive(Debug, Clone)]
struct EnumInfo {
    name: String,
    members: Vec<(String, i64)>,
}

/// A program-wide union declaration: the member types the union can hold,
/// resolved to their physical `Ty`. The active member is selected by index.
#[derive(Debug, Clone)]
struct UnionInfo {
    name: String,
    members: Vec<Ty>,
}

/// A named `error` type: members map to program-unique envelope tags.
#[derive(Debug, Clone)]
struct ErrorInfo {
    name: String,
    members: Vec<(String, u32)>,
}

/// Compiler that turns a whole `Program` into JIT code or a relocatable object.
pub struct Compiler {
    module: CodeModule,
    ptr_type: CLType,
    /// Struct name -> index into `struct_layouts`.
    struct_ids: HashMap<String, u32>,
    /// Layouts for every struct, indexed by `Ty::Struct(id).0`.
    struct_layouts: Vec<StructLayout>,
    /// Enum name -> index into `enums`.
    enum_ids: HashMap<String, u32>,
    /// Every enum, indexed by `Ty::Enum(id).0`.
    enums: Vec<EnumInfo>,
    /// Bare enum member name -> (enum id, value), so `RED` resolves anywhere.
    enum_consts: HashMap<String, (u32, i64)>,
    /// Union name -> index into `unions`.
    union_ids: HashMap<String, u32>,
    /// Every union, indexed by `Ty::Union(id).0`.
    unions: Vec<UnionInfo>,
    /// Union id -> data object holding its member-kind-code table.
    union_desc_ids: HashMap<u32, DataId>,
    /// Per-function global value for each union's member-kind-code table.
    union_desc_gvs: HashMap<u32, GlobalValue>,
    sigs: HashMap<String, cranelift_codegen::ir::Signature>,
    sig_tys: HashMap<String, (Vec<Ty>, Vec<Ty>)>,
    func_ids: HashMap<String, FuncId>,
    /// Imported host runtime functions (symbols installed by `runtime::install`).
    runtime_ids: HashMap<String, FuncId>,
    /// String literal bytes -> data object (declared/defined before functions).
    string_ids: HashMap<String, DataId>,
    /// Per-function global value for each string literal's data section.
    fn_gvs: HashMap<String, GlobalValue>,
    /// Struct layout id -> data object holding its `FieldDesc` table.
    struct_desc_ids: HashMap<u32, DataId>,
    /// Per-function global value for each struct's `FieldDesc` table.
    struct_desc_gvs: HashMap<u32, GlobalValue>,
    /// Module loader used to resolve `require` paths.
    loader: ModuleLoader,
    /// Modules loaded by `require`, in dependency order (dependencies first).
    modules: Vec<RequiredModule>,
    /// Module alias -> module path (e.g. `io` -> `std.io`).
    aliases: HashMap<String, String>,
    /// Plain function name -> fully-qualified name for alias-less requires
    /// (e.g. `sqrt` -> `std.math::sqrt`).
    plain_funcs: HashMap<String, String>,
    /// Module path -> item imported from it, for each module already loaded, so
    /// a `require` is processed once. `None` means the whole module was
    /// imported.
    loaded: HashMap<String, Option<String>>,
    /// Module alias -> the single item it exposes, for item imports under a
    /// scope (`"std.math.sqrt" s require` -> only `s.sqrt` resolves).
    item_aliases: HashMap<String, String>,
    /// Extra plain item imports recorded when the parent module was already
    /// loaded (e.g. `"std.math" math require` then `"std.math.sqrt" require`).
    extra_plain_items: Vec<(String, String)>,
    /// Error kind name (`AppError.NOT_FOUND`, `OUT_OF_MEMORY`, ...) ->
    /// program-unique tag. Tags are interned once per program so comparisons
    /// and envelope propagation agree across functions.
    error_ids: HashMap<String, u32>,
    /// Error type name -> index into `error_types`.
    error_type_ids: HashMap<String, u32>,
    /// Every named `error` declaration.
    error_types: Vec<ErrorInfo>,
    /// Fully-qualified names of functions declared `unsafe`; calling them from
    /// a non-unsafe context is rejected.
    unsafe_funcs: std::collections::HashSet<String>,
    /// Fully-qualified names declared `public` (exported across `require`).
    public_funcs: std::collections::HashSet<String>,
    /// Function bodies already passed to `define_function` (nested helpers).
    defined_funcs: std::collections::HashSet<String>,
    finalized: bool,
    source_path: String,
    /// Best-effort span for program-level errors (e.g. missing `main`).
    program_span: Span,
    /// Diagnostics collected during a compile (Stage 10 multi-error).
    errors: DiagnosticBatch,
    /// Maximum number of diagnostics to collect before aborting.
    error_limit: usize,
    /// Cranelift IR text captured after each function is lowered.
    ir_dump: String,
    /// When true, run full type / ownership / stack checks and lower to CLIF
    /// for analysis, but do not `define_function` or finalize a JIT module.
    /// Used by `ExecutionMode::Check` (Stage 13a).
    check_only: bool,
}

impl Compiler {
    /// In-process Cranelift JIT (default for `run` / `compile --target jit`).
    pub fn new() -> CResult<Self> {
        Self::with_module(CodeModule::new_jit()?)
    }

    /// Relocatable native object backend (`compile --target object`, Stage 13c).
    ///
    /// Host runtime symbols are declared as imports; linking them is CLI-side.
    pub fn new_object(module_name: &str) -> CResult<Self> {
        Self::with_module(CodeModule::new_object(module_name)?)
    }

    fn with_module(module: CodeModule) -> CResult<Self> {
        let ptr_type = module.isa().pointer_type();
        Ok(Self {
            module,
            ptr_type,
            struct_ids: HashMap::new(),
            struct_layouts: Vec::new(),
            enum_ids: HashMap::new(),
            enums: Vec::new(),
            enum_consts: HashMap::new(),
            union_ids: HashMap::new(),
            unions: Vec::new(),
            union_desc_ids: HashMap::new(),
            union_desc_gvs: HashMap::new(),
            sigs: HashMap::new(),
            sig_tys: HashMap::new(),
            func_ids: HashMap::new(),
            runtime_ids: HashMap::new(),
            string_ids: HashMap::new(),
            fn_gvs: HashMap::new(),
            struct_desc_ids: HashMap::new(),
            struct_desc_gvs: HashMap::new(),
            loader: ModuleLoader::new(),
            modules: Vec::new(),
            aliases: HashMap::new(),
            plain_funcs: HashMap::new(),
            loaded: HashMap::new(),
            item_aliases: HashMap::new(),
            extra_plain_items: Vec::new(),
            error_ids: HashMap::new(),
            error_type_ids: HashMap::new(),
            error_types: Vec::new(),
            unsafe_funcs: std::collections::HashSet::new(),
            public_funcs: std::collections::HashSet::new(),
            defined_funcs: std::collections::HashSet::new(),
            finalized: false,
            source_path: String::new(),
            program_span: Span::default(),
            error_limit: DEFAULT_ERROR_LIMIT,
            errors: DiagnosticBatch::with_limit(DEFAULT_ERROR_LIMIT),
            ir_dump: String::new(),
            check_only: false,
        })
    }

    /// Emit relocatable object bytes after a successful [`Self::compile`] on an
    /// object backend. Consumes the compiler (object product takes ownership).
    pub fn emit_object(self) -> CResult<Vec<u8>> {
        if !self.module.is_object() {
            return Err(CompileError::new(
                "cannot emit object: this compiler was built for JIT",
                self.program_span,
                "E391",
            )
            .with_help("use Compiler::new_object / Session::compile_object_source"));
        }
        if self.check_only {
            return Err(CompileError::new(
                "cannot emit object: this compiler was built in check-only mode",
                self.program_span,
                "E390",
            ));
        }
        self.module.finish_object()
    }

    /// Cranelift IR for every function lowered in the last successful compile.
    pub fn emit_ir(&self) -> String {
        self.ir_dump.clone()
    }

    /// Check-only mode: semantic analysis without installing JIT code.
    pub fn set_check_only(&mut self, check_only: bool) {
        self.check_only = check_only;
    }

    pub fn is_check_only(&self) -> bool {
        self.check_only
    }

    pub fn set_error_limit(&mut self, error_limit: usize) {
        self.error_limit = error_limit.max(1);
    }

    pub fn set_source_path(&mut self, path: impl Into<String>) {
        self.source_path = path.into();
    }

    /// Add a directory searched for user modules (`"a.b"` -> `a/b.yar`).
    pub fn add_module_search_path(&mut self, path: impl Into<std::path::PathBuf>) {
        self.loader.add_search_path(path);
    }

    /// Two-pass compilation: first register structs and declare every function
    /// (so whole-program calls resolve), then compile each body. Functions in
    /// modules loaded by `require` are declared and compiled alongside the
    /// main program's.
    ///
    /// On failure returns every collected diagnostic (capped), not only the
    /// first independent error.
    pub fn compile(&mut self, program: &Program) -> Result<(), DiagnosticBatch> {
        self.errors = DiagnosticBatch::with_limit(self.error_limit);
        match self.compile_inner(program) {
            Ok(()) => {
                if self.errors.is_empty() {
                    Ok(())
                } else {
                    Err(self.errors.take())
                }
            }
            Err(e) => {
                self.report(e);
                Err(self.errors.take())
            }
        }
    }

    fn report(&mut self, err: CompileError) -> bool {
        let mut diag = (*err.diagnostic).clone();
        if diag.path.is_empty() {
            diag.path = self.source_path.clone();
        }
        self.errors.push(diag)
    }

    fn compile_inner(&mut self, program: &Program) -> CResult<()> {
        self.program_span = program
            .items
            .iter()
            .find_map(|item| match &item.kind {
                StmtKind::Function(f) if f.name == "main" => Some(item.span),
                _ => None,
            })
            .or_else(|| {
                program.items.iter().find_map(|item| match &item.kind {
                    StmtKind::Function(_) => Some(item.span),
                    _ => None,
                })
            })
            .or_else(|| program.items.first().map(|item| item.span))
            .unwrap_or_default();
        self.modules.clear();
        self.aliases.clear();
        self.plain_funcs.clear();
        self.loaded.clear();
        self.item_aliases.clear();
        self.extra_plain_items.clear();
        self.enum_ids.clear();
        self.enums.clear();
        self.enum_consts.clear();
        self.union_ids.clear();
        self.unions.clear();
        self.union_desc_ids.clear();
        self.error_ids.clear();
        self.error_type_ids.clear();
        self.error_types.clear();
        self.unsafe_funcs.clear();
        self.public_funcs.clear();
        self.string_ids.clear();
        self.defined_funcs.clear();
        let mut loaded = Vec::new();
        self.load_requires(program, &mut loaded)?;
        self.modules = loaded;

        // Every unit to compile: `(module path, program)`. The main program
        // has no path; module functions get a fully-qualified name.
        let mut units: Vec<(Option<String>, Program)> = Vec::new();
        units.push((None, program.clone()));
        for m in &self.modules {
            units.push((Some(m.path.clone()), m.program.clone()));
        }

        // Pass A: register every struct name.
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Struct(d) = &item.kind {
                    self.struct_ids
                        .entry(d.name.clone())
                        .or_insert(self.struct_layouts.len() as u32);
                    self.struct_layouts.push(StructLayout {
                        name: d.name.clone(),
                        fields: Vec::new(),
                        size: 0,
                        align: 1,
                    });
                }
            }
        }

        // Pass A1: register every union name, so types referencing a union
        // resolve before members are known.
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Union(d) = &item.kind {
                    self.union_ids
                        .entry(d.name.clone())
                        .or_insert(self.unions.len() as u32);
                    self.unions.push(UnionInfo {
                        name: d.name.clone(),
                        members: Vec::new(),
                    });
                }
            }
        }

        // Pass A2: register every enum. Members get implicit ordinals starting
        // at 0 unless an explicit value is given; both names are bound.
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Enum(d) = &item.kind {
                    let id = self.enums.len() as u32;
                    self.enum_ids.insert(d.name.clone(), id);
                    let mut members = Vec::with_capacity(d.members.len());
                    let mut next = 0i64;
                    for m in &d.members {
                        let v = if let Some(raw) = &m.value {
                            let n = decode_int_literal(raw)
                                .map_err(|msg| CompileError::new(msg, item.span, "E363"))?;
                            if n < i64::MIN as i128 || n > i64::MAX as i128 {
                                return Err(CompileError::new(
                                    format!(
                                        "enum member '{}' value '{raw}' is out of range",
                                        m.name
                                    ),
                                    item.span,
                                    "E364",
                                ));
                            }
                            n as i64
                        } else {
                            next
                        };
                        next = v.saturating_add(1);
                        self.enum_consts.insert(m.name.clone(), (id, v));
                        members.push((m.name.clone(), v));
                    }
                    self.enums.push(EnumInfo {
                        name: d.name.clone(),
                        members,
                    });
                }
            }
        }

        // Pass A3: register every named `error` type (names first, then members
        // so injection can copy tags from another error type).
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Error(d) = &item.kind {
                    self.error_type_ids
                        .entry(d.name.clone())
                        .or_insert_with(|| {
                            let id = self.error_types.len() as u32;
                            self.error_types.push(ErrorInfo {
                                name: d.name.clone(),
                                members: Vec::new(),
                            });
                            id
                        });
                }
            }
        }
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Error(d) = &item.kind
                    && d.inject.is_none()
                {
                    self.fill_error_type(d)?;
                }
            }
        }
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Error(d) = &item.kind
                    && d.inject.is_some()
                {
                    self.fill_error_type(d)?;
                }
            }
        }

        // Pass B: resolve each struct's field types into a layout. Must happen
        // before function signatures are declared, since those may use structs.
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Struct(d) = &item.kind {
                    let mut fields = Vec::with_capacity(d.fields.len());
                    for f in &d.fields {
                        let ty = self.resolve_ty(&f.ty)?;
                        fields.push((f.name.clone(), ty));
                    }
                    let id = self.struct_ids[&d.name];
                    self.struct_layouts[id as usize] = layout(&d.name, fields);
                }
            }
        }

        // Pass B1: resolve every union's member types. Members must be
        // distinct (dispatch compares tags as indices) and no wider than a
        // pointer (the payload is inline).
        for (_, prog) in &units {
            for item in &prog.items {
                if let StmtKind::Union(d) = &item.kind {
                    let id = self.union_ids[&d.name];
                    if d.types.is_empty() {
                        return Err(CompileError::new(
                            format!("union '{}' must have at least one member type", d.name),
                            item.span,
                            "E346",
                        ));
                    }
                    let mut members = Vec::with_capacity(d.types.len());
                    for t in &d.types {
                        let mt = self.resolve_ty(t)?;
                        if mt.elem_size() > 8 {
                            return Err(CompileError::new(
                                format!("union member type '{mt:?}' is wider than 8 bytes",),
                                item.span,
                                "E346",
                            ));
                        }
                        if members.contains(&mt) {
                            return Err(CompileError::new(
                                format!("union '{}' has duplicate member type '{mt:?}'", d.name),
                                item.span,
                                "E346",
                            ));
                        }
                        members.push(mt);
                    }
                    self.unions[id as usize].members = members;
                }
            }
        }

        // Pass C: declare every function, then register module name bindings.
        for (path, prog) in &units {
            for item in &prog.items {
                match &item.kind {
                    StmtKind::Function(f) => {
                        let name = self.item_name(path.as_deref(), &f.name);
                        if f.is_unsafe {
                            self.unsafe_funcs.insert(name.clone());
                        }
                        if matches!(f.visibility, Some(Visibility::Public)) {
                            self.public_funcs.insert(name.clone());
                        }
                        self.declare_function(f, &name)?;
                    }
                    StmtKind::Implement(imp) => {
                        for f in &imp.functions {
                            let name = self
                                .item_name(path.as_deref(), &format!("{}::{}", imp.target, f.name));
                            if f.is_unsafe {
                                self.unsafe_funcs.insert(name.clone());
                                // Method calls resolve to `Type::method` without
                                // the module prefix.
                                self.unsafe_funcs
                                    .insert(format!("{}::{}", imp.target, f.name));
                            }
                            if matches!(f.visibility, Some(Visibility::Public)) {
                                self.public_funcs.insert(name.clone());
                                self.public_funcs
                                    .insert(format!("{}::{}", imp.target, f.name));
                            }
                            self.declare_function(f, &name)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.register_module_bindings()?;

        // Pass C2: string literals become read-only data sections, struct
        // field descriptors become data tables, and the host runtime functions
        // are imported so compiled code can call them.
        for (_, prog) in &units {
            self.declare_string_data(prog)?;
        }
        self.declare_struct_desc_data()?;
        self.declare_union_desc_data()?;
        self.declare_runtime_imports()?;

        // Pass D: compile every function. Independent function bodies keep
        // going after an error so several diagnostics can be reported.
        for (path, prog) in &units {
            for item in &prog.items {
                if self.errors.is_at_limit() {
                    break;
                }
                match &item.kind {
                    StmtKind::Function(f) => {
                        let name = self.item_name(path.as_deref(), &f.name);
                        if let Err(e) = self.compile_function(f, &name, path.as_deref(), false) {
                            self.report(e);
                        }
                    }
                    StmtKind::Implement(imp) => {
                        for f in &imp.functions {
                            if self.errors.is_at_limit() {
                                break;
                            }
                            let name = self
                                .item_name(path.as_deref(), &format!("{}::{}", imp.target, f.name));
                            if let Err(e) = self.compile_function(f, &name, path.as_deref(), true) {
                                self.report(e);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// The symbol name for `name` in a unit: the module path prefixes it.
    fn item_name(&self, module: Option<&str>, name: &str) -> String {
        match module {
            Some(path) => format!("{path}::{name}"),
            None => name.to_string(),
        }
    }

    /// Depth-first load of every module referenced by `program.items`,
    /// including `require` statements nested inside function bodies.
    fn load_requires(&mut self, program: &Program, out: &mut Vec<RequiredModule>) -> CResult<()> {
        for item in &program.items {
            match &item.kind {
                StmtKind::Require { path, alias } => self.load_one(path, alias, out)?,
                StmtKind::Function(f) => self.load_requires_stmts(&f.body, out)?,
                StmtKind::Implement(imp) => {
                    for f in &imp.functions {
                        self.load_requires_stmts(&f.body, out)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn load_requires_stmts(
        &mut self,
        stmts: &[Stmt],
        out: &mut Vec<RequiredModule>,
    ) -> CResult<()> {
        for s in stmts {
            match &s.kind {
                StmtKind::Require { path, alias } => self.load_one(path, alias, out)?,
                StmtKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.load_requires_stmts(then_branch, out)?;
                    self.load_requires_stmts(else_branch, out)?;
                }
                StmtKind::Defer { body } | StmtKind::Handle { body, .. } => {
                    self.load_requires_stmts(body, out)?
                }
                StmtKind::Unsafe { body } => self.load_requires_stmts(body, out)?,
                StmtKind::For { body, .. } => self.load_requires_stmts(body, out)?,
                StmtKind::Match {
                    cases, else_branch, ..
                } => {
                    for c in cases {
                        self.load_requires_stmts(&c.body, out)?;
                    }
                    self.load_requires_stmts(else_branch, out)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Load one module referenced by a `require` statement, deduplicated by the
    /// resolved module path (not the require path).
    ///
    /// Path resolution follows rule 1 of the `require` spec: `a.b.c` first
    /// tries module `a.b`; if it defines a function `c`, only `c` is imported
    /// (the `item`). Otherwise the full path is a module file.
    fn load_one(
        &mut self,
        path: &str,
        alias: &Option<String>,
        out: &mut Vec<RequiredModule>,
    ) -> CResult<()> {
        let (module_path, item) = self.resolve_require(path)?;
        if let Some(existing_item) = self.loaded.get(&module_path) {
            // Already loaded. If the previous import was only an item and this
            // one imports the whole module, widen the existing entry.
            if existing_item.is_some() && item.is_none() {
                if let Some(existing) = out.iter_mut().find(|m| m.path == module_path) {
                    existing.item = None;
                }
                self.loaded.insert(module_path.clone(), None);
            }
            // A later bare item import still needs a plain-name binding even
            // when the module was already loaded under an alias.
            if alias.is_none()
                && let Some(item_name) = item
            {
                self.extra_plain_items.push((module_path, item_name));
            }
            return Ok(());
        }
        self.loaded.insert(module_path.clone(), item.clone());
        let source = self.loader.load(&module_path)?;
        let tokens = Tokenizer::new(source).tokenize()?;
        let sub = parse(tokens)?;
        self.load_requires(&sub, out)?;
        out.push(RequiredModule {
            path: module_path,
            alias: alias.clone(),
            item,
            program: sub,
        });
        Ok(())
    }

    /// Resolve a `require` path per rule 1 of the spec: `a.b.c` first tries
    /// module `a.b`; if it defines a function `c`, only `c` is imported.
    /// Otherwise the full path is a module file. Warns (function wins) when
    /// both a module file and the function exist.
    fn resolve_require(&self, path: &str) -> CResult<(String, Option<String>)> {
        if let Some((parent, last)) = path.rsplit_once('.')
            && let Some(parent_source) = self.loader.try_load(parent)
        {
            let tokens = Tokenizer::new(parent_source).tokenize()?;
            let parent_prog = parse(tokens)?;
            if parent_prog
                .items
                .iter()
                .any(|i| matches!(&i.kind, StmtKind::Function(f) if f.name == last))
            {
                if self.loader.try_load(path).is_some() {
                    eprintln!(
                        "warning: '{}' is both a module and function '{}' of '{}'; importing the function",
                        path, last, parent
                    );
                }
                return Ok((parent.to_string(), Some(last.to_string())));
            }
        }
        Ok((path.to_string(), None))
    }

    /// Expose a loaded module's functions under their alias or plain names.
    ///
    /// With an alias (`"std.io" io require`), `io.func` resolves to the
    /// module's `func`. Without an alias, a whole-module import binds every
    /// function by its plain name, and an item import (`"std.math.sqrt"
    /// require`) binds only that single function.
    fn register_module_bindings(&mut self) -> CResult<()> {
        for m in &self.modules {
            if let Some(alias) = &m.alias {
                if let Some(existing) = self.aliases.get(alias) {
                    if existing != &m.path {
                        return Err(CompileError::new(
                            format!("module alias '{alias}' already bound to '{existing}'"),
                            Span::default(),
                            "E380",
                        ));
                    }
                } else {
                    self.aliases.insert(alias.clone(), m.path.clone());
                }
                // An item import under a scope exposes only that item
                // (`"std.math.sqrt" s require` -> only `s.sqrt` resolves).
                if let Some(item) = &m.item {
                    self.item_aliases.insert(alias.clone(), item.clone());
                }
            } else if let Some(item) = &m.item {
                // Item import by plain name: bind only the single function.
                let fq = format!("{}::{}", m.path, item);
                if let Some(prev) = self.plain_funcs.get(item)
                    && prev != &fq
                {
                    return Err(CompileError::new(
                        format!(
                            "function '{item}' is exported by both '{}' and '{fq}'",
                            prev
                        ),
                        Span::default(),
                        "E380",
                    ));
                }
                self.plain_funcs.insert(item.clone(), fq);
            } else {
                for item in &m.program.items {
                    if let StmtKind::Function(f) = &item.kind {
                        if !matches!(f.visibility, Some(Visibility::Public)) {
                            continue;
                        }
                        if self.func_ids.contains_key(&f.name) {
                            return Err(CompileError::new(
                                format!(
                                    "function '{}' from module '{}' conflicts with a function of the same name",
                                    f.name, m.path
                                ),
                                Span::default(),
                                "E380",
                            ));
                        }
                        let fq = format!("{}::{}", m.path, f.name);
                        if let Some(prev) = self.plain_funcs.get(&f.name)
                            && prev != &fq
                        {
                            return Err(CompileError::new(
                                format!(
                                    "function '{}' is exported by both '{}' and '{fq}'",
                                    f.name, prev
                                ),
                                Span::default(),
                                "E380",
                            ));
                        }
                        self.plain_funcs.insert(f.name.clone(), fq);
                    }
                }
            }
        }
        for (path, item) in self.extra_plain_items.clone() {
            let fq = format!("{path}::{item}");
            if let Some(prev) = self.plain_funcs.get(&item)
                && prev != &fq
            {
                return Err(CompileError::new(
                    format!("function '{item}' is exported by both '{prev}' and '{fq}'"),
                    Span::default(),
                    "E380",
                ));
            }
            self.plain_funcs.insert(item, fq);
        }
        Ok(())
    }

    /// Declare and define a read-only data object per unique string literal.
    /// Skips literals already interned from a previous unit so multi-module
    /// programs do not redefine `yarrow.str.N`.
    fn declare_string_data(&mut self, program: &Program) -> CResult<()> {
        let mut seen: Vec<&str> = Vec::new();
        collect_strings(&program.items, &mut seen);
        let mut next = self.string_ids.len();
        for s in seen {
            if self.string_ids.contains_key(s) {
                continue;
            }
            let name = format!("yarrow.str.{next}");
            next += 1;
            let id = self
                .module
                .declare_data(&name, Linkage::Local, false, false)?;
            let bytes = decode_string_literal(s)
                .map_err(|m| CompileError::new(m, Span::default(), "E363"))?;
            let mut desc = DataDescription::new();
            desc.set_align(1);
            desc.define(bytes.into_boxed_slice());
            self.module.define_data(id, &desc)?;
            self.string_ids.insert(s.to_string(), id);
        }
        Ok(())
    }

    /// Declare a read-only data object per struct holding its `FieldDesc`
    /// table (16 bytes per field: `u32 offset, u32 pad, u64 kind`), matching
    /// the runtime's `FieldDesc` layout. The table lets `yarrow_free_value`
    /// free a struct's heap fields.
    fn declare_struct_desc_data(&mut self) -> CResult<()> {
        for id in 0..self.struct_layouts.len() as u32 {
            let lay = self.struct_layout(id);
            let mut bytes: Vec<u8> = Vec::with_capacity(lay.fields.len() * 16);
            for f in &lay.fields {
                bytes.extend_from_slice(&(f.offset as u32).to_le_bytes());
                bytes.extend_from_slice(&0u32.to_le_bytes());
                bytes.extend_from_slice(&kind_code(f.ty).to_le_bytes());
            }
            let name = format!("yarrow.structdesc.{id}");
            let data_id = self
                .module
                .declare_data(&name, Linkage::Local, false, false)?;
            let mut desc = DataDescription::new();
            desc.set_align(8);
            desc.define(bytes.into_boxed_slice());
            self.module.define_data(data_id, &desc)?;
            self.struct_desc_ids.insert(id, data_id);
        }
        Ok(())
    }

    /// Declare a read-only data object per union holding its member-kind-code
    /// table (one `u64` per member). `yarrow_free_value` reads the union's tag
    /// (the active member index) to pick the right member kind when freeing
    /// the inline payload.
    fn declare_union_desc_data(&mut self) -> CResult<()> {
        for id in 0..self.unions.len() as u32 {
            let info = &self.unions[id as usize];
            let mut bytes: Vec<u8> = Vec::with_capacity(info.members.len() * 8);
            for m in &info.members {
                bytes.extend_from_slice(&kind_code(*m).to_le_bytes());
            }
            let name = format!("yarrow.uniondesc.{id}");
            let data_id = self
                .module
                .declare_data(&name, Linkage::Local, false, false)?;
            let mut desc = DataDescription::new();
            desc.set_align(8);
            desc.define(bytes.into_boxed_slice());
            self.module.define_data(data_id, &desc)?;
            self.union_desc_ids.insert(id, data_id);
        }
        Ok(())
    }

    /// Import every host runtime function so JIT code can `call` it. The
    /// signatures come from the runtime's [`HOST_FNS`] registry (one source of
    /// truth); scalar kind codes decode through [`scalar_ty`].
    fn declare_runtime_imports(&mut self) -> CResult<()> {
        for host in crate::runtime::HOST_FNS.iter() {
            let mut sig = self.module.make_signature();
            for &p in host.params {
                let ty = scalar_ty(p as u8);
                sig.params.push(AbiParam::new(ty.clty(self.ptr_type)));
            }
            for &r in host.returns {
                let ty = scalar_ty(r as u8);
                sig.returns.push(AbiParam::new(ty.clty(self.ptr_type)));
            }
            let id = self
                .module
                .declare_function(host.name, Linkage::Import, &sig)?;
            self.runtime_ids.insert(host.name.to_string(), id);
        }
        Ok(())
    }

    /// Run the compiled `main` function and return its result (if any) in a
    /// driver-displayable form. Supports void `main` and integer, float, bool
    /// and string results; other result types are rejected with `E360`.
    pub fn run_main(&mut self) -> CResult<RunResult> {
        if self.check_only {
            return Err(CompileError::new(
                "cannot run main: this compiler was built in check-only mode",
                self.program_span,
                "E390",
            )
            .with_help("use ExecutionMode::Jit (Session::compile_source) to run"));
        }
        if self.module.is_object() {
            return Err(CompileError::new(
                "cannot run main: this compiler was built for object emit",
                self.program_span,
                "E391",
            )
            .with_help("link the object and execute externally, or use ExecutionMode::Jit"));
        }
        self.finalize()?;
        let id = *self.func_ids.get("main").ok_or_else(|| {
            CompileError::new("program has no 'main' function", self.program_span, "E360")
                .with_note("running a `.yar` file requires a top-level `main` entry point")
                .with_help(
                    "add `main function do ... end`, optionally `with T` for a printable result",
                )
        })?;
        let (_, return_tys) = self.sig_tys.get("main").cloned().ok_or_else(|| {
            CompileError::new("missing signature for 'main'", self.program_span, "E360")
        })?;
        let sig = self.sigs.get("main").cloned().ok_or_else(|| {
            CompileError::new("missing signature for 'main'", self.program_span, "E360")
        })?;
        // A fallible `main` returns an envelope `(env, payload)`. Run it and
        // surface a non-zero env as a runtime failure; success is void.
        if error_return(&return_tys)?.is_some() {
            let ptr = self.module.get_finalized_function(id);
            unsafe {
                let f: extern "C" fn() -> (i64, i64) = std::mem::transmute(ptr);
                let (env, _payload) = f();
                if env != 0 {
                    return Err(CompileError::new(
                        format!("main returned error tag {env}"),
                        Span::default(),
                        "E360",
                    ));
                }
            }
            return Ok(RunResult::Void);
        }
        let ptr = self.module.get_finalized_function(id);
        unsafe {
            match return_tys.as_slice() {
                [] => {
                    let f: extern "C" fn() = std::mem::transmute(ptr);
                    f();
                    Ok(RunResult::Void)
                }
                [ty] => self.read_main_result(sig.returns[0].value_type, *ty, ptr as usize),
                _ => Err(CompileError::new(
                    "'main' must return at most one value to be runnable",
                    Span::default(),
                    "E360",
                )),
            }
        }
    }

    /// Reinterpret `main`'s single return value from its Cranelift slot.
    fn read_main_result(&self, clty: CLType, ty: Ty, ptr: usize) -> CResult<RunResult> {
        let _ = clty;
        unsafe {
            match ty {
                Ty::Bool => {
                    let f: extern "C" fn() -> i8 = std::mem::transmute(ptr);
                    Ok(RunResult::Bool(f() != 0))
                }
                Ty::I8 | Ty::U8 => {
                    let f: extern "C" fn() -> i8 = std::mem::transmute(ptr);
                    Ok(RunResult::Int(f() as i64))
                }
                Ty::I16 | Ty::U16 => {
                    let f: extern "C" fn() -> i16 = std::mem::transmute(ptr);
                    Ok(RunResult::Int(f() as i64))
                }
                Ty::I32 | Ty::U32 | Ty::Rune => {
                    let f: extern "C" fn() -> i32 = std::mem::transmute(ptr);
                    Ok(RunResult::Int(f() as i64))
                }
                Ty::I64 | Ty::U64 | Ty::Enum(_) => {
                    let f: extern "C" fn() -> i64 = std::mem::transmute(ptr);
                    Ok(RunResult::Int(f()))
                }
                Ty::F32 => {
                    let f: extern "C" fn() -> f32 = std::mem::transmute(ptr);
                    Ok(RunResult::Float(f() as f64))
                }
                Ty::F64 => {
                    let f: extern "C" fn() -> f64 = std::mem::transmute(ptr);
                    Ok(RunResult::Float(f()))
                }
                Ty::String => {
                    let f: extern "C" fn() -> u64 = std::mem::transmute(ptr);
                    let bytes = crate::runtime::string_bytes(f()).unwrap_or_default();
                    Ok(RunResult::Str(String::from_utf8_lossy(&bytes).into_owned()))
                }
                _ => Err(CompileError::new(
                    "unsupported 'main' return type for run_main",
                    Span::default(),
                    "E360",
                )),
            }
        }
    }

    /// Address of a compiled function after `compile` (JIT only).
    pub fn function_ptr(&mut self, name: &str) -> CResult<usize> {
        if self.module.is_object() {
            return Err(CompileError::new(
                "function_ptr is only available on the JIT backend",
                Span::default(),
                "E391",
            ));
        }
        self.finalize()?;
        let id = *self.func_ids.get(name).ok_or_else(|| {
            CompileError::new(
                format!("unknown function '{name}'"),
                Span::default(),
                "E361",
            )
        })?;
        Ok(self.module.get_finalized_function(id) as usize)
    }

    fn finalize(&mut self) -> CResult<()> {
        if !self.finalized {
            self.module.finalize_jit()?;
            self.finalized = true;
        }
        Ok(())
    }

    fn resolve_ty(&self, t: &crate::parser::ast::Type) -> CResult<Ty> {
        resolve(t, &|n| {
            if let Some(id) = self.struct_ids.get(n) {
                return Some(Ty::Struct(*id));
            }
            if let Some(id) = self.union_ids.get(n) {
                return Some(Ty::Union(*id));
            }
            if self.error_type_ids.contains_key(n) || self.is_error_type_path(n) {
                return Some(Ty::Error);
            }
            self.enum_ids.get(n).map(|id| Ty::Enum(*id))
        })
    }

    /// `error.Error` / bare `Error` / other named error types used in `|T Err|`.
    fn is_error_type_path(&self, path: &str) -> bool {
        let name = path.rsplit('.').next().unwrap_or(path);
        self.error_type_ids.contains_key(name) || self.error_type_ids.contains_key(path)
    }

    /// Expand function `with` types: a single `|T Err|` union literal becomes
    /// `[payload, Error]` for the envelope ABI; other forms resolve normally.
    fn resolve_return_tys(&self, returns: &[crate::parser::ast::Type]) -> CResult<Vec<Ty>> {
        if returns.len() == 1
            && let crate::parser::ast::TypeKind::Union(members) = &returns[0].kind
        {
            return self.fallible_union_returns(members);
        }
        returns.iter().map(|r| self.resolve_ty(r)).collect()
    }

    fn fallible_union_returns(&self, members: &[crate::parser::ast::Type]) -> CResult<Vec<Ty>> {
        if members.len() != 2 {
            return Err(CompileError::new(
                "a fallible return `|T Err|` must have exactly two members",
                Span::default(),
                "E308",
            ));
        }
        let left = self.resolve_ty(&members[0])?;
        let right = self.resolve_ty(&members[1])?;
        let left_err = self.type_is_error(&members[0], left);
        let right_err = self.type_is_error(&members[1], right);
        match (left_err, right_err) {
            (false, true) => Ok(vec![left, Ty::Error]),
            (true, false) => Ok(vec![right, Ty::Error]),
            (true, true) => Err(CompileError::new(
                "a fallible return needs one success type and one error type",
                Span::default(),
                "E308",
            )),
            (false, false) => Err(CompileError::new(
                "a fallible return `|T Err|` requires an error type as one member",
                Span::default(),
                "E308",
            )),
        }
    }

    fn type_is_error(&self, ast: &crate::parser::ast::Type, ty: Ty) -> bool {
        if ty == Ty::Error {
            return true;
        }
        match &ast.kind {
            crate::parser::ast::TypeKind::Named(n) => self.is_error_type_path(n),
            crate::parser::ast::TypeKind::Primitive(crate::parser::ast::Primitive::Error) => true,
            _ => false,
        }
    }

    /// Fill members for an `error` declaration, copying injected tags first.
    fn fill_error_type(&mut self, d: &crate::parser::ast::ErrorDecl) -> CResult<()> {
        let id = *self.error_type_ids.get(&d.name).ok_or_else(|| {
            CompileError::new(
                format!("unknown error type '{}'", d.name),
                Span::default(),
                "E302",
            )
        })?;
        if !self.error_types[id as usize].members.is_empty() {
            return Ok(());
        }
        let mut members = Vec::new();
        if let Some(inject) = &d.inject {
            let inj_id = self.lookup_error_type(inject)?;
            self.ensure_error_filled(inj_id)?;
            for (name, tag) in &self.error_types[inj_id as usize].members {
                members.push((name.clone(), *tag));
            }
        }
        for m in &d.members {
            // `Error.OUT_OF_MEMORY` shares the short tag `OUT_OF_MEMORY` so
            // `error.OUT_OF_MEMORY` comparisons agree after injection.
            let key = if d.name == "Error" {
                m.clone()
            } else {
                format!("{}.{}", d.name, m)
            };
            let tag = self.error_tag(&key)?;
            if members.iter().any(|(n, _)| n == m) {
                return Err(CompileError::new(
                    format!("error type '{}' has duplicate member '{m}'", d.name),
                    Span::default(),
                    "E308",
                ));
            }
            members.push((m.clone(), tag));
        }
        self.error_types[id as usize].members = members;
        Ok(())
    }

    fn lookup_error_type(&self, path: &str) -> CResult<u32> {
        let name = path.rsplit('.').next().unwrap_or(path);
        self.error_type_ids
            .get(path)
            .or_else(|| self.error_type_ids.get(name))
            .copied()
            .ok_or_else(|| {
                CompileError::new(
                    format!("unknown error type '{path}'"),
                    Span::default(),
                    "E302",
                )
            })
    }

    fn ensure_error_filled(&mut self, id: u32) -> CResult<()> {
        if !self.error_types[id as usize].members.is_empty() {
            return Ok(());
        }
        // Members are filled in declaration order; an empty inject target means
        // the source declaration has not been visited yet.
        Err(CompileError::new(
            format!(
                "error type '{}' has no members to inject (declare it before dependents)",
                self.error_types[id as usize].name
            ),
            Span::default(),
            "E308",
        ))
    }

    /// Tag for an error member written as a type case (`AppError.NOT_FOUND`,
    /// or the soft `error.TAG` form inside `handle`).
    fn error_member_tag_from_type(&self, ty: &crate::parser::ast::Type) -> Option<u32> {
        let crate::parser::ast::TypeKind::Named(path) = &ty.kind else {
            return None;
        };
        // Soft `error.TAG` form (module alias or keyword): look up the member
        // across declared error types / interned tags.
        if let Some(member) = path.strip_prefix("error.") {
            if let Some(tag) = self.error_ids.get(member) {
                return Some(*tag);
            }
            for info in &self.error_types {
                if let Some((_, tag)) = info.members.iter().find(|(n, _)| n == member) {
                    return Some(*tag);
                }
            }
            return None;
        }
        let (type_name, member) = path.rsplit_once('.')?;
        let id = self
            .error_type_ids
            .get(type_name)
            .or_else(|| self.error_type_ids.get(path))?;
        self.error_types[*id as usize]
            .members
            .iter()
            .find(|(n, _)| n == member)
            .map(|(_, tag)| *tag)
    }

    /// Intern an error kind name (`error.CustomError`) to a program-unique
    /// tag, so comparisons and envelope propagation agree across functions.
    /// Tags start at 1: `env == 0` is reserved for success.
    fn error_tag(&mut self, name: &str) -> CResult<u32> {
        let next = self.error_ids.len() as u32 + 1;
        Ok(*self.error_ids.entry(name.to_string()).or_insert(next))
    }

    fn struct_layout(&self, id: u32) -> &StructLayout {
        &self.struct_layouts[id as usize]
    }

    /// The struct index of `base`'s value, resolved statically. The base of a
    /// member access is always ultimately a variable (`point`, `self`, or a
    /// nested field), so no runtime type information is needed.
    fn base_struct(&self, st: &FnState, base: &Expr) -> CResult<u32> {
        /// A struct id after following pointer layers: `pointer<Foo>` resolves
        /// to `Foo`, a struct value stays itself, everything else is an error.
        fn struct_id(ty: Ty) -> Option<u32> {
            match ty {
                Ty::Struct(id) => Some(id),
                Ty::Ptr(code) => match elem_ty(code.into()) {
                    Ty::Struct(id) => Some(id),
                    _ => None,
                },
                _ => None,
            }
        }
        match base {
            Expr::Variable { name } => match st.vars.get(name) {
                Some((_, ty, _)) => struct_id(*ty).ok_or_else(|| {
                    CompileError::new(
                        format!("'{name}' is a {ty:?}, not a struct value"),
                        Span::default(),
                        "E340",
                    )
                }),
                None => Err(CompileError::new(
                    format!("unknown variable '{name}'"),
                    Span::default(),
                    "E340",
                )),
            },
            Expr::Member {
                base: inner,
                member,
            } => {
                let outer = self.base_struct(st, inner)?;
                let lay = self.struct_layout(outer);
                let field = lay
                    .fields
                    .iter()
                    .find(|f| f.name == *member)
                    .ok_or_else(|| {
                        CompileError::new(
                            format!("struct '{}' has no field '{member}'", lay.name),
                            Span::default(),
                            "E340",
                        )
                    })?;
                struct_id(field.ty).ok_or_else(|| {
                    CompileError::new(
                        format!("field '{member}' is not a struct value"),
                        Span::default(),
                        "E340",
                    )
                })
            }
            _ => Err(CompileError::new(
                "expected a struct value before '.'",
                Span::default(),
                "E340",
            )),
        }
    }

    /// Whether a member access's base chain crosses a `pointer<T>` at any
    /// level (a pointer-typed variable or a field that is itself a pointer).
    /// Such access dereferences memory and requires an unsafe context.
    fn base_is_pointer(&self, st: &FnState, base: &Expr) -> bool {
        match base {
            Expr::Variable { name } => {
                matches!(st.vars.get(name).map(|(_, ty, _)| ty), Some(Ty::Ptr(_)))
            }
            Expr::Member {
                base: inner,
                member,
            } => {
                let field_is_ptr = self
                    .base_struct(st, inner)
                    .ok()
                    .map(|sid| {
                        self.struct_layout(sid)
                            .fields
                            .iter()
                            .any(|f| f.name == *member && matches!(f.ty, Ty::Ptr(_)))
                    })
                    .unwrap_or(false);
                field_is_ptr || self.base_is_pointer(st, inner)
            }
            _ => false,
        }
    }

    fn find_field(&self, sid: u32, member: &str) -> CResult<types::FieldLayout> {
        let lay = self.struct_layout(sid);
        lay.fields
            .iter()
            .find(|f| f.name == member)
            .cloned()
            .ok_or_else(|| {
                CompileError::new(
                    format!("struct '{}' has no field '{member}'", lay.name),
                    Span::default(),
                    "E340",
                )
            })
    }

    /// Allocate a fresh frame slot for a struct and return a pointer to it.
    fn alloc_struct(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        id: u32,
    ) -> CResult<Value> {
        let lay = self.struct_layout(id);
        let size = b.ins().iconst(irtypes::I64, lay.size as i64);
        let out = self.rt_call(b, st, "alloc", vec![size])?;
        Ok(out[0])
    }

    /// Initialize `ptr` from a `{name value ...}` literal. Each key must be an
    /// identifier matching a struct field; missing fields are zeroed so the
    /// struct is always fully defined.
    fn init_struct_fields(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        id: u32,
        ptr: Value,
        pairs: &[(Expr, Expr)],
    ) -> CResult<()> {
        // Make sure the runtime knows this struct's field kinds so it can free
        // owned heap fields if a struct value is ever dropped.
        self.emit_register_struct(b, st, id)?;
        let fields = self.struct_layout(id).fields.clone();
        for (key, value_expr) in pairs {
            let field_name = match key {
                Expr::Variable { name } => name.as_str(),
                _ => {
                    return Err(CompileError::new(
                        "struct literal field names must be identifiers",
                        Span::default(),
                        "E340",
                    ));
                }
            };
            let field = fields
                .iter()
                .find(|f| f.name == field_name)
                .cloned()
                .ok_or_else(|| {
                    CompileError::new(
                        format!(
                            "struct '{}' has no field '{field_name}'",
                            self.struct_layout(id).name
                        ),
                        Span::default(),
                        "E340",
                    )
                })?;
            // A nested struct literal `{inner {v 9}}` allocates a fresh slot
            // for the inner struct and stores a pointer to it in the field.
            if let (Ty::Struct(inner_id), Expr::Map(inner_pairs)) = (field.ty, value_expr) {
                let inner_ptr = self.alloc_struct(b, st, inner_id)?;
                self.init_struct_fields(b, st, stack, inner_id, inner_ptr, inner_pairs)?;
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    inner_ptr,
                    ptr,
                    field.offset,
                );
                continue;
            }
            // An array field `{nums [1 2 3]}` stores a pointer to a fresh
            // array slot, using the declared element type (not the inferred
            // literal type, which would be I64 for integer elements).
            if let (Ty::Array { .. }, Expr::Array(elems)) = (field.ty, value_expr) {
                let field_ty = Compiler::infer_array_count(field.ty, elems)?;
                let Ty::Array { elem, count } = field_ty else {
                    unreachable!()
                };
                let arr_ptr = self.alloc_array(b, st, scalar_ty(elem), count)?;
                self.init_array_elements(b, st, stack, scalar_ty(elem), arr_ptr, elems)?;
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    arr_ptr,
                    ptr,
                    field.offset,
                );
                continue;
            }
            // A list field `{scores (10 20)}` builds a list with the declared
            // element type and stores its handle.
            if let (Ty::List { elem }, Expr::List(elems)) = (field.ty, value_expr) {
                let handle = self.emit_list_new(b, st, elem_ty(elem))?;
                self.init_list_elements(b, st, stack, elem_ty(elem), handle, elems)?;
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    handle,
                    ptr,
                    field.offset,
                );
                continue;
            }
            // A hashmap field `{lookup {"a" 1}}` builds a map with the declared
            // key/value types and stores its handle.
            if let (Ty::Hashmap { .. }, Expr::Map(pairs)) = (field.ty, value_expr) {
                let (handle, _, _) = self.emit_map_literal(b, st, stack, pairs, Some(field.ty))?;
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    handle,
                    ptr,
                    field.offset,
                );
                continue;
            }
            self.compile_expr(b, st, stack, value_expr)?;
            let slot = self.pop_slot(st, stack, "struct field value")?;
            let val = self.coerce_or_wrap(b, st, slot.value, slot.ty, field.ty)?;
            b.ins().store(
                cranelift_codegen::ir::MemFlagsData::trusted(),
                val,
                ptr,
                field.offset,
            );
        }
        for field in &fields {
            let provided = pairs
                .iter()
                .any(|(k, _)| matches!(k, Expr::Variable { name } if name == &field.name));
            if !provided {
                let zero = b.ins().iconst(field.ty.clty(self.ptr_type), 0);
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    zero,
                    ptr,
                    field.offset,
                );
            }
        }
        Ok(())
    }

    /// Allocate a fresh heap block for a fixed-size array of `count` elements
    /// of type `elem` and return a pointer to it. Heap (rather than frame)
    /// storage keeps the address valid while the array escapes into variables,
    /// regions or callees.
    fn alloc_array(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        elem: Ty,
        count: u32,
    ) -> CResult<Value> {
        let size = ((elem.elem_size() as u64) * (count as u64)).max(1) as i64;
        let size = b.ins().iconst(irtypes::I64, size);
        let out = self.rt_call(b, st, "alloc", vec![size])?;
        Ok(out[0])
    }

    /// Store every element of an `[a b c]` literal into `ptr`, coercing each
    /// to `elem`. The literal's own element types are inferred when `elem` is
    /// a scalar, but here the declared element type wins.
    fn init_array_elements(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        elem: Ty,
        ptr: Value,
        elems: &[Expr],
    ) -> CResult<()> {
        let elem_size = elem.elem_size() as i32;
        for (i, el) in elems.iter().enumerate() {
            self.compile_expr(b, st, stack, el)?;
            let slot = self.pop_slot(st, stack, "array element")?;
            let val = coerce(b, slot.value, slot.ty, elem, self.ptr_type, st.current_span)?;
            b.ins().store(
                cranelift_codegen::ir::MemFlagsData::trusted(),
                val,
                ptr,
                i as i32 * elem_size,
            );
        }
        Ok(())
    }

    /// Resolve the element count of an array typed value, filling in an
    /// uninferred (`count == 0`) declared size from the initializer length.
    fn infer_array_count(declared: Ty, elems: &[Expr]) -> CResult<Ty> {
        let Ty::Array { elem, count } = declared else {
            return Ok(declared);
        };
        if count != 0 && count != elems.len() as u32 {
            return Err(CompileError::new(
                format!(
                    "array initializer has {} element(s) but the type declares {count}",
                    elems.len()
                ),
                Span::default(),
                "E345",
            ));
        }
        Ok(Ty::Array {
            elem,
            count: elems.len() as u32,
        })
    }

    fn declare_function(&mut self, f: &Function, name: &str) -> CResult<()> {
        let mut param_tys = Vec::with_capacity(f.params.len());
        let mut sig = self.module.make_signature();
        for p in &f.params {
            let ty = self.resolve_ty(&p.ty)?;
            sig.params.push(AbiParam::new(ty.clty(self.ptr_type)));
            param_tys.push(ty);
        }
        let mut return_tys = self.resolve_return_tys(&f.returns)?;
        return_tys.retain(|t| *t != Ty::Void);
        for ty in &return_tys {
            if *ty == Ty::Error {
                // Envelope ABI replaces individual slots below.
                continue;
            }
            sig.returns.push(AbiParam::new(ty.clty(self.ptr_type)));
        }
        // A fallible function returns an envelope `(i64 env, i64 payload)`:
        // env is 0 on success or the error tag on failure, and payload carries
        // the success value (or 0).
        if error_return(&return_tys)?.is_some() {
            sig.returns.clear();
            sig.returns.push(AbiParam::new(irtypes::I64));
            sig.returns.push(AbiParam::new(irtypes::I64));
        }
        let id = self.module.declare_function(name, Linkage::Export, &sig)?;
        self.sigs.insert(name.to_string(), sig);
        self.sig_tys
            .insert(name.to_string(), (param_tys, return_tys));
        self.func_ids.insert(name.to_string(), id);
        Ok(())
    }

    fn compile_function(
        &mut self,
        f: &Function,
        name: &str,
        module: Option<&str>,
        is_method: bool,
    ) -> CResult<()> {
        // Nested functions are only callable from this body. Declare and
        // compile them first so calls in the parent resolve.
        let nested: Vec<Function> = f
            .body
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Function(nf) => Some(nf.clone()),
                _ => None,
            })
            .collect();
        for nf in &nested {
            let nname = format!("{name}::{}", nf.name);
            if !self.func_ids.contains_key(&nname) {
                if nf.is_unsafe {
                    self.unsafe_funcs.insert(nname.clone());
                }
                if matches!(nf.visibility, Some(Visibility::Public)) {
                    self.public_funcs.insert(nname.clone());
                }
                self.declare_function(nf, &nname)?;
            }
        }
        for nf in &nested {
            let nname = format!("{name}::{}", nf.name);
            // Compile each nested body once (parent is the only caller of this
            // path; top-level Pass D never sees nested decls).
            if self.sigs.contains_key(&nname) && !self.defined_funcs.contains(&nname) {
                self.defined_funcs.insert(nname.clone());
                self.compile_function(nf, &nname, module, false)?;
            }
        }

        let sig = self.sigs.get(name).cloned().unwrap();
        let id = *self.func_ids.get(name).unwrap();

        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let returns = self.resolve_return_tys(&f.returns)?;
        // `void` means "no value"; it contributes no return slot (matching
        // `declare_function`, which skips it in the signature).
        let returns: Vec<Ty> = returns.into_iter().filter(|t| *t != Ty::Void).collect();
        let error_value = error_return(&returns)?;
        // An unsafe function's whole body is an unsafe context.
        let unsafe_depth = u32::from(f.is_unsafe);
        let mut local_funcs = HashMap::new();
        for nf in &nested {
            local_funcs.insert(nf.name.clone(), format!("{name}::{}", nf.name));
        }
        let mut st = FnState {
            vars: HashMap::new(),
            loops: Vec::new(),
            returns,
            error_value,
            unsafe_depth,
            frefs: HashMap::new(),
            rt: HashMap::new(),
            module: module.map(str::to_string),
            owns: std::collections::HashSet::new(),
            borrowed: std::collections::HashSet::new(),
            moved: std::collections::HashSet::new(),
            moved_vars: std::collections::HashSet::new(),
            region_attached: HashMap::new(),
            region_freed: std::collections::HashSet::new(),
            deferred: Vec::new(),
            registered_descs: std::collections::HashSet::new(),
            registered_unions: std::collections::HashSet::new(),
            terminated: false,
            had_error: false,
            local_funcs,
            current_span: Span::default(),
            move_sites: HashMap::new(),
            borrow_sites: HashMap::new(),
        };

        // Import every declared function so any callee (free or method) can be
        // resolved later; frefs must be created before the FunctionBuilder
        // takes ownership of `ctx.func`.
        for (callee, &fid) in &self.func_ids {
            let fr = self.module.declare_func_in_func(fid, &mut ctx.func);
            st.frefs.insert(callee.clone(), fr);
        }
        for (name, &fid) in &self.runtime_ids {
            let fr = self.module.declare_func_in_func(fid, &mut ctx.func);
            st.rt.insert(name.clone(), fr);
        }
        // Global values for string literals referenced inside this function.
        self.fn_gvs.clear();
        for (text, &did) in &self.string_ids {
            let gv = self.module.declare_data_in_func(did, &mut ctx.func);
            self.fn_gvs.insert(text.clone(), gv);
        }
        // Global values for each struct's FieldDesc table.
        self.struct_desc_gvs.clear();
        for (&id, &did) in &self.struct_desc_ids {
            let gv = self.module.declare_data_in_func(did, &mut ctx.func);
            self.struct_desc_gvs.insert(id, gv);
        }
        // Global values for each union's member-kind table.
        self.union_desc_gvs.clear();
        for (&id, &did) in &self.union_desc_ids {
            let gv = self.module.declare_data_in_func(did, &mut ctx.func);
            self.union_desc_gvs.insert(id, gv);
        }

        let mut fbctx = FunctionBuilderContext::new();
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fbctx);

        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);

        let params_ty: Vec<Ty> = f
            .params
            .iter()
            .map(|p| self.resolve_ty(&p.ty))
            .collect::<CResult<_>>()?;
        let param_vals: Vec<Value> = b.block_params(entry).to_vec();
        let mut stack: Vec<Slot> = Vec::new();
        for (i, t) in params_ty.iter().enumerate() {
            let modifier = f.params[i].modifier;
            // `copy` deep-copies heap values into an owned local; other heap
            // params are borrowed from the caller. Scalars are always trivial.
            let (value, own) = if matches!(modifier, Some(ParamModifier::Copy)) && self.is_heap(*t)
            {
                let cloned = self.emit_deep_copy(&mut b, &mut st, param_vals[i], *t)?;
                (cloned, Own::Owned)
            } else if self.is_heap(*t) {
                (param_vals[i], Own::Borrow)
            } else {
                (param_vals[i], Own::Trivial)
            };
            // `mutable` on `reference<T>` is checked when the reference is
            // formed at the call site; the callee still receives a borrow.
            let _ = matches!(modifier, Some(ParamModifier::Mutable));
            stack.push(Slot { value, ty: *t, own });
        }

        // In a method body, the receiver is param 0. Bind `self` to it so a
        // `self const reference<Point>` declaration resolves without relying
        // on stack position.
        if is_method && let Some((t, v)) = params_ty.first().zip(param_vals.first()) {
            let var = b.declare_var(t.clty(self.ptr_type));
            b.def_var(var, *v);
            // The receiver is borrowed from the caller; the callee must not
            // free it at scope exit.
            let own = if self.is_heap(*t) {
                Own::Borrow
            } else {
                Own::Trivial
            };
            st.vars.insert("self".to_string(), (var, *t, own));
        }

        self.compile_body(&mut b, &mut st, &mut stack, &f.body)?;

        // Implicit termination for a function falling off the end. Skipped
        // when an explicit `return` already ended the flow (the current block
        // is dead and the compile-time stack was already drained).
        if st.had_error {
            // Body diagnostics were recorded; abandon IR so a half-built
            // function does not poison later units.
            if let Some(block) = b.current_block() {
                let has_terminator = b
                    .func
                    .layout
                    .last_inst(block)
                    .is_some_and(|inst| b.func.dfg.insts[inst].opcode().is_terminator());
                if !has_terminator {
                    b.ins().trap(TrapCode::unwrap_user(2));
                }
            }
            b.seal_all_blocks();
            b.finalize();
            self.module.clear_context(&mut ctx);
            return Ok(());
        } else if st.terminated {
            // The block is dead; nothing left to emit.
        } else if st.error_value.is_some() {
            let vals = self.pop_return_values(&mut b, &mut st, &mut stack)?;
            self.emit_scope_exit(&mut b, &mut st, &mut stack)?;
            b.ins().return_(&vals);
        } else if st.returns.is_empty() {
            self.emit_scope_exit(&mut b, &mut st, &mut stack)?;
            b.ins().return_(&[]);
        } else if stack.len() >= st.returns.len() {
            let vals = self.pop_return_values(&mut b, &mut st, &mut stack)?;
            self.emit_scope_exit(&mut b, &mut st, &mut stack)?;
            b.ins().return_(&vals);
        } else {
            b.ins().trap(TrapCode::unwrap_user(1));
        }

        b.seal_all_blocks();
        b.finalize();
        let ir_text = format!("IR for {name}:\n{}\n", ctx.func.display());
        self.ir_dump.push_str(&ir_text);
        if std::env::var("YARROW_DBG_IR").is_ok() {
            eprint!("{ir_text}");
        }
        if self.check_only {
            // Analysis complete; discard CLIF instead of installing it in the JIT.
            self.module.clear_context(&mut ctx);
            return Ok(());
        }
        if let Err(e) = self.module.define_function(id, &mut ctx) {
            return Err(e.into());
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    fn compile_body(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        stmts: &[Stmt],
    ) -> CResult<()> {
        for s in stmts {
            if self.errors.is_at_limit() {
                st.had_error = true;
                break;
            }
            match self.compile_stmt(b, st, stack, s) {
                Ok(()) => {}
                Err(e) => {
                    self.report(e);
                    st.had_error = true;
                    // Independent later statements should not inherit a
                    // corrupted operand stack from the failed one.
                    stack.clear();
                }
            }
        }
        Ok(())
    }

    fn compile_stmt(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        s: &Stmt,
    ) -> CResult<()> {
        st.current_span = s.span;
        match &s.kind {
            StmtKind::Expr(e) => {
                if self.emit_loop_control(b, st, e)? {
                    // `loop.break` / `loop.continue` terminate this block.
                } else {
                    self.compile_expr(b, st, stack, e)?;
                }
            }

            StmtKind::VarDecl {
                name,
                mutability,
                ty,
                value,
            } => {
                let mut t = self.resolve_ty(ty)?;
                // `self` was already bound to the receiver at function entry;
                // the `self const reference<Point>` declaration is a no-op.
                if name == "self" && st.vars.contains_key("self") {
                    return Ok(());
                }
                // Fill in an inferred array size from the initializer before
                // choosing how to build the value.
                if let (Ty::Array { .. }, Some(Expr::Array(elems))) = (t, value) {
                    t = Compiler::infer_array_count(t, elems)?;
                }
                let (_slot, val, val_ty) = match value {
                    Some(Expr::Map(pairs)) if matches!(t, Ty::Struct(_)) => {
                        // Struct literal `{x 5 y 20}`: allocate a slot and
                        // store each field by name.
                        let Ty::Struct(id) = t else { unreachable!() };
                        let ptr = self.alloc_struct(b, st, id)?;
                        self.init_struct_fields(b, st, stack, id, ptr, pairs)?;
                        (
                            Slot {
                                value: ptr,
                                ty: t,
                                own: Own::Owned,
                            },
                            ptr,
                            t,
                        )
                    }
                    Some(Expr::Seq(elems)) if matches!(t, Ty::Struct(_)) => {
                        // The parser merges every preceding stack op into the
                        // initializer; only the trailing map is the struct
                        // literal, the rest are side effects.
                        if let Some((Expr::Map(pairs), _)) = elems.last() {
                            for (el, span) in &elems[..elems.len() - 1] {
                                st.current_span = *span;
                                self.compile_expr(b, st, stack, el)?;
                            }
                            let Ty::Struct(id) = t else { unreachable!() };
                            let ptr = self.alloc_struct(b, st, id)?;
                            self.init_struct_fields(b, st, stack, id, ptr, pairs)?;
                            (
                                Slot {
                                    value: ptr,
                                    ty: t,
                                    own: Own::Owned,
                                },
                                ptr,
                                t,
                            )
                        } else {
                            for (el, span) in elems {
                                st.current_span = *span;
                                self.compile_expr(b, st, stack, el)?;
                            }
                            let slot = self.pop_slot(st, stack, "value")?;
                            (slot, slot.value, slot.ty)
                        }
                    }
                    Some(Expr::Seq(elems))
                        if matches!(t, Ty::Hashmap { .. } | Ty::List { .. } | Ty::Array { .. }) =>
                    {
                        // Same as struct: prior words (e.g. `… call unwrap`) are
                        // side effects; the trailing container is the value.
                        for (el, span) in &elems[..elems.len().saturating_sub(1)] {
                            st.current_span = *span;
                            self.compile_expr(b, st, stack, el)?;
                        }
                        let (last, last_span) = elems.last().ok_or_else(|| {
                            CompileError::new("empty initializer sequence", st.current_span, "E306")
                        })?;
                        st.current_span = *last_span;
                        match (t, last) {
                            (Ty::Hashmap { .. }, Expr::Map(pairs)) => {
                                let (handle, _, _) =
                                    self.emit_map_literal(b, st, stack, pairs, Some(t))?;
                                (
                                    Slot {
                                        value: handle,
                                        ty: t,
                                        own: Own::Owned,
                                    },
                                    handle,
                                    t,
                                )
                            }
                            (Ty::List { elem }, Expr::List(list_elems)) => {
                                let (handle, _) = self.emit_list_literal(
                                    b,
                                    st,
                                    stack,
                                    list_elems,
                                    Some(elem_ty(elem)),
                                )?;
                                (
                                    Slot {
                                        value: handle,
                                        ty: t,
                                        own: Own::Owned,
                                    },
                                    handle,
                                    t,
                                )
                            }
                            (Ty::Array { elem, count }, Expr::Array(arr_elems)) => {
                                let elem = scalar_ty(elem);
                                let ptr = self.alloc_array(b, st, elem, count)?;
                                self.init_array_elements(b, st, stack, elem, ptr, arr_elems)?;
                                (
                                    Slot {
                                        value: ptr,
                                        ty: t,
                                        own: Own::Owned,
                                    },
                                    ptr,
                                    t,
                                )
                            }
                            _ => {
                                self.compile_expr(b, st, stack, last)?;
                                let slot = self.pop_slot(st, stack, "value")?;
                                (slot, slot.value, slot.ty)
                            }
                        }
                    }
                    Some(Expr::Array(elems)) if matches!(t, Ty::Array { .. }) => {
                        let Ty::Array { elem, count } = t else {
                            unreachable!()
                        };
                        let elem = scalar_ty(elem);
                        let ptr = self.alloc_array(b, st, elem, count)?;
                        self.init_array_elements(b, st, stack, elem, ptr, elems)?;
                        (
                            Slot {
                                value: ptr,
                                ty: t,
                                own: Own::Owned,
                            },
                            ptr,
                            t,
                        )
                    }
                    Some(Expr::List(elems)) if matches!(t, Ty::List { .. }) => {
                        let Ty::List { elem } = t else { unreachable!() };
                        let (handle, _) =
                            self.emit_list_literal(b, st, stack, elems, Some(elem_ty(elem)))?;
                        (
                            Slot {
                                value: handle,
                                ty: t,
                                own: Own::Owned,
                            },
                            handle,
                            t,
                        )
                    }
                    Some(Expr::Map(pairs)) if matches!(t, Ty::Hashmap { .. }) => {
                        let (handle, _, _) = self.emit_map_literal(b, st, stack, pairs, Some(t))?;
                        (
                            Slot {
                                value: handle,
                                ty: t,
                                own: Own::Owned,
                            },
                            handle,
                            t,
                        )
                    }
                    Some(e) => {
                        self.compile_expr(b, st, stack, e)?;
                        let slot = self.pop_slot(st, stack, "value")?;
                        (slot, slot.value, slot.ty)
                    }
                    None => {
                        let slot = self.pop_slot(st, stack, "value")?;
                        (slot, slot.value, slot.ty)
                    }
                };
                let val = self.coerce_or_wrap(b, st, val, val_ty, t)?;
                self.claim(st, val, t);
                let var = b.declare_var(t.clty(self.ptr_type));
                b.def_var(var, val);
                let _ = mutability;
                // A value with an active borrow is not owned by this variable
                // (its true owner frees it); everything heap is owned here.
                let var_own = if st.borrowed.contains(&val) {
                    Own::Borrow
                } else if self.is_heap(t) {
                    Own::Owned
                } else {
                    Own::Trivial
                };
                st.vars.insert(name.clone(), (var, t, var_own));
            }

            StmtKind::Set { target, value } => match target {
                Expr::Variable { name } => {
                    self.require_not_moved_var(st, name)?;
                    let (var, t, _old_own) = st.vars.get(name).cloned().ok_or_else(|| {
                        CompileError::new(
                            format!("unknown variable '{name}'"),
                            st.current_span,
                            "E320",
                        )
                    })?;
                    let cur = b.use_var(var);
                    self.require_region_live(st, cur)?;
                    self.require_not_borrowed(st, cur, "set")?;
                    // A struct literal set re-initializes the existing
                    // storage in place, so the old value must NOT be freed
                    // first (the pointer is reused).
                    let trailing_map = match value {
                        Some(Expr::Map(_)) => true,
                        Some(Expr::Seq(elems)) => matches!(elems.last(), Some((Expr::Map(_), _))),
                        _ => false,
                    };
                    let reuses_ptr = trailing_map && matches!(t, Ty::Struct(_));
                    // Drop the value the variable currently owns (the runtime
                    // guards against double frees). Borrowed variables own
                    // nothing, so their value is left for its true owner.
                    if self.is_heap(t) && !reuses_ptr {
                        let old = Slot {
                            value: cur,
                            ty: t,
                            own: Own::Owned,
                        };
                        self.emit_drop(b, st, old)?;
                    }
                    let (_slot, val, val_ty) = match value {
                        Some(Expr::Map(pairs)) if matches!(t, Ty::Struct(_)) => {
                            let Ty::Struct(id) = t else { unreachable!() };
                            let ptr = b.use_var(var);
                            self.init_struct_fields(b, st, stack, id, ptr, pairs)?;
                            (
                                Slot {
                                    value: ptr,
                                    ty: t,
                                    own: Own::Owned,
                                },
                                ptr,
                                t,
                            )
                        }
                        Some(Expr::Seq(elems)) if matches!(t, Ty::Struct(_)) => {
                            // The parser merges every preceding stack op into
                            // the initializer; only the trailing map is the
                            // struct literal, the rest are side effects.
                            if let Some((Expr::Map(pairs), _)) = elems.last() {
                                for (el, span) in &elems[..elems.len() - 1] {
                                    st.current_span = *span;
                                    self.compile_expr(b, st, stack, el)?;
                                }
                                let Ty::Struct(id) = t else { unreachable!() };
                                let ptr = b.use_var(var);
                                self.init_struct_fields(b, st, stack, id, ptr, pairs)?;
                                (
                                    Slot {
                                        value: ptr,
                                        ty: t,
                                        own: Own::Owned,
                                    },
                                    ptr,
                                    t,
                                )
                            } else {
                                for (el, span) in elems {
                                    st.current_span = *span;
                                    self.compile_expr(b, st, stack, el)?;
                                }
                                let slot = self.pop_slot(st, stack, "value")?;
                                (slot, slot.value, slot.ty)
                            }
                        }
                        Some(Expr::Array(elems)) if matches!(t, Ty::Array { .. }) => {
                            let t = Compiler::infer_array_count(t, elems)?;
                            let Ty::Array { elem, count } = t else {
                                unreachable!()
                            };
                            let elem = scalar_ty(elem);
                            let ptr = self.alloc_array(b, st, elem, count)?;
                            self.init_array_elements(b, st, stack, elem, ptr, elems)?;
                            (
                                Slot {
                                    value: ptr,
                                    ty: t,
                                    own: Own::Owned,
                                },
                                ptr,
                                t,
                            )
                        }
                        Some(Expr::List(elems)) if matches!(t, Ty::List { .. }) => {
                            let Ty::List { elem } = t else { unreachable!() };
                            let (handle, _) =
                                self.emit_list_literal(b, st, stack, elems, Some(elem_ty(elem)))?;
                            (
                                Slot {
                                    value: handle,
                                    ty: t,
                                    own: Own::Owned,
                                },
                                handle,
                                t,
                            )
                        }
                        Some(Expr::Map(pairs)) if matches!(t, Ty::Hashmap { .. }) => {
                            let (handle, _, _) =
                                self.emit_map_literal(b, st, stack, pairs, Some(t))?;
                            (
                                Slot {
                                    value: handle,
                                    ty: t,
                                    own: Own::Owned,
                                },
                                handle,
                                t,
                            )
                        }
                        Some(e) => {
                            self.compile_expr(b, st, stack, e)?;
                            let slot = self.pop_slot(st, stack, "value")?;
                            (slot, slot.value, slot.ty)
                        }
                        None => {
                            let slot = self.pop_slot(st, stack, "value")?;
                            (slot, slot.value, slot.ty)
                        }
                    };
                    let val = self.coerce_or_wrap(b, st, val, val_ty, t)?;
                    self.claim(st, val, t);
                    b.def_var(var, val);
                    let var_own = if st.borrowed.contains(&val) {
                        Own::Borrow
                    } else if self.is_heap(t) {
                        Own::Owned
                    } else {
                        Own::Trivial
                    };
                    st.vars.insert(name.clone(), (var, t, var_own));
                }
                Expr::Member { base, member } => {
                    if self.base_is_pointer(st, base) {
                        self.require_unsafe(st, "field 'set' through a pointer")?;
                    }
                    let sid = self.base_struct(st, base)?;
                    let field = self.find_field(sid, member)?.clone();
                    self.compile_expr(b, st, stack, base)?;
                    let ptr = self.pop_slot(st, stack, "field set target")?;
                    let (val, val_ty) = match value {
                        Some(Expr::List(elems)) if matches!(field.ty, Ty::List { .. }) => {
                            let Ty::List { elem } = field.ty else {
                                unreachable!()
                            };
                            let (handle, _) =
                                self.emit_list_literal(b, st, stack, elems, Some(elem_ty(elem)))?;
                            (handle, field.ty)
                        }
                        Some(Expr::Map(pairs)) if matches!(field.ty, Ty::Hashmap { .. }) => {
                            let (handle, _, _) =
                                self.emit_map_literal(b, st, stack, pairs, Some(field.ty))?;
                            (handle, field.ty)
                        }
                        Some(e) => {
                            self.compile_expr(b, st, stack, e)?;
                            let slot = self.pop_slot(st, stack, "value")?;
                            (slot.value, slot.ty)
                        }
                        None => {
                            let slot = self.pop_slot(st, stack, "value")?;
                            (slot.value, slot.ty)
                        }
                    };
                    let val = self.coerce_or_wrap(b, st, val, val_ty, field.ty)?;
                    // Skipping the old member would leak it when switching a
                    // union field: free the previous value before overwriting
                    // (the runtime guards double frees on the struct drop).
                    if matches!(field.ty, Ty::Union(_)) {
                        let old = b.ins().load(
                            self.ptr_type,
                            cranelift_codegen::ir::MemFlagsData::trusted(),
                            ptr.value,
                            field.offset,
                        );
                        let kind = b.ins().iconst(irtypes::I64, kind_code(field.ty) as i64);
                        self.rt_call(b, st, "free_value", vec![old, kind])?;
                    }
                    b.ins().store(
                        cranelift_codegen::ir::MemFlagsData::trusted(),
                        val,
                        ptr.value,
                        field.offset,
                    );
                }
                _ => {
                    return Err(CompileError::unsupported(
                        "field 'set' is not yet supported",
                        st.current_span,
                        "E301",
                    ));
                }
            },

            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let prev = st.terminated;
                self.emit_if(b, st, stack, condition, then_branch, else_branch)?;
                // `emit_if` sets `terminated` when both branches return/break.
                if !st.terminated {
                    st.terminated = prev;
                }
            }

            StmtKind::Match {
                value,
                cases,
                else_branch,
            } => {
                let prev = st.terminated;
                self.emit_match(b, st, stack, value, cases, else_branch)?;
                st.terminated = prev;
            }

            StmtKind::For { source, body } => {
                let prev = st.terminated;
                self.emit_for(b, st, stack, source, body)?;
                st.terminated = prev;
            }

            StmtKind::Return { .. } => {
                self.emit_return(b, st, stack)?;
                st.terminated = true;
            }

            StmtKind::Function(_)
            | StmtKind::Struct(_)
            | StmtKind::Implement(_)
            | StmtKind::Enum(_)
            | StmtKind::Union(_)
            | StmtKind::Error(_)
            | StmtKind::Require { .. } => {
                // Only meaningful at program level; no-op inside a body.
            }

            StmtKind::Defer { body } => {
                // Schedule the body to run in reverse order at scope exit,
                // so a `myRegion @free_region call` runs after the region's
                // values have been dropped.
                st.deferred.push(body.clone());
            }

            StmtKind::Handle { body, fallback } => {
                let prev = st.terminated;
                self.emit_handle(b, st, stack, body, fallback.as_ref())?;
                st.terminated = prev;
            }

            StmtKind::Move { target, source } => self.emit_move(b, st, stack, target, source)?,

            StmtKind::Unsafe { body } => {
                // Compile the body with an active unsafe context.
                st.unsafe_depth += 1;
                let result = self.compile_body(b, st, stack, body);
                st.unsafe_depth -= 1;
                result?;
            }

            // The parser extracts `fallback` out of `handle` bodies into
            // `Handle.fallback`, so a bare `Fallback` statement is unreachable
            // here; treat it as a no-op rather than a hard error.
            StmtKind::Fallback { .. } => {}
        }
        Ok(())
    }

    fn emit_return(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<()> {
        let vals = if st.returns.is_empty() {
            Vec::new()
        } else {
            self.pop_return_values(b, st, stack)?
        };
        self.emit_scope_exit(b, st, stack)?;
        b.ins().return_(&vals);
        // The rest of the function is unreachable; the compile-time stack is
        // dead, so clear it to stop the implicit fallthrough return from
        // picking up leftovers (e.g. a method receiver).
        self.dead_block(b);
        Ok(())
    }

    fn pop_return_values(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<Vec<Value>> {
        if let Some(payload_ty) = st.error_value {
            // Fallible `|T Err|`: the body leaves either the success value or
            // an error value on the stack.
            let zero = b.ins().iconst(irtypes::I64, 0);
            if matches!(stack.last(), Some(s) if s.ty == Ty::Error) {
                let err = stack.pop().unwrap();
                return Ok(vec![err.value, zero]);
            }
            let payload = if payload_ty == Ty::Void {
                zero
            } else {
                let slot = self.pop_slot(st, stack, "return value")?;
                // The callee owns heap values it returns; the caller claims
                // them from the envelope, so the callee must not free them.
                if self.is_heap(slot.ty) {
                    st.moved.insert(slot.value);
                }
                coerce(
                    b,
                    slot.value,
                    slot.ty,
                    Ty::I64,
                    self.ptr_type,
                    st.current_span,
                )?
            };
            return Ok(vec![zero, payload]);
        }
        let n = st.returns.len();
        if stack.len() < n {
            return Err(CompileError::new(
                format!(
                    "function returns {n} value(s) but only {} on the stack",
                    stack.len()
                ),
                st.current_span,
                "E323",
            )
            .with_note(self.stack_effect_note(stack, &st.returns))
            .with_help("leave the declared return values on the stack before `return` or falling off the end"));
        }
        let tail = stack.split_off(stack.len() - n);
        let wants: Vec<Ty> = st.returns.clone();
        let mut out = Vec::with_capacity(n);
        for (slot, want) in tail.iter().zip(&wants) {
            // Heap-typed return values transfer ownership to the caller, so
            // the callee must not free them at scope exit (this also covers a
            // `myStr return` that borrows the value out of a variable).
            if self.is_heap(slot.ty) {
                st.moved.insert(slot.value);
            }
            out.push(self.coerce_or_wrap(b, st, slot.value, slot.ty, *want)?);
        }
        Ok(out)
    }

    /// `value unwrap`: if the top of the stack is an error envelope from a
    /// fallible call, keep the success payload or propagate the error when this
    /// function itself returns `|T Err|`. Applied to anything that cannot fail,
    /// `unwrap` is an identity. Rejected at compile time when the caller cannot
    /// error.
    fn emit_unwrap(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<()> {
        // Only error envelopes sit on the stack as `Ty::Error`.
        if !matches!(stack.last(), Some(s) if s.ty == Ty::Error) {
            return Ok(());
        }
        if st.error_value.is_none() {
            return Err(CompileError::new(
                "'unwrap' requires the caller to declare a fallible return (|T Err|)",
                st.current_span,
                "E308",
            )
            .with_primary_message("`unwrap` here")
            .with_note("`unwrap` propagates failure to the caller, so the caller must be fallible")
            .with_help("declare `with |T Err|` on this function, or use `handle ... fallback ... end` instead"));
        }
        let env = stack.pop().unwrap();
        let payload = stack.pop().unwrap();

        let zero = b.ins().iconst(irtypes::I64, 0);
        let ok = b.ins().icmp(IntCC::Equal, env.value, zero);
        let ok_blk = b.create_block();
        let err_blk = b.create_block();
        let void = payload.ty == Ty::Void;
        let payload_param = if void {
            None
        } else {
            Some(b.append_block_param(ok_blk, payload.ty.clty(self.ptr_type)))
        };
        let err_env_param = b.append_block_param(err_blk, irtypes::I64);
        // The envelope payload is only meaningful on success (the callee
        // returns 0 on error), so drop it from the ownership set before
        // branching; the success path re-claims its merge value.
        if !void {
            st.owns.remove(&payload.value);
        }
        let ok_args: Vec<BlockArg> = if void {
            vec![]
        } else {
            vec![BlockArg::Value(payload.value)]
        };
        b.ins()
            .brif(ok, ok_blk, &ok_args, err_blk, &[BlockArg::Value(env.value)]);

        // Error: propagate as this function's fallible return.
        b.switch_to_block(err_blk);
        stack.push(Slot {
            value: err_env_param,
            ty: Ty::Error,
            own: Own::Trivial,
        });
        self.emit_return(b, st, stack)?;
        self.dead_block(b);

        // Success: the payload flows out of the merge with its declared type.
        b.switch_to_block(ok_blk);
        if !void {
            self.claim(st, payload_param.unwrap(), payload.ty);
            stack.push(Slot {
                value: payload_param.unwrap(),
                ty: payload.ty,
                own: if self.is_heap(payload.ty) {
                    Own::Owned
                } else {
                    Own::Trivial
                },
            });
        }
        Ok(())
    }

    /// `expr handle <body> end`: if `expr` left an error envelope, run `<body>`
    /// with the error value on the stack; otherwise keep the payload and skip
    /// the body. The body's own result (`handle v end` fallback or a match
    /// consuming the error) is the result of the whole handle, merged with the
    /// success payload.
    fn emit_handle(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        body: &[Stmt],
        fallback: Option<&Expr>,
    ) -> CResult<()> {
        if !matches!(stack.last(), Some(s) if s.ty == Ty::Error) {
            return Ok(());
        }
        let env = stack.pop().unwrap();
        let payload = stack.pop().unwrap();
        let pre = stack.clone();

        let zero = b.ins().iconst(irtypes::I64, 0);
        let is_err = b.ins().icmp(IntCC::NotEqual, env.value, zero);
        let body_blk = b.create_block();
        let end = b.create_block();

        let void = payload.ty == Ty::Void;
        let success_tys: Vec<Ty> = if void { vec![] } else { vec![payload.ty] };
        for t in &success_tys {
            b.append_block_param(end, t.clty(self.ptr_type));
        }
        if !void {
            b.append_block_param(body_blk, payload.ty.clty(self.ptr_type));
        }
        b.append_block_param(body_blk, irtypes::I64);
        if !void {
            st.owns.remove(&payload.value);
        }
        let ok_args: Vec<BlockArg> = if void {
            vec![]
        } else {
            vec![BlockArg::Value(payload.value)]
        };
        let mut body_args: Vec<BlockArg> = if void {
            vec![]
        } else {
            vec![BlockArg::Value(payload.value)]
        };
        body_args.push(BlockArg::Value(env.value));
        b.ins().brif(is_err, body_blk, &body_args, end, &ok_args);

        // Error path: the payload is meaningless (0 from the callee), so it is
        // just dead; push the error value and run the body.
        b.switch_to_block(body_blk);
        let err = b.block_params(body_blk)[if void { 0 } else { 1 }];
        *stack = pre.clone();
        stack.push(Slot {
            value: err,
            ty: Ty::Error,
            own: Own::Trivial,
        });
        let err_idx = stack.len() - 1;
        self.compile_body(b, st, stack, body)?;
        // Drop the error slot if the body did not consume it. Match / other
        // control flow may replace the SSA value, so compare by slot index and
        // type rather than Value identity.
        if stack.len() > err_idx && stack[err_idx].ty == Ty::Error {
            stack.remove(err_idx);
        } else if let Some(i) = stack.iter().rposition(|s| s.ty == Ty::Error)
            && i >= pre.len()
        {
            stack.remove(i);
        }
        // A `fallback` value is pushed only on the error path, after the body.
        if let Some(fb) = fallback {
            self.compile_expr(b, st, stack, fb)?;
        }
        // The handle's result is what the body left beyond `pre`, merged with
        // the success payload.
        let results = stack.split_off(pre.len());
        if results.len() != success_tys.len() {
            return Err(CompileError::new(
                format!(
                    "'handle' body must leave {} value(s) to match the success value, left {}",
                    success_tys.len(),
                    results.len()
                ),
                st.current_span,
                "E328",
            )
            .with_note(self.stack_effect_note(&results, &success_tys))
            .with_help("make the handle body leave the same number and types of values as the success path"));
        }
        let mut args: Vec<BlockArg> = Vec::with_capacity(results.len());
        for (s, want) in results.iter().zip(&success_tys) {
            let v = coerce(b, s.value, s.ty, *want, self.ptr_type, st.current_span)?;
            args.push(BlockArg::Value(v));
        }
        b.ins().jump(end, &args);

        b.switch_to_block(end);
        *stack = pre;
        for (i, t) in success_tys.iter().enumerate() {
            let v = b.block_params(end)[i];
            self.claim(st, v, *t);
            stack.push(Slot {
                value: v,
                ty: *t,
                own: if self.is_heap(*t) {
                    Own::Owned
                } else {
                    Own::Trivial
                },
            });
        }
        Ok(())
    }

    /// Create a fresh block we can throw away dead code into. The previous
    /// block is already terminated by the caller (return/break/continue), so we
    /// just create and switch to the new block without emitting anything.
    fn dead_block(&mut self, b: &mut FunctionBuilder) {
        let dead = b.create_block();
        b.switch_to_block(dead);
    }

    /// Format a type the way users write it (`list<i32>`, not debug dumps).
    fn format_ty(&self, ty: Ty) -> String {
        match ty {
            Ty::Bool => "bool".to_string(),
            Ty::I8 => "i8".to_string(),
            Ty::I16 => "i16".to_string(),
            Ty::I32 => "i32".to_string(),
            Ty::I64 => "i64".to_string(),
            Ty::I128 => "i128".to_string(),
            Ty::U8 => "u8".to_string(),
            Ty::U16 => "u16".to_string(),
            Ty::U32 => "u32".to_string(),
            Ty::U64 => "u64".to_string(),
            Ty::U128 => "u128".to_string(),
            Ty::Rune => "rune".to_string(),
            Ty::F16 => "f16".to_string(),
            Ty::F32 => "f32".to_string(),
            Ty::F64 => "f64".to_string(),
            Ty::F128 => "f128".to_string(),
            Ty::Void => "void".to_string(),
            Ty::String => "string".to_string(),
            Ty::Error => "error".to_string(),
            Ty::List { elem } => format!("list<{}>", self.format_ty(elem_ty(elem))),
            Ty::Hashmap { key, value } => format!(
                "hashmap<{} {}>",
                self.format_ty(elem_ty(key)),
                self.format_ty(elem_ty(value))
            ),
            Ty::Array { elem, count } => {
                let elem_s = self.format_ty(scalar_ty(elem));
                if count == 0 {
                    format!("array<{elem_s}>")
                } else {
                    format!("array<{elem_s} {count}>")
                }
            }
            Ty::Ptr(code) => {
                if code == 0x50 {
                    "pointer<_>".to_string()
                } else {
                    format!("pointer<{}>", self.format_ty(elem_ty(u64::from(code))))
                }
            }
            Ty::Struct(id) => self
                .struct_layouts
                .get(id as usize)
                .map(|l| l.name.clone())
                .unwrap_or_else(|| format!("struct#{id}")),
            Ty::Union(id) => self
                .unions
                .get(id as usize)
                .map(|u| u.name.clone())
                .unwrap_or_else(|| format!("union#{id}")),
            Ty::Enum(id) => self
                .enums
                .get(id as usize)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| format!("enum#{id}")),
        }
    }

    fn format_stack_tys(&self, tys: &[Ty]) -> String {
        let parts: Vec<String> = tys.iter().map(|t| self.format_ty(*t)).collect();
        format!("[{}]", parts.join(", "))
    }

    fn format_stack_slots(&self, stack: &[Slot]) -> String {
        let tys: Vec<Ty> = stack.iter().map(|s| s.ty).collect();
        self.format_stack_tys(&tys)
    }

    /// Note of the form `stack: [found…] → expected [wanted…]`.
    fn stack_effect_note(&self, found: &[Slot], expected: &[Ty]) -> String {
        format!(
            "stack: {} → expected {}",
            self.format_stack_slots(found),
            self.format_stack_tys(expected)
        )
    }

    fn pop_slot(&self, st: &FnState, stack: &mut Vec<Slot>, what: &str) -> CResult<Slot> {
        stack.pop().ok_or_else(|| {
            CompileError::new(
                format!("missing operand for {what}"),
                st.current_span,
                "E362",
            )
            .with_note(format!(
                "stack: {} → expected at least one more value for {what}",
                self.format_stack_slots(stack)
            ))
            .with_help("push the required operand before this word, or check earlier pops")
        })
    }

    /// Fail with a stack-effect note before consuming operands when `stack`
    /// does not hold at least `n` values.
    fn require_stack(&self, st: &FnState, stack: &[Slot], n: usize, what: &str) -> CResult<()> {
        if stack.len() >= n {
            return Ok(());
        }
        Err(CompileError::new(
            format!("missing operand for {what}"),
            st.current_span,
            "E362",
        )
        .with_note(format!(
            "stack: {} → expected at least {n} value(s) for {what}",
            self.format_stack_slots(stack)
        ))
        .with_help("push the required operand before this word, or check earlier pops"))
    }

    /// Emit a call to an imported host runtime function.
    fn rt_call(
        &self,
        b: &mut FunctionBuilder,
        st: &FnState,
        name: &str,
        args: Vec<Value>,
    ) -> CResult<Vec<Value>> {
        let fref = st.rt.get(name).copied().ok_or_else(|| {
            CompileError::new(
                format!("missing runtime function '{name}'"),
                st.current_span,
                "E370",
            )
        })?;
        let inst = b.ins().call(fref, &args);
        Ok(b.inst_results(inst).to_vec())
    }

    /// Reject an unsafe operation outside an `unsafe` block or unsafe
    /// function. `what` names the operation for the error message.
    fn require_unsafe(&self, st: &FnState, what: &str) -> CResult<()> {
        if st.unsafe_depth == 0 {
            return Err(CompileError::new(
                format!("'{what}' requires an unsafe context"),
                st.current_span,
                "E370",
            )
            .with_primary_message("unsafe operation here")
            .with_note("raw pointers, `mem.allocate`/`free`/`load`/`store`, and `unsafe function` calls need an unsafe context")
            .with_help("wrap this in `unsafe ... end`, or mark the enclosing function `unsafe function`"));
        }
        Ok(())
    }

    /// Reject use of a variable whose value was transferred by `move`.
    fn require_not_moved_var(&self, st: &FnState, name: &str) -> CResult<()> {
        if st.moved_vars.contains(name) {
            let mut err = CompileError::new(
                format!("use after move: '{name}' no longer owns its value"),
                st.current_span,
                "E373",
            )
            .with_primary_message("value used here after move");
            if let Some(site) = st.move_sites.get(name).copied() {
                err = err.with_label(site, "value moved here");
            }
            err = err
                .with_note("after `move`, the source name is empty; use the new owner instead")
                .with_help("read from the destination variable, or avoid moving if you still need the source");
            return Err(err);
        }
        Ok(())
    }

    /// Reject mutating or consuming a value while a borrow of it is live.
    fn require_not_borrowed(&self, st: &FnState, value: Value, what: &str) -> CResult<()> {
        if st.borrowed.contains(&value) {
            let mut err = CompileError::new(
                format!("cannot {what} while a borrow is live; release the reference first"),
                st.current_span,
                "E374",
            )
            .with_primary_message(format!("cannot {what} here"))
            .with_note(
                "a live `borrow` (or region put) pins the owner until the reference is released",
            )
            .with_help(
                "pop or otherwise consume the reference before mutating or dropping the owner",
            );
            if let Some(site) = st.borrow_sites.get(&value).copied() {
                err = err.with_label(site, "borrow still live here");
            }
            return Err(err);
        }
        Ok(())
    }

    /// Reject a second overlapping borrow of the same value.
    fn require_can_borrow(&self, st: &FnState, value: Value) -> CResult<()> {
        if st.borrowed.contains(&value) {
            let mut err = CompileError::new(
                "second overlapping borrow of the same value",
                st.current_span,
                "E375",
            )
            .with_primary_message("second borrow attempted here")
            .with_note("Yarrow allows only one live borrow of a value at a time")
            .with_help("release the first reference (e.g. `pop`) before borrowing again");
            if let Some(site) = st.borrow_sites.get(&value).copied() {
                err = err.with_label(site, "first borrow is still live");
            }
            return Err(err);
        }
        Ok(())
    }

    /// Reject use of a value after its region was freed.
    fn require_region_live(&self, st: &FnState, value: Value) -> CResult<()> {
        if st.region_freed.contains(&value) {
            return Err(CompileError::new(
                "use after region free: value was attached to a freed region",
                st.current_span,
                "E376",
            )
            .with_note("values put into a region become invalid after `region.free`")
            .with_help(
                "finish using borrows of region-attached values before freeing the region",
            ));
        }
        Ok(())
    }

    /// Coerce `slot` to the 64-bit type runtime functions expect (pointers and
    /// ≤ 8-byte scalars round-trip through the low bytes).
    fn rt_arg(&self, b: &mut FunctionBuilder, st: &FnState, slot: Slot) -> CResult<Value> {
        if slot.ty.clty(self.ptr_type) == self.ptr_type {
            return Ok(slot.value);
        }
        coerce(
            b,
            slot.value,
            slot.ty,
            Ty::I64,
            self.ptr_type,
            st.current_span,
        )
    }

    /// The generic host-call path: `@name` (or a call to an undefined
    /// function) whose name lives in the runtime's [`HOST_FNS`] registry is
    /// lowered purely from that table's signature — pop `params` in order,
    /// coerce, call, push `returns`. No per-name compiler code.
    fn emit_host_call(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        name: &str,
    ) -> CResult<()> {
        let host = crate::runtime::HOST_FNS
            .iter()
            .find(|h| h.name == name)
            .ok_or_else(|| {
                CompileError::new(
                    format!("unknown host function '{name}'"),
                    st.current_span,
                    "E372",
                )
            })?;
        if host.safety == crate::runtime::Safety::Unsafe {
            self.require_unsafe(st, &format!("@{name}"))?;
        }
        let n = host.params.len();
        if stack.len() < n {
            return Err(CompileError::new(
                format!("'{name}' requires {n} argument(s)"),
                st.current_span,
                "E331",
            ));
        }
        let tail = stack.split_off(stack.len() - n);
        let mut args: Vec<Value> = Vec::with_capacity(n);
        for (i, slot) in tail.iter().enumerate() {
            let pt = scalar_ty(host.params[i] as u8);
            args.push(coerce(
                b,
                slot.value,
                slot.ty,
                pt,
                self.ptr_type,
                st.current_span,
            )?);
        }
        let fref = st.rt.get(name).copied().ok_or_else(|| {
            CompileError::new(
                format!("unregistered host function '{name}'"),
                st.current_span,
                "E370",
            )
        })?;
        let inst = b.ins().call(fref, &args);
        let results = b.inst_results(inst).to_vec();
        for (v, code) in results.into_iter().zip(host.returns) {
            let ty = scalar_ty(*code as u8);
            let own = if self.is_heap(ty) {
                Own::Owned
            } else {
                Own::Trivial
            };
            self.claim(st, v, ty);
            stack.push(Slot { value: v, ty, own });
        }
        Ok(())
    }

    /// Whether `ty` owns heap storage (strings, lists, hashmaps, structs,
    /// arrays). Struct and array instances are heap-allocated by the compiler
    /// so their addresses stay valid across calls; dropping them emits
    /// `yarrow_free_value`.
    fn is_heap(&self, ty: Ty) -> bool {
        matches!(
            ty,
            Ty::String
                | Ty::List { .. }
                | Ty::Hashmap { .. }
                | Ty::Struct(_)
                | Ty::Array { .. }
                | Ty::Union(_)
        )
    }

    /// Register a freshly-created heap handle as owned by the function.
    fn claim(&mut self, st: &mut FnState, value: Value, ty: Ty) {
        if self.is_heap(ty) {
            st.owns.insert(value);
        }
    }

    /// Emit `yarrow_free_value(handle, kind)` for an owned slot and forget its
    /// ownership. Moved values are skipped (their new owner handles the free).
    /// `set`/scope-exit drops are safe because the runtime guards double frees.
    fn emit_drop(&mut self, b: &mut FunctionBuilder, st: &mut FnState, slot: Slot) -> CResult<()> {
        if !slot.own.is_owned() || !self.is_heap(slot.ty) || st.moved.contains(&slot.value) {
            return Ok(());
        }
        if st.borrowed.contains(&slot.value) {
            let mut err = CompileError::new(
                "cannot drop owner while a borrow is live; release the reference first",
                st.current_span,
                "E374",
            )
            .with_primary_message("owner dropped here")
            .with_note("dropping an owner while a borrow is live would leave a dangling reference")
            .with_help("release the reference first, then drop or overwrite the owner");
            if let Some(site) = st.borrow_sites.get(&slot.value).copied() {
                err = err.with_label(site, "borrow still live here");
            }
            return Err(err);
        }
        let kind = b.ins().iconst(irtypes::I64, kind_code(slot.ty) as i64);
        self.rt_call(b, st, "free_value", vec![slot.value, kind])?;
        st.owns.remove(&slot.value);
        Ok(())
    }

    /// Consume a slot from the stack: release its borrow (if any) and drop it
    /// if it owns storage. Used wherever a popped value does not flow through.
    fn consume(&mut self, b: &mut FunctionBuilder, st: &mut FnState, slot: Slot) -> CResult<()> {
        // Popping / dropping a non-borrow view of a value while a borrow is
        // live would free (or discard) the owner too early.
        if slot.own != Own::Borrow && st.borrowed.contains(&slot.value) {
            let mut err = CompileError::new(
                "cannot pop/drop owner while a borrow is live; release the reference first",
                st.current_span,
                "E374",
            )
            .with_primary_message("owner popped here")
            .with_note("popping the owner while a borrow is live would discard it too early")
            .with_help("pop the reference first (or let it fall out of scope), then the owner");
            if let Some(site) = st.borrow_sites.get(&slot.value).copied() {
                err = err.with_label(site, "borrow still live here");
            }
            return Err(err);
        }
        if slot.own == Own::Borrow {
            st.borrowed.remove(&slot.value);
            st.borrow_sites.remove(&slot.value);
        }
        self.emit_drop(b, st, slot)
    }

    /// Emit `yarrow_register_struct_descs(id, table, count)` once per struct
    /// per function, so `yarrow_free_value` can free a struct's heap fields.
    fn emit_register_struct(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        id: u32,
    ) -> CResult<()> {
        if !st.registered_descs.insert(id) {
            return Ok(());
        }
        let gv = self.struct_desc_gvs.get(&id).copied().ok_or_else(|| {
            CompileError::new(
                format!("no field descriptors for struct #{id}"),
                st.current_span,
                "E371",
            )
        })?;
        let addr = b.ins().global_value(self.ptr_type, gv);
        let idv = b.ins().iconst(irtypes::I64, id as i64);
        let count = b
            .ins()
            .iconst(irtypes::I64, self.struct_layout(id).fields.len() as i64);
        self.rt_call(b, st, "register_struct_descs", vec![idv, addr, count])?;
        Ok(())
    }

    /// Emit `yarrow_register_union_descs(id, table, count)` once per union per
    /// function, so `yarrow_free_value` can free a union's active payload.
    fn emit_register_union(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        id: u32,
    ) -> CResult<()> {
        if !st.registered_unions.insert(id) {
            return Ok(());
        }
        let gv = self.union_desc_gvs.get(&id).copied().ok_or_else(|| {
            CompileError::new(
                format!("no member-kind table for union #{id}"),
                st.current_span,
                "E371",
            )
        })?;
        let addr = b.ins().global_value(self.ptr_type, gv);
        let idv = b.ins().iconst(irtypes::I64, id as i64);
        let count = b
            .ins()
            .iconst(irtypes::I64, self.unions[id as usize].members.len() as i64);
        self.rt_call(b, st, "register_union_descs", vec![idv, addr, count])?;
        Ok(())
    }

    /// Wrap `value` of type `from` into a fresh union block for union `id`,
    /// selecting the first member type it can coerce to. Returns the union
    /// handle (a pointer to the block). The caller owns the block; a heap
    /// source value is marked moved since the union's payload now owns it.
    fn emit_union_wrap(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        value: Value,
        from: Ty,
        id: u32,
    ) -> CResult<Value> {
        let info = &self.unions[id as usize];
        let idx = info
            .members
            .iter()
            .position(|m| coercible(from, *m))
            .ok_or_else(|| {
                CompileError::new(
                    format!(
                        "cannot convert '{from:?}' into union '{}' (members: {:?})",
                        info.name, info.members
                    ),
                    st.current_span,
                    "E309",
                )
            })?;
        let member = info.members[idx];
        let val = coerce(b, value, from, member, self.ptr_type, st.current_span)?;
        self.emit_register_union(b, st, id)?;
        let size = b.ins().iconst(irtypes::I64, 16);
        let out = self.rt_call(b, st, "alloc", vec![size])?;
        let out = out[0];
        let tag = b.ins().iconst(irtypes::I64, idx as i64);
        b.ins().store(
            cranelift_codegen::ir::MemFlagsData::trusted(),
            tag,
            out,
            UNION_TAG_OFFSET,
        );
        b.ins().store(
            cranelift_codegen::ir::MemFlagsData::trusted(),
            val,
            out,
            UNION_PAYLOAD_OFFSET,
        );
        if self.is_heap(from) {
            st.moved.insert(value);
        }
        Ok(out)
    }

    /// Coerce `value` of type `from` to `to`, wrapping a raw member value into
    /// a union when `to` is a union type. Identity when already a union of the
    /// same id. See [`Self::emit_union_wrap`].
    fn coerce_or_wrap(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        value: Value,
        from: Ty,
        to: Ty,
    ) -> CResult<Value> {
        match to {
            Ty::Union(id) if !matches!(from, Ty::Union(_)) => {
                self.emit_union_wrap(b, st, value, from, id)
            }
            _ => coerce(b, value, from, to, self.ptr_type, st.current_span),
        }
    }

    /// Function-scope exit: run deferred bodies in reverse, then drop every
    /// owned value left on the stack and owned variables.
    fn emit_scope_exit(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<()> {
        while let Some(body) = st.deferred.pop() {
            self.compile_body(b, st, stack, &body)?;
        }
        for slot in stack.drain(..) {
            self.consume(b, st, slot)?;
        }
        let mut var_slots: Vec<Slot> = Vec::new();
        for (var, ty, own) in st.vars.values() {
            // Borrowed variables own nothing; their true owner frees the
            // value at scope exit.
            if self.is_heap(*ty) && own.is_owned() {
                let v = b.use_var(*var);
                var_slots.push(Slot {
                    value: v,
                    ty: *ty,
                    own: *own,
                });
            }
        }
        for slot in var_slots {
            self.emit_drop(b, st, slot)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Control flow
    // ------------------------------------------------------------------

    fn eval_cond(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        e: &Expr,
    ) -> CResult<Value> {
        let before = stack.len();
        self.compile_expr(b, st, stack, e)?;
        if stack.len() != before + 1 {
            let found = &stack[before..];
            return Err(CompileError::new(
                "condition must evaluate to a single value",
                st.current_span,
                "E324",
            )
            .with_note(self.stack_effect_note(found, &[Ty::Bool]))
            .with_help("leave exactly one boolean on the stack for the condition"));
        }
        let slot = stack.pop().unwrap();
        if slot.ty.is_bool() {
            Ok(slot.value)
        } else {
            Err(
                CompileError::new("condition must be bool", st.current_span, "E324")
                    .with_primary_message(format!(
                        "expected bool, found {}",
                        self.format_ty(slot.ty)
                    ))
                    .with_note(format!(
                        "stack: [{}] → expected [bool]",
                        self.format_ty(slot.ty)
                    ))
                    .with_note("`if` and conditional `for` require a boolean condition")
                    .with_help(
                        "compare with `==`, `<`, `>`, or another relational/logical operator first",
                    ),
            )
        }
    }

    /// Like [`Self::eval_cond`], but for `match` case conditions: these may
    /// consume stack values (e.g. `error.X ==` compares against the match
    /// subject), so only the top result is required to be a boolean or integer.
    /// The stack balance relative to the subject is checked by the caller.
    fn eval_match_cond(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        e: &Expr,
    ) -> CResult<Value> {
        self.compile_expr(b, st, stack, e)?;
        let slot = stack.pop().ok_or_else(|| {
            CompileError::new(
                "condition must evaluate to a single value",
                st.current_span,
                "E324",
            )
            .with_note(format!(
                "stack: {} → expected [bool]",
                self.format_stack_slots(stack)
            ))
        })?;
        if slot.ty.is_int() || slot.ty.is_bool() {
            Ok(slot.value)
        } else {
            Err(CompileError::new(
                "condition must be a boolean or integer",
                st.current_span,
                "E324",
            )
            .with_primary_message(format!(
                "expected bool or integer, found {}",
                self.format_ty(slot.ty)
            ))
            .with_note(format!(
                "stack: [{}] → expected [bool]",
                self.format_ty(slot.ty)
            )))
        }
    }

    fn emit_if(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        condition: &Expr,
        then_branch: &[Stmt],
        else_branch: &[Stmt],
    ) -> CResult<()> {
        let pre = stack.clone();
        let cond = self.eval_cond(b, st, stack, condition)?;
        let then_blk = b.create_block();
        let else_blk = b.create_block();
        let merge = b.create_block();
        b.ins().brif(cond, then_blk, &[], else_blk, &[]);

        b.switch_to_block(then_blk);
        *stack = pre.clone();
        st.terminated = false;
        self.compile_body(b, st, stack, then_branch)?;
        let then_terminated = st.terminated;
        let then_stack = stack.clone();
        let then_extra: Vec<Slot> = then_stack[pre.len()..].to_vec();

        // When the then-branch continues, fix merge params from it and jump.
        let mut params: Vec<Value> = Vec::new();
        let mut merge_tys: Vec<Ty> = Vec::new();
        if !then_terminated {
            for s in &then_extra {
                let join_ty = if_merge_ty_from_then(s.ty);
                params.push(b.append_block_param(merge, join_ty.clty(self.ptr_type)));
                merge_tys.push(join_ty);
            }
            let mut tv: Vec<BlockArg> = Vec::with_capacity(then_extra.len());
            for (s, join_ty) in then_extra.iter().zip(&merge_tys) {
                let v = coerce(b, s.value, s.ty, *join_ty, self.ptr_type, st.current_span)?;
                tv.push(BlockArg::Value(v));
            }
            b.ins().jump(merge, &tv);
        }

        b.switch_to_block(else_blk);
        *stack = pre.clone();
        st.terminated = false;
        self.compile_body(b, st, stack, else_branch)?;
        let else_terminated = st.terminated;
        let else_stack = stack.clone();
        let else_extra: Vec<Slot> = else_stack[pre.len()..].to_vec();

        if then_terminated && else_terminated {
            st.terminated = true;
            b.switch_to_block(merge);
            self.dead_block(b);
            *stack = pre;
            return Ok(());
        }

        if !else_terminated {
            if then_terminated {
                // Merge shape comes from the else branch alone.
                for s in &else_extra {
                    params.push(b.append_block_param(merge, s.ty.clty(self.ptr_type)));
                    merge_tys.push(s.ty);
                }
                let ev: Vec<BlockArg> = else_extra
                    .iter()
                    .map(|s| BlockArg::Value(s.value))
                    .collect();
                b.ins().jump(merge, &ev);
            } else {
                if else_extra.len() != then_extra.len() {
                    return Err(CompileError::new(
                        "if/else branches must leave the same number of values",
                        st.current_span,
                        "E328",
                    )
                    .with_note(format!(
                        "stack: then {} else {} → expected matching branch effects",
                        self.format_stack_slots(&then_extra),
                        self.format_stack_slots(&else_extra)
                    ))
                    .with_help(
                        "leave the same number of values on both branches, or return from both",
                    ));
                }
                let mut ev: Vec<BlockArg> = Vec::with_capacity(then_extra.len());
                for (s, join_ty) in else_extra.iter().zip(&merge_tys) {
                    let v = coerce(b, s.value, s.ty, *join_ty, self.ptr_type, st.current_span)?;
                    ev.push(BlockArg::Value(v));
                }
                b.ins().jump(merge, &ev);
            }
        }

        b.switch_to_block(merge);
        st.terminated = false;
        *stack = pre;
        for (i, ty) in merge_tys.iter().enumerate() {
            stack.push(Slot {
                value: params[i],
                ty: *ty,
                own: Own::Trivial,
            });
        }
        Ok(())
    }

    /// `value match <case ...> else <body> end`.
    ///
    /// The subject is evaluated once and lives on the compile-time stack for
    /// the whole match (case conditions commonly `dup` it). Each case runs its
    /// condition and, if truthy, its body; if no case matches, the `else`
    /// branch runs. The subject is dropped when the match ends: the merge
    /// point carries only the values the chosen branch left *beyond* the
    /// subject, so the whole match behaves like a value-producing expression.
    ///
    /// An empty `value` (`Expr::variable("")`, produced when the parser saw a
    /// bare `match`) means there is no subject; conditions then operate on the
    /// stack as it was when the match started.
    ///
    /// On a union subject, a `Type` case dispatches on the active member's
    /// tag (compared against each case type's member index in order) and its
    /// body receives the member as a `reference<Type>` (a borrow with the
    /// member's physical type, so reads auto-deref). The borrow is released at
    /// the end of the case body and the union itself is left untouched.
    fn emit_match(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        value: &Expr,
        cases: &[MatchCase],
        else_branch: &[Stmt],
    ) -> CResult<()> {
        let pre = stack.clone();
        let subject = if matches!(value, Expr::Variable { name } if name.is_empty()) {
            None
        } else {
            self.compile_expr(b, st, stack, value)?;
            Some(self.pop_slot(st, stack, "match value")?)
        };
        let mut sub_stack = pre.clone();
        if let Some(s) = subject {
            sub_stack.push(s);
        }

        // Type cases are either union member types (`i32 case`) or error
        // members written as paths (`AppError.NOT_FOUND case`).
        let has_union_type_case = cases.iter().any(|c| {
            matches!(&c.kind, MatchCaseKind::Type(ty) if self.error_member_tag_from_type(ty).is_none())
        });
        let has_error_type_case = cases
            .iter()
            .any(|c| matches!(&c.kind, MatchCaseKind::Type(ty) if self.error_member_tag_from_type(ty).is_some()));

        // Error match inside `handle`: no explicit subject; the error tag is
        // already on the stack.
        let error_subject: Option<Slot> = if has_error_type_case {
            if let Some(s) = subject.filter(|s| s.ty == Ty::Error) {
                Some(s)
            } else if subject.is_none() {
                sub_stack.last().copied().filter(|s| s.ty == Ty::Error)
            } else {
                None
            }
        } else {
            None
        };
        if has_error_type_case && error_subject.is_none() {
            return Err(CompileError::new(
                "error member case requires an error subject (inside handle, or a fallible value)",
                st.current_span,
                "E308",
            ));
        }

        // Union type dispatch: the subject must be a union; its active-member
        // tag is loaded once before the branch structure.
        let subject_union: Option<(Slot, u32, Value)> = if has_union_type_case {
            let s = subject.ok_or_else(|| {
                CompileError::new(
                    "match type dispatch requires a subject value",
                    st.current_span,
                    "E308",
                )
            })?;
            let id = match s.ty {
                Ty::Union(id) => id,
                other => {
                    return Err(CompileError::new(
                        format!("match type dispatch requires a union subject, got {other:?}"),
                        st.current_span,
                        "E308",
                    ));
                }
            };
            let tag = b.ins().load(
                irtypes::I64,
                cranelift_codegen::ir::MemFlagsData::trusted(),
                s.value,
                UNION_TAG_OFFSET,
            );
            Some((s, id, tag))
        } else {
            None
        };

        let merge = b.create_block();
        let body_blks: Vec<Block> = (0..cases.len()).map(|_| b.create_block()).collect();
        let cond_blks: Vec<Block> = (0..cases.len().saturating_sub(1))
            .map(|_| b.create_block())
            .collect();
        let else_blk = b.create_block();

        let mut results_ty: Option<Vec<Ty>> = None;

        for (i, case) in cases.iter().enumerate() {
            let error_tag = match &case.kind {
                MatchCaseKind::Type(ty) => self.error_member_tag_from_type(ty),
                MatchCaseKind::Condition(_) => None,
            };
            let case_member: Option<Ty> = match (&case.kind, error_tag) {
                (MatchCaseKind::Type(ty), None) => {
                    let (_, id, _) = subject_union.as_ref().ok_or_else(|| {
                        CompileError::new(
                            "union member case requires a union subject",
                            st.current_span,
                            "E308",
                        )
                    })?;
                    let t = self.resolve_ty(ty)?;
                    let members = &self.unions[*id as usize].members;
                    if !members.contains(&t) {
                        return Err(CompileError::new(
                            format!(
                                "case type {t:?} is not a member of union '{}'",
                                self.unions[*id as usize].name
                            ),
                            st.current_span,
                            "E308",
                        )
                        .with_primary_message("invalid case type")
                        .with_note(format!(
                            "union '{}' members are: {:?}",
                            self.unions[*id as usize].name, members
                        ))
                        .with_help("use a `Type case` that matches a declared member, or handle it in `else`"));
                    }
                    Some(t)
                }
                _ => None,
            };
            if i > 0 {
                b.switch_to_block(cond_blks[i - 1]);
            }
            *stack = sub_stack.clone();
            let cond = match (&case.kind, case_member, error_tag) {
                (MatchCaseKind::Condition(expr), _, _) => {
                    let cond = self.eval_match_cond(b, st, stack, expr)?;
                    // The condition may keep the subject on the stack
                    // (`dup X ==`) or consume stack values (`error.X ==`
                    // compares against the subject), so it may leave at most
                    // the pre-condition stack height.
                    if stack.len() > sub_stack.len() {
                        return Err(CompileError::new(
                            "a 'match' case condition must leave the stack balanced",
                            st.current_span,
                            "E343",
                        ));
                    }
                    cond
                }
                (MatchCaseKind::Type(_), Some(mt), None) => {
                    let (_, id, tag) = subject_union.as_ref().unwrap();
                    let idx = self.unions[*id as usize]
                        .members
                        .iter()
                        .position(|m| *m == mt)
                        .unwrap();
                    let want = b.ins().iconst(irtypes::I64, idx as i64);
                    b.ins().icmp(IntCC::Equal, *tag, want)
                }
                (MatchCaseKind::Type(_), None, Some(tag)) => {
                    let err = error_subject.as_ref().unwrap();
                    let want = b.ins().iconst(irtypes::I64, i64::from(tag));
                    b.ins().icmp(IntCC::Equal, err.value, want)
                }
                _ => unreachable!(),
            };
            let false_target = if i + 1 < cases.len() {
                cond_blks[i]
            } else {
                else_blk
            };
            b.ins().brif(cond, body_blks[i], &[], false_target, &[]);

            b.switch_to_block(body_blks[i]);
            *stack = sub_stack.clone();
            let mut case_ref: Option<Slot> = None;
            if let Some(mt) = case_member {
                // Push the active member as a borrow reference: same physical
                // type as the member (auto-deref on read), owned by the union.
                let (s, _, _) = subject_union.as_ref().unwrap();
                let payload = b.ins().load(
                    mt.clty(self.ptr_type),
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    s.value,
                    UNION_PAYLOAD_OFFSET,
                );
                if self.is_heap(mt) {
                    st.moved.insert(payload);
                }
                st.borrowed.insert(payload);
                let slot = Slot {
                    value: payload,
                    ty: mt,
                    own: Own::Borrow,
                };
                case_ref = Some(slot);
                stack.push(slot);
            }
            self.compile_body(b, st, stack, &case.body)?;
            // The member borrow is transient: if the body left it on the
            // stack (unconsumed), release it so it does not count as a branch
            // result (the union keeps ownership of the payload). The body's
            // own results share the payload value only as non-borrow copies
            // (variable reads), so comparing the borrow slot catches exactly
            // the unconsumed reference.
            if let Some(r) = case_ref
                && stack.len() > sub_stack.len()
                && stack[sub_stack.len()].value == r.value
                && stack[sub_stack.len()].own == Own::Borrow
            {
                stack.remove(sub_stack.len());
            }
            if stack.len() < sub_stack.len() {
                return Err(CompileError::new(
                    "'match' branch underflowed the subject stack",
                    st.current_span,
                    "E343",
                ));
            }
            let results = stack.split_off(sub_stack.len());
            self.match_merge(b, st, merge, &mut results_ty, results)?;
        }

        // No cases: fall straight through to `else` (otherwise `else_blk` is
        // unreachable and the current block has no terminator).
        if cases.is_empty() {
            b.ins().jump(else_blk, &[]);
        }

        b.switch_to_block(else_blk);
        *stack = sub_stack.clone();
        self.compile_body(b, st, stack, else_branch)?;
        if stack.len() < sub_stack.len() {
            return Err(CompileError::new(
                "'match' branch underflowed the subject stack",
                st.current_span,
                "E343",
            ));
        }
        let results = stack.split_off(sub_stack.len());
        self.match_merge(b, st, merge, &mut results_ty, results)?;

        b.switch_to_block(merge);
        *stack = pre;
        for (i, t) in results_ty.iter().flatten().enumerate() {
            let p = b.block_params(merge)[i];
            stack.push(Slot {
                value: p,
                ty: *t,
                own: Own::Trivial,
            });
        }
        Ok(())
    }

    /// Append the merge block params for one match branch and jump to it. The
    /// first branch fixes the number/type of values every branch must leave;
    /// later branches are coerced to those types before jumping.
    fn match_merge(
        &mut self,
        b: &mut FunctionBuilder,
        st: &FnState,
        merge: Block,
        results_ty: &mut Option<Vec<Ty>>,
        results: Vec<Slot>,
    ) -> CResult<()> {
        let want: Vec<Ty> = match results_ty {
            None => {
                let want: Vec<Ty> = results.iter().map(|s| s.ty).collect();
                for t in &want {
                    b.append_block_param(merge, t.clty(self.ptr_type));
                }
                *results_ty = Some(want.clone());
                want
            }
            Some(prev) => {
                if prev.len() != results.len() {
                    return Err(CompileError::new(
                        "'match' branches must leave the same number of values",
                        st.current_span,
                        "E343",
                    ));
                }
                prev.clone()
            }
        };
        let mut args: Vec<BlockArg> = Vec::with_capacity(results.len());
        for (s, t) in results.iter().zip(&want) {
            let v = coerce(b, s.value, s.ty, *t, self.ptr_type, st.current_span)?;
            args.push(BlockArg::Value(v));
        }
        b.ins().jump(merge, &args);
        Ok(())
    }

    /// `for` is either a condition loop (`i 3 < for`) or an iterable loop
    /// (`numbers for`). Binders before `for` are gone; use `std.loop` for
    /// value/index (Stage 5). Until then, iterable loops still walk elements
    /// without binding names.
    fn emit_for(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        source: &Expr,
        body: &[Stmt],
    ) -> CResult<()> {
        if for_source_is_condition(source) {
            return self.emit_cond_for(b, st, stack, source, body);
        }
        self.emit_iter_for(b, st, stack, source, body)
    }

    /// Sequence form: `source` must be an array or list. Element and index are
    /// available via `std.loop` (`loop.value` / `loop.index`).
    fn emit_iter_for(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        source: &Expr,
        body: &[Stmt],
    ) -> CResult<()> {
        let pre = stack.clone();
        self.compile_expr(b, st, stack, source)?;
        let iterable = self.pop_slot(st, stack, "'for' iterable")?;
        // Resolve the element type and how to reach the elements. Arrays have
        // a compile-time length and their storage is the array slot itself;
        // lists store a length in the header and their elements behind
        // `List.data` (offset `LIST_DATA_OFFSET`).
        let (elem, base, total, elem_size) = match iterable.ty {
            Ty::Array { elem, count } => {
                let elem = scalar_ty(elem);
                let total = b.ins().iconst(irtypes::I64, count as i64);
                (elem, iterable.value, total, elem.elem_size() as i64)
            }
            Ty::List { elem } => {
                let elem = elem_ty(elem);
                let total = self.rt_call(b, st, "list_len", vec![iterable.value])?[0];
                let base = b.ins().load(
                    self.ptr_type,
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    iterable.value,
                    LIST_DATA_OFFSET,
                );
                (elem, base, total, elem.elem_size() as i64)
            }
            other => {
                return Err(CompileError::new(
                    format!("'for' requires an array or list iterable, got {other:?}"),
                    st.current_span,
                    "E344",
                ));
            }
        };

        let zero = b.ins().iconst(irtypes::I64, 0);
        let idx_v = b.declare_var(irtypes::I64);
        b.def_var(idx_v, zero);
        let ptr_v = b.declare_var(self.ptr_type);
        b.def_var(ptr_v, base);
        let count_v = b.declare_var(irtypes::I64);
        b.def_var(count_v, total);

        let header = b.create_block();
        let body_blk = b.create_block();
        let step = b.create_block();
        let end = b.create_block();
        b.ins().jump(header, &[]);

        b.switch_to_block(header);
        *stack = pre.clone();
        let idx = b.use_var(idx_v);
        let n = b.use_var(count_v);
        let more = b.ins().icmp(IntCC::UnsignedLessThan, idx, n);
        b.ins().brif(more, body_blk, &[], end, &[]);

        b.switch_to_block(body_blk);
        *stack = pre.clone();
        let idx = b.use_var(idx_v);
        let base = b.use_var(ptr_v);
        let stride = b.ins().iconst(irtypes::I64, elem_size);
        let off = b.ins().imul(idx, stride);
        let addr = b.ins().iadd(base, off);
        let elem_val = b.ins().load(
            elem.clty(self.ptr_type),
            cranelift_codegen::ir::MemFlagsData::trusted(),
            addr,
            0,
        );
        st.loops.push(LoopCtx {
            break_to: end,
            continue_to: step,
            value: Some((elem_val, elem)),
            index: Some(idx),
        });
        self.compile_body(b, st, stack, body)?;
        st.loops.pop();
        if stack.len() != pre.len() {
            return Err(CompileError::new(
                "'for' body must leave the stack balanced",
                st.current_span,
                "E325",
            ));
        }
        b.ins().jump(step, &[]);

        b.switch_to_block(step);
        let idx = b.use_var(idx_v);
        let next = b.ins().iadd_imm(idx, 1);
        b.def_var(idx_v, next);
        b.ins().jump(header, &[]);

        b.switch_to_block(end);
        *stack = pre;
        Ok(())
    }

    /// Condition form of `for`: repeatedly evaluate `condition` and run
    /// `body` while it is truthy.
    fn emit_cond_for(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        condition: &Expr,
        body: &[Stmt],
    ) -> CResult<()> {
        let pre = stack.clone();
        let header = b.create_block();
        let body_blk = b.create_block();
        let end = b.create_block();
        b.ins().jump(header, &[]);

        b.switch_to_block(header);
        *stack = pre.clone();
        let cond = self.eval_cond(b, st, stack, condition)?;
        b.ins().brif(cond, body_blk, &[], end, &[]);

        b.switch_to_block(body_blk);
        *stack = pre.clone();
        st.loops.push(LoopCtx {
            break_to: end,
            continue_to: header,
            value: None,
            index: None,
        });
        self.compile_body(b, st, stack, body)?;
        st.loops.pop();
        if stack.len() != pre.len() {
            return Err(CompileError::new(
                "for body must leave the stack balanced",
                st.current_span,
                "E325",
            ));
        }
        b.ins().jump(header, &[]);

        b.switch_to_block(end);
        *stack = pre;
        Ok(())
    }

    fn compile_expr(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        e: &Expr,
    ) -> CResult<()> {
        match e {
            Expr::Integer { value } => {
                let n = decode_int_literal(value)
                    .map_err(|m| CompileError::new(m, st.current_span, "E363"))?;
                // The front-end validates literals; lowering is currently 64-bit.
                if n < i64::MIN as i128 || n > i64::MAX as i128 {
                    return Err(CompileError::new(
                        format!(
                            "integer literal '{value}' is out of range for 64-bit code generation"
                        ),
                        st.current_span,
                        "E364",
                    ));
                }
                let ty = int_literal_ty(n);
                let v = b.ins().iconst(ty.clty(self.ptr_type), n as i64);
                stack.push(Slot {
                    value: v,
                    ty,
                    own: Own::Trivial,
                });
            }
            Expr::Float { value } => {
                let n = decode_float_literal(value)
                    .map_err(|m| CompileError::new(m, st.current_span, "E363"))?;
                let ty = float_literal_ty(n);
                let v = match ty {
                    Ty::F16 => {
                        use half::f16;
                        b.ins().f32const(f16::from_f64(n).to_f32())
                    }
                    Ty::F32 => b.ins().f32const(n as f32),
                    _ => b.ins().f64const(n),
                };
                stack.push(Slot {
                    value: v,
                    ty,
                    own: Own::Trivial,
                });
            }
            Expr::Bool { value } => {
                let v = b.ins().iconst(irtypes::I8, if *value { 1 } else { 0 });
                stack.push(Slot {
                    value: v,
                    ty: Ty::Bool,
                    own: Own::Trivial,
                });
            }
            Expr::Rune { value } => {
                let cp = decode_rune_literal(value)
                    .map_err(|m| CompileError::new(m, st.current_span, "E363"))?;
                let v = b.ins().iconst(irtypes::I32, cp as i64);
                stack.push(Slot {
                    value: v,
                    ty: Ty::Rune,
                    own: Own::Trivial,
                });
            }
            Expr::String { value } => self.emit_string(b, st, stack, value)?,
            Expr::Variable { name } => {
                self.require_not_moved_var(st, name)?;
                if let Some((var, t, _own)) = st.vars.get(name).cloned() {
                    let v = b.use_var(var);
                    self.require_region_live(st, v)?;
                    stack.push(Slot {
                        value: v,
                        ty: t,
                        own: Own::Trivial,
                    });
                } else if let Some((id, v)) = self.enum_consts.get(name) {
                    // A bare enum member name (`RED`) is a program-wide
                    // named constant; resolve it to its value.
                    let c = b.ins().iconst(irtypes::I64, *v);
                    stack.push(Slot {
                        value: c,
                        ty: Ty::Enum(*id),
                        own: Own::Trivial,
                    });
                } else {
                    return Err(CompileError::new(
                        format!("unknown variable '{name}'"),
                        st.current_span,
                        "E320",
                    ));
                }
            }
            Expr::Member { base, member } => {
                // `std.loop` helpers: `loop.value` / `loop.index` push the
                // current iterable binding (no `call`).
                if let Some((v, ty)) = self.loop_intrinsic_value(st, base, member) {
                    stack.push(Slot {
                        value: v,
                        ty,
                        own: Own::Trivial,
                    });
                    return Ok(());
                }
                // `error.CustomError` creates a tagged error value.
                if let Expr::Variable { name } = base.as_ref()
                    && name == "error"
                {
                    let tag = self.error_tag(member)?;
                    let v = b.ins().iconst(irtypes::I64, i64::from(tag));
                    stack.push(Slot {
                        value: v,
                        ty: Ty::Error,
                        own: Own::Trivial,
                    });
                    return Ok(());
                }
                // `AppError.NOT_FOUND` resolves a named error member.
                if let Expr::Variable { name } = base.as_ref()
                    && let Some(id) = self.error_type_ids.get(name)
                {
                    let info = &self.error_types[*id as usize];
                    let (_, tag) =
                        info.members
                            .iter()
                            .find(|(n, _)| n == member)
                            .ok_or_else(|| {
                                CompileError::new(
                                    format!("error type '{}' has no member '{member}'", info.name),
                                    st.current_span,
                                    "E320",
                                )
                            })?;
                    let v = b.ins().iconst(irtypes::I64, i64::from(*tag));
                    stack.push(Slot {
                        value: v,
                        ty: Ty::Error,
                        own: Own::Trivial,
                    });
                    return Ok(());
                }
                // `Color.RED` resolves an enum member through its enum type.
                if let Expr::Variable { name } = base.as_ref()
                    && let Some(id) = self.enum_ids.get(name)
                {
                    let info = &self.enums[*id as usize];
                    let (_, v) =
                        info.members
                            .iter()
                            .find(|(n, _)| n == member)
                            .ok_or_else(|| {
                                CompileError::new(
                                    format!("enum '{}' has no member '{member}'", info.name),
                                    st.current_span,
                                    "E320",
                                )
                            })?;
                    let c = b.ins().iconst(irtypes::I64, *v);
                    stack.push(Slot {
                        value: c,
                        ty: Ty::Enum(*id),
                        own: Own::Trivial,
                    });
                    return Ok(());
                }
                // Member access through a `pointer<T>` base dereferences
                // memory, which requires an unsafe context.
                if self.base_is_pointer(st, base) {
                    self.require_unsafe(st, "member access through a pointer")?;
                }
                let sid = self.base_struct(st, base)?;
                let field = self.find_field(sid, member)?;
                let fty = field.ty;
                self.compile_expr(b, st, stack, base)?;
                let ptr = self.pop_slot(st, stack, "field access")?;
                let val = b.ins().load(
                    fty.clty(self.ptr_type),
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    ptr.value,
                    field.offset,
                );
                stack.push(Slot {
                    value: val,
                    ty: fty,
                    own: Own::Trivial,
                });
            }
            Expr::TypeValue { name } => {
                // A type used as a value pushes its runtime kind code, so
                // `==` on type values is plain code equality (`myVar typeof
                // i32 ==`).
                let p = Primitive::parse_name(name).ok_or_else(|| {
                    CompileError::new(
                        format!("unknown type value '{name}'"),
                        st.current_span,
                        "E302",
                    )
                })?;
                let ty = primitive_ty(p).ok_or_else(|| {
                    CompileError::new(
                        format!("type value '{name}' is not yet supported"),
                        st.current_span,
                        "E301",
                    )
                })?;
                let v = b.ins().iconst(irtypes::I64, kind_code(ty) as i64);
                stack.push(Slot {
                    value: v,
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
            Expr::Unwrap { inner } => {
                self.compile_expr(b, st, stack, inner)?;
                self.emit_unwrap(b, st, stack)?;
            }
            Expr::Binary { op, left, right } => {
                self.compile_expr(b, st, stack, left)?;
                self.compile_expr(b, st, stack, right)?;
                self.require_stack(st, stack, 2, "operator")?;
                let r = self.pop_slot(st, stack, "operator")?;
                let l = self.pop_slot(st, stack, "operator")?;
                self.emit_bin(b, st, stack, *op, l, r)?;
            }
            Expr::Unary { op, operand } => {
                self.compile_expr(b, st, stack, operand)?;
                let slot = self.pop_slot(st, stack, "unary operator")?;
                self.emit_not(b, st, stack, *op, slot)?;
            }
            Expr::Call { target } => self.emit_call(b, st, stack, target)?,
            Expr::ApplyBin(op) => {
                self.require_stack(st, stack, 2, "operator")?;
                let r = self.pop_slot(st, stack, "operator")?;
                let l = self.pop_slot(st, stack, "operator")?;
                self.emit_bin(b, st, stack, *op, l, r)?;
            }
            Expr::ApplyUn(op) => {
                let slot = self.pop_slot(st, stack, "unary operator")?;
                self.emit_not(b, st, stack, *op, slot)?;
            }
            Expr::StackOp(op) => self.emit_stackop(b, st, stack, *op)?,
            Expr::Seq(elems) => {
                for (el, span) in elems {
                    st.current_span = *span;
                    self.compile_expr(b, st, stack, el)?;
                }
            }
            Expr::Array(elems) => {
                // Standalone array literal: infer the element type from the
                // elements (integers become I64), allocate a slot and store.
                let mut vals: Vec<Slot> = Vec::with_capacity(elems.len());
                let mut elem_ty: Option<Ty> = None;
                for el in elems {
                    self.compile_expr(b, st, stack, el)?;
                    let slot = self.pop_slot(st, stack, "array element")?;
                    elem_ty = Some(match elem_ty {
                        None => slot.ty,
                        Some(t) => common_type(t, slot.ty).ok_or_else(|| {
                            CompileError::new(
                                format!(
                                    "array literal elements have incompatible types {t:?} and {:?}",
                                    slot.ty
                                ),
                                st.current_span,
                                "E345",
                            )
                        })?,
                    });
                    vals.push(slot);
                }
                let elem_ty = elem_ty.ok_or_else(|| {
                    CompileError::new(
                        "empty array literal needs a type annotation",
                        st.current_span,
                        "E345",
                    )
                })?;
                let ptr = self.alloc_array(b, st, elem_ty, elems.len() as u32)?;
                let elem_size = elem_ty.elem_size() as i32;
                for (i, slot) in vals.iter().enumerate() {
                    let val = coerce(
                        b,
                        slot.value,
                        slot.ty,
                        elem_ty,
                        self.ptr_type,
                        st.current_span,
                    )?;
                    b.ins().store(
                        cranelift_codegen::ir::MemFlagsData::trusted(),
                        val,
                        ptr,
                        i as i32 * elem_size,
                    );
                }
                let aty = Ty::Array {
                    elem: elem_ty.scalar_code().unwrap(),
                    count: elems.len() as u32,
                };
                self.claim(st, ptr, aty);
                stack.push(Slot {
                    value: ptr,
                    ty: aty,
                    own: Own::Owned,
                });
            }
            Expr::List(elems) => {
                let (handle, code) = self.emit_list_literal(b, st, stack, elems, None)?;
                let ty = Ty::List { elem: code };
                self.claim(st, handle, ty);
                stack.push(Slot {
                    value: handle,
                    ty,
                    own: Own::Owned,
                });
            }
            Expr::Map(pairs) => {
                // A standalone `{...}` with identifier keys is a struct
                // literal (only meaningful inside a typed var decl); with
                // literal keys it is a hashmap literal.
                let all_idents = pairs
                    .iter()
                    .all(|(k, _)| matches!(k, Expr::Variable { .. }));
                if all_idents {
                    return Err(CompileError::new(
                        "struct literal requires a declared struct type",
                        st.current_span,
                        "E340",
                    ));
                }
                let (handle, kcode, vcode) = self.emit_map_literal(b, st, stack, pairs, None)?;
                self.claim(
                    st,
                    handle,
                    Ty::Hashmap {
                        key: kcode,
                        value: vcode,
                    },
                );
                stack.push(Slot {
                    value: handle,
                    ty: Ty::Hashmap {
                        key: kcode,
                        value: vcode,
                    },
                    own: Own::Owned,
                });
            }
            Expr::Typeof { inner } => {
                self.compile_expr(b, st, stack, inner)?;
                self.emit_typeof(b, st, stack)?;
            }
            Expr::Borrow { inner } => {
                self.compile_expr(b, st, stack, inner)?;
                self.emit_borrow(b, st, stack)?;
            }
            Expr::Load { inner } => {
                self.compile_expr(b, st, stack, inner)?;
                self.emit_load(b, st, stack)?;
            }
            Expr::Store { addr, value } => {
                self.require_unsafe(st, "store through a pointer")?;
                self.compile_expr(b, st, stack, addr)?;
                self.compile_expr(b, st, stack, value)?;
                let value = self.pop_slot(st, stack, "'store'")?;
                let addr = self.pop_slot(st, stack, "'store'")?;
                let Ty::Ptr(code) = addr.ty else {
                    return Err(CompileError::new(
                        format!("'store' requires a pointer target, got {:?}", addr.ty),
                        st.current_span,
                        "E341",
                    ));
                };
                if code == 0x50 {
                    return Err(CompileError::new(
                        "cannot 'store' through a pointer to an unknown type",
                        st.current_span,
                        "E341",
                    ));
                }
                let pointee = elem_ty(code.into());
                let val = self.coerce_or_wrap(b, st, value.value, value.ty, pointee)?;
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    val,
                    addr.value,
                    0,
                );
            }
            Expr::ApplyTypeof => self.emit_typeof(b, st, stack)?,
            Expr::ApplyBorrow => self.emit_borrow(b, st, stack)?,
            Expr::ApplyLoad => self.emit_load(b, st, stack)?,
            Expr::Builtin { name } => {
                // Per-name words (`@map_get`, `@string_len`, the raw memory
                // words ...) first; anything not defined inline falls through
                // to the generic host-call path over `HOST_FNS`.
                if !self.emit_builtin(b, st, stack, name)? {
                    self.emit_host_call(b, st, stack, name)?;
                }
            }
        }
        Ok(())
    }

    /// Lower `pointer<T> load`: pop an address and push the pointee read from
    /// it. Handles are passed through as trivial values, matching `@list_get`
    /// (the runtime guards double frees).
    fn emit_load(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<()> {
        self.require_unsafe(st, "load through a pointer")?;
        let addr = self.pop_slot(st, stack, "'load'")?;
        let Ty::Ptr(code) = addr.ty else {
            return Err(CompileError::new(
                format!("'load' requires a pointer, got {:?}", addr.ty),
                st.current_span,
                "E341",
            ));
        };
        if code == 0x50 {
            return Err(CompileError::new(
                "cannot 'load' through a pointer to an unknown type",
                st.current_span,
                "E341",
            ));
        }
        let pointee = elem_ty(code.into());
        let val = b.ins().load(
            pointee.clty(self.ptr_type),
            cranelift_codegen::ir::MemFlagsData::trusted(),
            addr.value,
            0,
        );
        stack.push(Slot {
            value: val,
            ty: pointee,
            own: Own::Trivial,
        });
        Ok(())
    }

    /// Lower a string literal: reference its read-only data section and copy
    /// it into a runtime string handle.
    fn emit_string(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        value: &str,
    ) -> CResult<()> {
        let gv = self.fn_gvs.get(value).copied().ok_or_else(|| {
            CompileError::new(
                "string literal has no data section",
                st.current_span,
                "E371",
            )
        })?;
        let addr = b.ins().global_value(self.ptr_type, gv);
        // The lexeme carries the surrounding quotes, so the byte length must
        // come from the decoded literal (matching the data section contents).
        let len = decode_string_literal(value)
            .map_err(|m| CompileError::new(m, st.current_span, "E363"))?
            .len() as i64;
        let len = b.ins().iconst(irtypes::I64, len);
        let out = self.rt_call(b, st, "str_new", vec![addr, len])?;
        self.claim(st, out[0], Ty::String);
        stack.push(Slot {
            value: out[0],
            ty: Ty::String,
            own: Own::Owned,
        });
        Ok(())
    }

    /// Lower a `(a b c)` list literal. When `declared` carries the list type
    /// its element type wins; otherwise it is inferred from the elements.
    /// Returns the list handle and the element code.
    fn emit_list_literal(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        elems: &[Expr],
        declared: Option<Ty>,
    ) -> CResult<(Value, u64)> {
        let elem = if let Some(declared) = declared {
            declared
        } else {
            let mut t: Option<Ty> = None;
            for el in elems {
                self.compile_expr(b, st, stack, el)?;
                let slot = self.pop_slot(st, stack, "list element")?;
                t = Some(match t {
                    None => slot.ty,
                    Some(prev) => common_type(prev, slot.ty).ok_or_else(|| {
                        CompileError::new(
                            format!(
                                "list literal elements have incompatible types {prev:?} and {:?}",
                                slot.ty
                            ),
                            st.current_span,
                            "E345",
                        )
                    })?,
                });
            }
            t.ok_or_else(|| {
                CompileError::new(
                    "empty list literal needs a type annotation",
                    st.current_span,
                    "E345",
                )
            })?
        };
        let code = elem_code(elem).ok_or_else(|| {
            CompileError::new(
                format!("list element type {elem:?} is not supported"),
                st.current_span,
                "E345",
            )
        })?;
        let handle = self.emit_list_new(b, st, elem)?;
        self.init_list_elements(b, st, stack, elem, handle, elems)?;
        Ok((handle, code))
    }

    /// Allocate an empty list via the runtime.
    fn emit_list_new(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        elem: Ty,
    ) -> CResult<Value> {
        let size = b.ins().iconst(irtypes::I64, elem.elem_size() as i64);
        let out = self.rt_call(b, st, "list_new", vec![size])?;
        Ok(out[0])
    }

    /// Push every element of a list literal into `handle`.
    fn init_list_elements(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        elem: Ty,
        handle: Value,
        elems: &[Expr],
    ) -> CResult<()> {
        for el in elems {
            self.compile_expr(b, st, stack, el)?;
            let slot = self.pop_slot(st, stack, "list element")?;
            let val = coerce(b, slot.value, slot.ty, elem, self.ptr_type, st.current_span)?;
            let arg = self.rt_arg(
                b,
                st,
                Slot {
                    value: val,
                    ty: elem,
                    own: Own::Trivial,
                },
            )?;
            self.rt_call(b, st, "list_push", vec![handle, arg])?;
            if self.is_heap(elem) {
                // The list stores the element's handle and now owns it; the
                // temporary must not free it.
                st.moved.insert(slot.value);
            } else {
                // Scalar element: the list copied the value, drop the temp.
                self.consume(b, st, slot)?;
            }
        }
        Ok(())
    }

    /// Lower a `{k v ...}` hashmap literal. When `declared` carries the map
    /// type, its key/value types win; otherwise they are inferred. Returns the
    /// map handle and the key/value codes.
    fn emit_map_literal(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        pairs: &[(Expr, Expr)],
        declared: Option<Ty>,
    ) -> CResult<(Value, u64, u64)> {
        let (kt, vt) = match declared {
            Some(Ty::Hashmap { key, value }) => (elem_ty(key), elem_ty(value)),
            Some(_) => {
                return Err(CompileError::new(
                    "map literal requires a hashmap type",
                    st.current_span,
                    "E306",
                ));
            }
            None => {
                let mut kt: Option<Ty> = None;
                let mut vt: Option<Ty> = None;
                for (k, v) in pairs {
                    self.compile_expr(b, st, stack, k)?;
                    let ks = self.pop_slot(st, stack, "map key")?;
                    kt = Some(merge_type(kt, ks.ty)?);
                    self.compile_expr(b, st, stack, v)?;
                    let vs = self.pop_slot(st, stack, "map value")?;
                    vt = Some(merge_type(vt, vs.ty)?);
                }
                let kt = kt.ok_or_else(|| {
                    CompileError::new(
                        "empty map literal needs a type annotation",
                        st.current_span,
                        "E306",
                    )
                })?;
                let vt = vt.ok_or_else(|| {
                    CompileError::new(
                        "empty map literal needs a type annotation",
                        st.current_span,
                        "E306",
                    )
                })?;
                (kt, vt)
            }
        };
        let kcode = elem_code(kt).ok_or_else(|| {
            CompileError::new(
                format!("map key type {kt:?} is not supported"),
                st.current_span,
                "E306",
            )
        })?;
        let vcode = elem_code(vt).ok_or_else(|| {
            CompileError::new(
                format!("map value type {vt:?} is not supported"),
                st.current_span,
                "E306",
            )
        })?;
        let keys_string = b
            .ins()
            .iconst(irtypes::I64, if kt == Ty::String { 1 } else { 0 });
        let out = self.rt_call(b, st, "map_new", vec![keys_string])?;
        let handle = out[0];
        for (k, v) in pairs {
            self.compile_expr(b, st, stack, k)?;
            let ks = self.pop_slot(st, stack, "map key")?;
            let karg = coerce(b, ks.value, ks.ty, kt, self.ptr_type, st.current_span)?;
            let karg = self.rt_arg(
                b,
                st,
                Slot {
                    value: karg,
                    ty: kt,
                    own: Own::Trivial,
                },
            )?;
            self.compile_expr(b, st, stack, v)?;
            let vs = self.pop_slot(st, stack, "map value")?;
            let varg = coerce(b, vs.value, vs.ty, vt, self.ptr_type, st.current_span)?;
            let varg = self.rt_arg(
                b,
                st,
                Slot {
                    value: varg,
                    ty: vt,
                    own: Own::Trivial,
                },
            )?;
            self.rt_call(b, st, "map_insert", vec![handle, karg, varg])?;
            // The map stores the key/value handles directly and owns any heap
            // storage they point to; the temporaries must not free them.
            if self.is_heap(kt) {
                st.moved.insert(ks.value);
            }
            if self.is_heap(vt) {
                st.moved.insert(vs.value);
            }
        }
        Ok((handle, kcode, vcode))
    }

    /// Lower a `@name` builtin. Borrows/moves are pointer identity; regions are
    /// no-ops; strings/lists/maps delegate to the host runtime.
    fn emit_builtin(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        name: &str,
    ) -> CResult<bool> {
        match name {
            // Raw memory words (inlined, not host calls): `addr @load` reads a
            // 64-bit word, `addr value @store` writes one. Typed access goes
            // through the `pointer<T>` layer (`load`, member access, `set`).
            "load" => {
                self.require_unsafe(st, "@load")?;
                let addr = self.pop_slot(st, stack, "'@load'")?;
                let val = b.ins().load(
                    irtypes::I64,
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    addr.value,
                    0,
                );
                stack.push(Slot {
                    value: val,
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
            "store" => {
                self.require_unsafe(st, "@store")?;
                let value = self.pop_slot(st, stack, "'@store'")?;
                let addr = self.pop_slot(st, stack, "'@store'")?;
                let val = coerce(
                    b,
                    value.value,
                    value.ty,
                    Ty::I64,
                    self.ptr_type,
                    st.current_span,
                )?;
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    val,
                    addr.value,
                    0,
                );
            }
            "make_region" => {
                let out = self.rt_call(b, st, "region_new", vec![])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
            "free_region" => {
                let region = self.pop_slot(st, stack, "'@free_region'")?;
                // Values still borrowed from this region would escape the free.
                let attached: Vec<Value> = st
                    .region_attached
                    .iter()
                    .filter_map(|(val, reg)| {
                        if *reg == region.value {
                            Some(*val)
                        } else {
                            None
                        }
                    })
                    .collect();
                for val in &attached {
                    if st.borrowed.contains(val) {
                        let mut err = CompileError::new(
                            "region escape: cannot free region while a borrow of an attached value is live",
                            st.current_span,
                            "E376",
                        )
                        .with_primary_message("region freed here")
                        .with_note("freeing a region invalidates every value attached to it")
                        .with_help("release borrows of attached values (e.g. `pop`) before `region.free`");
                        if let Some(site) = st.borrow_sites.get(val).copied() {
                            err = err.with_label(site, "borrow of attached value still live");
                        }
                        return Err(err);
                    }
                    st.region_attached.remove(val);
                    st.region_freed.insert(*val);
                }
                self.rt_call(b, st, "region_free", vec![region.value])?;
            }
            "put_region" => {
                let region = self.pop_slot(st, stack, "'@put_region'")?;
                let value = self.pop_slot(st, stack, "'@put_region'")?;
                if !value.ty.is_pointer() {
                    return Err(CompileError::new(
                        format!(
                            "'@put_region' requires a reference, struct, array, string or container, got {:?}",
                            value.ty
                        ),
                        st.current_span,
                        "E372",
                    ));
                }
                self.require_region_live(st, value.value)?;
                self.require_not_borrowed(st, value.value, "put into region")?;
                let kind = b.ins().iconst(irtypes::I64, kind_code(value.ty) as i64);
                self.rt_call(
                    b,
                    st,
                    "region_register",
                    vec![value.value, kind, region.value],
                )?;
                // The region now owns the value; the stack must not free it.
                st.moved.insert(value.value);
                st.region_attached.insert(value.value, region.value);
                st.borrowed.insert(value.value);
                st.borrow_sites.insert(value.value, st.current_span);
                stack.push(Slot {
                    value: value.value,
                    ty: value.ty,
                    own: Own::Borrow,
                });
            }

            "string_join" => {
                let sep = self.pop_slot(st, stack, "'@string_join'")?;
                let right = self.pop_slot(st, stack, "'@string_join'")?;
                let left = self.pop_slot(st, stack, "'@string_join'")?;
                for s in [&left, &right, &sep] {
                    if s.ty != Ty::String {
                        return Err(CompileError::new(
                            format!("'@string_join' requires string operands, got {:?}", s.ty),
                            st.current_span,
                            "E372",
                        ));
                    }
                }
                let joined = self.rt_call(b, st, "str_join", vec![left.value, sep.value])?;
                let joined = self.rt_call(b, st, "str_join", vec![joined[0], right.value])?;
                self.consume(b, st, left)?;
                self.consume(b, st, sep)?;
                self.consume(b, st, right)?;
                self.claim(st, joined[0], Ty::String);
                stack.push(Slot {
                    value: joined[0],
                    ty: Ty::String,
                    own: Own::Owned,
                });
            }
            "string_len" => {
                let s = self.pop_slot(st, stack, "'@string_len'")?;
                if s.ty != Ty::String {
                    return Err(CompileError::new(
                        format!("'@string_len' requires a string, got {:?}", s.ty),
                        st.current_span,
                        "E372",
                    ));
                }
                let out = self.rt_call(b, st, "str_len", vec![s.value])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }

            "list_push" => {
                let value = self.pop_slot(st, stack, "'@list_push'")?;
                let list = self.pop_slot(st, stack, "'@list_push'")?;
                let Ty::List { elem } = list.ty else {
                    return Err(CompileError::new(
                        format!("'@list_push' requires a list, got {:?}", list.ty),
                        st.current_span,
                        "E372",
                    ));
                };
                self.require_region_live(st, list.value)?;
                self.require_not_borrowed(st, list.value, "mutate")?;
                let elem_ty = elem_ty(elem);
                let val = coerce(
                    b,
                    value.value,
                    value.ty,
                    elem_ty,
                    self.ptr_type,
                    st.current_span,
                )?;
                let arg = self.rt_arg(
                    b,
                    st,
                    Slot {
                        value: val,
                        ty: elem_ty,
                        own: Own::Trivial,
                    },
                )?;
                self.rt_call(b, st, "list_push", vec![list.value, arg])?;
                stack.push(list);
            }
            "list_len" => {
                let list = self.pop_slot(st, stack, "'@list_len'")?;
                if !matches!(list.ty, Ty::List { .. }) {
                    return Err(CompileError::new(
                        format!("'@list_len' requires a list, got {:?}", list.ty),
                        st.current_span,
                        "E372",
                    ));
                }
                let out = self.rt_call(b, st, "list_len", vec![list.value])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
            "list_get" => {
                let idx = self.pop_slot(st, stack, "'@list_get'")?;
                let list = self.pop_slot(st, stack, "'@list_get'")?;
                let Ty::List { elem } = list.ty else {
                    return Err(CompileError::new(
                        format!("'@list_get' requires a list, got {:?}", list.ty),
                        st.current_span,
                        "E372",
                    ));
                };
                let elem_ty = elem_ty(elem);
                let idx = coerce(
                    b,
                    idx.value,
                    idx.ty,
                    Ty::I64,
                    self.ptr_type,
                    st.current_span,
                )?;
                let len = self.rt_call(b, st, "list_len", vec![list.value])?;
                let inb = b.ins().icmp(IntCC::UnsignedLessThan, idx, len[0]);
                b.ins().trapz(inb, TrapCode::unwrap_user(1));
                let base = b.ins().load(
                    self.ptr_type,
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    list.value,
                    LIST_DATA_OFFSET,
                );
                let off = b.ins().imul_imm(idx, elem_ty.elem_size() as i64);
                let addr = b.ins().iadd(base, off);
                let val = b.ins().load(
                    elem_ty.clty(self.ptr_type),
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    addr,
                    0,
                );
                stack.push(Slot {
                    value: val,
                    ty: elem_ty,
                    own: Own::Trivial,
                });
            }
            "list_set" => {
                let value = self.pop_slot(st, stack, "'@list_set'")?;
                let idx = self.pop_slot(st, stack, "'@list_set'")?;
                let list = self.pop_slot(st, stack, "'@list_set'")?;
                let Ty::List { elem } = list.ty else {
                    return Err(CompileError::new(
                        format!("'@list_set' requires a list, got {:?}", list.ty),
                        st.current_span,
                        "E372",
                    ));
                };
                self.require_region_live(st, list.value)?;
                self.require_not_borrowed(st, list.value, "mutate")?;
                let elem_ty = elem_ty(elem);
                let idx = coerce(
                    b,
                    idx.value,
                    idx.ty,
                    Ty::I64,
                    self.ptr_type,
                    st.current_span,
                )?;
                let len = self.rt_call(b, st, "list_len", vec![list.value])?;
                let inb = b.ins().icmp(IntCC::UnsignedLessThan, idx, len[0]);
                b.ins().trapz(inb, TrapCode::unwrap_user(1));
                let base = b.ins().load(
                    self.ptr_type,
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    list.value,
                    LIST_DATA_OFFSET,
                );
                let off = b.ins().imul_imm(idx, elem_ty.elem_size() as i64);
                let addr = b.ins().iadd(base, off);
                let val = coerce(
                    b,
                    value.value,
                    value.ty,
                    elem_ty,
                    self.ptr_type,
                    st.current_span,
                )?;
                b.ins()
                    .store(cranelift_codegen::ir::MemFlagsData::trusted(), val, addr, 0);
                stack.push(list);
            }

            "map_get" => {
                let key = self.pop_slot(st, stack, "'@map_get'")?;
                let map = self.pop_slot(st, stack, "'@map_get'")?;
                let Ty::Hashmap {
                    key: kcode,
                    value: vcode,
                } = map.ty
                else {
                    return Err(CompileError::new(
                        format!("'@map_get' requires a hashmap, got {:?}", map.ty),
                        st.current_span,
                        "E372",
                    ));
                };
                let kt = elem_ty(kcode);
                let vt = elem_ty(vcode);
                let karg = coerce(b, key.value, key.ty, kt, self.ptr_type, st.current_span)?;
                let karg = self.rt_arg(
                    b,
                    st,
                    Slot {
                        value: karg,
                        ty: kt,
                        own: Own::Trivial,
                    },
                )?;
                let slot = b.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    1,
                    0,
                ));
                let found_ptr = b.ins().stack_addr(self.ptr_type, slot, 0);
                let out = self.rt_call(b, st, "map_get", vec![map.value, karg, found_ptr])?;
                let val = out[0];
                let found = b.ins().load(
                    irtypes::I8,
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    found_ptr,
                    0,
                );
                let val = if vt.clty(self.ptr_type) == self.ptr_type {
                    val
                } else {
                    coerce(b, val, Ty::I64, vt, self.ptr_type, st.current_span)?
                };
                stack.push(Slot {
                    value: val,
                    ty: vt,
                    own: Own::Trivial,
                });
                stack.push(Slot {
                    value: found,
                    ty: Ty::Bool,
                    own: Own::Trivial,
                });
            }
            "map_set" => {
                let value = self.pop_slot(st, stack, "'@map_set'")?;
                let key = self.pop_slot(st, stack, "'@map_set'")?;
                let map = self.pop_slot(st, stack, "'@map_set'")?;
                let Ty::Hashmap {
                    key: kcode,
                    value: vcode,
                } = map.ty
                else {
                    return Err(CompileError::new(
                        format!("'@map_set' requires a hashmap, got {:?}", map.ty),
                        st.current_span,
                        "E372",
                    ));
                };
                self.require_region_live(st, map.value)?;
                self.require_not_borrowed(st, map.value, "mutate")?;
                let kt = elem_ty(kcode);
                let vt = elem_ty(vcode);
                let karg = coerce(b, key.value, key.ty, kt, self.ptr_type, st.current_span)?;
                let karg = self.rt_arg(
                    b,
                    st,
                    Slot {
                        value: karg,
                        ty: kt,
                        own: Own::Trivial,
                    },
                )?;
                let varg = coerce(b, value.value, value.ty, vt, self.ptr_type, st.current_span)?;
                let varg = self.rt_arg(
                    b,
                    st,
                    Slot {
                        value: varg,
                        ty: vt,
                        own: Own::Trivial,
                    },
                )?;
                self.rt_call(b, st, "map_insert", vec![map.value, karg, varg])?;
                stack.push(map);
            }
            "map_len" => {
                let map = self.pop_slot(st, stack, "'@map_len'")?;
                if !matches!(map.ty, Ty::Hashmap { .. }) {
                    return Err(CompileError::new(
                        format!("'@map_len' requires a hashmap, got {:?}", map.ty),
                        st.current_span,
                        "E372",
                    ));
                }
                let out = self.rt_call(b, st, "map_len", vec![map.value])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }

            "print" => {
                let s = self.pop_slot(st, stack, "'@print'")?;
                if s.ty != Ty::String {
                    return Err(CompileError::new(
                        format!("'@print' requires a string, got {:?}", s.ty),
                        st.current_span,
                        "E372",
                    ));
                }
                self.rt_call(b, st, "print_str", vec![s.value])?;
            }
            "print_int" => {
                let v = self.pop_slot(st, stack, "'@print_int'")?;
                let arg = coerce(b, v.value, v.ty, Ty::I64, self.ptr_type, st.current_span)?;
                self.rt_call(b, st, "print_int", vec![arg])?;
            }
            "print_float" => {
                let v = self.pop_slot(st, stack, "'@print_float'")?;
                let arg = coerce(b, v.value, v.ty, Ty::F64, self.ptr_type, st.current_span)?;
                self.rt_call(b, st, "print_float", vec![arg])?;
            }
            "print_newline" => {
                self.rt_call(b, st, "print_newline", Vec::new())?;
            }
            "print_array" => {
                let v = self.pop_slot(st, stack, "'@print_array'")?;
                if !matches!(v.ty, Ty::Array { .. }) {
                    return Err(CompileError::new(
                        format!("'@print_array' requires an array, got {:?}", v.ty),
                        st.current_span,
                        "E372",
                    ));
                }
                let kind = b.ins().iconst(irtypes::I64, kind_code(v.ty) as i64);
                self.rt_call(b, st, "print_array", vec![v.value, kind])?;
            }
            "print_list" => {
                let v = self.pop_slot(st, stack, "'@print_list'")?;
                if !matches!(v.ty, Ty::List { .. }) {
                    return Err(CompileError::new(
                        format!("'@print_list' requires a list, got {:?}", v.ty),
                        st.current_span,
                        "E372",
                    ));
                }
                let kind = b.ins().iconst(irtypes::I64, kind_code(v.ty) as i64);
                self.rt_call(b, st, "print_list", vec![v.value, kind])?;
            }
            "print_hashmap" => {
                let v = self.pop_slot(st, stack, "'@print_hashmap'")?;
                if !matches!(v.ty, Ty::Hashmap { .. }) {
                    return Err(CompileError::new(
                        format!("'@print_hashmap' requires a hashmap, got {:?}", v.ty),
                        st.current_span,
                        "E372",
                    ));
                }
                let kind = b.ins().iconst(irtypes::I64, kind_code(v.ty) as i64);
                self.rt_call(b, st, "print_hashmap", vec![v.value, kind])?;
            }

            _ => return Ok(false),
        }
        Ok(true)
    }

    fn emit_call(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        target: &Expr,
    ) -> CResult<()> {
        let name = match target {
            Expr::Variable { name } => {
                if let Some(fq) = st.local_funcs.get(name) {
                    fq.clone()
                } else if let Some(mod_path) = &st.module {
                    let fq = format!("{mod_path}::{name}");
                    if self.func_ids.contains_key(&fq) {
                        fq
                    } else if let Some(plain) = self.plain_funcs.get(name) {
                        plain.clone()
                    } else {
                        name.clone()
                    }
                } else if let Some(plain) = self.plain_funcs.get(name) {
                    plain.clone()
                } else {
                    name.clone()
                }
            }
            Expr::Member { base, member } => {
                if let Expr::Variable { name } = base.as_ref() {
                    if let Some(path) = self.aliases.get(name) {
                        // An item import under a scope exposes only that item.
                        if let Some(item) = self.item_aliases.get(name)
                            && item != member
                        {
                            return Err(CompileError::new(
                                format!(
                                    "module '{}' only exports '{}' (not '{member}')",
                                    path, item
                                ),
                                st.current_span,
                                "E330",
                            ));
                        }
                        let fq = format!("{path}::{member}");
                        // Intrinsic std APIs (`std.region::*`, generalized
                        // `std.list::*`) are resolved without a Yarrow body.
                        if self.is_std_intrinsic(&fq) {
                            fq
                        } else if !self.func_ids.contains_key(&fq) {
                            return Err(CompileError::new(
                                format!("module '{path}' has no function '{member}'"),
                                st.current_span,
                                "E330",
                            ));
                        } else {
                            fq
                        }
                    } else {
                        self.method_name(st, base, member)?
                    }
                } else {
                    self.method_name(st, base, member)?
                }
            }
            _ => {
                return Err(CompileError::new(
                    "'call' target must be a function name",
                    st.current_span,
                    "E329",
                ));
            }
        };
        // Polymorphic / host-wrapping std calls: lower without a Yarrow body.
        if self.is_std_intrinsic(&name) {
            return self.emit_std_intrinsic(b, st, stack, &name);
        }
        // Calling an `unsafe` function requires an unsafe context.
        if self.unsafe_funcs.contains(&name) {
            self.require_unsafe(st, &format!("call to '{name}'"))?;
        }
        self.require_public_export(&name, st.module.as_deref())?;
        // Undefined function names fall back to the host registry: the call
        // is lowered generically from the table's signature.
        if !self.sig_tys.contains_key(&name) {
            if crate::runtime::HOST_FNS.iter().any(|h| h.name == name) {
                return self.emit_host_call(b, st, stack, &name);
            }
            return Err(CompileError::new(
                format!("unknown function '{name}'"),
                st.current_span,
                "E330",
            ));
        }
        let (param_tys, return_tys) = self.sig_tys.get(&name).cloned().ok_or_else(|| {
            CompileError::new("missing function signature", st.current_span, "E330")
        })?;
        let n = param_tys.len();
        if stack.len() < n {
            return Err(CompileError::new(
                format!("call to '{name}' requires {n} argument(s)"),
                st.current_span,
                "E331",
            ));
        }
        let tail = stack.split_off(stack.len() - n);
        let mut args: Vec<Value> = Vec::with_capacity(n);
        let mut owned_temps: Vec<Slot> = Vec::new();
        for (i, slot) in tail.iter().enumerate() {
            // Passing a borrow into a callee consumes that stack borrow from
            // the caller's point of view (the reference lives in the callee).
            if slot.own == Own::Borrow {
                st.borrowed.remove(&slot.value);
                st.borrow_sites.remove(&slot.value);
            }
            // An owned value passed by value to a callee is borrowed by the
            // callee (never freed there). The caller drops it once the call
            // returns; this frees immediately (in the current block) so the
            // drop stays dominance-correct inside loops. Variable values are
            // Trivial here — the variable drop at scope exit handles them.
            if slot.own.is_owned() && self.is_heap(slot.ty) {
                owned_temps.push(*slot);
            }
            let arg = self.coerce_or_wrap(b, st, slot.value, slot.ty, param_tys[i])?;
            // Wrapping a raw member into a union freshly allocates a block the
            // caller owns and must free after the call.
            if matches!(param_tys[i], Ty::Union(_)) && !matches!(slot.ty, Ty::Union(_)) {
                owned_temps.push(Slot {
                    value: arg,
                    ty: param_tys[i],
                    own: Own::Owned,
                });
            }
            args.push(arg);
        }

        let fref = st.frefs.get(&name).copied().ok_or_else(|| {
            CompileError::new(
                format!("unregistered callee '{name}'"),
                st.current_span,
                "E330",
            )
        })?;
        let call_inst = b.ins().call(fref, &args);
        let results: Vec<Value> = b.inst_results(call_inst).to_vec();
        if let Some(payload_ty) = error_return(&return_tys)? {
            // Fallible `|T Err|` callee: results are `(env, payload)`. Push the
            // payload (as its declared type) followed by the envelope tag so
            // `unwrap`/`handle` pop the tag first.
            let env = results[0];
            let payload = if payload_ty == Ty::Void {
                results[1]
            } else {
                coerce(
                    b,
                    results[1],
                    Ty::I64,
                    payload_ty,
                    self.ptr_type,
                    st.current_span,
                )?
            };
            self.claim(st, payload, payload_ty);
            stack.push(Slot {
                value: payload,
                ty: payload_ty,
                own: if self.is_heap(payload_ty) {
                    Own::Owned
                } else {
                    Own::Trivial
                },
            });
            stack.push(Slot {
                value: env,
                ty: Ty::Error,
                own: Own::Trivial,
            });
        } else {
            for (v, t) in results.into_iter().zip(&return_tys) {
                // Heap-typed return values transfer ownership to the caller.
                self.claim(st, v, *t);
                stack.push(Slot {
                    value: v,
                    ty: *t,
                    own: Own::Owned,
                });
            }
        }
        for slot in owned_temps {
            self.emit_drop(b, st, slot)?;
        }
        Ok(())
    }

    /// Resolve a `base.member` call as a struct method `Struct::member`.
    fn method_name(&self, st: &FnState, base: &Expr, member: &str) -> CResult<String> {
        let sid = self.base_struct(st, base)?;
        let sname = self.struct_layout(sid).name.clone();
        let method = format!("{sname}::{member}");
        if !self.func_ids.contains_key(&method) {
            return Err(CompileError::new(
                format!("struct '{sname}' has no method '{member}'"),
                st.current_span,
                "E342",
            ));
        }
        Ok(method)
    }

    /// `value typeof`: pop the value and push its static type as a runtime kind
    /// code. Heap values may arrive as borrows (variable access, `dup`), which
    /// are released here — the data stays owned by its owner. References report
    /// their pointee type (a `reference<T>` has the physical type `T`).
    fn emit_typeof(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<()> {
        let slot = self.pop_slot(st, stack, "'typeof'")?;
        self.consume(b, st, slot)?;
        let v = b.ins().iconst(irtypes::I64, kind_code(slot.ty) as i64);
        stack.push(Slot {
            value: v,
            ty: Ty::I64,
            own: Own::Trivial,
        });
        Ok(())
    }

    /// `value borrow`: push a non-owning reference to the value. The value must
    /// be a heap value (reference, struct, array, string or container). The
    /// stack no longer owns it (its original owner still does), so any drop
    /// here is skipped via `moved`; the borrow is tracked so consuming the
    /// reference releases it.
    fn emit_borrow(
        &mut self,
        _b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<()> {
        let s = self.pop_slot(st, stack, "'borrow'")?;
        if !s.ty.is_pointer() {
            return Err(CompileError::new(
                format!(
                    "'borrow' requires a reference, struct, array, string or container, got {:?}",
                    s.ty
                ),
                st.current_span,
                "E341",
            ));
        }
        self.require_region_live(st, s.value)?;
        self.require_can_borrow(st, s.value)?;
        if s.own.is_owned() {
            st.moved.insert(s.value);
        }
        st.borrowed.insert(s.value);
        st.borrow_sites.insert(s.value, st.current_span);
        stack.push(Slot {
            value: s.value,
            ty: s.ty,
            own: Own::Borrow,
        });
        Ok(())
    }

    /// `source target move`: transfer ownership of `source`'s storage to the
    /// variable `target`. The source is marked moved (its old owner stops
    /// freeing it) and the target is rebound to the same storage, dropping the
    /// value the target previously owned.
    fn emit_move(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        target: &str,
        source: &Expr,
    ) -> CResult<()> {
        let src_name = match source {
            Expr::Variable { name } => {
                self.require_not_moved_var(st, name)?;
                Some(name.clone())
            }
            _ => None,
        };
        self.compile_expr(b, st, stack, source)?;
        let src = self.pop_slot(st, stack, "'move' source")?;
        if !src.ty.is_pointer() {
            return Err(CompileError::new(
                format!(
                    "'move' requires a reference, struct, array, string or container, got {:?}",
                    src.ty
                ),
                st.current_span,
                "E341",
            ));
        }
        self.require_region_live(st, src.value)?;
        self.require_not_borrowed(st, src.value, "move")?;
        let (var, ty, _own) = st.vars.get(target).cloned().ok_or_else(|| {
            CompileError::new(
                format!("unknown variable '{target}'"),
                st.current_span,
                "E320",
            )
        })?;
        // Type-check the transfer (exact type match or a valid coercion).
        coerce(b, src.value, src.ty, ty, self.ptr_type, st.current_span)?;
        // Drop the value the target currently owns (the runtime guards double
        // frees), then rebind it to the source's storage.
        if self.is_heap(ty) {
            let old = Slot {
                value: b.use_var(var),
                ty,
                own: Own::Owned,
            };
            self.emit_drop(b, st, old)?;
        }
        st.moved.insert(src.value);
        if let Some(name) = src_name {
            st.moved_vars.insert(name.clone());
            st.move_sites.insert(name.clone(), st.current_span);
            if let Some((svar, sty, _)) = st.vars.get(&name).cloned() {
                st.vars.insert(name, (svar, sty, Own::Trivial));
            }
        }
        self.claim(st, src.value, ty);
        b.def_var(var, src.value);
        let var_own = if st.borrowed.contains(&src.value) {
            Own::Borrow
        } else if self.is_heap(ty) {
            Own::Owned
        } else {
            Own::Trivial
        };
        st.vars.insert(target.to_string(), (var, ty, var_own));
        Ok(())
    }

    fn emit_stackop(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        op: StackOp,
    ) -> CResult<()> {
        match op {
            StackOp::Dup => {
                let s = self.pop_slot(st, stack, "dup")?;
                // A duplicate is a second live reference to the same storage;
                // demote the owner to a borrow so only one side drops it.
                let own = if s.own.is_owned() {
                    st.moved.insert(s.value);
                    Own::Borrow
                } else {
                    s.own
                };
                stack.push(Slot {
                    value: s.value,
                    ty: s.ty,
                    own,
                });
                stack.push(Slot {
                    value: s.value,
                    ty: s.ty,
                    own,
                });
            }
            StackOp::Unrot => {
                self.require_stack(st, stack, 3, "unrot")?;
                let a = self.pop_slot(st, stack, "unrot")?;
                let second = self.pop_slot(st, stack, "unrot")?;
                let third = self.pop_slot(st, stack, "unrot")?;
                stack.push(a);
                stack.push(third);
                stack.push(second);
            }
            StackOp::Swap => {
                self.require_stack(st, stack, 2, "swap")?;
                let top = self.pop_slot(st, stack, "swap")?;
                let sec = self.pop_slot(st, stack, "swap")?;
                stack.push(top);
                stack.push(sec);
            }
            StackOp::Rot => {
                self.require_stack(st, stack, 3, "rot")?;
                let a = self.pop_slot(st, stack, "rot")?;
                let second = self.pop_slot(st, stack, "rot")?;
                let third = self.pop_slot(st, stack, "rot")?;
                stack.push(second);
                stack.push(third);
                stack.push(a);
            }
            StackOp::Pop => {
                let slot = self.pop_slot(st, stack, "pop")?;
                self.consume(b, st, slot)?;
            }
            StackOp::Drop => {
                // `drop` clears the whole stack and releases borrows on it.
                while let Some(slot) = stack.pop() {
                    self.consume(b, st, slot)?;
                }
            }
        }
        Ok(())
    }

    /// `loop.break` / `loop.continue` as expression statements.
    /// Returns `true` when `e` was a loop-control intrinsic.
    fn emit_loop_control(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        e: &Expr,
    ) -> CResult<bool> {
        let Some((alias, member)) = loop_helper_parts(e) else {
            return Ok(false);
        };
        if !self.is_std_loop_alias(alias) {
            return Ok(false);
        }
        match member {
            "break" => {
                let loop_ctx = st.loops.last().ok_or_else(|| {
                    CompileError::new("'loop.break' outside of a loop", st.current_span, "E321")
                })?;
                b.ins().jump(loop_ctx.break_to, &[]);
                self.dead_block(b);
                Ok(true)
            }
            "continue" => {
                let loop_ctx = st.loops.last().ok_or_else(|| {
                    CompileError::new("'loop.continue' outside of a loop", st.current_span, "E322")
                })?;
                b.ins().jump(loop_ctx.continue_to, &[]);
                self.dead_block(b);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Resolve `loop.value` / `loop.index` to the current iterable binding.
    fn loop_intrinsic_value(&self, st: &FnState, base: &Expr, member: &str) -> Option<(Value, Ty)> {
        let Expr::Variable { name } = base else {
            return None;
        };
        if !self.is_std_loop_alias(name) {
            return None;
        }
        let loop_ctx = st.loops.last()?;
        match member {
            "value" => loop_ctx.value,
            "index" => loop_ctx.index.map(|v| (v, Ty::I64)),
            _ => None,
        }
    }

    fn is_std_loop_alias(&self, alias: &str) -> bool {
        self.aliases.get(alias).is_some_and(|p| p == "std.loop")
    }

    /// Fully-qualified std APIs implemented as compiler intrinsics rather than
    /// ordinary Yarrow bodies (polymorphic host wrappers).
    fn is_std_intrinsic(&self, fq: &str) -> bool {
        matches!(
            fq,
            "std.region::create"
                | "std.region::put"
                | "std.region::free"
                | "std.list::push_last"
                | "std.list::len"
                | "std.list::get"
                | "std.list::put"
                | "std.map::len"
                | "std.map::get"
                | "std.map::put"
        )
    }

    /// Lower a std intrinsic call (`region.*`, generalized `list.*` / `map.*`).
    fn emit_std_intrinsic(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        fq: &str,
    ) -> CResult<()> {
        match fq {
            "std.region::create" => {
                self.emit_builtin(b, st, stack, "make_region")?;
                Ok(())
            }
            "std.region::free" => {
                self.emit_builtin(b, st, stack, "free_region")?;
                Ok(())
            }
            "std.region::put" => {
                self.emit_builtin(b, st, stack, "put_region")?;
                Ok(())
            }
            "std.list::push_last" => {
                self.emit_builtin(b, st, stack, "list_push")?;
                Ok(())
            }
            "std.list::len" => {
                self.emit_builtin(b, st, stack, "list_len")?;
                Ok(())
            }
            "std.list::get" => {
                self.emit_builtin(b, st, stack, "list_get")?;
                Ok(())
            }
            "std.list::put" => {
                self.emit_builtin(b, st, stack, "list_set")?;
                Ok(())
            }
            "std.map::len" => {
                self.emit_builtin(b, st, stack, "map_len")?;
                Ok(())
            }
            "std.map::get" => {
                self.emit_builtin(b, st, stack, "map_get")?;
                Ok(())
            }
            "std.map::put" => {
                self.emit_builtin(b, st, stack, "map_set")?;
                Ok(())
            }
            _ => Err(CompileError::new(
                format!("unknown std intrinsic '{fq}'"),
                st.current_span,
                "E330",
            )),
        }
    }

    /// Non-`public` functions do not export across `require`.
    fn require_public_export(&self, fq: &str, caller_module: Option<&str>) -> CResult<()> {
        let Some((mod_path, _)) = fq.split_once("::") else {
            return Ok(());
        };
        // `Type::method` in the main program has no module path dots-only form;
        // module paths contain `.` (e.g. `std.io`, `helpers.greet`).
        if !mod_path.contains('.') {
            return Ok(());
        }
        if caller_module == Some(mod_path) {
            return Ok(());
        }
        if self.public_funcs.contains(fq) {
            return Ok(());
        }
        Err(CompileError::new(
            format!("'{fq}' is not public and cannot be used across 'require'"),
            Span::default(),
            "E381",
        )
        .with_note("only `public` functions are visible to other modules via `require`")
        .with_help(format!(
            "mark the function `public`, or call it only from within `{mod_path}`"
        )))
    }

    /// Deep-copy a heap value for a `copy` parameter. Scalars never reach here.
    fn emit_deep_copy(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        value: Value,
        ty: Ty,
    ) -> CResult<Value> {
        match ty {
            Ty::String => {
                // Clone by joining with an empty string.
                let zero = b.ins().iconst(self.ptr_type, 0);
                let zlen = b.ins().iconst(irtypes::I64, 0);
                let empty = self.rt_call(b, st, "str_new", vec![zero, zlen])?;
                let out = self.rt_call(b, st, "str_join", vec![value, empty[0]])?;
                Ok(out[0])
            }
            other => Err(CompileError::unsupported(
                format!("'copy' for {other:?} is not yet supported"),
                st.current_span,
                "E336",
            )),
        }
    }

    fn emit_not(
        &mut self,
        b: &mut FunctionBuilder,
        st: &FnState,
        stack: &mut Vec<Slot>,
        _op: UnOp,
        slot: Slot,
    ) -> CResult<()> {
        let v = match slot.ty {
            Ty::Bool => {
                let one = b.ins().iconst(irtypes::I8, 1);
                b.ins().bxor(slot.value, one)
            }
            t if t.is_int() => {
                let all = b.ins().iconst(t.clty(self.ptr_type), -1);
                b.ins().bxor(slot.value, all)
            }
            t if t.is_float() => b.ins().fneg(slot.value),
            _ => {
                return Err(CompileError::new(
                    format!("'not' requires a bool, int or float, got {:?}", slot.ty),
                    st.current_span,
                    "E332",
                ));
            }
        };
        stack.push(Slot {
            value: v,
            ty: slot.ty,
            own: Own::Trivial,
        });
        Ok(())
    }

    // ------------------------------------------------------------------
    // Binary operators
    // ------------------------------------------------------------------

    fn emit_bin(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        op: BinOp,
        l: Slot,
        r: Slot,
    ) -> CResult<()> {
        use BinOp::*;

        // Pointer arithmetic: `pointer<T> n +` (and `pointer<T> n -`) advance
        // the address by a byte offset. The result keeps the pointee type.
        let ptr_math = match (op, &l.ty, &r.ty) {
            (Plus, Ty::Ptr(_), _) if r.ty.is_int() => Some((&l, &r, false)),
            (Plus, _, Ty::Ptr(_)) if l.ty.is_int() => Some((&r, &l, false)),
            (Minus, Ty::Ptr(_), _) if r.ty.is_int() => Some((&l, &r, true)),
            _ => None,
        };
        if let Some((ptr, off, sub)) = ptr_math {
            self.require_unsafe(st, "pointer arithmetic")?;
            let off = coerce(
                b,
                off.value,
                off.ty,
                Ty::I64,
                self.ptr_type,
                st.current_span,
            )?;
            let addr = if sub {
                b.ins().isub(ptr.value, off)
            } else {
                b.ins().iadd(ptr.value, off)
            };
            stack.push(Slot {
                value: addr,
                ty: ptr.ty,
                own: Own::Trivial,
            });
            return Ok(());
        }

        // String concatenation before the pointer-like rejection: `string` is a
        // heap handle (`is_pointer`), but `~` is defined on it.
        if op == Concat {
            let lt = l.ty;
            let rt = r.ty;
            if lt != Ty::String || rt != Ty::String {
                return Err(CompileError::new(
                    format!("'~' requires string operands, got {lt:?} and {rt:?}"),
                    st.current_span,
                    "E335",
                ));
            }
            let out = self.rt_call(b, st, "str_join", vec![l.value, r.value])?;
            self.claim(st, out[0], Ty::String);
            stack.push(Slot {
                value: out[0],
                ty: Ty::String,
                own: Own::Owned,
            });
            return Ok(());
        }
        if op == Plus && (l.ty == Ty::String || r.ty == Ty::String) {
            return Err(CompileError::new(
                "string concatenation uses '~', not '+'",
                st.current_span,
                "E335",
            ));
        }

        if l.ty.is_pointer() || r.ty.is_pointer() {
            return Err(CompileError::new(
                format!(
                    "operand type {:?} cannot be used with '{:?}' (only address +/- byte offset is defined on pointers)",
                    if l.ty.is_pointer() { l.ty } else { r.ty },
                    op
                ),
                st.current_span,
                "E333",
            ));
        }

        let common = common_type(l.ty, r.ty).ok_or_else(|| {
            CompileError::new(
                format!(
                    "incompatible operand types {:?} and {:?} for {:?}",
                    l.ty, r.ty, op
                ),
                st.current_span,
                "E333",
            )
        })?;

        match op {
            Plus if common == Ty::String => {
                return Err(CompileError::new(
                    "string concatenation uses '~', not '+'",
                    st.current_span,
                    "E335",
                ));
            }
            Concat => {
                // Handled above.
                unreachable!()
            }
            Plus | Minus | Mul | Mod | Pow => {
                if common.is_float() {
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type, st.current_span)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type, st.current_span)?;
                    let v = match op {
                        Plus => b.ins().fadd(ll, rr),
                        Minus => b.ins().fsub(ll, rr),
                        Mul => b.ins().fmul(ll, rr),
                        Mod | Pow => {
                            return Err(float_mod_pow_error(op, st.current_span));
                        }
                        _ => unreachable!(),
                    };
                    stack.push(Slot {
                        value: v,
                        ty: common,
                        own: Own::Trivial,
                    });
                } else {
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type, st.current_span)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type, st.current_span)?;
                    let v = match op {
                        Plus => b.ins().iadd(ll, rr),
                        Minus => b.ins().isub(ll, rr),
                        Mul => b.ins().imul(ll, rr),
                        Mod => {
                            if common.is_signed() {
                                b.ins().srem(ll, rr)
                            } else {
                                b.ins().urem(ll, rr)
                            }
                        }
                        Pow => {
                            let lw =
                                coerce(b, ll, common, Ty::I64, self.ptr_type, st.current_span)?;
                            let rw =
                                coerce(b, rr, common, Ty::I64, self.ptr_type, st.current_span)?;
                            self.emit_int_pow(b, lw, rw, irtypes::I64)?
                        }
                        _ => unreachable!(),
                    };
                    stack.push(Slot {
                        value: v,
                        ty: common,
                        own: Own::Trivial,
                    });
                }
            }

            Div => {
                if common.is_float() {
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type, st.current_span)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type, st.current_span)?;
                    let v = b.ins().fdiv(ll, rr);
                    stack.push(Slot {
                        value: v,
                        ty: common,
                        own: Own::Trivial,
                    });
                } else {
                    // `10 4 /` yields 2.5: promote integers to f64.
                    let ll = coerce(b, l.value, l.ty, Ty::F64, self.ptr_type, st.current_span)?;
                    let rr = coerce(b, r.value, r.ty, Ty::F64, self.ptr_type, st.current_span)?;
                    let v = b.ins().fdiv(ll, rr);
                    stack.push(Slot {
                        value: v,
                        ty: Ty::F64,
                        own: Own::Trivial,
                    });
                }
            }

            Fdiv => {
                if common.is_float() {
                    return Err(CompileError::new(
                        "'//' (floor divide) requires integer operands",
                        st.current_span,
                        "E335",
                    ));
                }
                let ll = coerce(b, l.value, l.ty, common, self.ptr_type, st.current_span)?;
                let rr = coerce(b, r.value, r.ty, common, self.ptr_type, st.current_span)?;
                let v = if common.is_signed() {
                    b.ins().sdiv(ll, rr)
                } else {
                    b.ins().udiv(ll, rr)
                };
                stack.push(Slot {
                    value: v,
                    ty: common,
                    own: Own::Trivial,
                });
            }

            Eq | Ne | Gt | Gte | Lt | Lte => {
                if common == Ty::String {
                    let out = self.rt_call(b, st, "str_cmp", vec![l.value, r.value])?;
                    let cmp = out[0];
                    let zero = b.ins().iconst(irtypes::I64, 0);
                    let cc = match op {
                        Eq => IntCC::Equal,
                        Ne => IntCC::NotEqual,
                        Gt => IntCC::SignedGreaterThan,
                        Gte => IntCC::SignedGreaterThanOrEqual,
                        Lt => IntCC::SignedLessThan,
                        Lte => IntCC::SignedLessThanOrEqual,
                        _ => unreachable!(),
                    };
                    let res = b.ins().icmp(cc, cmp, zero);
                    stack.push(Slot {
                        value: res,
                        ty: Ty::Bool,
                        own: Own::Trivial,
                    });
                } else if common.is_float() {
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type, st.current_span)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type, st.current_span)?;
                    let fcc = match op {
                        Eq => FloatCC::Equal,
                        Ne => FloatCC::NotEqual,
                        Gt => FloatCC::GreaterThan,
                        Gte => FloatCC::GreaterThanOrEqual,
                        Lt => FloatCC::LessThan,
                        Lte => FloatCC::LessThanOrEqual,
                        _ => unreachable!(),
                    };
                    let res = b.ins().fcmp(fcc, ll, rr);
                    stack.push(Slot {
                        value: res,
                        ty: Ty::Bool,
                        own: Own::Trivial,
                    });
                } else {
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type, st.current_span)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type, st.current_span)?;
                    let cc = match op {
                        Eq => IntCC::Equal,
                        Ne => IntCC::NotEqual,
                        Gt if common.is_signed() => IntCC::SignedGreaterThan,
                        Gt => IntCC::UnsignedGreaterThan,
                        Gte if common.is_signed() => IntCC::SignedGreaterThanOrEqual,
                        Gte => IntCC::UnsignedGreaterThanOrEqual,
                        Lt if common.is_signed() => IntCC::SignedLessThan,
                        Lt => IntCC::UnsignedLessThan,
                        Lte if common.is_signed() => IntCC::SignedLessThanOrEqual,
                        Lte => IntCC::UnsignedLessThanOrEqual,
                        _ => unreachable!(),
                    };
                    let res = b.ins().icmp(cc, ll, rr);
                    stack.push(Slot {
                        value: res,
                        ty: Ty::Bool,
                        own: Own::Trivial,
                    });
                }
            }

            And | Or | Xor => {
                if !(common.is_bool() || common.is_int()) {
                    return Err(CompileError::new(
                        "'and'/'or'/'xor' require bool or integer operands",
                        st.current_span,
                        "E336",
                    ));
                }
                let ll = coerce(b, l.value, l.ty, common, self.ptr_type, st.current_span)?;
                let rr = coerce(b, r.value, r.ty, common, self.ptr_type, st.current_span)?;
                let v = match op {
                    And => b.ins().band(ll, rr),
                    Or => b.ins().bor(ll, rr),
                    Xor => b.ins().bxor(ll, rr),
                    _ => unreachable!(),
                };
                stack.push(Slot {
                    value: v,
                    ty: common,
                    own: Own::Trivial,
                });
            }

            Lshift | Rshift => {
                // Shift both operands to 64-bit; the result is 64-bit.
                let ll = coerce(b, l.value, l.ty, Ty::I64, self.ptr_type, st.current_span)?;
                let rr = coerce(b, r.value, r.ty, Ty::I64, self.ptr_type, st.current_span)?;
                let v = match op {
                    Lshift => b.ins().ishl(ll, rr),
                    Rshift => {
                        if common.is_signed() {
                            b.ins().sshr(ll, rr)
                        } else {
                            b.ins().ushr(ll, rr)
                        }
                    }
                    _ => unreachable!(),
                };
                stack.push(Slot {
                    value: v,
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
        }
        Ok(())
    }

    /// Inline integer exponentiation loop: `base ^ exp` with exp >= 0.
    fn emit_int_pow(
        &mut self,
        b: &mut FunctionBuilder,
        base: Value,
        exp: Value,
        cl: CLType,
    ) -> CResult<Value> {
        let zero = b.ins().iconst(cl, 0);
        let one = b.ins().iconst(cl, 1);
        let res_v = b.declare_var(cl);
        let i_v = b.declare_var(cl);
        b.def_var(res_v, one);
        b.def_var(i_v, zero);

        let header = b.create_block();
        let body = b.create_block();
        let end = b.create_block();
        b.ins().jump(header, &[]);

        b.switch_to_block(header);
        let i = b.use_var(i_v);
        let more = b.ins().icmp(IntCC::UnsignedLessThan, i, exp);
        b.ins().brif(more, body, &[], end, &[]);

        b.switch_to_block(body);
        let acc = b.use_var(res_v);
        let acc = b.ins().imul(acc, base);
        b.def_var(res_v, acc);
        let i = b.use_var(i_v);
        let next = b.ins().iadd_imm(i, 1);
        b.def_var(i_v, next);
        b.ins().jump(header, &[]);

        b.switch_to_block(end);
        Ok(b.use_var(res_v))
    }
}

/// Byte offset of the `data` field inside the runtime `List` header.
const LIST_DATA_OFFSET: i32 = 24;

/// Byte offset of a union block's active-member tag.
const UNION_TAG_OFFSET: i32 = 0;
/// Byte offset of a union block's inline payload.
const UNION_PAYLOAD_OFFSET: i32 = 8;

/// Collect every distinct string literal appearing in a statement list.
fn collect_strings<'a>(stmts: &'a [Stmt], out: &mut Vec<&'a str>) {
    fn push<'a>(s: &'a str, out: &mut Vec<&'a str>) {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    fn walk_expr<'a>(e: &'a Expr, out: &mut Vec<&'a str>) {
        match e {
            Expr::String { value } => push(value, out),
            Expr::Array(es) | Expr::List(es) => {
                for el in es {
                    walk_expr(el, out);
                }
            }
            Expr::Seq(es) => {
                for (el, _) in es {
                    walk_expr(el, out);
                }
            }
            Expr::Map(pairs) => {
                for (k, v) in pairs {
                    walk_expr(k, out);
                    walk_expr(v, out);
                }
            }
            Expr::Member { base, .. }
            | Expr::Call { target: base }
            | Expr::Unwrap { inner: base }
            | Expr::Typeof { inner: base }
            | Expr::Borrow { inner: base } => walk_expr(base, out),
            Expr::Unary { operand, .. } => walk_expr(operand, out),
            Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            _ => {}
        }
    }
    for s in stmts {
        match &s.kind {
            StmtKind::Expr(e) => walk_expr(e, out),
            StmtKind::VarDecl { value, .. } | StmtKind::Set { value, .. } => {
                if let Some(v) = value {
                    walk_expr(v, out);
                }
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                walk_expr(condition, out);
                collect_strings(then_branch, out);
                collect_strings(else_branch, out);
            }
            StmtKind::For { source, body } => {
                walk_expr(source, out);
                collect_strings(body, out);
            }
            StmtKind::Match {
                value,
                cases,
                else_branch,
            } => {
                walk_expr(value, out);
                for c in cases {
                    match &c.kind {
                        MatchCaseKind::Condition(expr) => walk_expr(expr, out),
                        MatchCaseKind::Type(_) => {}
                    }
                    collect_strings(&c.body, out);
                }
                collect_strings(else_branch, out);
            }
            StmtKind::Return { value: Some(v) } => walk_expr(v, out),
            StmtKind::Return { value: None } => {}
            StmtKind::Defer { body } => collect_strings(body, out),
            StmtKind::Unsafe { body } => collect_strings(body, out),
            StmtKind::Handle { body, fallback } => {
                collect_strings(body, out);
                if let Some(fb) = fallback {
                    walk_expr(fb, out);
                }
            }
            StmtKind::Function(f) => collect_strings(&f.body, out),
            StmtKind::Implement(imp) => {
                for f in &imp.functions {
                    collect_strings(&f.body, out);
                }
            }
            _ => {}
        }
    }
}

/// Fold a running inferred container element type with a new candidate.
fn merge_type(prev: Option<Ty>, next: Ty) -> CResult<Ty> {
    match prev {
        None => Ok(next),
        Some(prev) => common_type(prev, next).ok_or_else(|| {
            CompileError::new(
                format!("incompatible element types {prev:?} and {next:?}"),
                Span::default(),
                "E345",
            )
        }),
    }
}

/// Heuristic: condition `for` leaves a comparison/bool phrase; iterable `for`
/// leaves a container name or other non-bool expression.
fn for_source_is_condition(source: &Expr) -> bool {
    use BinOp::*;
    match source {
        Expr::Bool { .. } => true,
        Expr::Binary { op, .. } => matches!(op, Eq | Ne | Gt | Gte | Lt | Lte | And | Or),
        Expr::Unary { .. } | Expr::ApplyUn(_) => true,
        Expr::ApplyBin(op) => matches!(op, Eq | Ne | Gt | Gte | Lt | Lte | And | Or),
        Expr::Seq(xs) => xs.last().is_some_and(|(e, _)| for_source_is_condition(e)),
        _ => false,
    }
}

/// Smallest unsigned/signed integer type that fits `n` (TYPE_SYSTEM.md).
fn int_literal_ty(n: i128) -> Ty {
    if n >= 0 {
        if n <= u8::MAX as i128 {
            Ty::U8
        } else if n <= u16::MAX as i128 {
            Ty::U16
        } else if n <= u32::MAX as i128 {
            Ty::U32
        } else {
            Ty::U64
        }
    } else if n >= i8::MIN as i128 {
        Ty::I8
    } else if n >= i16::MIN as i128 {
        Ty::I16
    } else if n >= i32::MIN as i128 {
        Ty::I32
    } else {
        Ty::I64
    }
}

fn if_merge_ty_from_then(t: Ty) -> Ty {
    // Merge block params are created before the else branch is compiled. Float
    // results must be wide enough for a wider else branch (see `common_type`).
    if t.is_float() { Ty::F64 } else { t }
}

fn float_literal_ty(n: f64) -> Ty {
    match float_literal_kind(n) {
        FloatLiteralKind::F16 => Ty::F16,
        FloatLiteralKind::F32 => Ty::F32,
        FloatLiteralKind::F64 => Ty::F64,
    }
}

fn float_mod_pow_error(op: BinOp, span: Span) -> CompileError {
    use BinOp::*;
    let (sym, what) = match op {
        Mod => ("%", "remainder"),
        Pow => ("^", "exponentiation"),
        _ => unreachable!("float_mod_pow_error called for non-mod/pow op"),
    };
    CompileError::unsupported(
        format!("'{sym}' is integer {what}, not defined on floats"),
        span,
        "E334",
    )
    .with_note("use integer operands for '%' and '^'")
    .with_help("for float division use '/'; there is no float power operator yet")
}

fn loop_helper_parts(e: &Expr) -> Option<(&str, &str)> {
    match e {
        Expr::Member { base, member } => {
            if let Expr::Variable { name } = base.as_ref() {
                Some((name.as_str(), member.as_str()))
            } else {
                None
            }
        }
        _ => None,
    }
}
