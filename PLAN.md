# Yarrow Implementation Plan

Status of the language as-specified in `docs/syntax.yar` versus the implemented
pipeline in `crates/yarrow-core` (tokenizer -> parser -> compiler -> Cranelift JIT).

## Pipeline status

- **Tokenizer** — complete; all spec tokens present (`tokenizer/token_kind.rs`).
- **Parser** — parses the whole spec into an AST: containers, `for`, `match`,
  `handle`, `defer`, `require`, structs/enums/unions, generics, and type-unions.
- **Compiler** — everything in section 1 lowers to JIT:
  - done: numeric/control core, structs/methods/`self`, `match`, `for` over
    fixed-size arrays, strings/lists/hashmaps/`@sqrt` via a heap host runtime,
    modules/`require` with an embedded std library, and the ownership model
    (`@move`, `@borrow`, reverse-order `defer`, heap regions, struct/array
    drop/free).
  - remaining: enums, unions, error handling (`unwrap`/`handle`/`error`),
    fixed-array indexing, and the spec-divergence items in section 3
    (milestones 6–15).

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
  layouts (`layout`/`FieldLayout`/`StructLayout`, heap-allocated via
  `yarrow_alloc`); literals `{x 5 y 20}` init fields (nested structs recurse,
  missing fields zeroed); `point.x` loads / `10 point.x set` stores;
  `Point implement distance function` lowers to `Point::distance` and
  `point.distance call` resolves by receiver type; `self` is auto-bound to the
  method receiver; `@borrow`/`@move` implement ownership transfer.
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
  types, heap-allocated blocks; array sizes are inferred when omitted
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
  `extern "C"` symbols. `@borrow`/`@move` transfer ownership (see milestone 5);
  `@make_region`/`@free_region`/`@put_region` implement heap regions; `@sqrt`
  coerces ints/floats to `F64`. Heap values are dropped when popped,
  overwritten, or at scope exit.
- **`defer`/`handle`** — `defer` bodies compile in reverse order at scope exit
  (removed the `E301`); `handle` still requires error handling (milestone 6).

## 2. Parsed but NOT compiled

- **Array indexing** — fixed-size `array<T n>` compiles for scalar elements,
  but there is no `index`/`get`/`set` word yet (lists have `@list_get`/
  `@list_set`).
- **Enums** — `Color enum RED GREEN end` parses but members never become values.
- **Unions** — `Value union i32 string end` parses but is unresolved (`E308`).
- **Error handling** — `error`/`Error` types unresolved (`E302/E303`),
  `with value or Error` unions, `unwrap` (`E301`), `handle` (its body runs
  inline with no error-catching), `error.CustomError`.
- **Unknown builtins** — builtins not handled by `emit_builtin` (I/O words,
  list/map removals, etc.) fall through to `E301`.

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
  there is no array `index`/`get`/`set` word yet, and array values alias the
  same heap block on assignment (no copy semantics).

## 4. Infrastructure gaps

- No garbage collector; the heap host runtime
  covers strings/lists/maps/structs/arrays/print/`sqrt`; freed on drop per the
  ownership model (`@move`, `@borrow`, regions), but no cycle collection.
- Structs and arrays are real layouts backed by heap blocks (`yarrow_alloc`)
  that own their fields; assigning variables aliases pointers (no copy
  semantics), with ownership tracked by the compiler and enforced by runtime
  frees/regions.
- Coercion gaps: `int -> bool` via `ireduce` on a comparison result (correct for
  > 8-bit); `bool -> int` only widens; no `float -> int` truncation test coverage.

## Suggested milestones (priority order)

1. **Structs, field access, methods, `self`** — done. Layouts are computed from
   declarations and used for field loads/stores; method calls resolve by
   receiver type; `self` is bound to the receiver; `@borrow`/`@move` implement
   ownership transfer.
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
4. **Modules/`require` + std library** — done. `ModuleLoader` resolves dotted
   paths to embedded std modules (`std.io`, `std.math.sqrt`, `std.string`,
   `std.list`, `std.map`) or `path/to/module.yar` on the search path; requires
   load depth-first and compile into the same JIT module. Functions are
   fully-qualified as `{module}::{func}`, alias-less requires expose plain
   names, `alias.func` calls resolve via the alias, and the CLI adds the source
   file's directory to the search path. Inline `require` alias binding only
   applies when the next token is not a declaration/statement keyword.
5. **Memory model (borrows, regions, ownership)** — done. Stack/vars own their
   heap values and drop them at scope exit; `@move` transfers ownership and
   marks the source moved; `@borrow` pushes a borrow reference released on pop;
   `defer` runs in reverse order at scope exit; heap regions own registered
   values and free them on `@region_free`/exit; structs and arrays are
   `yarrow_alloc` blocks whose fields (strings/lists/maps/structs/arrays) are
   freed recursively.
6. **Error handling as values** — the `error`/`Error` types and
   `with T or Error` return unions resolve; `error.CustomError` creates a
   tagged error value; `unwrap` pushes the value or propagates the error;
   `handle ... end` catches it (`error.X == case`, `else`) and its `handle v
end` fallback form pushes `v` on error. This is what the spec's example
   program (`docs/syntax.yar` lines 225–284) still needs: `unwrap`, `handle`,
   and `with ... or Error` returns.
7. **Void/flexible `run_main`** — a `main` with no `with` clause (spec line 65)
   currently fails `E360`; `run_main()` accepts exactly one I64/I32/I8 return,
   so void mains and non-numeric returns (strings, floats) cannot run.
8. **Enums** — `Color enum RED GREEN end` lowers to named constant values with
   the member names bound (implicit ordinals, explicit values if specified).
9. **Unions** — `Value union i32 string end` becomes a tagged one-of type;
   `val` declarations hold one member at a time, `set` switches the active
   member and drops the old one, and reads must be matched/tagged.
10. **Fixed-array indexing** — an `index`/`get`/`set` word over
    `array<T n>` with bounds checks (mirroring `@list_get`/`@list_set`), and
    lifting the scalar-only element restriction (`E344`) so arrays and `for`
    iterate string/container/struct elements.
11. **Spec literal typing** — `42 -> u8`, `-900 -> i16`, `3.14 -> f16`:
    infer the smallest type a literal fits instead of pinning every int to I64
    and float to F64.
12. **`drop` semantics** — `drop` empties the whole stack and releases every
    borrow (the spec says one value for `pop`, all for `drop`); `Pop` and
    `Drop` currently lower identically.
13. **128-bit numbers** — `i128/u128/f128` arithmetic, comparisons, and
    conversions to/from floats (removes `E310`); currently 128-bit values
    exist in the type system but are effectively unusable.
14. **Float mod/pow** — `%` and `^` over floats (removes `E334`), matching the
    spec's `10 4 /` float division and general numeric operators.
15. **Expanded std library & builtins** — I/O words (`open_file`,
    `close_file`, reads), list/map removal words, and std modules
    (`std.list`, `std.map`) generalized beyond `i32`/`i64` element types.

## Definition of done

The compiler is feature-complete when the remaining `E301`/`E303`–`E308`/"not
yet supported" branches (milestones 6–15) are replaced with real codegen, the
spec-divergence items above match `docs/syntax.yar`, and the spec's full
example program (`docs/syntax.yar` lines 225–284) compiles and runs.
