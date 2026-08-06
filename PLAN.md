# Yarrow Implementation Plan

Status of the language as-specified in `docs/syntax.yar` versus the implemented
pipeline in `crates/yarrow-core` (tokenizer -> parser -> compiler -> Cranelift JIT).

## Pipeline status

| Stage     | State                                                                                                                                                                                              |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tokenizer | Complete — all spec tokens present (`tokenizer/token_kind.rs`)                                                                                                                                     |
| Parser    | Parses the whole spec into AST; containers, `for`, `match`, `handle`, `defer`, `require`, structs/enums/unions, generics, and type-unions all parse                                                |
| Compiler  | Core numeric/control subset + structs/field-access/methods/`self` lower to JIT; containers, strings, match, for, enums, unions, error handling, memory model are `E301`–`E308` "not yet supported" |

Most remaining work is in the **compiler**, not the front-end.

Signed literals (`-900`, `+300`) and scientific-notation floats (`1e3`, `-1.5e-3`)
lex at the tokenizer as numeric tokens with no unary operators required
(`tokenize.rs` `handle_number`). Literal lexemes are decoded and validated in
`parser/literals.rs` (`decode_int_literal` -> `i128`, `decode_float_literal`,
`decode_rune_literal`); the parser validates them at the token's real location
and the compiler reuses the same decoders.

## 1. Working today (compiles and runs)

- Int arithmetic `+ - * // % ^` (pow via inline loop), float `/`, comparisons
  `== != > >= < <=`, logical/bitwise `and or xor not`, `lshift`/`rshift`.
- Literals: integers (forced to I64), floats (F64), bools (I8), runes (I32).
- Stack ops `dup over swap rot pop`; `set`; var declarations
  `mutable/const/static`; functions with params + multi-return, `call`, `return`,
  and stack-based implicit fallthrough return.
- `if`/`else` and `while` plus `break`/`continue` (conditions precede the
  keyword, matching the spec).
- **Structs, field access, methods, `self`** — struct declarations compute real
  layouts (`layout`/`FieldLayout`/`StructLayout`, frame-slot backed); literals
  `{x 5 y 20}` init fields (nested structs recurse, missing fields zeroed);
  `point.x` loads / `10 point.x set` stores; `Point implement distance function`
  lowers to `Point::distance` and `point.distance call` resolves by receiver
  type; `self` is auto-bound to the method receiver; `@borrow`/`@move` are
  handled as pointer identity (references reuse the same pointer).

## 2. Parsed but NOT compiled (currently `E301`)

- **Strings** — `Expr::String`, the `string` type (`E303`), `@string_join`,
  string comparison/indexing.
- **Match** — `score match ... case ... else ... end`.
- **For loops** — iterable loop (needs containers/iterables).
- **Containers** — array/list/hashmap literals and the `array<i32 3>`,
  `list<i32>`, `hashmap` types (`E304/305/306`), indexing, builtins such as
  `@list_push`.
- **Enums** — `Color enum RED GREEN end` parses but members never become values.
- **Unions** — `E308`.
- **Modules** — `require` is silently ignored (no loader, no symbol resolution,
  no std library: `sqrt`, `io.write_line`).
- **All builtins** (`@list_push`, `@make_region`, `@free_region`, `@put_region`,
  ...) — `Expr::Builtin`.
- **Error handling** — `error`/`Error` types unresolved (`E302/E303`),
  `with value or Error` unions, `unwrap`, `handle`, `error.CustomError`.
- **Defer** — `defer ... call end`.
- **Memory model** — ownership, borrows, regions; structs/references are plain
  pointers with no forcing.

## 3. Compiles but diverges from the spec (semantic mismatches)

- `drop` pops 1 like `pop`; the spec says `drop` empties the whole stack (and
  releases borrows). `compiler/mod.rs` treats `Pop | Drop` identically.
- **Literal typing** — spec: `42 -> u8`, `-900 -> i16`, `3.14 -> f16` (smallest
  fitting type); the compiler pins all int literals to I64 and floats to F64,
  with no smallest-fit inference.
- **Void `main`** — `run_main()` requires exactly one return value (`E360`), so
  a void `main` (spec line 65) cannot run.
- **128-bit** — `i128/u128/f128` map to Cranelift types but 128->float
  conversions are rejected (`E310`) and 128-bit arithmetic is untested.
- **Float mod/pow** — `%`/`^` on floats are unsupported (`E334`).

## 4. Infrastructure gaps

- No require/module resolver, no standard-library prelude, no runtime for
  heap/regions/IO.
- Structs are real layouts backed by frame slots, but structs are only ever
  pointers: assigning a struct variable aliases (no copy semantics), and no
  ownership/borrow enforcement exists yet.
- Coercion gaps: `int -> bool` via `ireduce` on a comparison result (correct for
  > 8-bit); `bool -> int` only widens; no `float -> int` truncation test coverage.

## Suggested milestones (priority order)

1. **Structs, field access, methods, `self`** — done. Layouts are computed from
   declarations and used for field loads/stores; method calls resolve by
   receiver type; `self` is bound to the receiver; `@borrow`/`@move` are
   pointer identity (ownership rules still unimplemented).
2. **Match + For loops** — extends loop/merge machinery; small scope.
3. **Strings, containers, builtins** — largest chunk, plus heap/runtime;
   unblocks lists, arrays, and string ops.
4. **Modules/`require` + std library** — orthogonal scaffolding; needs a loader
   and symbol table.
5. **Memory model (borrows, regions, ownership)** — enforcement and heap
   management; depends on references/containers landing first.

## Definition of done for the compiler milestone (current numeric/control core)

`Compiler` is considered feature-complete when all `E301`/`E303`–`E308`/"not yet
supported" branches in the compiler are replaced with real codegen and the
spec's full example program (`docs/syntax.yar`) compiles and runs.
