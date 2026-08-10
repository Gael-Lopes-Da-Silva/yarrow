# Yarrow Implementation Plan

Status of the language as-specified in `docs/syntax.yar` versus the implemented
pipeline in `crates/yarrow-core` (tokenizer -> parser -> compiler -> Cranelift JIT).

`docs/syntax.yar` is the source of truth. The front-end was rewritten in
commits `1c236a4` (tokenizer) and `c56c1f7` (parser); the compiler port is
**in progress** — the build is restored (Stage 0), the new operators are in
(Stage 1), and control flow is back (Stage 2: `for` over arrays/lists,
`handle` + `fallback`, `match`, `break`/`continue`). Unions still need their
Stage 3 rework.

## Pipeline status

- **Tokenizer** — complete for the new spec (`tokenizer/token_kind.rs`,
  `tokenize.rs`). Dropped tokens: `DotDot Question Exclamation Ampersand Bar
Colon SemiColon Comma At Arrow Equal Boolean Type While Default As Over`.
  Added tokens/keywords: `typeof`, `unrot`, `borrow`, `move`, `fallback`
  (`>=`/`<=` and `//` exist). There is no `@` token and no `while` keyword
  anymore.
- **Parser** — complete for the new AST (`parser/mod.rs`, `ast.rs`,
  `literals.rs`). No `Stmt::While`; `for` is the only loop and comes in three
  forms; `match` cases dispatch either on a boolean condition or on a union
  member type; `handle` carries a `fallback`; types can be used as values
  (`typeof`); `move`/`borrow` are statements/operators, not `@` builtins.
- **Compiler** — **builds** (Stage 0 done: `cargo check` green). The old
  `While`/`Builtin`/`Over` variants are gone; `for` handles the three new
  shapes (`emit_for` + `emit_cond_for`, `_` discards), `handle` patterns carry
  `fallback`, and `match` dispatches on `MatchCaseKind` (`Condition` working,
  `Type` still `E308`). Stage 1 done: `typeof` (type values as runtime kind
  codes, `Expr::TypeValue`/`Typeof`/`ApplyTypeof`), `borrow`
  (`Expr::Borrow`/`ApplyBorrow` via `emit_borrow`), and `move`
  (`Stmt::Move`/`emit_move`, use-after-move tracking limited to skip-free).
  Stage 2 done: `for` iterates **lists** as well as arrays (list length via
  `yarrow_list_len`, elements through `List.data` at `LIST_DATA_OFFSET`);
  `handle` lowers its extracted `fallback` on the error path (the one-line
  form works, error values matched with `error.X ==`, success payloads pass
  through); the implicit function fallthrough return is skipped after an
  explicit `return` (`FnState.terminated`, saved/restored around compound
  statements); `collect_strings` walks `Handle.fallback`.
  The embedded std library still uses `@`-builtins, which the new tokenizer
  cannot lex.
- **Runtime** — complete for the current host heap (strings/lists/maps/
  structs/arrays, regions, kind codes) but built around the `@`-builtin model;
  it will shrink to a tiny, data-registered host surface (Stage 4–5).
- **Std library** — embedded Yarrow modules in `compiler/modules.rs`
  (`std.io`, `std.math.sqrt`, `std.string`, `std.list`, `std.map`) written in
  the old `@`-builtin syntax; must be rewritten as pure-Yarrow **source files**
  in `crates/yarrow-core/lib/std/` (auto-discovered by a `build.rs`) and
  extended with `std.math`, `std.region`, `std.fs`. `require` resolves a
  dotted path as a _function in the parent module_ with a module-file
  fallback (function wins on ambiguity, with a warning).

### Current build

```
cargo check    # green
cargo clippy   # 2 pre-existing warnings: dead emit_builtin (deleted in
               #   Stage 5) and a parser collapsible-if; LIST_DATA_OFFSET is
               #   now live (used by list iteration)
cargo run -- <file.yar>   # runs new-syntax programs (no @-builtin std yet)
```

## Architecture notes

### Values and memory today

Scalars live in registers. Heap values are opaque `u64` handles pointing at
runtime headers tagged by a kind code (`runtime.rs`: `KIND_STRING = 0x10`,
`KIND_LIST = 0x20`, `KIND_MAP = 0x30`, `KIND_STRUCT = 0x40`, `KIND_PTR = 0x50`,
`KIND_ARRAY = 0x60`). Structs are heap blocks whose fields are laid out with
their own kind-code bytes (`compiler/mod.rs:527`). The compiler already emits
the loads/stores that build and read these headers — but **Yarrow-level code
cannot**: `pointer<T>` types are rejected (`compiler/types.rs:364`, `E307`)
and there is no load/store/address-arithmetic word. That is the single biggest
gap for the std library.

### Ownership model

Stack values are dropped when popped; variables own their values and drop
them at scope exit; `borrow` pushes a borrow reference (released on pop);
`move` transfers ownership and marks the source moved; `defer` runs in reverse
order; heap regions own registered values and free them as a unit; structs and
arrays free their fields recursively. Compile-time checks reject use-after-move
and popping/`set`-ing while borrowed.

### Host bridge (target design)

The language has **no named builtins and no per-name compiler code**. The
compiler's call lowering is generic: an undefined function name falls back to a
data table of host functions. The irreducible host surface is kept tiny —
memory and OS I/O only:

- `alloc(size) -> ptr` (today: `yarrow_alloc`)
- `free(ptr)` (today: `yarrow_free_value`)
- OS syscalls for `std.io`/`std.fs`: `write(fd, ptr, len)`, `open(ptr, len,
mode)`, `read(fd, ...)`, `close(fd)`

Everything else — strings, lists, maps, regions, formatting, `sqrt`, I/O
wrappers (`print` = `write(1, ...)`) — is **implemented in Yarrow** in the
std library, which requires the memory-access capability from Stage 4.

### Libraries and `require`

Language-source libraries live under `crates/yarrow-core/lib/`:

- `lib/std/` — the embedded std library, authored as `.yar` source files and
  embedded into the binary by a `build.rs` that globs `lib/std/**/*.yar`,
  maps each file to its dotted module name (`math.yar` -> `std.math`), and
  generates the `STD_MODULES` table consumed by `compiler/modules.rs`.
- `lib/vendor/` — future: wrappers/rewrites of third-party libraries (e.g.
  raylib).
- `lib/core/` — future: the compiler's own non-user Yarrow code.

`require` resolves a dotted path through the module tree; the last segment is
an item lookup in the parent module with a module-file fallback:

1. `a.b.c` resolves module `a.b` first; if it defines function `c`, only `c`
   is imported (by plain name, or under an alias). If `a/b/c.yar` exists too,
   the function wins and the compiler warns about the ambiguity.
2. Otherwise `a/b/c.yar` is imported as a module.
3. No parent module file (`std.io`) -> the full path is a module file
   (`std/io.yar`).

## Implemented so far

Written against the old spec (as of commit `184218c`); all of this must be
kept working through the port. Details are historical and will be validated
again during Stage 0.

- **Numeric/control core** — int `+ - * // % ^` (pow via inline loop), float
  `/`, comparisons `== != > >= < <=`, logical/bitwise `and or xor not`,
  `lshift`/`rshift`; `if`/`else`; stack ops `dup swap rot pop` (new: `unrot`,
  `over` removed).
- **Structs, field access, methods, `self`** — real layouts
  (`layout`/`FieldLayout`/`StructLayout`, heap blocks via `yarrow_alloc`);
  `{x 5 y 20}` literals (nested recursion, missing fields zeroed); `point.x`
  loads / `10 point.x set` stores; `Point implement distance function` lowers
  to `Point::distance`; `self` auto-bound to the receiver; auto-deref on
  member reads.
- **Match (value form)** — subject evaluated once and kept on the stack
  (conditions `dup` it); first truthy case wins, else `else`; subject dropped
  at the end; bare `match` runs conditions against the current stack.
- **`for`** — over fixed-size arrays (`numbers value for`, `[a b c] i for`),
  `break`/`continue`, nested loops. Will be reworked for the three new forms.
- **Fixed-size arrays** — `array<T n>` with `[a b c]` literals, size inferred
  when omitted, scalar-only elements (`E344` otherwise), heap blocks.
- **Strings/lists/hashmaps** — literals and types, host handles, `+`
  concatenation, `@string_*`/`@list_*`/`@map_*` builtins (all `@` builtins are
  scheduled for deletion in favor of pure-Yarrow std).
- **Modules/`require`** — `ModuleLoader` resolves dotted paths to embedded
  std modules or `path/to/module.yar`; loads depth-first into the same JIT
  module; alias vs. main-scope binding; CLI adds the source directory to the
  search path.
- **Error handling as values** — `error`/`Error` types, `with T or Error`
  return unions, `error.X` creation, `unwrap` (propagate if the function can
  error, else trap), `handle ... end` with `error.X == case` matching; error
  kind names interned per program.
- **Ownership** — stack/variable ownership, `move`/`borrow` operators
  (`Stmt::Move`, `Expr::Borrow`/`ApplyBorrow`), reverse-order `defer`, heap
  regions, recursive drops. `moved` is consulted to skip double frees, but a
  _compile-time use-after-move error_ is still not enforced (matches the old
  compiler).
- **`typeof` / type values** — type values (`i32`, `string`, ...) push their
  runtime kind code as an `I64`; `value typeof` pops the value (releasing heap
  borrows) and pushes its static type code, so `myVar typeof i32 ==` is code
  equality. References report their pointee type (`reference<T>` has physical
  type `T`).
- **Flexible `run_main`** — `RunResult` (`Void`/`Int`/`Bool`/`Float`/`Str`);
  still rejects struct/container/pointer results, `with T or Error` mains and
  128-bit/`F16` results (`E360`).
- **Enums** — named constants with implicit/explicit ordinals, `Color.RED` and
  bare `RED` push the value, `Color` resolves as a type, physical `I64`.

## Roadmap

Each stage ends with a green `cargo check` and, where noted, a runnable
example from `docs/syntax.yar`.

### Stage 0 — Restore the build ✅

Port the compiler to the new AST. Purely mechanical. **Done.**

- Fix the 14 pattern errors in `compiler/mod.rs`: remove `While` handling and
  `emit_while` (the conditional loop becomes `for` with a bool `source`);
  adapt `Stmt::For { source, value, index }`; add `fallback` to
  `Stmt::Handle` patterns; match on `MatchCaseKind` (start with `Condition`);
  rename `Expr::Builtin` → `Expr::TypeValue`; replace `StackOp::Over` with
  `Unrot`.
- Update the `load_requires_stmts` traversal for the new shapes.
- Files: `compiler/mod.rs`.
- Gate: `cargo check` green; core numeric/control programs run again. (`@`
  std modules still fail to lex at runtime — expected until Stage 5.)

### Stage 1 — New operators ✅

**Done.** All gates pass.

- `StackOp::Unrot` — `[1 2 3]` → `[3 1 2]` (landed in Stage 0).
- **`typeof`** — `Expr::TypeValue`/`Typeof`/`ApplyTypeof`. Type values are
  pushed as kind codes. `typeof` pops a value and pushes its _static_ type;
  heap values arrive as borrows which `typeof` releases (leaving the data
  owned); references report their pointee type. `==` on type values is code
  equality (`myVar typeof i32 ==`).
- **`borrow`** — `Expr::Borrow`/`ApplyBorrow` replace the `@borrow` path in
  `emit_builtin`; keep borrow tracking.
- **`move`** — `Stmt::Move { target, source }` replaces `@move`; keep
  use-after-move errors.
- Remove the dead `@borrow`/`@move` arms from `emit_builtin`.
- Files: `compiler/mod.rs`, `compiler/types.rs` (type-value representation).
- Gate: `typeof`/`borrow`/`move` examples from the docs compile and run.

### Stage 2 — Control flow ✅

- **`for`**, three forms: condition (`counter 5 < for` — the former `while`),
  value (`numbers value for`), value+index (`numbers value index for`);
  `_` discard bindings; arrays and lists as iterables; `break`/`continue`.
- **`handle` + `fallback`** — `Stmt::Fallback` lowered via the extracted
  `Handle.fallback` (incl. the one-line `risky call handle "x" fallback end`).
- **`match` (value form)** — rework to `MatchCaseKind::Condition`.
- Files: `compiler/mod.rs`.
- Gate: docs `for`/`match`/`handle` examples run.
- Verified: condition/value/index `for` over arrays and lists; `handle` with
  fallback on error, payload pass-through on success, `error.X ==` match cases
  with `else`; `unwrap` error propagation; string payloads/fallbacks; `break`/
  `continue`; explicit `return` in `with T or Error` functions (no bogus
  fallthrough re-return).

### Stage 3 — Unions

- `UnionDecl` (currently `E308`) → a **tagged one-of type**: a member kind
  code tag plus an inline payload sized to the largest member. Add
  `Ty::Union`; `val`/`set` hold and switch the active member (old one
  dropped).
- **`match` type dispatch** — `MatchCaseKind::Type`: compare the tag against
  each case type's kind code in order, `else` branch; each branch receives the
  member as a `reference<Type>` that auto-derefs on read; the borrow is
  released at the end of the match, leaving the union untouched. Validate:
  member types distinct, case type must be a member.
- Files: `compiler/mod.rs`, `compiler/types.rs`.
- Gate: docs `union_function` example compiles and runs.

### Stage 4 — Memory access in the language (largest stage)

Gives the std library the ability to manipulate heap headers directly, so the
host surface can shrink.

- Enable `pointer<T>` (remove `E307`): a typed raw pointer, represented as an
  address; type information is compile-time.
- Typed **load** (auto-deref reads, as `reference<T>` already does) and
  **store** through pointers (extend `set`), plus **address arithmetic**
  (pointer + byte offset; integer/pointer conversions as needed).
- Expose `alloc`/`free` to Yarrow through the generic host bridge.
- Extend the ownership/borrow/region compile-time checks so raw pointers
  cannot alias a borrowed value or outlive their region.
- **Host registry + generic lowering** — a data table
  `{name, signature, extern "C" fn}` in `runtime.rs` and one generic
  host-call path in the compiler (no per-name match arms). Replace the
  ad-hoc `extern` symbol imports with it.
- Files: `docs/syntax.yar` (spec), tokenizer/parser if new words are needed,
  `compiler/mod.rs`, `compiler/types.rs`, `runtime.rs`.
- Gate: hand-written Yarrow functions can allocate, read/write headers and
  build/free heap values (e.g. a list push by hand).

### Stage 5 — Std library in pure Yarrow

- Move the std to real source files: create `crates/yarrow-core/lib/std/`,
  add a `build.rs` that globs `lib/std/**/*.yar` into the generated
  `STD_MODULES` table (`include_str!` per entry, `rerun-if-changed` on the
  directory), and delete the inline `const STD_IO = r#"..."#` literals from
  `compiler/modules.rs`.
- Rework `require` resolution: `RequiredModule` gains `item: Option<String>`;
  a parent-first resolver (parse the parent module, check the last segment as
  a function, fall back to the module file) replaces the one-path-one-file
  lookup, with an ambiguity warning when both match; `register_module_bindings`
  exposes only the item for item imports.
- Author the modules in new syntax on top of the memory-access words:
  - `std.io` — `write_line`, `print`, `print_int`, `print_float` (over
    `write(1, ...)`; formatting loops in Yarrow).
  - `std.string` — `string_len`, `string_join`, comparisons.
  - `std.list` — `list_push`, `list_get`, `list_set`, `list_len`.
  - `std.map` — `map_get`, `map_set`, `map_len`.
  - `std.region` — `make_region`, `put_region`, `free_region`.
  - `std.math` — `sqrt` and friends (pure arithmetic).
  - `std.fs` — `open_file`, `close_file`, `read_line`.
- Layout is flat: one file per module (`io.yar`, `math.yar` with `sqrt`
  inside, ...); sub-folder module files remain possible but std does not
  need them.
- Honor `require` alias-vs-main-scope semantics everywhere.
- Delete `emit_builtin` and the now-redundant runtime container/string/print
  helpers; keep only the tiny host surface.
- Files: `compiler/modules.rs` (+ new `build.rs`, `lib/std/`),
  `compiler/mod.rs`, `runtime.rs`.
- Gate: the full docs example program (`docs/syntax.yar`) compiles and runs.

### Stage 6 — Remaining spec conformance

Remaining old milestones reviewed against the new spec:

- **Literal smallest-fit typing** — `42 → u8`, `-900 → i16`, `1_000 → u16`,
  `3.14 → f16` (the tokenizer/parser already keep lexemes lossless; the
  compiler currently pins ints to `I64` and floats to `F64`).
- **`drop` semantics** — the spec: `pop` removes one value, `drop` empties the
  whole stack and releases every borrow; they currently lower identically.
- **128-bit numbers** — `i128/u128/f128` arithmetic, comparisons and
  float conversions (removes `E310`).
- **Float `%`/`^`** — removes `E334`.
- **`run_main` coverage** — decide which remaining result kinds to support.
- **Fixed-array indexing** — **dropped**: the new spec has no indexing syntax
  (iteration is via `for`).
- Gate: the full docs example runs; no `E301`–`E308` remnants for spec
  features; build green with the std library fully in Yarrow except the tiny
  host surface.

## Open design questions

To be settled during Stage 4, not blocking earlier stages:

- How pointer arithmetic is spelled (pointer + int ops vs. a dedicated offset
  word) and whether `alloc` returns `pointer<void>` or a `u64`.
- Alias rules: can a raw pointer alias a borrowed value without tripping the
  compile-time checks, and how regions guard raw pointers.
- Whether `pointer<T>` load/store needs new syntax or reuses the
  `reference<T>` auto-deref model with a store through it.

## Definition of done

The compiler is feature-complete when `cargo check` is green, the new-front-end
port (Stages 0–3) is in place, Yarrow has generic memory access with the tiny
host surface (Stage 4), the std library is pure Yarrow authored as source
files under `crates/yarrow-core/lib/std/`, embedded by `build.rs`, with
`require` resolution matching the spec (Stage 5), the spec's full example
program (`docs/syntax.yar`) compiles and runs, and the remaining
spec-conformance items (Stage 6) match `docs/syntax.yar`.

## Building and running

```
cargo check                       # must stay green after every stage
cargo clippy                      # must stay green after every stage
cargo run -- <file.yar>           # tokenize + parse + compile + run
```

The driver lives at `src/main.rs` (the `yarrow-cli` crate is an empty stub);
modules required from user files resolve relative to the source file's
directory.
