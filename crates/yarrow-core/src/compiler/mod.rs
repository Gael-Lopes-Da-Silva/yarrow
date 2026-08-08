//! A Cranelift JIT compiler for Yarrow programs.
//!
//! The compiler mirrors the parser's operand-stack model: statements are balanced
//! against a compile-time value stack (`Vec<Slot>`), and binary operators the
//! parser left as runtime `ApplyBin`/`ApplyUn`/`StackOp` ops are lowered by
//! popping operands off that same stack.

mod errors;
mod modules;
mod types;

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    AbiParam, Block, BlockArg, FuncRef, GlobalValue, InstBuilder as _, StackSlotData,
    StackSlotKind, TrapCode, Type as CLType, Value, types as irtypes,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module, default_libcall_names};

use crate::parser::ast::{BinOp, Expr, Function, MatchCase, Program, StackOp, Stmt, UnOp};
use crate::parser::literals::{
    decode_float_literal, decode_int_literal, decode_rune_literal, decode_string_literal,
};
use crate::parser::parse;
use crate::tokenizer::Tokenizer;
use crate::tokenizer::token::Location;

use modules::{ModuleLoader, RequiredModule};

pub use errors::CompileError;
use types::CResult;
pub use types::Ty;
use types::{
    StructLayout, coerce, common_type, elem_code, elem_ty, error_return, kind_code, layout,
    resolve, scalar_ty,
};

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
    vars: HashMap<String, (Variable, Ty)>,
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
    /// Values with an active borrow (from `@borrow` or a heap-value `dup`).
    borrowed: std::collections::HashSet<Value>,
    /// Values moved away with `@move`; using or moving them again is an error.
    moved: std::collections::HashSet<Value>,
    /// `defer` bodies, run in reverse order at scope exit.
    deferred: Vec<Vec<Stmt>>,
    /// Struct layout ids whose field descriptors were already registered in
    /// the runtime this function.
    registered_descs: std::collections::HashSet<u32>,
    /// Payload type of this function's `with T or Error` return envelope, if
    /// it returns an error. `None` means the function cannot error.
    error_value: Option<Ty>,
}

struct LoopCtx {
    break_to: Block,
    continue_to: Block,
}

/// JIT compiler that turns a whole `Program` into a single linked module.
pub struct Compiler {
    module: JITModule,
    ptr_type: CLType,
    /// Struct name -> index into `struct_layouts`.
    struct_ids: HashMap<String, u32>,
    /// Layouts for every struct, indexed by `Ty::Struct(id).0`.
    struct_layouts: Vec<StructLayout>,
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
    /// (e.g. `sqrt` -> `std.math.sqrt::sqrt`).
    plain_funcs: HashMap<String, String>,
    /// Module paths already loaded, so a `require` is processed once.
    loaded: std::collections::HashSet<String>,
    /// Error kind name (`CustomError`, `OutOfMemory`, ...) -> program-unique
    /// tag. Tags are interned once per program so `error.X ==` comparisons
    /// and `with T or Error` propagation agree across functions.
    error_ids: HashMap<String, u32>,
    finalized: bool,
}

impl Compiler {
    pub fn new() -> CResult<Self> {
        let mut jb = JITBuilder::new(default_libcall_names())
            .map_err(|e| CompileError::new(e.to_string(), Location::default(), "E350"))?;
        crate::runtime::install_runtime(&mut jb);
        let module = JITModule::new(jb);
        let ptr_type = module.isa().pointer_type();
        Ok(Self {
            module,
            ptr_type,
            struct_ids: HashMap::new(),
            struct_layouts: Vec::new(),
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
            loaded: std::collections::HashSet::new(),
            error_ids: HashMap::new(),
            finalized: false,
        })
    }

    /// Add a directory searched for user modules (`"a.b"` -> `a/b.yar`).
    pub fn add_module_search_path(&mut self, path: impl Into<std::path::PathBuf>) {
        self.loader.add_search_path(path);
    }

    /// Two-pass compilation: first register structs and declare every function
    /// (so whole-program calls resolve), then compile each body. Functions in
    /// modules loaded by `require` are declared and compiled alongside the
    /// main program's.
    pub fn compile(&mut self, program: &Program) -> CResult<()> {
        self.modules.clear();
        self.aliases.clear();
        self.plain_funcs.clear();
        self.loaded.clear();
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
                if let Stmt::Struct(d) = item {
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

        // Pass B: resolve each struct's field types into a layout. Must happen
        // before function signatures are declared, since those may use structs.
        for (_, prog) in &units {
            for item in &prog.items {
                if let Stmt::Struct(d) = item {
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

        // Pass C: declare every function, then register module name bindings.
        for (path, prog) in &units {
            for item in &prog.items {
                match item {
                    Stmt::Function(f) => {
                        self.declare_function(f, &self.item_name(path.as_deref(), &f.name))?;
                    }
                    Stmt::Implement(imp) => {
                        for f in &imp.functions {
                            let name = self
                                .item_name(path.as_deref(), &format!("{}::{}", imp.target, f.name));
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
        self.declare_runtime_imports()?;

        // Pass D: compile every function.
        for (path, prog) in &units {
            for item in &prog.items {
                match item {
                    Stmt::Function(f) => {
                        let name = self.item_name(path.as_deref(), &f.name);
                        self.compile_function(f, &name, path.as_deref(), false)?;
                    }
                    Stmt::Implement(imp) => {
                        for f in &imp.functions {
                            let name = self
                                .item_name(path.as_deref(), &format!("{}::{}", imp.target, f.name));
                            self.compile_function(f, &name, path.as_deref(), true)?;
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
            match item {
                Stmt::Require { path, alias } => self.load_one(path, alias, out)?,
                Stmt::Function(f) => self.load_requires_stmts(&f.body, out)?,
                Stmt::Implement(imp) => {
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
            match s {
                Stmt::Require { path, alias } => self.load_one(path, alias, out)?,
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.load_requires_stmts(then_branch, out)?;
                    self.load_requires_stmts(else_branch, out)?;
                }
                Stmt::While { body, .. } | Stmt::Defer { body } | Stmt::Handle { body } => {
                    self.load_requires_stmts(body, out)?
                }
                Stmt::For { body, .. } => self.load_requires_stmts(body, out)?,
                Stmt::Match {
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

    fn load_one(
        &mut self,
        path: &str,
        alias: &Option<String>,
        out: &mut Vec<RequiredModule>,
    ) -> CResult<()> {
        if self.loaded.contains(path) {
            return Ok(());
        }
        self.loaded.insert(path.to_string());
        let source = self.loader.load(path)?;
        let tokens = Tokenizer::new(source).tokenize()?;
        let sub = parse(tokens)?;
        self.load_requires(&sub, out)?;
        out.push(RequiredModule {
            path: path.to_string(),
            alias: alias.clone(),
            program: sub,
        });
        Ok(())
    }

    /// Expose a loaded module's functions under their alias or plain names.
    ///
    /// With an alias (`"std.io" require io`), `io.func` resolves to the
    /// module's `func`. Without an alias, every function is callable by its
    /// plain name (`"std.math.sqrt" require` makes `sqrt call` work).
    fn register_module_bindings(&mut self) -> CResult<()> {
        for m in &self.modules {
            if let Some(alias) = &m.alias {
                if let Some(existing) = self.aliases.get(alias) {
                    if existing != &m.path {
                        return Err(CompileError::new(
                            format!("module alias '{alias}' already bound to '{existing}'"),
                            Location::default(),
                            "E380",
                        ));
                    }
                } else {
                    self.aliases.insert(alias.clone(), m.path.clone());
                }
            } else {
                for item in &m.program.items {
                    if let Stmt::Function(f) = item {
                        if self.func_ids.contains_key(&f.name) {
                            return Err(CompileError::new(
                                format!(
                                    "function '{}' from module '{}' conflicts with a function of the same name",
                                    f.name, m.path
                                ),
                                Location::default(),
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
                                Location::default(),
                                "E380",
                            ));
                        }
                        self.plain_funcs.insert(f.name.clone(), fq);
                    }
                }
            }
        }
        Ok(())
    }

    /// Declare and define a read-only data object per unique string literal.
    fn declare_string_data(&mut self, program: &Program) -> CResult<()> {
        let mut seen: Vec<&str> = Vec::new();
        collect_strings(&program.items, &mut seen);
        for (i, s) in seen.into_iter().enumerate() {
            let name = format!("yarrow.str.{i}");
            let id = self
                .module
                .declare_data(&name, Linkage::Local, false, false)?;
            let bytes = decode_string_literal(s)
                .map_err(|m| CompileError::new(m, Location::default(), "E363"))?;
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

    /// Import every host runtime function so JIT code can `call` it.
    fn declare_runtime_imports(&mut self) -> CResult<()> {
        for (name, params, returns) in RUNTIME_SIGS {
            let mut sig = self.module.make_signature();
            for &p in *params {
                sig.params.push(AbiParam::new(p));
            }
            for &r in *returns {
                sig.returns.push(AbiParam::new(r));
            }
            let id = self.module.declare_function(name, Linkage::Import, &sig)?;
            self.runtime_ids.insert(name.to_string(), id);
        }
        Ok(())
    }

    /// Run the compiled `main` function and return its single integer result.
    pub fn run_main(&mut self) -> CResult<i64> {
        self.finalize()?;
        let id = *self.func_ids.get("main").ok_or_else(|| {
            CompileError::new(
                "program has no 'main' function",
                Location::default(),
                "E360",
            )
        })?;
        let sig = self.sigs.get("main").cloned().ok_or_else(|| {
            CompileError::new("missing signature for 'main'", Location::default(), "E360")
        })?;
        if sig.returns.len() != 1 {
            return Err(CompileError::new(
                "'main' must return exactly one value to be runnable",
                Location::default(),
                "E360",
            ));
        }
        let ret = sig.returns[0].value_type;
        let ptr = self.module.get_finalized_function(id);
        unsafe {
            if ret == irtypes::I64 {
                let f: extern "C" fn() -> i64 = std::mem::transmute(ptr);
                Ok(f())
            } else if ret == irtypes::I32 {
                let f: extern "C" fn() -> i32 = std::mem::transmute(ptr);
                Ok(f() as i64)
            } else if ret == irtypes::I8 {
                let f: extern "C" fn() -> i8 = std::mem::transmute(ptr);
                Ok(f() as i64)
            } else {
                Err(CompileError::new(
                    "unsupported 'main' return type for run_main",
                    Location::default(),
                    "E360",
                ))
            }
        }
    }

    /// Address of a compiled function after `compile`.
    pub fn function_ptr(&mut self, name: &str) -> CResult<usize> {
        self.finalize()?;
        let id = *self.func_ids.get(name).ok_or_else(|| {
            CompileError::new(
                format!("unknown function '{name}'"),
                Location::default(),
                "E361",
            )
        })?;
        Ok(self.module.get_finalized_function(id) as usize)
    }

    fn finalize(&mut self) -> CResult<()> {
        if !self.finalized {
            self.module.finalize_definitions()?;
            self.finalized = true;
        }
        Ok(())
    }

    fn resolve_ty(&self, t: &crate::parser::ast::Type) -> CResult<Ty> {
        resolve(t, &|n| self.struct_ids.get(n).copied())
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
        match base {
            Expr::Variable { name } => match st.vars.get(name) {
                Some((_, Ty::Struct(id))) => Ok(*id),
                Some((_, other)) => Err(CompileError::new(
                    format!("'{name}' is a {other:?}, not a struct value"),
                    Location::default(),
                    "E340",
                )),
                None => Err(CompileError::new(
                    format!("unknown variable '{name}'"),
                    Location::default(),
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
                            Location::default(),
                            "E340",
                        )
                    })?;
                match field.ty {
                    Ty::Struct(id) => Ok(id),
                    _ => Err(CompileError::new(
                        format!("field '{member}' is not a struct value"),
                        Location::default(),
                        "E340",
                    )),
                }
            }
            _ => Err(CompileError::new(
                "expected a struct value before '.'",
                Location::default(),
                "E340",
            )),
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
                    Location::default(),
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
        let out = self.rt_call(b, st, "yarrow_alloc", vec![size])?;
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
                        Location::default(),
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
                        Location::default(),
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
            let slot = self.pop_slot(stack, "struct field value")?;
            let val = coerce(b, slot.value, slot.ty, field.ty, self.ptr_type)?;
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
        let out = self.rt_call(b, st, "yarrow_alloc", vec![size])?;
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
            let slot = self.pop_slot(stack, "array element")?;
            let val = coerce(b, slot.value, slot.ty, elem, self.ptr_type)?;
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
                Location::default(),
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
        let mut return_tys = Vec::with_capacity(f.returns.len());
        let mut sig = self.module.make_signature();
        for p in &f.params {
            let ty = self.resolve_ty(p)?;
            sig.params.push(AbiParam::new(ty.clty(self.ptr_type)));
            param_tys.push(ty);
        }
        for r in &f.returns {
            let ty = self.resolve_ty(r)?;
            sig.returns.push(AbiParam::new(ty.clty(self.ptr_type)));
            return_tys.push(ty);
        }
        // A `with T or Error` function returns an envelope `(i64 env, i64
        // payload)`: env is 0 on success or the error tag on failure, and
        // payload carries the success value (or 0).
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
        let sig = self.sigs.get(name).cloned().unwrap();
        let id = *self.func_ids.get(name).unwrap();

        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let returns = f
            .returns
            .iter()
            .map(|r| self.resolve_ty(r))
            .collect::<CResult<Vec<_>>>()?;
        let error_value = error_return(&returns)?;
        let mut st = FnState {
            vars: HashMap::new(),
            loops: Vec::new(),
            returns,
            error_value,
            frefs: HashMap::new(),
            rt: HashMap::new(),
            module: module.map(str::to_string),
            owns: std::collections::HashSet::new(),
            borrowed: std::collections::HashSet::new(),
            moved: std::collections::HashSet::new(),
            deferred: Vec::new(),
            registered_descs: std::collections::HashSet::new(),
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

        let mut fbctx = FunctionBuilderContext::new();
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fbctx);

        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);

        let params_ty: Vec<Ty> = f
            .params
            .iter()
            .map(|p| self.resolve_ty(p))
            .collect::<CResult<_>>()?;
        let param_vals: Vec<Value> = b.block_params(entry).to_vec();
        let mut stack: Vec<Slot> = Vec::new();
        for (i, t) in params_ty.iter().enumerate() {
            // Heap-typed params are borrowed from the caller; the callee must
            // not free them.
            let own = if self.is_heap(*t) {
                Own::Borrow
            } else {
                Own::Trivial
            };
            stack.push(Slot {
                value: param_vals[i],
                ty: *t,
                own,
            });
        }

        // In a method body, the receiver is param 0. Bind `self` to it so a
        // `self const reference<Point>` declaration resolves without relying
        // on stack position.
        if is_method && let Some((t, v)) = params_ty.first().zip(param_vals.first()) {
            let var = b.declare_var(t.clty(self.ptr_type));
            b.def_var(var, *v);
            st.vars.insert("self".to_string(), (var, *t));
        }

        self.compile_body(&mut b, &mut st, &mut stack, &f.body)?;

        // Implicit termination for a function falling off the end.
        if st.error_value.is_some() {
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
        if std::env::var("YARROW_DBG_IR").is_ok() {
            eprintln!("IR for {name}:\n{}", ctx.func.display());
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
            self.compile_stmt(b, st, stack, s)?;
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
        match s {
            Stmt::Expr(e) => self.compile_expr(b, st, stack, e)?,

            Stmt::VarDecl {
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
                        if let Some(Expr::Map(pairs)) = elems.last() {
                            for el in &elems[..elems.len() - 1] {
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
                            for el in elems {
                                self.compile_expr(b, st, stack, el)?;
                            }
                            let slot = self.pop_slot(stack, "value")?;
                            (slot, slot.value, slot.ty)
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
                        let slot = self.pop_slot(stack, "value")?;
                        (slot, slot.value, slot.ty)
                    }
                    None => {
                        let slot = self.pop_slot(stack, "value")?;
                        (slot, slot.value, slot.ty)
                    }
                };
                let val = coerce(b, val, val_ty, t, self.ptr_type)?;
                self.claim(st, val, t);
                let var = b.declare_var(t.clty(self.ptr_type));
                b.def_var(var, val);
                let _ = mutability;
                st.vars.insert(name.clone(), (var, t));
            }

            Stmt::Set { target, value } => match target {
                Expr::Variable { name } => {
                    let (var, t) = st.vars.get(name).cloned().ok_or_else(|| {
                        CompileError::new(
                            format!("unknown variable '{name}'"),
                            Location::default(),
                            "E320",
                        )
                    })?;
                    // A struct literal set re-initializes the existing
                    // storage in place, so the old value must NOT be freed
                    // first (the pointer is reused).
                    let trailing_map = match value {
                        Some(Expr::Map(_)) => true,
                        Some(Expr::Seq(elems)) => matches!(elems.last(), Some(Expr::Map(_))),
                        _ => false,
                    };
                    let reuses_ptr = trailing_map && matches!(t, Ty::Struct(_));
                    // Drop the value the variable currently owns (the runtime
                    // guards against double frees).
                    if self.is_heap(t) && !reuses_ptr {
                        let old = Slot {
                            value: b.use_var(var),
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
                            if let Some(Expr::Map(pairs)) = elems.last() {
                                for el in &elems[..elems.len() - 1] {
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
                                for el in elems {
                                    self.compile_expr(b, st, stack, el)?;
                                }
                                let slot = self.pop_slot(stack, "value")?;
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
                            let slot = self.pop_slot(stack, "value")?;
                            (slot, slot.value, slot.ty)
                        }
                        None => {
                            let slot = self.pop_slot(stack, "value")?;
                            (slot, slot.value, slot.ty)
                        }
                    };
                    let val = coerce(b, val, val_ty, t, self.ptr_type)?;
                    self.claim(st, val, t);
                    b.def_var(var, val);
                }
                Expr::Member { base, member } => {
                    let sid = self.base_struct(st, base)?;
                    let field = self.find_field(sid, member)?.clone();
                    self.compile_expr(b, st, stack, base)?;
                    let ptr = self.pop_slot(stack, "field set target")?;
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
                            let slot = self.pop_slot(stack, "value")?;
                            (slot.value, slot.ty)
                        }
                        None => {
                            let slot = self.pop_slot(stack, "value")?;
                            (slot.value, slot.ty)
                        }
                    };
                    let val = coerce(b, val, val_ty, field.ty, self.ptr_type)?;
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
                        Location::default(),
                        "E301",
                    ));
                }
            },

            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => self.emit_if(b, st, stack, condition, then_branch, else_branch)?,

            Stmt::While { condition, body } => self.emit_while(b, st, stack, condition, body)?,

            Stmt::Match {
                value,
                cases,
                else_branch,
            } => self.emit_match(b, st, stack, value, cases, else_branch)?,

            Stmt::For {
                iterable,
                var,
                body,
            } => self.emit_for(b, st, stack, iterable, var, body)?,

            Stmt::Return { .. } => self.emit_return(b, st, stack)?,

            Stmt::Break => {
                let loop_ctx = st.loops.last().ok_or_else(|| {
                    CompileError::new("'break' outside of a loop", Location::default(), "E321")
                })?;
                b.ins().jump(loop_ctx.break_to, &[]);
                self.dead_block(b);
            }

            Stmt::Continue => {
                let loop_ctx = st.loops.last().ok_or_else(|| {
                    CompileError::new("'continue' outside of a loop", Location::default(), "E322")
                })?;
                b.ins().jump(loop_ctx.continue_to, &[]);
                self.dead_block(b);
            }

            Stmt::Function(_)
            | Stmt::Struct(_)
            | Stmt::Implement(_)
            | Stmt::Enum(_)
            | Stmt::Union(_)
            | Stmt::Require { .. } => {
                // Only meaningful at program level; no-op inside a body.
            }

            Stmt::Defer { body } => {
                // Schedule the body to run in reverse order at scope exit,
                // so a `myRegion @free_region call` runs after the region's
                // values have been dropped.
                st.deferred.push(body.clone());
            }

            Stmt::Handle { body } => self.emit_handle(b, st, stack, body)?,
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
            // `with T or Error`: the body leaves either the success value or
            // an error value on the stack.
            let zero = b.ins().iconst(irtypes::I64, 0);
            if matches!(stack.last(), Some(s) if s.ty == Ty::Error) {
                let err = stack.pop().unwrap();
                return Ok(vec![err.value, zero]);
            }
            let payload = if payload_ty == Ty::Void {
                zero
            } else {
                let slot = self.pop_slot(stack, "return value")?;
                // The callee owns heap values it returns; the caller claims
                // them from the envelope, so the callee must not free them.
                if self.is_heap(slot.ty) {
                    st.moved.insert(slot.value);
                }
                coerce(b, slot.value, slot.ty, Ty::I64, self.ptr_type)?
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
                Location::default(),
                "E323",
            ));
        }
        let tail = stack.split_off(stack.len() - n);
        let mut out = Vec::with_capacity(n);
        for (slot, want) in tail.iter().zip(&st.returns) {
            // Heap-typed return values transfer ownership to the caller, so
            // the callee must not free them at scope exit (this also covers a
            // `myStr return` that borrows the value out of a variable).
            if self.is_heap(slot.ty) {
                st.moved.insert(slot.value);
            }
            out.push(coerce(b, slot.value, slot.ty, *want, self.ptr_type)?);
        }
        Ok(out)
    }

    /// `value unwrap`: if the top of the stack is an error envelope from a
    /// `with T or Error` call, keep the success payload or propagate the error
    /// (return it when this function itself returns an error, otherwise trap).
    /// Applied to anything that cannot fail, `unwrap` is an identity.
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

        // Error: propagate as this function's error return, or trap. Filling
        // this block first lets the success block below remain the live
        // continuation for the rest of the function.
        b.switch_to_block(err_blk);
        if st.error_value.is_some() {
            stack.push(Slot {
                value: err_env_param,
                ty: Ty::Error,
                own: Own::Trivial,
            });
            self.emit_return(b, st, stack)?;
        } else {
            b.ins().trap(TrapCode::unwrap_user(1));
        }
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
        // Drop the error slot if the body did not consume it.
        if stack.len() > err_idx && stack[err_idx].ty == Ty::Error && stack[err_idx].value == err {
            stack.remove(err_idx);
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
                Location::default(),
                "E328",
            ));
        }
        let mut args: Vec<BlockArg> = Vec::with_capacity(results.len());
        for (s, want) in results.iter().zip(&success_tys) {
            let v = coerce(b, s.value, s.ty, *want, self.ptr_type)?;
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

    fn pop_slot(&self, stack: &mut Vec<Slot>, what: &str) -> CResult<Slot> {
        stack.pop().ok_or_else(|| {
            CompileError::new(
                format!("missing operand for {what}"),
                Location::default(),
                "E362",
            )
        })
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
                Location::default(),
                "E370",
            )
        })?;
        let inst = b.ins().call(fref, &args);
        Ok(b.inst_results(inst).to_vec())
    }

    /// Coerce `slot` to the 64-bit type runtime functions expect (pointers and
    /// ≤ 8-byte scalars round-trip through the low bytes).
    fn rt_arg(&self, b: &mut FunctionBuilder, slot: Slot) -> CResult<Value> {
        if slot.ty.clty(self.ptr_type) == self.ptr_type {
            return Ok(slot.value);
        }
        coerce(b, slot.value, slot.ty, Ty::I64, self.ptr_type)
    }

    /// Whether `ty` owns heap storage (strings, lists, hashmaps, structs,
    /// arrays). Struct and array instances are heap-allocated by the compiler
    /// so their addresses stay valid across calls; dropping them emits
    /// `yarrow_free_value`.
    fn is_heap(&self, ty: Ty) -> bool {
        matches!(
            ty,
            Ty::String | Ty::List { .. } | Ty::Hashmap { .. } | Ty::Struct(_) | Ty::Array { .. }
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
        let kind = b.ins().iconst(irtypes::I64, kind_code(slot.ty) as i64);
        self.rt_call(b, st, "yarrow_free_value", vec![slot.value, kind])?;
        st.owns.remove(&slot.value);
        Ok(())
    }

    /// Consume a slot from the stack: release its borrow (if any) and drop it
    /// if it owns storage. Used wherever a popped value does not flow through.
    fn consume(&mut self, b: &mut FunctionBuilder, st: &mut FnState, slot: Slot) -> CResult<()> {
        if slot.own == Own::Borrow {
            st.borrowed.remove(&slot.value);
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
                Location::default(),
                "E371",
            )
        })?;
        let addr = b.ins().global_value(self.ptr_type, gv);
        let idv = b.ins().iconst(irtypes::I64, id as i64);
        let count = b
            .ins()
            .iconst(irtypes::I64, self.struct_layout(id).fields.len() as i64);
        self.rt_call(
            b,
            st,
            "yarrow_register_struct_descs",
            vec![idv, addr, count],
        )?;
        Ok(())
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
            self.emit_drop(b, st, slot)?;
        }
        let mut var_slots: Vec<Slot> = Vec::new();
        for (var, ty) in st.vars.values() {
            if self.is_heap(*ty) {
                let v = b.use_var(*var);
                var_slots.push(Slot {
                    value: v,
                    ty: *ty,
                    own: Own::Owned,
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
            return Err(CompileError::new(
                "condition must evaluate to a single value",
                Location::default(),
                "E324",
            ));
        }
        let slot = stack.pop().unwrap();
        if slot.ty.is_int() || slot.ty.is_bool() {
            Ok(slot.value)
        } else {
            Err(CompileError::new(
                "condition must be a boolean or integer",
                Location::default(),
                "E324",
            ))
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
                Location::default(),
                "E324",
            )
        })?;
        if slot.ty.is_int() || slot.ty.is_bool() {
            Ok(slot.value)
        } else {
            Err(CompileError::new(
                "condition must be a boolean or integer",
                Location::default(),
                "E324",
            ))
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

        // Compile the `then` branch and immediately jump out of its block so it
        // is terminated before we switch away.
        b.switch_to_block(then_blk);
        *stack = pre.clone();
        self.compile_body(b, st, stack, then_branch)?;
        let then_stack = stack.clone();
        let then_extra = &then_stack[pre.len()..];

        // Merge params must exist before any jump targets `merge`.
        let mut params: Vec<Value> = Vec::with_capacity(then_extra.len());
        for s in then_extra {
            params.push(b.append_block_param(merge, s.ty.clty(self.ptr_type)));
        }
        let tv: Vec<BlockArg> = then_extra
            .iter()
            .map(|s| BlockArg::Value(s.value))
            .collect();
        b.ins().jump(merge, &tv);

        b.switch_to_block(else_blk);
        *stack = pre.clone();
        self.compile_body(b, st, stack, else_branch)?;
        let else_stack = stack.clone();
        if else_stack.len() != then_stack.len() {
            return Err(CompileError::new(
                "if/else branches must leave the same number of values",
                Location::default(),
                "E328",
            ));
        }
        // Coerce the else-branch values to the then-branch's merge types so
        // branches that differ only by width (I32 vs I64) still merge.
        let mut ev: Vec<BlockArg> = Vec::with_capacity(then_extra.len());
        for (s, want) in else_stack[pre.len()..].iter().zip(then_extra) {
            let v = coerce(b, s.value, s.ty, want.ty, self.ptr_type)?;
            ev.push(BlockArg::Value(v));
        }
        b.ins().jump(merge, &ev);

        b.switch_to_block(merge);
        *stack = pre;
        for (i, s) in then_extra.iter().enumerate() {
            stack.push(Slot {
                value: params[i],
                ty: s.ty,
                own: Own::Trivial,
            });
        }
        Ok(())
    }

    fn emit_while(
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
        });
        self.compile_body(b, st, stack, body)?;
        st.loops.pop();
        if stack.len() != pre.len() {
            return Err(CompileError::new(
                "while body must leave the stack balanced",
                Location::default(),
                "E325",
            ));
        }
        b.ins().jump(header, &[]);

        b.switch_to_block(end);
        *stack = pre;
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
            Some(self.pop_slot(stack, "match value")?)
        };
        let mut sub_stack = pre.clone();
        if let Some(s) = subject {
            sub_stack.push(s);
        }

        let merge = b.create_block();
        let body_blks: Vec<Block> = (0..cases.len()).map(|_| b.create_block()).collect();
        let cond_blks: Vec<Block> = (0..cases.len().saturating_sub(1))
            .map(|_| b.create_block())
            .collect();
        let else_blk = b.create_block();

        let mut results_ty: Option<Vec<Ty>> = None;

        for (i, case) in cases.iter().enumerate() {
            if i > 0 {
                b.switch_to_block(cond_blks[i - 1]);
            }
            *stack = sub_stack.clone();
            let cond = self.eval_match_cond(b, st, stack, &case.condition)?;
            // The condition may keep the subject on the stack (`dup X ==`) or
            // consume stack values (`error.X ==` compares against the subject),
            // so it may leave at most the pre-condition stack height.
            if stack.len() > sub_stack.len() {
                return Err(CompileError::new(
                    "a 'match' case condition must leave the stack balanced",
                    Location::default(),
                    "E343",
                ));
            }
            let false_target = if i + 1 < cases.len() {
                cond_blks[i]
            } else {
                else_blk
            };
            b.ins().brif(cond, body_blks[i], &[], false_target, &[]);

            b.switch_to_block(body_blks[i]);
            *stack = sub_stack.clone();
            self.compile_body(b, st, stack, &case.body)?;
            let results = stack.split_off(sub_stack.len());
            self.match_merge(b, merge, &mut results_ty, results)?;
        }

        b.switch_to_block(else_blk);
        *stack = sub_stack.clone();
        self.compile_body(b, st, stack, else_branch)?;
        let results = stack.split_off(sub_stack.len());
        self.match_merge(b, merge, &mut results_ty, results)?;

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
                        Location::default(),
                        "E343",
                    ));
                }
                prev.clone()
            }
        };
        let mut args: Vec<BlockArg> = Vec::with_capacity(results.len());
        for (s, t) in results.iter().zip(&want) {
            let v = coerce(b, s.value, s.ty, *t, self.ptr_type)?;
            args.push(BlockArg::Value(v));
        }
        b.ins().jump(merge, &args);
        Ok(())
    }

    /// `iterable var for <body> end` over a fixed-size array. The iterable is
    /// evaluated once, then each element is loaded into `var` in order.
    fn emit_for(
        &mut self,
        b: &mut FunctionBuilder,
        st: &mut FnState,
        stack: &mut Vec<Slot>,
        iterable: &Expr,
        var: &str,
        body: &[Stmt],
    ) -> CResult<()> {
        let pre = stack.clone();
        self.compile_expr(b, st, stack, iterable)?;
        let arr = self.pop_slot(stack, "'for' iterable")?;
        let (elem, count) = match arr.ty {
            Ty::Array { elem, count } => (scalar_ty(elem), count),
            other => {
                return Err(CompileError::new(
                    format!("'for' requires an array iterable, got {other:?}"),
                    Location::default(),
                    "E344",
                ));
            }
        };
        let elem_size = elem.elem_size() as i64;

        let zero = b.ins().iconst(irtypes::I64, 0);
        let total = b.ins().iconst(irtypes::I64, count as i64);
        let idx_v = b.declare_var(irtypes::I64);
        b.def_var(idx_v, zero);
        let ptr_v = b.declare_var(self.ptr_type);
        b.def_var(ptr_v, arr.value);
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
        let val = b.ins().load(
            elem.clty(self.ptr_type),
            cranelift_codegen::ir::MemFlagsData::trusted(),
            addr,
            0,
        );
        let loop_var = b.declare_var(elem.clty(self.ptr_type));
        b.def_var(loop_var, val);
        let saved = st.vars.insert(var.to_string(), (loop_var, elem));
        st.loops.push(LoopCtx {
            break_to: end,
            continue_to: step,
        });
        self.compile_body(b, st, stack, body)?;
        st.loops.pop();
        if let Some(saved) = saved {
            st.vars.insert(var.to_string(), saved);
        } else {
            st.vars.remove(var);
        }
        if stack.len() != pre.len() {
            return Err(CompileError::new(
                "'for' body must leave the stack balanced",
                Location::default(),
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
                    .map_err(|m| CompileError::new(m, Location::default(), "E363"))?;
                // The front-end validates literals; lowering is currently 64-bit.
                if n < i64::MIN as i128 || n > i64::MAX as i128 {
                    return Err(CompileError::new(
                        format!(
                            "integer literal '{value}' is out of range for 64-bit code generation"
                        ),
                        Location::default(),
                        "E364",
                    ));
                }
                let v = b.ins().iconst(irtypes::I64, n as i64);
                stack.push(Slot {
                    value: v,
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
            Expr::Float { value } => {
                let n = decode_float_literal(value)
                    .map_err(|m| CompileError::new(m, Location::default(), "E363"))?;
                let v = b.ins().f64const(n);
                stack.push(Slot {
                    value: v,
                    ty: Ty::F64,
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
                    .map_err(|m| CompileError::new(m, Location::default(), "E363"))?;
                let v = b.ins().iconst(irtypes::I32, cp as i64);
                stack.push(Slot {
                    value: v,
                    ty: Ty::Rune,
                    own: Own::Trivial,
                });
            }
            Expr::String { value } => self.emit_string(b, st, stack, value)?,
            Expr::Variable { name } => {
                let (var, t) = st.vars.get(name).cloned().ok_or_else(|| {
                    CompileError::new(
                        format!("unknown variable '{name}'"),
                        Location::default(),
                        "E320",
                    )
                })?;
                let v = b.use_var(var);
                stack.push(Slot {
                    value: v,
                    ty: t,
                    own: Own::Trivial,
                });
            }
            Expr::Member { base, member } => {
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
                let sid = self.base_struct(st, base)?;
                let field = self.find_field(sid, member)?;
                let fty = field.ty;
                self.compile_expr(b, st, stack, base)?;
                let ptr = self.pop_slot(stack, "field access")?;
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
            Expr::Builtin { name } => self.emit_builtin(b, st, stack, name)?,
            Expr::Unwrap { inner } => {
                self.compile_expr(b, st, stack, inner)?;
                self.emit_unwrap(b, st, stack)?;
            }
            Expr::Binary { op, left, right } => {
                self.compile_expr(b, st, stack, left)?;
                self.compile_expr(b, st, stack, right)?;
                let r = self.pop_slot(stack, "operator")?;
                let l = self.pop_slot(stack, "operator")?;
                self.emit_bin(b, st, stack, *op, l, r)?;
            }
            Expr::Unary { op, operand } => {
                self.compile_expr(b, st, stack, operand)?;
                let slot = self.pop_slot(stack, "unary operator")?;
                self.emit_not(b, stack, *op, slot)?;
            }
            Expr::Call { target } => self.emit_call(b, st, stack, target)?,
            Expr::ApplyBin(op) => {
                let r = self.pop_slot(stack, "operator")?;
                let l = self.pop_slot(stack, "operator")?;
                self.emit_bin(b, st, stack, *op, l, r)?;
            }
            Expr::ApplyUn(op) => {
                let slot = self.pop_slot(stack, "unary operator")?;
                self.emit_not(b, stack, *op, slot)?;
            }
            Expr::StackOp(op) => self.emit_stackop(b, st, stack, *op)?,
            Expr::Seq(elems) => {
                for el in elems {
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
                    let slot = self.pop_slot(stack, "array element")?;
                    elem_ty = Some(match elem_ty {
                        None => slot.ty,
                        Some(t) => common_type(t, slot.ty).ok_or_else(|| {
                            CompileError::new(
                                format!(
                                    "array literal elements have incompatible types {t:?} and {:?}",
                                    slot.ty
                                ),
                                Location::default(),
                                "E345",
                            )
                        })?,
                    });
                    vals.push(slot);
                }
                let elem_ty = elem_ty.ok_or_else(|| {
                    CompileError::new(
                        "empty array literal needs a type annotation",
                        Location::default(),
                        "E345",
                    )
                })?;
                let ptr = self.alloc_array(b, st, elem_ty, elems.len() as u32)?;
                let elem_size = elem_ty.elem_size() as i32;
                for (i, slot) in vals.iter().enumerate() {
                    let val = coerce(b, slot.value, slot.ty, elem_ty, self.ptr_type)?;
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
                        Location::default(),
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
        }
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
                Location::default(),
                "E371",
            )
        })?;
        let addr = b.ins().global_value(self.ptr_type, gv);
        // The lexeme carries the surrounding quotes, so the byte length must
        // come from the decoded literal (matching the data section contents).
        let len = decode_string_literal(value)
            .map_err(|m| CompileError::new(m, Location::default(), "E363"))?
            .len() as i64;
        let len = b.ins().iconst(irtypes::I64, len);
        let out = self.rt_call(b, st, "yarrow_str_new", vec![addr, len])?;
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
    ) -> CResult<(Value, u32)> {
        let elem = if let Some(declared) = declared {
            declared
        } else {
            let mut t: Option<Ty> = None;
            for el in elems {
                self.compile_expr(b, st, stack, el)?;
                let slot = self.pop_slot(stack, "list element")?;
                t = Some(match t {
                    None => slot.ty,
                    Some(prev) => common_type(prev, slot.ty).ok_or_else(|| {
                        CompileError::new(
                            format!(
                                "list literal elements have incompatible types {prev:?} and {:?}",
                                slot.ty
                            ),
                            Location::default(),
                            "E345",
                        )
                    })?,
                });
            }
            t.ok_or_else(|| {
                CompileError::new(
                    "empty list literal needs a type annotation",
                    Location::default(),
                    "E345",
                )
            })?
        };
        let code = elem_code(elem).ok_or_else(|| {
            CompileError::new(
                format!("list element type {elem:?} is not supported"),
                Location::default(),
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
        let out = self.rt_call(b, st, "yarrow_list_new", vec![size])?;
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
            let slot = self.pop_slot(stack, "list element")?;
            let val = coerce(b, slot.value, slot.ty, elem, self.ptr_type)?;
            let arg = self.rt_arg(
                b,
                Slot {
                    value: val,
                    ty: elem,
                    own: Own::Trivial,
                },
            )?;
            self.rt_call(b, st, "yarrow_list_push", vec![handle, arg])?;
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
    ) -> CResult<(Value, u32, u32)> {
        let (kt, vt) = match declared {
            Some(Ty::Hashmap { key, value }) => (elem_ty(key), elem_ty(value)),
            Some(_) => {
                return Err(CompileError::new(
                    "map literal requires a hashmap type",
                    Location::default(),
                    "E306",
                ));
            }
            None => {
                let mut kt: Option<Ty> = None;
                let mut vt: Option<Ty> = None;
                for (k, v) in pairs {
                    self.compile_expr(b, st, stack, k)?;
                    let ks = self.pop_slot(stack, "map key")?;
                    kt = Some(merge_type(kt, ks.ty)?);
                    self.compile_expr(b, st, stack, v)?;
                    let vs = self.pop_slot(stack, "map value")?;
                    vt = Some(merge_type(vt, vs.ty)?);
                }
                let kt = kt.ok_or_else(|| {
                    CompileError::new(
                        "empty map literal needs a type annotation",
                        Location::default(),
                        "E306",
                    )
                })?;
                let vt = vt.ok_or_else(|| {
                    CompileError::new(
                        "empty map literal needs a type annotation",
                        Location::default(),
                        "E306",
                    )
                })?;
                (kt, vt)
            }
        };
        let kcode = elem_code(kt).ok_or_else(|| {
            CompileError::new(
                format!("map key type {kt:?} is not supported"),
                Location::default(),
                "E306",
            )
        })?;
        let vcode = elem_code(vt).ok_or_else(|| {
            CompileError::new(
                format!("map value type {vt:?} is not supported"),
                Location::default(),
                "E306",
            )
        })?;
        let keys_string = b
            .ins()
            .iconst(irtypes::I64, if kt == Ty::String { 1 } else { 0 });
        let out = self.rt_call(b, st, "yarrow_map_new", vec![keys_string])?;
        let handle = out[0];
        for (k, v) in pairs {
            self.compile_expr(b, st, stack, k)?;
            let ks = self.pop_slot(stack, "map key")?;
            let karg = coerce(b, ks.value, ks.ty, kt, self.ptr_type)?;
            let karg = self.rt_arg(
                b,
                Slot {
                    value: karg,
                    ty: kt,
                    own: Own::Trivial,
                },
            )?;
            self.compile_expr(b, st, stack, v)?;
            let vs = self.pop_slot(stack, "map value")?;
            let varg = coerce(b, vs.value, vs.ty, vt, self.ptr_type)?;
            let varg = self.rt_arg(
                b,
                Slot {
                    value: varg,
                    ty: vt,
                    own: Own::Trivial,
                },
            )?;
            self.rt_call(b, st, "yarrow_map_insert", vec![handle, karg, varg])?;
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
    ) -> CResult<()> {
        match name {
            "borrow" => {
                let s = self.pop_slot(stack, name)?;
                if !s.ty.is_pointer() {
                    return Err(CompileError::new(
                        format!(
                            "'{name}' requires a reference, struct, array, string or container"
                        ),
                        Location::default(),
                        "E341",
                    ));
                }
                // A borrow is a non-owning reference: the stack no longer owns
                // the value (its original owner still does), so any drop here
                // is skipped via `moved`.
                if s.own.is_owned() {
                    st.moved.insert(s.value);
                }
                st.borrowed.insert(s.value);
                stack.push(Slot {
                    value: s.value,
                    ty: s.ty,
                    own: Own::Borrow,
                });
            }

            "move" => {
                // `source destination @move` rebinds `destination` to
                // `source`'s storage and marks the source as moved.
                let dest = self.pop_slot(stack, name)?;
                let src = self.pop_slot(stack, name)?;
                if !src.ty.is_pointer() || !dest.ty.is_pointer() {
                    return Err(CompileError::new(
                        "'@move' requires a reference, struct, array, string or container"
                            .to_string(),
                        Location::default(),
                        "E341",
                    ));
                }
                st.moved.insert(src.value);
                let mut target = None;
                for (var, ty) in st.vars.values() {
                    if b.use_var(*var) == dest.value {
                        target = Some((*var, *ty));
                        break;
                    }
                }
                match target {
                    Some((var, ty)) => {
                        self.claim(st, src.value, ty);
                        b.def_var(var, src.value);
                    }
                    None => {
                        // No matching variable (e.g. a fresh owned value on the
                        // stack): leave the moved value on the stack instead.
                        stack.push(Slot {
                            value: src.value,
                            ty: src.ty,
                            own: Own::Borrow,
                        });
                    }
                }
            }

            "make_region" => {
                let out = self.rt_call(b, st, "yarrow_region_new", vec![])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
            "free_region" => {
                let region = self.pop_slot(stack, "'@free_region'")?;
                self.rt_call(b, st, "yarrow_region_free", vec![region.value])?;
            }
            "put_region" => {
                let region = self.pop_slot(stack, "'@put_region'")?;
                let value = self.pop_slot(stack, "'@put_region'")?;
                if !value.ty.is_pointer() {
                    return Err(CompileError::new(
                        format!(
                            "'@put_region' requires a reference, struct, array, string or container, got {:?}",
                            value.ty
                        ),
                        Location::default(),
                        "E372",
                    ));
                }
                let kind = b.ins().iconst(irtypes::I64, kind_code(value.ty) as i64);
                self.rt_call(
                    b,
                    st,
                    "yarrow_region_register",
                    vec![value.value, kind, region.value],
                )?;
                // The region now owns the value; the stack must not free it.
                st.moved.insert(value.value);
                stack.push(Slot {
                    value: value.value,
                    ty: value.ty,
                    own: Own::Borrow,
                });
            }

            "string_join" => {
                let sep = self.pop_slot(stack, "'@string_join'")?;
                let right = self.pop_slot(stack, "'@string_join'")?;
                let left = self.pop_slot(stack, "'@string_join'")?;
                for s in [&left, &right, &sep] {
                    if s.ty != Ty::String {
                        return Err(CompileError::new(
                            format!("'@string_join' requires string operands, got {:?}", s.ty),
                            Location::default(),
                            "E372",
                        ));
                    }
                }
                let joined = self.rt_call(b, st, "yarrow_str_join", vec![left.value, sep.value])?;
                let joined =
                    self.rt_call(b, st, "yarrow_str_join", vec![joined[0], right.value])?;
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
                let s = self.pop_slot(stack, "'@string_len'")?;
                if s.ty != Ty::String {
                    return Err(CompileError::new(
                        format!("'@string_len' requires a string, got {:?}", s.ty),
                        Location::default(),
                        "E372",
                    ));
                }
                let out = self.rt_call(b, st, "yarrow_str_len", vec![s.value])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }

            "list_push" => {
                let value = self.pop_slot(stack, "'@list_push'")?;
                let list = self.pop_slot(stack, "'@list_push'")?;
                let Ty::List { elem } = list.ty else {
                    return Err(CompileError::new(
                        format!("'@list_push' requires a list, got {:?}", list.ty),
                        Location::default(),
                        "E372",
                    ));
                };
                let elem_ty = elem_ty(elem);
                let val = coerce(b, value.value, value.ty, elem_ty, self.ptr_type)?;
                let arg = self.rt_arg(
                    b,
                    Slot {
                        value: val,
                        ty: elem_ty,
                        own: Own::Trivial,
                    },
                )?;
                self.rt_call(b, st, "yarrow_list_push", vec![list.value, arg])?;
                stack.push(list);
            }
            "list_len" => {
                let list = self.pop_slot(stack, "'@list_len'")?;
                if !matches!(list.ty, Ty::List { .. }) {
                    return Err(CompileError::new(
                        format!("'@list_len' requires a list, got {:?}", list.ty),
                        Location::default(),
                        "E372",
                    ));
                }
                let out = self.rt_call(b, st, "yarrow_list_len", vec![list.value])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }
            "list_get" => {
                let idx = self.pop_slot(stack, "'@list_get'")?;
                let list = self.pop_slot(stack, "'@list_get'")?;
                let Ty::List { elem } = list.ty else {
                    return Err(CompileError::new(
                        format!("'@list_get' requires a list, got {:?}", list.ty),
                        Location::default(),
                        "E372",
                    ));
                };
                let elem_ty = elem_ty(elem);
                let idx = coerce(b, idx.value, idx.ty, Ty::I64, self.ptr_type)?;
                let len = self.rt_call(b, st, "yarrow_list_len", vec![list.value])?;
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
                let value = self.pop_slot(stack, "'@list_set'")?;
                let idx = self.pop_slot(stack, "'@list_set'")?;
                let list = self.pop_slot(stack, "'@list_set'")?;
                let Ty::List { elem } = list.ty else {
                    return Err(CompileError::new(
                        format!("'@list_set' requires a list, got {:?}", list.ty),
                        Location::default(),
                        "E372",
                    ));
                };
                let elem_ty = elem_ty(elem);
                let idx = coerce(b, idx.value, idx.ty, Ty::I64, self.ptr_type)?;
                let len = self.rt_call(b, st, "yarrow_list_len", vec![list.value])?;
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
                let val = coerce(b, value.value, value.ty, elem_ty, self.ptr_type)?;
                b.ins()
                    .store(cranelift_codegen::ir::MemFlagsData::trusted(), val, addr, 0);
                stack.push(list);
            }

            "map_get" => {
                let key = self.pop_slot(stack, "'@map_get'")?;
                let map = self.pop_slot(stack, "'@map_get'")?;
                let Ty::Hashmap {
                    key: kcode,
                    value: vcode,
                } = map.ty
                else {
                    return Err(CompileError::new(
                        format!("'@map_get' requires a hashmap, got {:?}", map.ty),
                        Location::default(),
                        "E372",
                    ));
                };
                let kt = elem_ty(kcode);
                let vt = elem_ty(vcode);
                let karg = coerce(b, key.value, key.ty, kt, self.ptr_type)?;
                let karg = self.rt_arg(
                    b,
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
                let out =
                    self.rt_call(b, st, "yarrow_map_get", vec![map.value, karg, found_ptr])?;
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
                    coerce(b, val, Ty::I64, vt, self.ptr_type)?
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
                let value = self.pop_slot(stack, "'@map_set'")?;
                let key = self.pop_slot(stack, "'@map_set'")?;
                let map = self.pop_slot(stack, "'@map_set'")?;
                let Ty::Hashmap {
                    key: kcode,
                    value: vcode,
                } = map.ty
                else {
                    return Err(CompileError::new(
                        format!("'@map_set' requires a hashmap, got {:?}", map.ty),
                        Location::default(),
                        "E372",
                    ));
                };
                let kt = elem_ty(kcode);
                let vt = elem_ty(vcode);
                let karg = coerce(b, key.value, key.ty, kt, self.ptr_type)?;
                let karg = self.rt_arg(
                    b,
                    Slot {
                        value: karg,
                        ty: kt,
                        own: Own::Trivial,
                    },
                )?;
                let varg = coerce(b, value.value, value.ty, vt, self.ptr_type)?;
                let varg = self.rt_arg(
                    b,
                    Slot {
                        value: varg,
                        ty: vt,
                        own: Own::Trivial,
                    },
                )?;
                self.rt_call(b, st, "yarrow_map_insert", vec![map.value, karg, varg])?;
                stack.push(map);
            }
            "map_len" => {
                let map = self.pop_slot(stack, "'@map_len'")?;
                if !matches!(map.ty, Ty::Hashmap { .. }) {
                    return Err(CompileError::new(
                        format!("'@map_len' requires a hashmap, got {:?}", map.ty),
                        Location::default(),
                        "E372",
                    ));
                }
                let out = self.rt_call(b, st, "yarrow_map_len", vec![map.value])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::I64,
                    own: Own::Trivial,
                });
            }

            "print" => {
                let s = self.pop_slot(stack, "'@print'")?;
                if s.ty != Ty::String {
                    return Err(CompileError::new(
                        format!("'@print' requires a string, got {:?}", s.ty),
                        Location::default(),
                        "E372",
                    ));
                }
                self.rt_call(b, st, "yarrow_print_str", vec![s.value])?;
            }
            "print_int" => {
                let v = self.pop_slot(stack, "'@print_int'")?;
                let arg = coerce(b, v.value, v.ty, Ty::I64, self.ptr_type)?;
                self.rt_call(b, st, "yarrow_print_int", vec![arg])?;
            }
            "print_float" => {
                let v = self.pop_slot(stack, "'@print_float'")?;
                let arg = coerce(b, v.value, v.ty, Ty::F64, self.ptr_type)?;
                self.rt_call(b, st, "yarrow_print_float", vec![arg])?;
            }
            "print_newline" => {
                self.rt_call(b, st, "yarrow_print_newline", Vec::new())?;
            }

            "sqrt" => {
                let v = self.pop_slot(stack, "'@sqrt'")?;
                if !v.ty.is_float() && !v.ty.is_int() {
                    return Err(CompileError::new(
                        format!("'@sqrt' requires a number, got {:?}", v.ty),
                        Location::default(),
                        "E372",
                    ));
                }
                let arg = coerce(b, v.value, v.ty, Ty::F64, self.ptr_type)?;
                let out = self.rt_call(b, st, "yarrow_sqrt", vec![arg])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::F64,
                    own: Own::Trivial,
                });
            }

            _ => {
                return Err(CompileError::unsupported(
                    format!("builtin '{name}' is not yet supported"),
                    Location::default(),
                    "E301",
                ));
            }
        }
        Ok(())
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
                if let Some(mod_path) = &st.module {
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
                        let fq = format!("{path}::{member}");
                        if !self.func_ids.contains_key(&fq) {
                            return Err(CompileError::new(
                                format!("module '{path}' has no function '{member}'"),
                                Location::default(),
                                "E330",
                            ));
                        }
                        fq
                    } else {
                        self.method_name(st, base, member)?
                    }
                } else {
                    self.method_name(st, base, member)?
                }
            }
            Expr::Builtin { name: _ } => {
                // `@borrow call` / `@move call` apply the builtin itself.
                self.compile_expr(b, st, stack, target)?;
                return Ok(());
            }
            _ => {
                return Err(CompileError::new(
                    "'call' target must be a function name",
                    Location::default(),
                    "E329",
                ));
            }
        };
        let (param_tys, return_tys) = self.sig_tys.get(&name).cloned().ok_or_else(|| {
            CompileError::new(
                format!("unknown function '{name}'"),
                Location::default(),
                "E330",
            )
        })?;
        let n = param_tys.len();
        if stack.len() < n {
            return Err(CompileError::new(
                format!("call to '{name}' requires {n} argument(s)"),
                Location::default(),
                "E331",
            ));
        }
        let tail = stack.split_off(stack.len() - n);
        let mut args: Vec<Value> = Vec::with_capacity(n);
        let mut owned_temps: Vec<Slot> = Vec::new();
        for (i, slot) in tail.iter().enumerate() {
            // An owned value passed by value to a callee is borrowed by the
            // callee (never freed there). The caller drops it once the call
            // returns; this frees immediately (in the current block) so the
            // drop stays dominance-correct inside loops. Variable values are
            // Trivial here — the variable drop at scope exit handles them.
            if slot.own.is_owned() && self.is_heap(slot.ty) {
                owned_temps.push(*slot);
            }
            args.push(coerce(b, slot.value, slot.ty, param_tys[i], self.ptr_type)?);
        }

        let fref = st.frefs.get(&name).copied().ok_or_else(|| {
            CompileError::new(
                format!("unregistered callee '{name}'"),
                Location::default(),
                "E330",
            )
        })?;
        let call_inst = b.ins().call(fref, &args);
        let results: Vec<Value> = b.inst_results(call_inst).to_vec();
        if let Some(payload_ty) = error_return(&return_tys)? {
            // `with T or Error` callee: results are `(env, payload)`. Push the
            // payload (as its declared type) followed by the envelope tag so
            // `unwrap`/`handle` pop the tag first.
            let env = results[0];
            let payload = if payload_ty == Ty::Void {
                results[1]
            } else {
                coerce(b, results[1], Ty::I64, payload_ty, self.ptr_type)?
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
                Location::default(),
                "E342",
            ));
        }
        Ok(method)
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
                let s = self.pop_slot(stack, "dup")?;
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
            StackOp::Over => {
                let top = self.pop_slot(stack, "over")?;
                let sec = self.pop_slot(stack, "over")?;
                stack.push(sec);
                stack.push(top);
                stack.push(sec);
            }
            StackOp::Swap => {
                let top = self.pop_slot(stack, "swap")?;
                let sec = self.pop_slot(stack, "swap")?;
                stack.push(top);
                stack.push(sec);
            }
            StackOp::Rot => {
                let a = self.pop_slot(stack, "rot")?;
                let second = self.pop_slot(stack, "rot")?;
                let third = self.pop_slot(stack, "rot")?;
                stack.push(second);
                stack.push(third);
                stack.push(a);
            }
            StackOp::Pop | StackOp::Drop => {
                let slot = self.pop_slot(stack, "pop/drop")?;
                self.consume(b, st, slot)?;
            }
        }
        Ok(())
    }

    fn emit_not(
        &mut self,
        b: &mut FunctionBuilder,
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
                    Location::default(),
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
        let common = common_type(l.ty, r.ty).ok_or_else(|| {
            CompileError::new(
                format!(
                    "incompatible operand types {:?} and {:?} for {:?}",
                    l.ty, r.ty, op
                ),
                Location::default(),
                "E333",
            )
        })?;

        match op {
            Plus if common == Ty::String => {
                let out = self.rt_call(b, st, "yarrow_str_join", vec![l.value, r.value])?;
                stack.push(Slot {
                    value: out[0],
                    ty: Ty::String,
                    own: Own::Trivial,
                });
            }
            Plus | Minus | Mul | Mod | Pow => {
                if common.is_float() {
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type)?;
                    let v = match op {
                        Plus => b.ins().fadd(ll, rr),
                        Minus => b.ins().fsub(ll, rr),
                        Mul => b.ins().fmul(ll, rr),
                        _ => {
                            return Err(CompileError::unsupported(
                                "float 'mod'/'^' are not yet supported",
                                Location::default(),
                                "E334",
                            ));
                        }
                    };
                    stack.push(Slot {
                        value: v,
                        ty: common,
                        own: Own::Trivial,
                    });
                } else {
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type)?;
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
                            let lw = coerce(b, ll, common, Ty::I64, self.ptr_type)?;
                            let rw = coerce(b, rr, common, Ty::I64, self.ptr_type)?;
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
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type)?;
                    let v = b.ins().fdiv(ll, rr);
                    stack.push(Slot {
                        value: v,
                        ty: common,
                        own: Own::Trivial,
                    });
                } else {
                    // `10 4 /` yields 2.5: promote integers to f64.
                    let ll = coerce(b, l.value, l.ty, Ty::F64, self.ptr_type)?;
                    let rr = coerce(b, r.value, r.ty, Ty::F64, self.ptr_type)?;
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
                        Location::default(),
                        "E335",
                    ));
                }
                let ll = coerce(b, l.value, l.ty, common, self.ptr_type)?;
                let rr = coerce(b, r.value, r.ty, common, self.ptr_type)?;
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
                    let out = self.rt_call(b, st, "yarrow_str_cmp", vec![l.value, r.value])?;
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
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type)?;
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
                    let ll = coerce(b, l.value, l.ty, common, self.ptr_type)?;
                    let rr = coerce(b, r.value, r.ty, common, self.ptr_type)?;
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
                        Location::default(),
                        "E336",
                    ));
                }
                let ll = coerce(b, l.value, l.ty, common, self.ptr_type)?;
                let rr = coerce(b, r.value, r.ty, common, self.ptr_type)?;
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
                let ll = coerce(b, l.value, l.ty, Ty::I64, self.ptr_type)?;
                let rr = coerce(b, r.value, r.ty, Ty::I64, self.ptr_type)?;
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

/// Host runtime functions imported by compiled code: (symbol, params, returns).
/// Pointers, integers and floats are passed by value in a single register.
const RUNTIME_SIGS: &[(&str, &[CLType], &[CLType])] = &[
    ("yarrow_alloc", &[irtypes::I64], &[irtypes::I64]),
    ("yarrow_free", &[irtypes::I64], &[]),
    (
        "yarrow_str_new",
        &[irtypes::I64, irtypes::I64],
        &[irtypes::I64],
    ),
    ("yarrow_str_len", &[irtypes::I64], &[irtypes::I64]),
    (
        "yarrow_str_join",
        &[irtypes::I64, irtypes::I64],
        &[irtypes::I64],
    ),
    (
        "yarrow_str_cmp",
        &[irtypes::I64, irtypes::I64],
        &[irtypes::I64],
    ),
    ("yarrow_list_new", &[irtypes::I64], &[irtypes::I64]),
    ("yarrow_list_len", &[irtypes::I64], &[irtypes::I64]),
    ("yarrow_list_push", &[irtypes::I64, irtypes::I64], &[]),
    ("yarrow_list_free", &[irtypes::I64], &[]),
    ("yarrow_map_new", &[irtypes::I64], &[irtypes::I64]),
    (
        "yarrow_map_insert",
        &[irtypes::I64, irtypes::I64, irtypes::I64],
        &[],
    ),
    (
        "yarrow_map_get",
        &[irtypes::I64, irtypes::I64, irtypes::I64],
        &[irtypes::I64],
    ),
    ("yarrow_map_len", &[irtypes::I64], &[irtypes::I64]),
    ("yarrow_print_str", &[irtypes::I64], &[]),
    ("yarrow_print_int", &[irtypes::I64], &[]),
    ("yarrow_print_float", &[irtypes::F64], &[]),
    ("yarrow_print_newline", &[], &[]),
    ("yarrow_sqrt", &[irtypes::F64], &[irtypes::F64]),
    ("yarrow_free_value", &[irtypes::I64, irtypes::I64], &[]),
    (
        "yarrow_register_struct_descs",
        &[irtypes::I64, irtypes::I64, irtypes::I64],
        &[],
    ),
    ("yarrow_region_new", &[], &[irtypes::I64]),
    (
        "yarrow_region_register",
        &[irtypes::I64, irtypes::I64, irtypes::I64],
        &[],
    ),
    ("yarrow_region_free", &[irtypes::I64], &[]),
];

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
            Expr::Array(es) | Expr::List(es) | Expr::Seq(es) => {
                for el in es {
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
            | Expr::Unwrap { inner: base } => walk_expr(base, out),
            Expr::Unary { operand, .. } => walk_expr(operand, out),
            Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            _ => {}
        }
    }
    for s in stmts {
        match s {
            Stmt::Expr(e) => walk_expr(e, out),
            Stmt::VarDecl { value, .. } | Stmt::Set { value, .. } => {
                if let Some(v) = value {
                    walk_expr(v, out);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                walk_expr(condition, out);
                collect_strings(then_branch, out);
                collect_strings(else_branch, out);
            }
            Stmt::While { condition, body }
            | Stmt::For {
                iterable: condition,
                body,
                ..
            } => {
                walk_expr(condition, out);
                collect_strings(body, out);
            }
            Stmt::Match {
                value,
                cases,
                else_branch,
            } => {
                walk_expr(value, out);
                for c in cases {
                    walk_expr(&c.condition, out);
                    collect_strings(&c.body, out);
                }
                collect_strings(else_branch, out);
            }
            Stmt::Return { value: Some(v) } => walk_expr(v, out),
            Stmt::Return { value: None } => {}
            Stmt::Defer { body } | Stmt::Handle { body } => collect_strings(body, out),
            Stmt::Function(f) => collect_strings(&f.body, out),
            Stmt::Implement(imp) => {
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
                Location::default(),
                "E345",
            )
        }),
    }
}
