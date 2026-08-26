//! AST interpreter for checked Yarrow programs (Stages 13b / 21).
//!
//! Design choice: **tree-walk** the checked AST with an explicit operand stack,
//! calling into [`crate::runtime`] for heap strings and printing. A stack
//! bytecode VM can replace this later without changing the Session surface
//! (`interpret_source` / [`EvalContext`]).
//!
//! ## Corpus coverage (Stage 21)
//!
//! Interprets cleanly (stdout matches JIT `run`):
//! - `docs/examples/valid/01_hello.yar`
//! - `docs/examples/valid/02_arithmetic_and_stack.yar`
//! - `docs/examples/valid/03_variables_and_typeof.yar`
//! - `docs/examples/valid/04_functions.yar`
//! - `docs/examples/valid/05_control_flow.yar`
//! - `docs/examples/valid/12_modules.yar`
//!
//! Supported surface: `require` + module / nested calls, literals, arithmetic /
//! comparisons / bool ops / shifts, string `~`, stack words, `@print` /
//! `@print_newline` / `@print_int`, variables / `set`, `if` / `match` (value) /
//! condition and array `for`, `typeof` / type values, array literals, `std.loop`
//! `value` / `index`, void `main` and simple scalar / string returns.
//!
//! Still out of scope (clear `E393`): structs / unions / enums as values,
//! regions / defer, unsafe / pointers, fallible `unwrap` / `handle`, lists /
//! maps, method calls on structs.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::compiler::CompileError;
use crate::compiler::RunResult;
use crate::compiler::modules::{ModuleLoader, RequiredModule};
use crate::diagnostics::Span;
use crate::parser::ast::{
    BinOp, Expr, Function, MatchCase, MatchCaseKind, Program, StackOp, Stmt, StmtKind, Type,
    TypeKind, UnOp, Visibility,
};
use crate::parser::literals::{decode_float_literal, decode_int_literal, decode_string_literal};
use crate::parser::parse;
use crate::runtime::{self, KIND_STRING, free_value};
use crate::tokenizer::Tokenizer;

/// Runtime value on the interpreter operand stack / in a local.
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
    /// Fixed array (Stage 21 iterable `for`).
    Array(Vec<Value>),
}

impl Value {
    fn drop_owned(self) {
        match self {
            Value::Str {
                handle,
                owned: true,
            } => free_value(handle, KIND_STRING),
            Value::Array(elems) => {
                for e in elems {
                    e.drop_owned();
                }
            }
            _ => {}
        }
    }

    fn clone_for_stack(&self) -> Value {
        match self {
            Value::Str { handle, .. } => Value::Str {
                handle: *handle,
                owned: false,
            },
            Value::Array(elems) => {
                Value::Array(elems.iter().map(|e| e.clone_for_stack()).collect())
            }
            other => other.clone(),
        }
    }
}

/// Operand-stack slot: value plus the runtime kind code used by `typeof`.
#[derive(Debug, Clone)]
struct Slot {
    value: Value,
    kind: u64,
}

impl Slot {
    fn drop_owned(self) {
        self.value.drop_owned();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Next,
    Return,
}

/// One registered function body (top-level, nested, or from a required module).
#[derive(Debug, Clone)]
struct FuncEntry {
    /// Fully-qualified name (`demo::add`, `std.io::write_line`, `main`).
    fq: String,
    module: Option<String>,
    function: Function,
}

struct LoopCtx {
    value: Option<Slot>,
    index: Option<i64>,
}

struct Frame {
    locals: HashMap<String, Slot>,
    loops: Vec<LoopCtx>,
}

/// Stack-machine interpreter over a checked AST.
pub struct Interpreter {
    loader: ModuleLoader,
    /// Fully-qualified name → entry.
    funcs: HashMap<String, FuncEntry>,
    /// Module alias → module path (`io` → `std.io`).
    aliases: HashMap<String, String>,
    /// Bare name → fq name for alias-less requires.
    plain_funcs: HashMap<String, String>,
    /// Alias → single exported item for item imports under a scope.
    item_aliases: HashMap<String, String>,
    /// Bare item imports recorded when the parent module was already loaded
    /// (e.g. `"std.math" math require` then `"std.math.sqrt" require`).
    extra_plain_items: Vec<(String, String)>,
    #[allow(dead_code)]
    public_funcs: std::collections::HashSet<String>,
    modules: Vec<RequiredModule>,
}

/// REPL-oriented wrapper around [`Interpreter`] (Stage 13b surface).
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
            item_aliases: HashMap::new(),
            extra_plain_items: Vec::new(),
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
        self.item_aliases.clear();
        self.extra_plain_items.clear();
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
        Ok(match stack.pop().unwrap().value {
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
            Value::Array(_) => {
                return Err(InterpretError::unsupported(
                    "array as entry return value",
                    Span::default(),
                ));
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
                let fq = match &module {
                    Some(path) => format!("{path}::{}", f.name),
                    None => f.name.clone(),
                };
                self.register_function_tree(module.clone(), fq, f)?;
            }
        }
        Ok(())
    }

    fn register_function_tree(
        &mut self,
        module: Option<String>,
        fq: String,
        f: &Function,
    ) -> IResult<()> {
        if matches!(f.visibility, Some(Visibility::Public)) {
            self.public_funcs.insert(fq.clone());
        }
        for stmt in &f.body {
            if let StmtKind::Function(nf) = &stmt.kind {
                let nested = format!("{fq}::{}", nf.name);
                self.register_function_tree(module.clone(), nested, nf)?;
            }
        }
        self.funcs.insert(
            fq.clone(),
            FuncEntry {
                fq,
                module,
                function: f.clone(),
            },
        );
        Ok(())
    }

    fn register_module_bindings(&mut self) -> IResult<()> {
        for m in &self.modules {
            if let Some(alias) = &m.alias {
                self.aliases.insert(alias.clone(), m.path.clone());
                if let Some(item) = &m.item {
                    self.item_aliases.insert(alias.clone(), item.clone());
                }
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
        for (module_path, item) in self.extra_plain_items.clone() {
            let fq = format!("{module_path}::{item}");
            self.plain_funcs.insert(item, fq);
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
            // A later bare item import still needs a plain-name binding even
            // when the module was already loaded under an alias.
            if alias.is_none()
                && let Some(item_name) = item
            {
                self.extra_plain_items.push((module_path, item_name));
            }
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

    fn call_function(&mut self, entry: &FuncEntry, stack: &mut Vec<Slot>) -> IResult<()> {
        let n = entry.function.params.len();
        if stack.len() < n {
            return Err(InterpretError::new(
                format!("call to '{}' requires {n} argument(s)", entry.function.name),
                Span::default(),
                "E331",
            ));
        }
        let args: Vec<Slot> = stack.split_off(stack.len() - n);
        let mut local_stack = args;
        let mut frame = Frame {
            locals: HashMap::new(),
            loops: Vec::new(),
        };
        let _flow = self.eval_body(
            &entry.function.body,
            &mut local_stack,
            &mut frame,
            entry.module.as_deref(),
            &entry.fq,
        )?;
        let ret_n = entry
            .function
            .returns
            .iter()
            .filter(|t| {
                !matches!(
                    t.kind,
                    TypeKind::Primitive(crate::parser::ast::Primitive::Void)
                )
            })
            .count();
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
            for (_, slot) in frame.locals.drain() {
                slot.drop_owned();
            }
            stack.extend(rets);
            return Ok(());
        }
        for (_, slot) in frame.locals.drain() {
            slot.drop_owned();
        }
        Ok(())
    }
    fn eval_body(
        &mut self,
        body: &[Stmt],
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
    ) -> IResult<Flow> {
        for stmt in body {
            match self.eval_stmt(stmt, stack, frame, module, caller_fq)? {
                Flow::Return => return Ok(Flow::Return),
                Flow::Next => {}
            }
        }
        Ok(Flow::Next)
    }

    fn eval_stmt(
        &mut self,
        stmt: &Stmt,
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
    ) -> IResult<Flow> {
        match &stmt.kind {
            StmtKind::Expr(e) => {
                self.eval_expr(e, stack, frame, module, caller_fq, stmt.span)?;
                Ok(Flow::Next)
            }
            StmtKind::Require { .. } | StmtKind::Function(_) => Ok(Flow::Next),
            StmtKind::Struct(_)
            | StmtKind::Implement(_)
            | StmtKind::Enum(_)
            | StmtKind::Union(_)
            | StmtKind::Error(_) => Ok(Flow::Next),
            StmtKind::VarDecl {
                name, ty, value, ..
            } => {
                let kind = type_kind_code(ty).ok_or_else(|| {
                    InterpretError::unsupported(format!("binding type {ty:?}"), stmt.span)
                })?;
                let slot = match value {
                    Some(e) => {
                        self.eval_expr(e, stack, frame, module, caller_fq, stmt.span)?;
                        let mut s = self.pop(stack, stmt.span, "value")?;
                        s.kind = kind;
                        s
                    }
                    None => {
                        let mut s = self.pop(stack, stmt.span, "value")?;
                        s.kind = kind;
                        s
                    }
                };
                if let Some(old) = frame.locals.insert(name.clone(), slot) {
                    old.drop_owned();
                }
                Ok(Flow::Next)
            }
            StmtKind::Set { target, value } => {
                let Expr::Variable { name } = target else {
                    return Err(InterpretError::unsupported(
                        "complex 'set' target",
                        stmt.span,
                    ));
                };
                if let Some(e) = value {
                    self.eval_expr(e, stack, frame, module, caller_fq, stmt.span)?;
                }
                let mut slot = self.pop(stack, stmt.span, "set")?;
                let kind = frame.locals.get(name).map(|s| s.kind).ok_or_else(|| {
                    InterpretError::new(format!("unknown variable '{name}'"), stmt.span, "E320")
                })?;
                slot.kind = kind;
                if let Some(old) = frame.locals.insert(name.clone(), slot) {
                    old.drop_owned();
                }
                Ok(Flow::Next)
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let pre = stack.len();
                let cond = self.eval_cond(condition, stack, frame, module, caller_fq, stmt.span)?;
                while stack.len() > pre {
                    stack.pop().unwrap().drop_owned();
                }
                let branch = if cond { then_branch } else { else_branch };
                self.eval_body(branch, stack, frame, module, caller_fq)
            }
            StmtKind::Match {
                value,
                cases,
                else_branch,
            } => self.eval_match(
                value,
                cases,
                else_branch,
                stack,
                frame,
                module,
                caller_fq,
                stmt.span,
            ),
            StmtKind::For { source, body } => {
                if for_source_is_condition(source) {
                    self.eval_cond_for(source, body, stack, frame, module, caller_fq, stmt.span)
                } else {
                    self.eval_iter_for(source, body, stack, frame, module, caller_fq, stmt.span)
                }
            }
            StmtKind::Return { value } => {
                if let Some(v) = value {
                    self.eval_expr(v, stack, frame, module, caller_fq, stmt.span)?;
                }
                Ok(Flow::Return)
            }
            other => Err(InterpretError::unsupported(
                format!("statement {other:?}"),
                stmt.span,
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn eval_match(
        &mut self,
        value: &Expr,
        cases: &[MatchCase],
        else_branch: &[Stmt],
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
        span: Span,
    ) -> IResult<Flow> {
        let pre = stack.len();
        let has_subject = !matches!(value, Expr::Variable { name } if name.is_empty());
        if has_subject {
            self.eval_expr(value, stack, frame, module, caller_fq, span)?;
        }
        let sub_len = stack.len();

        for case in cases {
            match &case.kind {
                MatchCaseKind::Condition(cond) => {
                    while stack.len() > sub_len {
                        stack.pop().unwrap().drop_owned();
                    }
                    if stack.len() < sub_len {
                        return Err(InterpretError::new("match subject missing", span, "E343"));
                    }
                    let truthy =
                        self.eval_match_cond(cond, stack, frame, module, caller_fq, span)?;
                    if truthy {
                        while stack.len() > sub_len {
                            stack.pop().unwrap().drop_owned();
                        }
                        let flow = self.eval_body(&case.body, stack, frame, module, caller_fq)?;
                        let extras: Vec<Slot> = stack.split_off(sub_len);
                        while stack.len() > pre {
                            stack.pop().unwrap().drop_owned();
                        }
                        stack.extend(extras);
                        return Ok(flow);
                    }
                }
                MatchCaseKind::Type(_) => {
                    return Err(InterpretError::unsupported("type-dispatch match", span));
                }
            }
        }

        while stack.len() > sub_len {
            stack.pop().unwrap().drop_owned();
        }
        let flow = self.eval_body(else_branch, stack, frame, module, caller_fq)?;
        let extras: Vec<Slot> = stack.split_off(sub_len);
        while stack.len() > pre {
            stack.pop().unwrap().drop_owned();
        }
        stack.extend(extras);
        Ok(flow)
    }

    #[allow(clippy::too_many_arguments)]
    fn eval_cond_for(
        &mut self,
        condition: &Expr,
        body: &[Stmt],
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
        span: Span,
    ) -> IResult<Flow> {
        let pre = stack.len();
        loop {
            while stack.len() > pre {
                stack.pop().unwrap().drop_owned();
            }
            let cond = self.eval_cond(condition, stack, frame, module, caller_fq, span)?;
            while stack.len() > pre {
                stack.pop().unwrap().drop_owned();
            }
            if !cond {
                break;
            }
            frame.loops.push(LoopCtx {
                value: None,
                index: None,
            });
            let flow = self.eval_body(body, stack, frame, module, caller_fq)?;
            frame.loops.pop();
            if flow == Flow::Return {
                return Ok(Flow::Return);
            }
            if stack.len() != pre {
                return Err(InterpretError::new(
                    "for body must leave the stack balanced",
                    span,
                    "E325",
                ));
            }
        }
        Ok(Flow::Next)
    }

    #[allow(clippy::too_many_arguments)]
    fn eval_iter_for(
        &mut self,
        source: &Expr,
        body: &[Stmt],
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
        span: Span,
    ) -> IResult<Flow> {
        let pre = stack.len();
        self.eval_expr(source, stack, frame, module, caller_fq, span)?;
        let iterable = self.pop(stack, span, "'for' iterable")?;
        let elems = match iterable.value {
            Value::Array(elems) => elems,
            other => {
                other.drop_owned();
                return Err(InterpretError::unsupported("non-array iterable for", span));
            }
        };
        for (i, elem) in elems.into_iter().enumerate() {
            while stack.len() > pre {
                stack.pop().unwrap().drop_owned();
            }
            let elem_slot = Slot {
                kind: infer_value_kind(&elem),
                value: elem,
            };
            frame.loops.push(LoopCtx {
                value: Some(elem_slot),
                index: Some(i as i64),
            });
            let flow = self.eval_body(body, stack, frame, module, caller_fq)?;
            if let Some(ctx) = frame.loops.pop()
                && let Some(v) = ctx.value
            {
                v.drop_owned();
            }
            if flow == Flow::Return {
                return Ok(Flow::Return);
            }
            if stack.len() != pre {
                return Err(InterpretError::new(
                    "for body must leave the stack balanced",
                    span,
                    "E325",
                ));
            }
        }
        Ok(Flow::Next)
    }

    fn eval_cond(
        &mut self,
        e: &Expr,
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
        span: Span,
    ) -> IResult<bool> {
        let before = stack.len();
        self.eval_expr(e, stack, frame, module, caller_fq, span)?;
        if stack.len() != before + 1 {
            return Err(InterpretError::new(
                "condition must evaluate to a single value",
                span,
                "E324",
            ));
        }
        let slot = stack.pop().unwrap();
        match slot.value {
            Value::Bool(b) => Ok(b),
            other => {
                other.drop_owned();
                Err(InterpretError::new("condition must be bool", span, "E324"))
            }
        }
    }

    fn eval_match_cond(
        &mut self,
        e: &Expr,
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
        span: Span,
    ) -> IResult<bool> {
        self.eval_expr(e, stack, frame, module, caller_fq, span)?;
        let slot = self.pop(stack, span, "match condition")?;
        match slot.value {
            Value::Bool(b) => Ok(b),
            Value::Int(n) => Ok(n != 0),
            other => {
                other.drop_owned();
                Err(InterpretError::new(
                    "match condition must be bool or integer",
                    span,
                    "E324",
                ))
            }
        }
    }
    fn eval_expr(
        &mut self,
        expr: &Expr,
        stack: &mut Vec<Slot>,
        frame: &mut Frame,
        module: Option<&str>,
        caller_fq: &str,
        span: Span,
    ) -> IResult<()> {
        match expr {
            Expr::Integer { value } => {
                let n =
                    decode_int_literal(value).map_err(|m| InterpretError::new(m, span, "E363"))?;
                let kind = int_literal_kind(n);
                stack.push(Slot {
                    value: Value::Int(n as i64),
                    kind,
                });
                Ok(())
            }
            Expr::Float { value } => {
                let f = decode_float_literal(value)
                    .map_err(|m| InterpretError::new(m, span, "E363"))?;
                let kind = float_literal_kind(f);
                stack.push(Slot {
                    value: Value::Float(f),
                    kind,
                });
                Ok(())
            }
            Expr::Bool { value } => {
                stack.push(Slot {
                    value: Value::Bool(*value),
                    kind: 0,
                });
                Ok(())
            }
            Expr::String { value } => {
                let bytes = decode_string_literal(value)
                    .map_err(|m| InterpretError::new(m, span, "E363"))?;
                let handle = runtime::yarrow_str_new(bytes.as_ptr() as u64, bytes.len() as u64);
                stack.push(Slot {
                    value: Value::Str {
                        handle,
                        owned: true,
                    },
                    kind: 16,
                });
                Ok(())
            }
            Expr::Array(elems) => {
                let mut out = Vec::with_capacity(elems.len());
                for e in elems {
                    self.eval_expr(e, stack, frame, module, caller_fq, span)?;
                    out.push(self.pop(stack, span, "array element")?.value);
                }
                let count = out.len() as u64;
                let elem_kind = out.first().map(infer_value_kind).unwrap_or(4);
                let kind = 0x60 | (elem_kind << 8) | (count << 40);
                stack.push(Slot {
                    value: Value::Array(out),
                    kind,
                });
                Ok(())
            }
            Expr::Variable { name } => {
                let local = frame.locals.get(name).ok_or_else(|| {
                    InterpretError::new(format!("unknown variable '{name}'"), span, "E320")
                })?;
                stack.push(Slot {
                    value: local.value.clone_for_stack(),
                    kind: local.kind,
                });
                Ok(())
            }
            Expr::Member { base, member } => {
                if let Expr::Variable { name } = base.as_ref()
                    && self.is_std_loop_alias(name)
                {
                    let loop_ctx = frame.loops.last().ok_or_else(|| {
                        InterpretError::new("loop.value / loop.index outside a for", span, "E393")
                    })?;
                    match member.as_str() {
                        "value" => {
                            let slot = loop_ctx.value.as_ref().ok_or_else(|| {
                                InterpretError::new(
                                    "loop.value is not available in this for",
                                    span,
                                    "E393",
                                )
                            })?;
                            stack.push(Slot {
                                value: slot.value.clone_for_stack(),
                                kind: slot.kind,
                            });
                            return Ok(());
                        }
                        "index" => {
                            let idx = loop_ctx.index.ok_or_else(|| {
                                InterpretError::new(
                                    "loop.index is not available in this for",
                                    span,
                                    "E393",
                                )
                            })?;
                            stack.push(Slot {
                                value: Value::Int(idx),
                                kind: 4,
                            });
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                Err(InterpretError::unsupported(
                    "bare name/member as a value (use `call`)",
                    span,
                ))
            }
            Expr::TypeValue { name } => {
                let kind = primitive_kind_code(name).ok_or_else(|| {
                    InterpretError::new(format!("unknown type value '{name}'"), span, "E302")
                })?;
                stack.push(Slot {
                    value: Value::Int(kind as i64),
                    kind: 4,
                });
                Ok(())
            }
            Expr::Typeof { inner } => {
                self.eval_expr(inner, stack, frame, module, caller_fq, span)?;
                self.apply_typeof(stack, span)
            }
            Expr::ApplyTypeof => self.apply_typeof(stack, span),
            Expr::Seq(elems) => {
                for (e, s) in elems {
                    self.eval_expr(e, stack, frame, module, caller_fq, *s)?;
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
                self.eval_expr(left, stack, frame, module, caller_fq, span)?;
                self.eval_expr(right, stack, frame, module, caller_fq, span)?;
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
                self.eval_expr(operand, stack, frame, module, caller_fq, span)?;
                let v = self.pop(stack, span, "unary")?;
                self.eval_un(*op, v, stack, span)
            }
            Expr::StackOp(op) => self.eval_stack_op(*op, stack, span),
            Expr::Builtin { name } => self.eval_builtin(name, stack, span),
            Expr::Call { target } => self.eval_call(target, stack, module, caller_fq, span),
            _ => Err(InterpretError::unsupported(
                format!("expression {expr:?}"),
                span,
            )),
        }
    }

    fn apply_typeof(&self, stack: &mut Vec<Slot>, span: Span) -> IResult<()> {
        let slot = self.pop(stack, span, "'typeof'")?;
        let code = slot.kind;
        slot.drop_owned();
        stack.push(Slot {
            value: Value::Int(code as i64),
            kind: 4,
        });
        Ok(())
    }

    fn eval_call(
        &mut self,
        target: &Expr,
        stack: &mut Vec<Slot>,
        module: Option<&str>,
        caller_fq: &str,
        span: Span,
    ) -> IResult<()> {
        let name = self.resolve_call_name(target, module, caller_fq, span)?;
        let entry = self.funcs.get(&name).cloned().ok_or_else(|| {
            InterpretError::new(format!("unknown function '{name}'"), span, "E330")
        })?;
        let n = entry.function.params.len();
        let arg_start = stack.len().saturating_sub(n);
        let mut to_free = Vec::new();
        for s in &stack[arg_start..] {
            if let Value::Str {
                handle,
                owned: true,
            } = &s.value
            {
                to_free.push(*handle);
            }
        }
        for s in &mut stack[arg_start..] {
            if let Value::Str { owned, .. } = &mut s.value {
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
        caller_fq: &str,
        span: Span,
    ) -> IResult<String> {
        match target {
            Expr::Variable { name } => {
                let nested = format!("{caller_fq}::{name}");
                if self.funcs.contains_key(&nested) {
                    return Ok(nested);
                }
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
                    if let Some(item) = self.item_aliases.get(name)
                        && item != member
                    {
                        return Err(InterpretError::new(
                            format!("module '{path}' only exports '{item}' (not '{member}')"),
                            span,
                            "E330",
                        ));
                    }
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

    fn is_std_loop_alias(&self, alias: &str) -> bool {
        self.aliases.get(alias).is_some_and(|p| p == "std.loop")
    }

    fn eval_builtin(&mut self, name: &str, stack: &mut Vec<Slot>, span: Span) -> IResult<()> {
        match name {
            "print" => {
                let v = self.pop(stack, span, "@print")?;
                match v.value {
                    Value::Str { handle, owned } => {
                        runtime::yarrow_print_str(handle);
                        if owned {
                            free_value(handle, KIND_STRING);
                        }
                    }
                    other => {
                        other.drop_owned();
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
                let n = match v.value {
                    Value::Int(n) => n,
                    Value::Bool(b) => b as i64,
                    other => {
                        other.drop_owned();
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

    fn eval_un(&self, op: UnOp, v: Slot, stack: &mut Vec<Slot>, span: Span) -> IResult<()> {
        match op {
            UnOp::Not => match v.value {
                Value::Bool(b) => stack.push(Slot {
                    value: Value::Bool(!b),
                    kind: 0,
                }),
                Value::Int(n) => stack.push(Slot {
                    value: Value::Int(!n),
                    kind: v.kind,
                }),
                other => {
                    other.drop_owned();
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

    fn eval_stack_op(&mut self, op: StackOp, stack: &mut Vec<Slot>, span: Span) -> IResult<()> {
        match op {
            StackOp::Dup => {
                let v = self.peek(stack, span)?;
                let cloned = Slot {
                    value: v.value.clone_for_stack(),
                    kind: v.kind,
                };
                stack.push(cloned);
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
    fn eval_bin(&mut self, op: BinOp, l: Slot, r: Slot, span: Span) -> IResult<Slot> {
        use BinOp::*;
        if op == Concat {
            let (lh, lo) = match l.value {
                Value::Str { handle, owned } => (handle, owned),
                other => {
                    other.drop_owned();
                    r.drop_owned();
                    return Err(InterpretError::new(
                        "'~' requires string operands",
                        span,
                        "E335",
                    ));
                }
            };
            let (rh, ro) = match r.value {
                Value::Str { handle, owned } => (handle, owned),
                other => {
                    other.drop_owned();
                    if lo {
                        free_value(lh, KIND_STRING);
                    }
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
            return Ok(Slot {
                value: Value::Str {
                    handle: out,
                    owned: true,
                },
                kind: 16,
            });
        }

        match (&l.value, &r.value, op) {
            (Value::Float(_), Value::Float(_), _)
            | (Value::Float(_), Value::Int(_), _)
            | (Value::Int(_), Value::Float(_), _) => {
                let a = match l.value {
                    Value::Float(f) => f,
                    Value::Int(n) => n as f64,
                    _ => unreachable!(),
                };
                let b = match r.value {
                    Value::Float(f) => f,
                    Value::Int(n) => n as f64,
                    _ => unreachable!(),
                };
                self.eval_float_bin(op, a, b, span)
            }
            (Value::Int(a), Value::Int(b), Div) => Ok(Slot {
                value: Value::Float((*a as f64) / (*b as f64)),
                kind: 14,
            }),
            (Value::Int(a), Value::Int(b), _) => self.eval_int_bin(op, *a, *b, l.kind, span),
            (Value::Bool(a), Value::Bool(b), _) => self.eval_bool_bin(op, *a, *b, span),
            (Value::Bool(a), Value::Int(b), And | Or | Xor) => {
                self.eval_int_bin(op, *a as i64, *b, 4, span)
            }
            (Value::Int(a), Value::Bool(b), And | Or | Xor) => {
                self.eval_int_bin(op, *a, *b as i64, l.kind, span)
            }
            _ => {
                l.drop_owned();
                r.drop_owned();
                Err(InterpretError::new(
                    format!("incompatible operands for {op:?}"),
                    span,
                    "E333",
                ))
            }
        }
    }

    fn eval_int_bin(&self, op: BinOp, a: i64, b: i64, kind: u64, span: Span) -> IResult<Slot> {
        use BinOp::*;
        Ok(match op {
            Plus => Slot {
                value: Value::Int(a.wrapping_add(b)),
                kind,
            },
            Minus => Slot {
                value: Value::Int(a.wrapping_sub(b)),
                kind,
            },
            Mul => Slot {
                value: Value::Int(a.wrapping_mul(b)),
                kind,
            },
            Mod => Slot {
                value: Value::Int(a.wrapping_rem(b)),
                kind,
            },
            Fdiv => Slot {
                value: Value::Int(a.wrapping_div(b)),
                kind,
            },
            Pow => {
                if b < 0 {
                    return Err(InterpretError::new(
                        "negative exponent in integer '^'",
                        span,
                        "E334",
                    ));
                }
                Slot {
                    value: Value::Int(int_pow(a, b as u32)),
                    kind,
                }
            }
            Div => Slot {
                value: Value::Float((a as f64) / (b as f64)),
                kind: 14,
            },
            Eq => Slot {
                value: Value::Bool(a == b),
                kind: 0,
            },
            Ne => Slot {
                value: Value::Bool(a != b),
                kind: 0,
            },
            Gt => Slot {
                value: Value::Bool(a > b),
                kind: 0,
            },
            Gte => Slot {
                value: Value::Bool(a >= b),
                kind: 0,
            },
            Lt => Slot {
                value: Value::Bool(a < b),
                kind: 0,
            },
            Lte => Slot {
                value: Value::Bool(a <= b),
                kind: 0,
            },
            And => Slot {
                value: Value::Int(a & b),
                kind,
            },
            Or => Slot {
                value: Value::Int(a | b),
                kind,
            },
            Xor => Slot {
                value: Value::Int(a ^ b),
                kind,
            },
            Lshift => Slot {
                value: Value::Int(a.wrapping_shl(b as u32)),
                kind,
            },
            Rshift => Slot {
                value: Value::Int(a.wrapping_shr(b as u32)),
                kind,
            },
            Concat => unreachable!(),
        })
    }

    fn eval_float_bin(&self, op: BinOp, a: f64, b: f64, span: Span) -> IResult<Slot> {
        use BinOp::*;
        Ok(match op {
            Plus => Slot {
                value: Value::Float(a + b),
                kind: 14,
            },
            Minus => Slot {
                value: Value::Float(a - b),
                kind: 14,
            },
            Mul => Slot {
                value: Value::Float(a * b),
                kind: 14,
            },
            Div => Slot {
                value: Value::Float(a / b),
                kind: 14,
            },
            Eq => Slot {
                value: Value::Bool(a == b),
                kind: 0,
            },
            Ne => Slot {
                value: Value::Bool(a != b),
                kind: 0,
            },
            Gt => Slot {
                value: Value::Bool(a > b),
                kind: 0,
            },
            Gte => Slot {
                value: Value::Bool(a >= b),
                kind: 0,
            },
            Lt => Slot {
                value: Value::Bool(a < b),
                kind: 0,
            },
            Lte => Slot {
                value: Value::Bool(a <= b),
                kind: 0,
            },
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

    fn eval_bool_bin(&self, op: BinOp, a: bool, b: bool, span: Span) -> IResult<Slot> {
        use BinOp::*;
        Ok(match op {
            And => Slot {
                value: Value::Bool(a & b),
                kind: 0,
            },
            Or => Slot {
                value: Value::Bool(a | b),
                kind: 0,
            },
            Xor => Slot {
                value: Value::Bool(a ^ b),
                kind: 0,
            },
            Eq => Slot {
                value: Value::Bool(a == b),
                kind: 0,
            },
            Ne => Slot {
                value: Value::Bool(a != b),
                kind: 0,
            },
            _ => {
                return Err(InterpretError::new(
                    format!("invalid bool op {op:?}"),
                    span,
                    "E336",
                ));
            }
        })
    }

    fn pop(&self, stack: &mut Vec<Slot>, span: Span, what: &str) -> IResult<Slot> {
        stack
            .pop()
            .ok_or_else(|| InterpretError::new(format!("missing operand for {what}"), span, "E362"))
    }

    fn peek<'a>(&self, stack: &'a [Slot], span: Span) -> IResult<&'a Slot> {
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

/// Runtime kind codes aligned with `compiler::types::kind_code` for primitives.
fn primitive_kind_code(name: &str) -> Option<u64> {
    Some(match name {
        "bool" => 0,
        "i8" => 1,
        "i16" => 2,
        "i32" => 3,
        "i64" => 4,
        "u8" => 6,
        "u16" => 7,
        "u32" => 8,
        "u64" => 9,
        "rune" => 11,
        "f16" => 12,
        "f32" => 13,
        "f64" => 14,
        "string" => 16,
        _ => return None,
    })
}

fn type_kind_code(ty: &Type) -> Option<u64> {
    match &ty.kind {
        TypeKind::Primitive(p) => {
            use crate::parser::ast::Primitive::*;
            Some(match p {
                Bool => 0,
                I8 => 1,
                I16 => 2,
                I32 => 3,
                I64 => 4,
                U8 => 6,
                U16 => 7,
                U32 => 8,
                U64 => 9,
                Rune => 11,
                F16 => 12,
                F32 => 13,
                F64 => 14,
                String => 16,
                _ => return None,
            })
        }
        TypeKind::Array { element, size } => {
            let elem = type_kind_code(element)?;
            let count = size.unwrap_or(0);
            Some(0x60 | (elem << 8) | (count << 40))
        }
        TypeKind::Named(name) => primitive_kind_code(name),
        _ => None,
    }
}

fn int_literal_kind(n: i128) -> u64 {
    if n >= 0 {
        if n <= u8::MAX as i128 {
            6
        } else if n <= u16::MAX as i128 {
            7
        } else if n <= u32::MAX as i128 {
            8
        } else {
            9
        }
    } else if n >= i8::MIN as i128 {
        1
    } else if n >= i16::MIN as i128 {
        2
    } else if n >= i32::MIN as i128 {
        3
    } else {
        4
    }
}

fn float_literal_kind(n: f64) -> u64 {
    if n as f32 as f64 == n && n.is_finite() {
        13
    } else {
        14
    }
}

fn infer_value_kind(v: &Value) -> u64 {
    match v {
        Value::Bool(_) => 0,
        Value::Int(_) => 4,
        Value::Float(_) => 14,
        Value::Str { .. } => 16,
        Value::Array(elems) => {
            let count = elems.len() as u64;
            let elem = elems.first().map(infer_value_kind).unwrap_or(4);
            0x60 | (elem << 8) | (count << 40)
        }
    }
}
