//! AST interpreter for checked Yarrow programs (Stage 13b).
//!
//! Design choice: **tree-walk** the checked AST with an explicit operand stack,
//! calling into [`crate::runtime`] for heap strings and printing. A stack
//! bytecode VM can replace this later without changing the Session surface
//! (`interpret_source` / [`EvalContext`]).
//!
//! ## Scope (MVP)
//!
//! In scope (gate examples):
//! - `docs/examples/valid/01_hello.yar`
//! - `docs/examples/valid/02_arithmetic_and_stack.yar`
//!
//! Supported surface: `require` + module calls, literals, arithmetic /
//! comparisons / bool ops / shifts, string `~`, stack words (`dup`/`swap`/
//! `rot`/`drop`/`pop`), `@print` / `@print_newline`, user function calls with
//! stack params, void `main`.
//!
//! Out of scope for this MVP (fail with a clear interpret error): control
//! flow (`if`/`for`/`match`), variables/`set`/`move`, structs/unions/regions,
//! unsafe, fallible `unwrap`/`handle`, most std intrinsics, non-void `main`
//! returns beyond simple scalars.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::compiler::CompileError;
use crate::compiler::RunResult;
use crate::compiler::modules::{ModuleLoader, RequiredModule};
use crate::diagnostics::Span;
use crate::parser::ast::{
    BinOp, Expr, Function, Program, StackOp, Stmt, StmtKind, UnOp, Visibility,
};
use crate::parser::literals::{decode_float_literal, decode_int_literal, decode_string_literal};
use crate::parser::parse;
use crate::runtime::{self, KIND_STRING, free_value};
use crate::tokenizer::Tokenizer;

/// Runtime value on the interpreter operand stack.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    /// Heap string handle from [`runtime::yarrow_str_new`].
    Str {
        handle: u64,
        owned: bool,
    },
}

impl Value {
    fn drop_owned(self) {
        if let Value::Str {
            handle,
            owned: true,
        } = self
        {
            free_value(handle, KIND_STRING);
        }
    }
}

/// Error while interpreting an already-checked program.
#[derive(Debug, Clone)]
pub struct InterpretError {
    pub message: String,
    pub code: String,
    pub span: Span,
}

impl InterpretError {
    fn new(message: impl Into<String>, span: Span, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: code.into(),
            span,
        }
    }

    fn unsupported(what: impl Into<String>, span: Span) -> Self {
        Self::new(
            format!("interpreter does not support {} yet", what.into()),
            span,
            "E393",
        )
    }

    pub fn into_compile_error(self) -> CompileError {
        CompileError::new(self.message, self.span, self.code)
    }
}

type IResult<T> = Result<T, InterpretError>;

/// One registered function body (top-level or from a required module).
#[derive(Debug, Clone)]
struct FuncEntry {
    module: Option<String>,
    function: Function,
}

/// Stack-machine interpreter over a checked AST.
pub struct Interpreter {
    loader: ModuleLoader,
    /// Fully-qualified name (`std.io::write_line` or `main`) → entry.
    funcs: HashMap<String, FuncEntry>,
    /// Module alias → module path (`io` → `std.io`).
    aliases: HashMap<String, String>,
    /// Bare name → fq name for alias-less requires.
    plain_funcs: HashMap<String, String>,
    /// Public exports (fq names). Reserved for visibility checks as parity grows.
    #[allow(dead_code)]
    public_funcs: std::collections::HashSet<String>,
    modules: Vec<RequiredModule>,
}

/// REPL-oriented wrapper around [`Interpreter`] (Stage 13b surface).
///
/// Whole-file `main` is supported now; incremental chunk eval can grow later.
pub struct EvalContext {
    interp: Interpreter,
}

impl EvalContext {
    pub fn new() -> Self {
        Self {
            interp: Interpreter::new(),
        }
    }

    pub fn add_module_search_path(&mut self, path: impl Into<PathBuf>) {
        self.interp.add_module_search_path(path);
    }

    /// Load a checked program (registers `require`s and functions). Does not run `main`.
    pub fn load_program(&mut self, program: &Program) -> IResult<()> {
        self.interp.load_program(program)
    }

    /// Execute `main` and return a driver-displayable result.
    pub fn run_main(&mut self) -> IResult<RunResult> {
        self.run_entry(crate::DEFAULT_ENTRY_NAME)
    }

    /// Execute the named top-level entry and return a driver-displayable result.
    pub fn run_entry(&mut self, name: &str) -> IResult<RunResult> {
        self.interp.run_entry(name)
    }
}

impl Default for EvalContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            loader: ModuleLoader::new(),
            funcs: HashMap::new(),
            aliases: HashMap::new(),
            plain_funcs: HashMap::new(),
            public_funcs: std::collections::HashSet::new(),
            modules: Vec::new(),
        }
    }

    pub fn add_module_search_path(&mut self, path: impl Into<PathBuf>) {
        self.loader.add_search_path(path);
    }

    pub fn load_program(&mut self, program: &Program) -> IResult<()> {
        self.funcs.clear();
        self.aliases.clear();
        self.plain_funcs.clear();
        self.public_funcs.clear();
        self.modules.clear();

        let mut loaded = Vec::new();
        self.load_requires(program, &mut loaded)?;
        self.modules = loaded;

        self.register_unit(None, program)?;
        for m in &self.modules.clone() {
            self.register_unit(Some(m.path.clone()), &m.program)?;
        }
        self.register_module_bindings()?;
        Ok(())
    }

    pub fn run_main(&mut self) -> IResult<RunResult> {
        self.run_entry(crate::DEFAULT_ENTRY_NAME)
    }

    pub fn run_entry(&mut self, name: &str) -> IResult<RunResult> {
        let entry = self.funcs.get(name).cloned().ok_or_else(|| {
            InterpretError::new(
                format!("program has no '{name}' function"),
                Span::default(),
                "E360",
            )
        })?;
        let mut stack = Vec::new();
        self.call_function(&entry, &mut stack)?;
        if entry.function.returns.is_empty() {
            while let Some(v) = stack.pop() {
                v.drop_owned();
            }
            return Ok(RunResult::Void);
        }
        if stack.len() != 1 {
            return Err(InterpretError::new(
                format!(
                    "{name} left {} value(s) on the stack; expected 1",
                    stack.len()
                ),
                Span::default(),
                "E393",
            ));
        }
        Ok(match stack.pop().unwrap() {
            Value::Int(n) => RunResult::Int(n),
            Value::Float(f) => RunResult::Float(f),
            Value::Bool(b) => RunResult::Bool(b),
            Value::Str { handle, owned } => {
                let bytes = runtime::string_bytes(handle).unwrap_or_default();
                let s = String::from_utf8_lossy(&bytes).into_owned();
                if owned {
                    free_value(handle, KIND_STRING);
                }
                RunResult::Str(s)
            }
        })
    }

    fn register_unit(&mut self, module: Option<String>, program: &Program) -> IResult<()> {
        for item in &program.items {
            if let StmtKind::Function(f) = &item.kind {
                let keep = module.is_none() || matches!(f.visibility, Some(Visibility::Public));
                if !keep {
                    continue;
                }
                let name = match &module {
                    Some(path) => format!("{path}::{}", f.name),
                    None => f.name.clone(),
                };
                if matches!(f.visibility, Some(Visibility::Public)) {
                    self.public_funcs.insert(name.clone());
                }
                self.funcs.insert(
                    name,
                    FuncEntry {
                        module: module.clone(),
                        function: f.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    fn register_module_bindings(&mut self) -> IResult<()> {
        for m in &self.modules {
            if let Some(alias) = &m.alias {
                self.aliases.insert(alias.clone(), m.path.clone());
            } else if let Some(item) = &m.item {
                let fq = format!("{}::{item}", m.path);
                self.plain_funcs.insert(item.clone(), fq);
            } else {
                for item in &m.program.items {
                    if let StmtKind::Function(f) = &item.kind
                        && matches!(f.visibility, Some(Visibility::Public))
                    {
                        let fq = format!("{}::{}", m.path, f.name);
                        self.plain_funcs.insert(f.name.clone(), fq);
                    }
                }
            }
        }
        Ok(())
    }

    fn load_requires(&mut self, program: &Program, out: &mut Vec<RequiredModule>) -> IResult<()> {
        self.load_requires_stmts(&program.items, out)
    }

    fn load_requires_stmts(
        &mut self,
        stmts: &[Stmt],
        out: &mut Vec<RequiredModule>,
    ) -> IResult<()> {
        for s in stmts {
            match &s.kind {
                StmtKind::Require { path, alias } => self.load_one(path, alias, out)?,
                StmtKind::Function(f) => self.load_requires_stmts(&f.body, out)?,
                StmtKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.load_requires_stmts(then_branch, out)?;
                    self.load_requires_stmts(else_branch, out)?;
                }
                StmtKind::For { body, .. } => self.load_requires_stmts(body, out)?,
                StmtKind::Match {
                    cases, else_branch, ..
                } => {
                    for c in cases {
                        self.load_requires_stmts(&c.body, out)?;
                    }
                    self.load_requires_stmts(else_branch, out)?;
                }
                StmtKind::Defer { body } | StmtKind::Unsafe { body } => {
                    self.load_requires_stmts(body, out)?;
                }
                StmtKind::Handle { body, .. } => self.load_requires_stmts(body, out)?,
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
    ) -> IResult<()> {
        let (module_path, item) = self.resolve_require(path)?;
        if out.iter().any(|m| m.path == module_path) {
            return Ok(());
        }
        let source = self.loader.load(&module_path).map_err(|e| {
            InterpretError::new(e.message().to_string(), e.span(), e.code().to_string())
        })?;
        let tokens = Tokenizer::new(source)
            .tokenize()
            .map_err(|e| InterpretError::new(e.message, Span::from_location(e.location), e.code))?;
        let sub = parse(tokens).map_err(|batch| {
            let d = batch.iter().next();
            InterpretError::new(
                d.map(|x| x.message.clone())
                    .unwrap_or_else(|| "parse error in required module".into()),
                d.and_then(|x| x.primary_span()).unwrap_or_default(),
                d.map(|x| x.code.clone()).unwrap_or_else(|| "E200".into()),
            )
        })?;
        self.load_requires(&sub, out)?;
        out.push(RequiredModule {
            path: module_path,
            alias: alias.clone(),
            item,
            program: sub,
        });
        Ok(())
    }

    fn resolve_require(&self, path: &str) -> IResult<(String, Option<String>)> {
        if let Some((parent, last)) = path.rsplit_once('.')
            && let Some(parent_source) = self.loader.try_load(parent)
        {
            let tokens = Tokenizer::new(parent_source).tokenize().map_err(|e| {
                InterpretError::new(e.message, Span::from_location(e.location), e.code)
            })?;
            let parent_prog = parse(tokens).map_err(|_| {
                InterpretError::new("failed to parse parent module", Span::default(), "E380")
            })?;
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

    fn call_function(&mut self, entry: &FuncEntry, stack: &mut Vec<Value>) -> IResult<()> {
        let n = entry.function.params.len();
        if stack.len() < n {
            return Err(InterpretError::new(
                format!("call to '{}' requires {n} argument(s)", entry.function.name),
                Span::default(),
                "E331",
            ));
        }
        let args: Vec<Value> = stack.split_off(stack.len() - n);
        // Params arrive on a fresh stack (same order as compiler).
        let mut local_stack = args;
        self.eval_body(
            &entry.function.body,
            &mut local_stack,
            entry.module.as_deref(),
        )?;
        // Return values: leave whatever the body left (checked programs balance).
        let ret_n = entry.function.returns.len();
        if ret_n == 0 {
            while let Some(v) = local_stack.pop() {
                v.drop_owned();
            }
        } else {
            if local_stack.len() < ret_n {
                return Err(InterpretError::new(
                    format!(
                        "function '{}' returned {} value(s), expected {ret_n}",
                        entry.function.name,
                        local_stack.len()
                    ),
                    Span::default(),
                    "E328",
                ));
            }
            let rets = local_stack.split_off(local_stack.len() - ret_n);
            while let Some(v) = local_stack.pop() {
                v.drop_owned();
            }
            stack.extend(rets);
        }
        Ok(())
    }

    fn eval_body(
        &mut self,
        body: &[Stmt],
        stack: &mut Vec<Value>,
        module: Option<&str>,
    ) -> IResult<()> {
        for stmt in body {
            self.eval_stmt(stmt, stack, module)?;
        }
        Ok(())
    }

    fn eval_stmt(
        &mut self,
        stmt: &Stmt,
        stack: &mut Vec<Value>,
        module: Option<&str>,
    ) -> IResult<()> {
        match &stmt.kind {
            StmtKind::Expr(e) => self.eval_expr(e, stack, module, stmt.span),
            StmtKind::Require { .. } => Ok(()), // already loaded
            StmtKind::Function(_)
            | StmtKind::Struct(_)
            | StmtKind::Implement(_)
            | StmtKind::Enum(_)
            | StmtKind::Union(_)
            | StmtKind::Error(_) => Ok(()),
            StmtKind::Return { value } => {
                let mut rets = Vec::new();
                if let Some(v) = value {
                    self.eval_expr(v, stack, module, stmt.span)?;
                    rets.push(self.pop(stack, stmt.span, "return")?);
                }
                while let Some(v) = stack.pop() {
                    v.drop_owned();
                }
                stack.extend(rets);
                Ok(())
            }
            other => Err(InterpretError::unsupported(
                format!("statement {other:?}"),
                stmt.span,
            )),
        }
    }

    fn eval_expr(
        &mut self,
        expr: &Expr,
        stack: &mut Vec<Value>,
        module: Option<&str>,
        span: Span,
    ) -> IResult<()> {
        match expr {
            Expr::Integer { value } => {
                let n =
                    decode_int_literal(value).map_err(|m| InterpretError::new(m, span, "E363"))?;
                stack.push(Value::Int(n as i64));
                Ok(())
            }
            Expr::Float { value } => {
                let f = decode_float_literal(value)
                    .map_err(|m| InterpretError::new(m, span, "E363"))?;
                stack.push(Value::Float(f));
                Ok(())
            }
            Expr::Bool { value } => {
                stack.push(Value::Bool(*value));
                Ok(())
            }
            Expr::String { value } => {
                let bytes = decode_string_literal(value)
                    .map_err(|m| InterpretError::new(m, span, "E363"))?;
                let handle = runtime::yarrow_str_new(bytes.as_ptr() as u64, bytes.len() as u64);
                stack.push(Value::Str {
                    handle,
                    owned: true,
                });
                Ok(())
            }
            Expr::Seq(elems) => {
                for (e, s) in elems {
                    self.eval_expr(e, stack, module, *s)?;
                }
                Ok(())
            }
            Expr::ApplyBin(op) => {
                let r = self.pop(stack, span, "operator")?;
                let l = self.pop(stack, span, "operator")?;
                let out = self.eval_bin(*op, l, r, span)?;
                stack.push(out);
                Ok(())
            }
            Expr::Binary { op, left, right } => {
                self.eval_expr(left, stack, module, span)?;
                self.eval_expr(right, stack, module, span)?;
                let r = self.pop(stack, span, "operator")?;
                let l = self.pop(stack, span, "operator")?;
                let out = self.eval_bin(*op, l, r, span)?;
                stack.push(out);
                Ok(())
            }
            Expr::ApplyUn(op) => {
                let v = self.pop(stack, span, "unary")?;
                self.eval_un(*op, v, stack, span)
            }
            Expr::Unary { op, operand } => {
                self.eval_expr(operand, stack, module, span)?;
                let v = self.pop(stack, span, "unary")?;
                self.eval_un(*op, v, stack, span)
            }
            Expr::StackOp(op) => self.eval_stack_op(*op, stack, span),
            Expr::Builtin { name } => self.eval_builtin(name, stack, span),
            Expr::Call { target } => self.eval_call(target, stack, module, span),
            Expr::Variable { .. } | Expr::Member { .. } => Err(InterpretError::unsupported(
                "bare name/member as a value (use `call`)",
                span,
            )),
            _ => Err(InterpretError::unsupported(
                format!("expression {expr:?}"),
                span,
            )),
        }
    }

    fn eval_call(
        &mut self,
        target: &Expr,
        stack: &mut Vec<Value>,
        module: Option<&str>,
        span: Span,
    ) -> IResult<()> {
        let name = self.resolve_call_name(target, module, span)?;
        let entry = self.funcs.get(&name).cloned().ok_or_else(|| {
            InterpretError::new(format!("unknown function '{name}'"), span, "E330")
        })?;
        let n = entry.function.params.len();
        let arg_start = stack.len().saturating_sub(n);
        let mut to_free = Vec::new();
        for v in &stack[arg_start..] {
            if let Value::Str {
                handle,
                owned: true,
            } = v
            {
                to_free.push(*handle);
            }
        }
        for v in &mut stack[arg_start..] {
            if let Value::Str { owned, .. } = v {
                *owned = false;
            }
        }
        self.call_function(&entry, stack)?;
        for handle in to_free {
            free_value(handle, KIND_STRING);
        }
        Ok(())
    }

    fn resolve_call_name(
        &self,
        target: &Expr,
        module: Option<&str>,
        span: Span,
    ) -> IResult<String> {
        match target {
            Expr::Variable { name } => {
                if let Some(mod_path) = module {
                    let fq = format!("{mod_path}::{name}");
                    if self.funcs.contains_key(&fq) {
                        return Ok(fq);
                    }
                }
                if let Some(plain) = self.plain_funcs.get(name) {
                    return Ok(plain.clone());
                }
                Ok(name.clone())
            }
            Expr::Member { base, member } => {
                if let Expr::Variable { name } = base.as_ref()
                    && let Some(path) = self.aliases.get(name)
                {
                    return Ok(format!("{path}::{member}"));
                }
                Err(InterpretError::unsupported(
                    "method call / complex call target",
                    span,
                ))
            }
            _ => Err(InterpretError::new(
                "'call' target must be a function name",
                span,
                "E329",
            )),
        }
    }

    fn eval_builtin(&mut self, name: &str, stack: &mut Vec<Value>, span: Span) -> IResult<()> {
        match name {
            "print" => {
                let v = self.pop(stack, span, "@print")?;
                match v {
                    Value::Str { handle, owned } => {
                        runtime::yarrow_print_str(handle);
                        if owned {
                            free_value(handle, KIND_STRING);
                        }
                    }
                    _ => {
                        return Err(InterpretError::new(
                            "'@print' requires a string",
                            span,
                            "E372",
                        ));
                    }
                }
                Ok(())
            }
            "print_newline" => {
                runtime::yarrow_print_newline();
                Ok(())
            }
            "print_int" => {
                let v = self.pop(stack, span, "@print_int")?;
                let n = match v {
                    Value::Int(n) => n,
                    Value::Bool(b) => b as i64,
                    _ => {
                        return Err(InterpretError::new(
                            "'@print_int' requires an integer",
                            span,
                            "E372",
                        ));
                    }
                };
                runtime::yarrow_print_int(n);
                Ok(())
            }
            other => Err(InterpretError::unsupported(
                format!("builtin @{other}"),
                span,
            )),
        }
    }

    fn eval_un(&self, op: UnOp, v: Value, stack: &mut Vec<Value>, span: Span) -> IResult<()> {
        match op {
            UnOp::Not => match v {
                Value::Bool(b) => stack.push(Value::Bool(!b)),
                Value::Int(n) => stack.push(Value::Int(!n)),
                _ => {
                    return Err(InterpretError::new(
                        "'not' requires bool or integer",
                        span,
                        "E336",
                    ));
                }
            },
        }
        Ok(())
    }

    fn eval_stack_op(&mut self, op: StackOp, stack: &mut Vec<Value>, span: Span) -> IResult<()> {
        match op {
            StackOp::Dup => {
                let v = self.peek(stack, span)?.clone();
                // dup of owned string: borrow semantics (don't double-own).
                let v = match v {
                    Value::Str { handle, .. } => Value::Str {
                        handle,
                        owned: false,
                    },
                    other => other,
                };
                stack.push(v);
                Ok(())
            }
            StackOp::Swap => {
                if stack.len() < 2 {
                    return Err(InterpretError::new("swap requires 2 values", span, "E362"));
                }
                let n = stack.len();
                stack.swap(n - 1, n - 2);
                Ok(())
            }
            StackOp::Rot => {
                if stack.len() < 3 {
                    return Err(InterpretError::new("rot requires 3 values", span, "E362"));
                }
                let c = stack.pop().unwrap();
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(b);
                stack.push(c);
                stack.push(a);
                Ok(())
            }
            StackOp::Unrot => {
                if stack.len() < 3 {
                    return Err(InterpretError::new("unrot requires 3 values", span, "E362"));
                }
                let c = stack.pop().unwrap();
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(c);
                stack.push(a);
                stack.push(b);
                Ok(())
            }
            StackOp::Pop => {
                let v = self.pop(stack, span, "pop")?;
                v.drop_owned();
                Ok(())
            }
            StackOp::Drop => {
                while let Some(v) = stack.pop() {
                    v.drop_owned();
                }
                Ok(())
            }
        }
    }

    fn eval_bin(&mut self, op: BinOp, l: Value, r: Value, span: Span) -> IResult<Value> {
        use BinOp::*;
        if op == Concat {
            let (lh, lo) = match l {
                Value::Str { handle, owned } => (handle, owned),
                _ => {
                    return Err(InterpretError::new(
                        "'~' requires string operands",
                        span,
                        "E335",
                    ));
                }
            };
            let (rh, ro) = match r {
                Value::Str { handle, owned } => (handle, owned),
                _ => {
                    return Err(InterpretError::new(
                        "'~' requires string operands",
                        span,
                        "E335",
                    ));
                }
            };
            let out = runtime::yarrow_str_join(lh, rh);
            if lo {
                free_value(lh, KIND_STRING);
            }
            if ro {
                free_value(rh, KIND_STRING);
            }
            return Ok(Value::Str {
                handle: out,
                owned: true,
            });
        }

        // Numeric / bool ops.
        match (&l, &r, op) {
            (Value::Float(_), Value::Float(_), _)
            | (Value::Float(_), Value::Int(_), _)
            | (Value::Int(_), Value::Float(_), _) => {
                let a = match l {
                    Value::Float(f) => f,
                    Value::Int(n) => n as f64,
                    _ => unreachable!(),
                };
                let b = match r {
                    Value::Float(f) => f,
                    Value::Int(n) => n as f64,
                    _ => unreachable!(),
                };
                self.eval_float_bin(op, a, b, span)
            }
            (Value::Int(a), Value::Int(b), Div) => Ok(Value::Float((*a as f64) / (*b as f64))),
            (Value::Int(a), Value::Int(b), _) => self.eval_int_bin(op, *a, *b, span),
            (Value::Bool(a), Value::Bool(b), _) => self.eval_bool_bin(op, *a, *b, span),
            (Value::Bool(a), Value::Int(b), And | Or | Xor) => {
                self.eval_int_bin(op, *a as i64, *b, span)
            }
            (Value::Int(a), Value::Bool(b), And | Or | Xor) => {
                self.eval_int_bin(op, *a, *b as i64, span)
            }
            _ => Err(InterpretError::new(
                format!("incompatible operands for {op:?}"),
                span,
                "E333",
            )),
        }
    }

    fn eval_int_bin(&self, op: BinOp, a: i64, b: i64, span: Span) -> IResult<Value> {
        use BinOp::*;
        Ok(match op {
            Plus => Value::Int(a.wrapping_add(b)),
            Minus => Value::Int(a.wrapping_sub(b)),
            Mul => Value::Int(a.wrapping_mul(b)),
            Mod => Value::Int(a.wrapping_rem(b)),
            Fdiv => Value::Int(a.wrapping_div(b)),
            Pow => {
                if b < 0 {
                    return Err(InterpretError::new(
                        "negative exponent in integer '^'",
                        span,
                        "E334",
                    ));
                }
                Value::Int(int_pow(a, b as u32))
            }
            Div => Value::Float((a as f64) / (b as f64)),
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            Gt => Value::Bool(a > b),
            Gte => Value::Bool(a >= b),
            Lt => Value::Bool(a < b),
            Lte => Value::Bool(a <= b),
            And => Value::Int(a & b),
            Or => Value::Int(a | b),
            Xor => Value::Int(a ^ b),
            Lshift => Value::Int(a.wrapping_shl(b as u32)),
            Rshift => Value::Int(a.wrapping_shr(b as u32)),
            Concat => unreachable!(),
        })
    }

    fn eval_float_bin(&self, op: BinOp, a: f64, b: f64, span: Span) -> IResult<Value> {
        use BinOp::*;
        Ok(match op {
            Plus => Value::Float(a + b),
            Minus => Value::Float(a - b),
            Mul => Value::Float(a * b),
            Div => Value::Float(a / b),
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            Gt => Value::Bool(a > b),
            Gte => Value::Bool(a >= b),
            Lt => Value::Bool(a < b),
            Lte => Value::Bool(a <= b),
            Mod | Pow => {
                let sym = if matches!(op, Mod) { "%" } else { "^" };
                let what = if matches!(op, Mod) {
                    "remainder"
                } else {
                    "exponentiation"
                };
                return Err(InterpretError::new(
                    format!("'{sym}' is integer {what}, not defined on floats"),
                    span,
                    "E334",
                ));
            }
            Fdiv => {
                return Err(InterpretError::new(
                    "integer floor division '//' is not defined on floats",
                    span,
                    "E334",
                ));
            }
            And | Or | Xor | Lshift | Rshift | Concat => {
                return Err(InterpretError::new(
                    format!("invalid float op {op:?}"),
                    span,
                    "E333",
                ));
            }
        })
    }

    fn eval_bool_bin(&self, op: BinOp, a: bool, b: bool, span: Span) -> IResult<Value> {
        use BinOp::*;
        Ok(match op {
            And => Value::Bool(a & b),
            Or => Value::Bool(a | b),
            Xor => Value::Bool(a ^ b),
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            _ => {
                return Err(InterpretError::new(
                    format!("invalid bool op {op:?}"),
                    span,
                    "E336",
                ));
            }
        })
    }

    fn pop(&self, stack: &mut Vec<Value>, span: Span, what: &str) -> IResult<Value> {
        stack
            .pop()
            .ok_or_else(|| InterpretError::new(format!("missing operand for {what}"), span, "E362"))
    }

    fn peek<'a>(&self, stack: &'a [Value], span: Span) -> IResult<&'a Value> {
        stack
            .last()
            .ok_or_else(|| InterpretError::new("missing operand for dup", span, "E362"))
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

fn int_pow(base: i64, mut exp: u32) -> i64 {
    let mut result = 1i64;
    let mut b = base;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result.wrapping_mul(b);
        }
        b = b.wrapping_mul(b);
        exp >>= 1;
    }
    result
}
