//! A Cranelift JIT compiler for Yarrow programs.
//!
//! The compiler mirrors the parser's operand-stack model: statements are balanced
//! against a compile-time value stack (`Vec<Slot>`), and binary operators the
//! parser left as runtime `ApplyBin`/`ApplyUn`/`StackOp` ops are lowered by
//! popping operands off that same stack.

mod errors;
mod types;

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    AbiParam, Block, BlockArg, FuncRef, InstBuilder as _, StackSlotData, StackSlotKind, TrapCode,
    Type as CLType, Value, types as irtypes,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, default_libcall_names};

use crate::parser::ast::{BinOp, Expr, Function, Program, StackOp, Stmt, UnOp};
use crate::parser::literals::{decode_float_literal, decode_int_literal, decode_rune_literal};
use crate::tokenizer::token::Location;

pub use errors::CompileError;
use types::CResult;
pub use types::Ty;
use types::{StructLayout, coerce, common_type, layout, resolve};

/// A value on the compile-time operand stack: a Cranelift SSA value plus the
/// physical `Ty` it carries.
#[derive(Debug, Clone, Copy)]
struct Slot {
    value: Value,
    ty: Ty,
}

/// Per-function lowering state.
struct FnState {
    vars: HashMap<String, (Variable, Ty)>,
    loops: Vec<LoopCtx>,
    returns: Vec<Ty>,
    frefs: HashMap<String, FuncRef>,
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
    finalized: bool,
}

impl Compiler {
    pub fn new() -> CResult<Self> {
        let jb = JITBuilder::new(default_libcall_names())
            .map_err(|e| CompileError::new(e.to_string(), Location::default(), "E350"))?;
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
            finalized: false,
        })
    }

    /// Two-pass compilation: first register structs and declare every function
    /// (so whole-program calls resolve), then compile each body.
    pub fn compile(&mut self, program: &Program) -> CResult<()> {
        // Pass A: register every struct name.
        for item in &program.items {
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

        // Pass B: resolve each struct's field types into a layout. Must happen
        // before function signatures are declared, since those may use structs.
        for item in &program.items {
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

        // Pass C: declare every function.
        for item in &program.items {
            match item {
                Stmt::Function(f) => self.declare_function(f, &f.name)?,
                Stmt::Implement(imp) => {
                    for f in &imp.functions {
                        self.declare_function(f, &format!("{}::{}", imp.target, f.name))?;
                    }
                }
                _ => {}
            }
        }

        // Pass D: compile every function.
        for item in &program.items {
            match item {
                Stmt::Function(f) => self.compile_function(f, &f.name)?,
                Stmt::Implement(imp) => {
                    for f in &imp.functions {
                        self.compile_function(f, &format!("{}::{}", imp.target, f.name))?;
                    }
                }
                _ => {}
            }
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
    fn alloc_struct(&mut self, b: &mut FunctionBuilder, id: u32) -> Value {
        let lay = self.struct_layout(id);
        let data = StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            lay.size,
            lay.align.trailing_zeros() as u8,
        );
        let slot = b.create_sized_stack_slot(data);
        b.ins().stack_addr(self.ptr_type, slot, 0)
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
                let inner_ptr = self.alloc_struct(b, inner_id);
                self.init_struct_fields(b, st, stack, inner_id, inner_ptr, inner_pairs)?;
                b.ins().store(
                    cranelift_codegen::ir::MemFlagsData::trusted(),
                    inner_ptr,
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
        let id = self.module.declare_function(name, Linkage::Export, &sig)?;
        self.sigs.insert(name.to_string(), sig);
        self.sig_tys
            .insert(name.to_string(), (param_tys, return_tys));
        self.func_ids.insert(name.to_string(), id);
        Ok(())
    }

    fn compile_function(&mut self, f: &Function, name: &str) -> CResult<()> {
        let sig = self.sigs.get(name).cloned().unwrap();
        let id = *self.func_ids.get(name).unwrap();

        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let mut st = FnState {
            vars: HashMap::new(),
            loops: Vec::new(),
            returns: f
                .returns
                .iter()
                .map(|r| self.resolve_ty(r))
                .collect::<CResult<_>>()?,
            frefs: HashMap::new(),
        };

        // Import every declared function so any callee (free or method) can be
        // resolved later; frefs must be created before the FunctionBuilder
        // takes ownership of `ctx.func`.
        for (callee, &fid) in &self.func_ids {
            let fr = self.module.declare_func_in_func(fid, &mut ctx.func);
            st.frefs.insert(callee.clone(), fr);
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
            stack.push(Slot {
                value: param_vals[i],
                ty: *t,
            });
        }

        // In a method body, the receiver is param 0. Bind `self` to it so a
        // `self const reference<Point>` declaration resolves without relying
        // on stack position.
        if name.contains("::")
            && let Some((t, v)) = params_ty.first().zip(param_vals.first())
        {
            let var = b.declare_var(t.clty(self.ptr_type));
            b.def_var(var, *v);
            st.vars.insert("self".to_string(), (var, *t));
        }

        self.compile_body(&mut b, &mut st, &mut stack, &f.body)?;

        // Implicit termination for a function falling off the end.
        if st.returns.is_empty() {
            b.ins().return_(&[]);
        } else if stack.len() >= st.returns.len() {
            let vals = self.pop_return_values(&mut b, &st, &mut stack)?;
            b.ins().return_(&vals);
        } else {
            b.ins().trap(TrapCode::unwrap_user(1));
        }

        b.seal_all_blocks();
        b.finalize();
        self.module.define_function(id, &mut ctx)?;
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
                let t = self.resolve_ty(ty)?;
                // `self` was already bound to the receiver at function entry;
                // the `self const reference<Point>` declaration is a no-op.
                if name == "self" && st.vars.contains_key("self") {
                    return Ok(());
                }
                let (val, val_ty) = match value {
                    Some(Expr::Map(pairs)) if matches!(t, Ty::Struct(_)) => {
                        // Struct literal `{x 5 y 20}`: allocate a slot and
                        // store each field by name.
                        let Ty::Struct(id) = t else { unreachable!() };
                        let ptr = self.alloc_struct(b, id);
                        self.init_struct_fields(b, st, stack, id, ptr, pairs)?;
                        (ptr, t)
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
                let val = coerce(b, val, val_ty, t, self.ptr_type)?;
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
                    let (val, val_ty) = match value {
                        Some(Expr::Map(pairs)) if matches!(t, Ty::Struct(_)) => {
                            let Ty::Struct(id) = t else { unreachable!() };
                            let ptr = b.use_var(var);
                            self.init_struct_fields(b, st, stack, id, ptr, pairs)?;
                            return Ok(());
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
                    let val = coerce(b, val, val_ty, t, self.ptr_type)?;
                    b.def_var(var, val);
                }
                Expr::Member { base, member } => {
                    let sid = self.base_struct(st, base)?;
                    let field = self.find_field(sid, member)?.clone();
                    self.compile_expr(b, st, stack, base)?;
                    let ptr = self.pop_slot(stack, "field set target")?;
                    let (val, val_ty) = match value {
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

            Stmt::Match { .. } => {
                return Err(CompileError::unsupported(
                    "'match' is not yet supported",
                    Location::default(),
                    "E301",
                ));
            }

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

            Stmt::Defer { .. } | Stmt::Handle { .. } | Stmt::For { .. } => {
                return Err(CompileError::unsupported(
                    "'for'/'defer'/'handle' are not yet supported",
                    Location::default(),
                    "E301",
                ));
            }
        }
        Ok(())
    }

    fn emit_return(
        &mut self,
        b: &mut FunctionBuilder,
        st: &FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<()> {
        if st.returns.is_empty() {
            b.ins().return_(&[]);
        } else {
            let vals = self.pop_return_values(b, st, stack)?;
            b.ins().return_(&vals);
        }
        // The rest of the function is unreachable; the compile-time stack is
        // dead, so clear it to stop the implicit fallthrough return from
        // picking up leftovers (e.g. a method receiver).
        stack.clear();
        self.dead_block(b);
        Ok(())
    }

    fn pop_return_values(
        &self,
        b: &mut FunctionBuilder,
        st: &FnState,
        stack: &mut Vec<Slot>,
    ) -> CResult<Vec<Value>> {
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
            out.push(coerce(b, slot.value, slot.ty, *want, self.ptr_type)?);
        }
        Ok(out)
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
        let ev: Vec<BlockArg> = else_stack[pre.len()..]
            .iter()
            .map(|s| BlockArg::Value(s.value))
            .collect();
        b.ins().jump(merge, &ev);

        b.switch_to_block(merge);
        *stack = pre;
        for (i, s) in then_extra.iter().enumerate() {
            stack.push(Slot {
                value: params[i],
                ty: s.ty,
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

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

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
                });
            }
            Expr::Float { value } => {
                let n = decode_float_literal(value)
                    .map_err(|m| CompileError::new(m, Location::default(), "E363"))?;
                let v = b.ins().f64const(n);
                stack.push(Slot {
                    value: v,
                    ty: Ty::F64,
                });
            }
            Expr::Bool { value } => {
                let v = b.ins().iconst(irtypes::I8, if *value { 1 } else { 0 });
                stack.push(Slot {
                    value: v,
                    ty: Ty::Bool,
                });
            }
            Expr::Rune { value } => {
                let cp = decode_rune_literal(value)
                    .map_err(|m| CompileError::new(m, Location::default(), "E363"))?;
                let v = b.ins().iconst(irtypes::I32, cp as i64);
                stack.push(Slot {
                    value: v,
                    ty: Ty::Rune,
                });
            }
            Expr::String { .. } => {
                return Err(CompileError::unsupported(
                    "string values are not yet supported",
                    Location::default(),
                    "E301",
                ));
            }
            Expr::Variable { name } => {
                let (var, t) = st.vars.get(name).cloned().ok_or_else(|| {
                    CompileError::new(
                        format!("unknown variable '{name}'"),
                        Location::default(),
                        "E320",
                    )
                })?;
                let v = b.use_var(var);
                stack.push(Slot { value: v, ty: t });
            }
            Expr::Member { base, member } => {
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
                });
            }
            Expr::Builtin { name } if matches!(name.as_str(), "borrow" | "move") => {
                // A borrow/move of a struct is the same pointer; ownership is
                // checked at compile time (not yet enforced), so both are
                // identity at codegen time.
                let s = self.pop_slot(stack, name)?;
                if !s.ty.is_pointer() {
                    return Err(CompileError::new(
                        format!("'{name}' requires a reference or struct value"),
                        Location::default(),
                        "E341",
                    ));
                }
                stack.push(s);
            }
            Expr::Builtin { name } => {
                return Err(CompileError::unsupported(
                    format!("builtin '{name}' is not yet supported"),
                    Location::default(),
                    "E301",
                ));
            }
            Expr::Unwrap { .. } => {
                return Err(CompileError::unsupported(
                    "'unwrap' is not yet supported",
                    Location::default(),
                    "E301",
                ));
            }
            Expr::Binary { op, left, right } => {
                self.compile_expr(b, st, stack, left)?;
                self.compile_expr(b, st, stack, right)?;
                let r = self.pop_slot(stack, "operator")?;
                let l = self.pop_slot(stack, "operator")?;
                self.emit_bin(b, stack, *op, l, r)?;
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
                self.emit_bin(b, stack, *op, l, r)?;
            }
            Expr::ApplyUn(op) => {
                let slot = self.pop_slot(stack, "unary operator")?;
                self.emit_not(b, stack, *op, slot)?;
            }
            Expr::StackOp(op) => self.emit_stackop(stack, *op)?,
            Expr::Seq(elems) => {
                for el in elems {
                    self.compile_expr(b, st, stack, el)?;
                }
            }
            Expr::Array(_) | Expr::List(_) | Expr::Map(_) => {
                return Err(CompileError::unsupported(
                    "container literals are not yet supported",
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
            Expr::Variable { name } => name.clone(),
            Expr::Member { base, member } => {
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
                method
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
        for (i, slot) in tail.iter().enumerate() {
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
        for (v, t) in results.into_iter().zip(&return_tys) {
            stack.push(Slot { value: v, ty: *t });
        }
        Ok(())
    }

    fn emit_stackop(&mut self, stack: &mut Vec<Slot>, op: StackOp) -> CResult<()> {
        match op {
            StackOp::Dup => {
                let s = self.pop_slot(stack, "dup")?;
                stack.push(s);
                stack.push(s);
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
                let _ = self.pop_slot(stack, "pop/drop")?;
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
        });
        Ok(())
    }

    // ------------------------------------------------------------------
    // Binary operators
    // ------------------------------------------------------------------

    fn emit_bin(
        &mut self,
        b: &mut FunctionBuilder,
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
                    });
                } else {
                    // `10 4 /` yields 2.5: promote integers to f64.
                    let ll = coerce(b, l.value, l.ty, Ty::F64, self.ptr_type)?;
                    let rr = coerce(b, r.value, r.ty, Ty::F64, self.ptr_type)?;
                    let v = b.ins().fdiv(ll, rr);
                    stack.push(Slot {
                        value: v,
                        ty: Ty::F64,
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
                });
            }

            Eq | Ne | Gt | Gte | Lt | Lte => {
                if common.is_float() {
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
