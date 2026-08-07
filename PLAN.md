# Yarrow Implementation Plan

Status of the language as-specified in `docs/syntax.yar` versus the implemented
pipeline in `crates/yarrow-core` (tokenizer -> parser -> compiler -> Cranelift JIT).

## Pipeline status

| Stage     | State                                                                                                                                                                                              |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tokenizer | Complete — all spec tokens present (`tokenizer/token_kind.rs`)                                                                                                                                     |
| Parser    | Parses the whole spec into AST; containers, `for`, `match`, `handle`, `defer`, `require`, structs/enums/unions, generics, and type-unions all parse                                                |
| Compiler  | Core numeric/control subset + structs/methods/`self` + `match` + `for` over fixed-size arrays + strings/lists/hashmaps/`@sqrt` via a heap host runtime all lower to JIT; enums, unions, modules/`require`, error handling, and the ownership memory model are `E301`–`E308` "not yet supported" |

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
- **Match** — `score match ... case ... else ... end`. The subject is evaluated
  once and lives on the compile-time stack for the whole match (conditions
  commonly `dup` it); the first truthy case runs its body, otherwise `else`;
  the subject is dropped at the end so the match yields only the chosen
  branch's value(s). A bare `match` (no subject) runs conditions against the
  stack as it was when the match started.
- **`for` loops over fixed-size arrays** — `numbers value for ... end` and
  `[12 27 36] i for ... end`; supports `break`/`continue` and nested loops.
- **Fixed-size arrays** — `array<i32 3>` types with literal `[a b c]`
  initializers (standalone or in var decls/struct fields), scalar element
  types, frame-slot storage; array sizes are inferred when omitted
  (`array<i32>`).
- **Strings** — `"..."` literals lower through read-only data sections
  (`declare_data` + `GlobalValue`) into `yarrow_str_new` handles; the `string`
  type resolves; `@print`, `@string_len`, `@string_join` (left, right, sep),
  string comparison (`== != > >= < <=` via `yarrow_str_cmp`), and `+`
  concatenation (`yarrow_str_join`).
- **Lists/hashmaps** — `(a b c)` and `{k v}` literals lower to host handles
  (`yarrow_list_new`/`yarrow_map_new`); `list<i32>` / `hashmap<k v>` types
  resolve; `@list_len`/`@list_get`/`@list_set`/`@list_push`,
  `@map_len`/`@map_get`/`@map_set` (`@map_get` pushes `(value, found)`, value
  then found flag); typed list/map var decls, struct fields of list/hashmap
  type, int/string literal keys and values; `@list_get`/`@list_set`
  bounds-check via `trapz`.
- **Builtins & heap runtime** — `runtime.rs` implements `yarrow_alloc`/
  `yarrow_free` plus the string/list/map/print ops, imported as typed
  `extern "C"` symbols. `@borrow`/`@move` are pointer identity;
  `@make_region`/`@free_region`/`@put_region` are no-ops; `@sqrt` coerces
  ints/floats to `F64`. Memory intentionally leaks (no GC/ownership yet).
- **`defer`/`handle`** — bodies compile inline (removed the `E301`).

## 2. Parsed but NOT compiled (currently `E301`)

- **Array indexing** — fixed-size `array<T n>` compiles for scalar elements,
  but there is no `index`/`get`/`set` word yet (lists have `@list_get`/
  `@list_set`).
- **Enums** — `Color enum RED GREEN end` parses but members never become values.
- **Unions** — `E308`.
- **Modules** — `require` is silently ignored (no loader, no symbol resolution,
  no std library beyond the builtin runtime functions).
- **Error handling** — `error`/`Error` types unresolved (`E302/E303`),
  `with value or Error` unions, `unwrap` (`E301`), `handle`,
  `error.CustomError`.
- **Unknown builtins** — builtins not handled by `emit_builtin` (I/O words,
  list/map removals, etc.) fall through to `E301`.
- **Memory model** — ownership, borrows, regions; structs/references/arrays are
  plain pointers with no forcing; runtime memory leaks (no GC).

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
- **Arrays** — element types are restricted to scalars (`E344` otherwise),
  there is no array `index`/`get`/`set` word yet, and array values are
  pointers into frame slots (copies alias the same storage).

## 4. Infrastructure gaps

- No require/module resolver or standard-library prelude; the heap host runtime
  covers strings/lists/maps/print/`sqrt`, but there is no GC and
  regions/ownership are not enforced.
- Structs and arrays are real layouts backed by frame slots, but are only ever
  pointers: assigning them aliases (no copy semantics), and no ownership/borrow
  enforcement exists yet.
- Coercion gaps: `int -> bool` via `ireduce` on a comparison result (correct for
  > 8-bit); `bool -> int` only widens; no `float -> int` truncation test coverage.

## Suggested milestones (priority order)

1. **Structs, field access, methods, `self`** — done. Layouts are computed from
   declarations and used for field loads/stores; method calls resolve by
   receiver type; `self` is bound to the receiver; `@borrow`/`@move` are
   pointer identity (ownership rules still unimplemented).
2. **Match + For loops** — done. `match` lowers to a chain of conditional
   branches over the subject (with the subject dropped at the end); `for`
   iterates fixed-size arrays (`array<T n>` + `[a b c]` literals landed to
   support it) with `break`/`continue`. Also fixed a latent parser bug where
   `array<i32 3>`/`hashmap<k v>` type args were dropped, and made `if`/`else`
   (and match) merge blocks coerce branch values so mixed-width branches (I32
   vs I64) type-check.
3. **Strings, containers, builtins** — done. Strings, lists/hashmaps
   (literals, types, `@list_*`/`@map_*` builtins), `@sqrt`, and
   `@borrow`/`@move`/`@make_region`/`@free_region`/`@put_region` lower to a
   heap host runtime in `runtime.rs` (`yarrow_alloc`, `yarrow_str_*`,
   `yarrow_list_*`, `yarrow_map_*`, prints). `defer`/`handle` bodies compile
   inline. Array indexing (a fixed-array `get`/`set` word) remains.
4. **Modules/`require` + std library** — orthogonal scaffolding; needs a loader
   and symbol table.
5. **Memory model (borrows, regions, ownership)** — enforcement and heap
   management; depends on references/containers landing first.

## Definition of done for the compiler milestone (current numeric/control core)

`Compiler` is considered feature-complete when all `E301`/`E303`–`E308`/"not yet
supported" branches in the compiler are replaced with real codegen and the
spec's full example program (`docs/syntax.yar`) compiles and runs.
