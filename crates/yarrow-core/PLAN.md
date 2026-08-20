# Yarrow Core Implementation Plan

Tracks progress against `docs/syntax.yar` (source of truth). Context and rules live in `AGENTS.md`.

## Pipeline status

| Component           | Status                               | Notes                               |
| ------------------- | ------------------------------------ | ----------------------------------- |
| Tokenizer           | ✅ Complete                          | `crates/yarrow-core/src/tokenizer/` |
| Parser              | ✅ Complete                          | Current AST + `require` forms       |
| Compiler            | 🟡 Stages 0–5 mostly done            | Stage 6 next                        |
| Runtime             | ✅ Complete                          | Host heap + `Safety` metadata       |
| Std library         | 🟡 Pure-Yarrow migration in progress | Stage 5                             |
| Unsafe memory model | ✅ Complete                          | Stage 4                             |

---

## Stage 0: Restore the build ✅

Port compiler to the new AST. Done.

## Stage 1: New operators ✅

- `unrot`, `typeof`, `borrow`, `move`
- Ownership / borrow / use-after-move tracking

Done.

## Stage 2: Control flow ✅

Done.

## Stage 3: Unions ✅

Done. Member borrows use normal `reference<T>`.

## Stage 4: Memory access and unsafe ✅

- `unsafe` keyword, `unsafe … end`, `name unsafe function`
- `Safety { Safe, Unsafe }` on host functions
- Reject unsafe ops outside unsafe context (`E370`)
- Typed `load`/`store`, pointer arithmetic, member access through `pointer<T>`, raw alloc/free

Gate: `pointer_function` + `unsafe … end` compile and run; bare unsafe ops fail with `E370`.

Done.

## Stage 5: Std library in pure Yarrow 🟡

Move std to `crates/yarrow-core/lib/std/` via `build.rs` / `STD_MODULES`.

### 5.1 Build infrastructure ✅

- Recursive glob of `lib/std/**/*.yar`
- Emit `STD_MODULES` table
- Verified: `"std.io" io require`, `"std.math" require`, etc.

### 5.2 `std.mem` ✅

- `alloc`, `free`, `load`, `store` as `unsafe function`s wrapping `@`-primitives
- Parser disambiguation for `load`/`store` as both keywords and member names

### 5.3 Item-import resolution ✅

- Parent-first: `"std.math.sqrt" require` imports only the function
- Function wins over module file; warn on ambiguity

### 5.4 Remaining standard modules

| Module       | Status  | Notes                                                           |
| ------------ | ------- | --------------------------------------------------------------- |
| `std.io`     | Partial | `write_line`, `print`, …; keep low-level OS unsafe where needed |
| `std.string` | Partial | `len`, `join`, comparisons                                      |
| `std.list`   | Partial | `push`, `get`, `set`, `len`                                     |
| `std.map`    | Partial | `get`, `set`, `len`                                             |
| `std.region` | Partial | `make_region`, `put_region`, `free_region`                      |
| `std.math`   | Partial | `sqrt` and pure arithmetic                                      |
| `std.fs`     | Partial | High-level file ops; raw OS may be unsafe                       |

### 5.5 Safe abstractions over unsafe

Prefer the pattern: unsafe implementation → establish invariant → safe public API.
The user should not need unsafe merely because an implementation internally uses raw memory.
This is an important part of keeping the language pleasant while still allowing systems-level programming.

### 5.6 Host surface cleanup

- Remove redundant `emit_builtin` helpers once pure-Yarrow std covers them
- Keep only the tiny host surface (`@alloc`/`@free`/`@load`/`@store` behind `std.mem`)
- Honor `require` alias vs main-scope semantics everywhere

### Stage 5 gate

`docs/syntax.yar` compiles and runs end-to-end, including `unsafe` regions and `mem.*` calls.

## Stage 6: Remaining spec conformance

- Smallest-fit literal typing
- `drop` semantics
- 128-bit numbers
- Float `%` / `^`
- `run_main` coverage
- Full `docs/syntax.yar` execution
- Ownership/borrow validation:
  - Use-after-move → compile error
  - Borrowed mutation → compile error
  - Borrowed pop → compile error
  - Region escape → compile error
  - Raw pointer validity stays the programmer’s responsibility inside `unsafe`

---

## Definition of done

1. `docs/syntax.yar` is the language source of truth and runs.
2. Stack ownership and safe borrowing are compile-time checked.
3. Regions provide structured heap lifetime management.
4. Safe references cannot escape their owner/region.
5. No user-visible lifetime parameters.
6. Raw memory ops require `unsafe`; `unsafe` does not disable type/stack/ownership/borrow checks.
7. Std library is pure Yarrow except for a tiny host surface.
8. `require` follows `"<path>" [<scope>] require` and its resolution rules.
9. `cargo check` and `cargo clippy` stay green.
