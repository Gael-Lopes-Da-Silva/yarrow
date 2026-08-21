# AST

Abstract syntax tree for Yarrow, derived from [`SYNTAX.md`](SYNTAX.md) and illustrated by [`GRAMMAR.md`](GRAMMAR.md).

Yarrow is postfix and stack-based. Control-flow and declarations form a conventional tree. Stack terms (`word` in the grammar) are kept as flat expression nodes: they describe stack effects, not nested binary trees of operands.

Notation:

- Algebraic datatypes in a Rust-like sketch
- `Vec<T>` for ordered repetition
- `Option<T>` for optional children
- Spans / source locations are assumed on every node and omitted below

```
Program
├── Expression   (words / stack terms)
├── Statement
├── Type
└── Pattern      (match cases)
```

---

## Program

A program is a sequence of top-level items with exactly one `main` function. `main` may appear anywhere among them; the AST stores it as a distinguished field so entry is explicit.

```rust
Program {
    items: Vec<TopLevel>,
    main: MainFunction,
}

TopLevel =
      Require
    | Function
    | Struct
    | Implement
    | Enum
    | Union
    | Error

MainFunction {
    body: Vec<Statement>,
    returns: Option<Type>,   // optional; integer/float return becomes the process exit code
}
```

`main` is the only entity public by default. It has no parameter list in surface syntax.

---

## Visibility and modifiers

```rust
Visibility = Public | Private

Mutability = Mutable | Const | Static

ParamModifier = Copy | Mutable   // on a parameter type
// Copy: deep-copy into the local stack
// Mutable: on reference<T>, requires a mutable pointee
```

Default visibility for user-declared entities and fields is private. `main` is public by default.

---

## Type

Types appear in declarations, parameters, return clauses, `typeof` comparisons, and union match cases.

```rust
Type =
      Primitive(Primitive)
    | Named(QualifiedName)           // user type or scoped name
    | Array { element: Type, size: Option<u64> }
    | List { element: Type }
    | Hashmap { key: Type, value: Type }
    | Pointer { inner: Type }
    | Reference { inner: Type }
    | UnionLiteral(Vec<Type>)        // |T U ...| anonymous union

Primitive =
      Void | Bool | String | Rune
    | I8 | I16 | I32 | I64
    | U8 | U16 | U32 | U64
    | F16 | F32 | F64

QualifiedName {
    parts: Vec<Ident>,               // a.b.c
}
```

Notes from the grammar:

- Type arguments inside `<>` are whitespace-separated (`array<i32 3>`, `hashmap<string i32>`).
- Empty container literals carry no element type until a typed context (e.g. a variable declaration) supplies one.
- `pointer<T>` is a typed raw address at compile time; at runtime it is an address.
- There is no lifetime syntax on `reference<T>`; ownership and regions are the model.

---

## Expression

An expression is one stack term: a push, a name lookup, an operator, or a keyword op. Sequences of expressions are statements (`Statement::Word`) or appear inside container literals and match conditions.

```rust
Expression =
      Literal(Literal)
    | Container(Container)
    | Name(QualifiedName)            // variable, field path, scoped fn, enum member, ...
    | TypeValue(Type)                // type used as a value (typeof / comparisons)
    | Operator(Operator)
    | StackOp(StackOp)
    | MemoryOp(MemoryOp)
    | CallOp(CallOp)
    | Typeof                         // keyword `typeof`
```

### Literals

```rust
Literal =
      Integer { lexeme: String }     // decimal, 0b..., 0x...; underscores kept or normalized
    | Float { lexeme: String }
    | String { value: String }
    | Rune { value: String }
    | Bool { value: bool }
```

Integer literals take the smallest fitting integer type (positive → unsigned, negative → signed). Floats take the smallest fitting float. That assignment is a type-check concern, not a distinct AST variant.

### Containers

```rust
Container =
      List(Vec<Expression>)          // ( ... )
    | Array(Vec<Expression>)         // [ ... ]
    | Hashmap(Vec<(Expression, Expression)>)   // { k v ... } with literal keys
    | Struct(Vec<(Ident, Expression)>)         // { field value ... } with identifier keys
    | EmptyMapOrStruct               // {} needs a typed context
```

Surface `{ ... }` is one production. The AST distinguishes hashmap vs struct by key shape (literal vs identifier), matching the grammar comments.

### Operators and keyword ops

```rust
Operator =
      Arithmetic(ArithmeticOp)
    | Concat                               // `~`; string join, not overloaded `+`
    | Logical(LogicalOp)
    | Comparison(ComparisonOp)
    | Bitwise(BitwiseOp)

ArithmeticOp = Add | Sub | Mul | Div | FloorDiv | Mod | Pow
LogicalOp    = And | Or | Not
ComparisonOp = Eq | Ne | Gt | Lt | Ge | Le
BitwiseOp    = And | Or | Xor | Lshift | Rshift | Not
// `and` / `or` / `not` are overloaded: bool → logical, integer → bitwise
// `+` is arithmetic / pointer offset only; concatenation is `~`

StackOp  = Drop | Dup | Swap | Rot | Unrot | Pop
MemoryOp = Borrow | Move | Load | Store
CallOp   = Call | Unwrap
```

Stack effects (informal):

| Node                                                         | Effect                        |
| ------------------------------------------------------------ | ----------------------------- |
| Literal / Container / Name / TypeValue                       | push                          |
| binary Operator, Store, Move                                 | pop 2, push 0 or 1            |
| unary Operator, Typeof, Borrow, Load, Call, Unwrap, Dup, Pop | pop 1 (Call/Unwrap may push)  |
| Drop                                                         | clear stack / release borrows |
| Swap / Rot / Unrot                                           | rearrange                     |

Exact typing and ownership of these effects belong to the type and memory models.

---

## Statement

Statements are the sequenced body of functions, blocks, and top-level items that are not pure declarations.

```rust
Statement =
      Require(Require)
    | Function(Function)
    | VarDecl(VarDecl)
    | Assign(Assign)
    | If(If)
    | Match(Match)
    | For(For)
    | Defer(Defer)
    | Unsafe(Unsafe)
    | Handle(Handle)
    | Return
    | Word(Expression)               // one stack term as a statement
```

Nested `function` declarations are allowed inside bodies; they are only callable from that enclosing body.

### Require

```rust
Require {
    path: String,                    // string literal module path
    alias: Option<Ident>,            // omit → import into current scope
}
```

Surface form: `"path" [alias] require`.

### Function

```rust
Function {
    name: Ident,
    visibility: Option<Visibility>,
    is_unsafe: bool,
    params: Vec<Parameter>,
    body: Vec<Statement>,
    returns: Option<Type>,
}

Parameter {
    ty: Type,
    modifier: Option<ParamModifier>, // copy | mutable
}
```

Parameters are moved onto the local stack in declaration order (first declared = deepest). Body bindings such as `name const Type` pop from the top, so the last parameter binds first.

`unsafe function` marks the function as unsafe to call. Call sites still need an `unsafe` block. Unsafe does not disable borrow or ownership checking.

### Variables and assignment

```rust
VarDecl {
    target: LValue,                  // value already on the stack
    mutability: Mutability,
    ty: Type,
}

Assign {
    target: LValue,                  // new value already on the stack
}

LValue = QualifiedName               // name or field path (e.g. point.x)
```

- `mutable` / `const`: runtime storage; declaration pops and owns.
- `static`: compile-time constant; initializer must be known at compile time.
- Reading a variable pushes a copy (copy types) or a borrow (non-copy types).

### Control flow

```rust
If {
    // condition bool already on the stack before `if`
    then_branch: Vec<Statement>,
    else_branch: Option<Vec<Statement>>,
}

Match {
    // subject already on the stack before `match` (value, union, or error in handle)
    cases: Vec<MatchCase>,
    else_branch: Option<Vec<Statement>>,
}

MatchCase {
    pattern: Pattern,
    body: Vec<Statement>,
}

For {
    // condition bool or iterable already on the stack before `for`
    body: Vec<Statement>,
}

Defer {
    body: Vec<Statement>,            // run at scope exit; multiple defers run in reverse
}

Unsafe {
    body: Vec<Statement>,            // marks where unsafe ops occur
}

Handle {
    // wraps a preceding fallible call (typically `... call handle`)
    body: Vec<Statement>,            // optional handler (often an inner Match)
    fallback: Expression,            // word before `fallback`
}
```

`return` is a statement with no child: it returns the top of the stack and drops the rest. Absence of `with Type` means void (except optional return on `main`).

Match semantics that shape the AST (not separate nodes):

- Subject is borrowed for the duration; the original stack is restored afterward.
- Value / error cases: words before `case` leave a bool (`Pattern::Condition`).
- Union cases: the word before `case` is a member type (`Pattern::Type`); the branch receives `reference<Member>`.

---

## Pattern

Patterns appear only as match-case discriminators.

```rust
Pattern =
      Condition(Vec<Expression>)     // words before `case` producing a bool
    | Type(Type)                     // union member type before `case`
```

Examples:

```text
dup 85 == case                 → Condition([StackOp(Dup), Literal(85), Operator(Eq)])
i32 case                       → Type(Primitive(I32))
error.OUT_OF_MEMORY == case    → Condition([Name(error.OUT_OF_MEMORY), Operator(Eq)])
```

The condition form stores the flat word sequence from the grammar (`word { word } case`) as ordered `Expression` nodes.

---

## Declarations (top-level and nested)

These are both `TopLevel` variants and, where the grammar allows, embeddable statements (`Require`, `Function`).

### Struct

```rust
Struct {
    name: Ident,
    visibility: Option<Visibility>,
    fields: Vec<Field>,
}

Field {
    ty: Type,
    name: Ident,
    visibility: Option<Visibility>,
}
```

### Implement

```rust
Implement {
    target: Ident,                   // type being implemented
    methods: Vec<Function>,
}
```

Methods are ordinary `Function` nodes. Receivers appear as the first parameter type (typically `reference<T>` or `reference<T> mutable`).

### Enum

```rust
Enum {
    name: Ident,
    underlying: Option<Type>,        // default i32 if absent
    members: Vec<EnumMember>,
}

EnumMember {
    name: Ident,
    value: Option<IntegerLiteral>,   // explicit discriminant; else sequential
}
```

### Union

```rust
Union {
    name: Ident,
    members: Vec<Type>,              // at least one; member types must be distinct
}
```

### Error

```rust
Error {
    name: Ident,
    inject: Option<QualifiedName>,   // optional: inject members from another error type
    members: Vec<Ident>,
}
```

Error types behave like enums specialized for error handling and `|T Err|` return unions.

---

## Mapping from surface syntax

| Syntax (EBNF)     | AST node                                       |
| ----------------- | ---------------------------------------------- |
| `program`         | `Program`                                      |
| `main_function`   | `MainFunction`                                 |
| `require_stmt`    | `Require`                                      |
| `function_decl`   | `Function`                                     |
| `struct_decl`     | `Struct`                                       |
| `implement_block` | `Implement`                                    |
| `enum_decl`       | `Enum`                                         |
| `union_decl`      | `Union`                                        |
| `error_decl`      | `Error`                                        |
| `var_decl`        | `VarDecl`                                      |
| `assignment`      | `Assign`                                       |
| `if_stmt`         | `If`                                           |
| `match_stmt`      | `Match`                                        |
| `match_case`      | `MatchCase` + `Pattern`                        |
| `for_stmt`        | `For`                                          |
| `defer_stmt`      | `Defer`                                        |
| `unsafe_block`    | `Unsafe`                                       |
| `handle_stmt`     | `Handle`                                       |
| `return_stmt`     | `Return`                                       |
| `word`            | `Expression` (via `Statement::Word` or nested) |
| `type`            | `Type`                                         |
| `lvalue`          | `LValue` / `QualifiedName`                     |

Operands that sit on the stack before a keyword (`if`, `for`, `match`, `set`, declarations) are not children of those nodes in the surface AST: they are preceding `Word` statements (or preceding expressions in a container). Implementations may optionally reassociate a trailing condition expression into `If` / `For` / `Match` during parsing; either representation is fine so long as stack order is preserved for checking.

---

## Example

Source:

```yarrow
add function
	i32
	i32
do
	+
	return
end with i32

main function do
	3 4 add call
end
```

AST (sketch):

```text
Program
  items:
    Function
      name: add
      params: [i32, i32]
      body: [Word(Operator(+)), Return]
      returns: Some(i32)
  main:
    MainFunction
      body:
        Word(Literal(3))
        Word(Literal(4))
        Word(Name(add))
        Word(CallOp(Call))
      returns: None
```
