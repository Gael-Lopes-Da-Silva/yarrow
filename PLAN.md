# Yarrow Implementation Plan

This plan tracks the Yarrow language as specified in `docs/syntax.yar` — the **source of truth** — against the implemented pipeline in `crates/yarrow-core` (tokenizer → parser → compiler → Cranelift JIT). It is written to be executed by a coding agent: each stage lists concrete tasks, the files involved, and an explicit pass/fail gate.

## How to read this plan

- **Part 1 — Architecture**: the language's design model (ownership, safe references, modules).
- **Part 2 — Unsafe memory model**: the safety boundary every unsafe feature must honor.
- **Part 3 — Stages**: ordered implementation work, each with a **gate** (code that must compile, code that must fail).
- **Parts 4–6**: open questions, definition of done, build commands.

Whenever a section says "spec" it means `docs/syntax.yar`; where the implementation and the spec diverge, that divergence is called out explicitly and a task is listed to close it.

## Memory model in one sentence

Yarrow uses a **stack/ownership/region model rather than explicit user-visible lifetime parameters**. Safe references are validated through ownership, borrow, scope, and region information. Raw pointers are explicitly unsafe and do **not** participate in the safe borrow/lifetime model.

## Pipeline status

| Component           | Status                                                    | Notes                                                                         |
| ------------------- | --------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Tokenizer           | ✅ Complete for the current spec                          | `crates/yarrow-core/src/tokenizer/`                                           |
| Parser              | ✅ Complete for the current AST                           | Includes the current `require` argument order (`"<path>" [<scope>] require`). |
| Compiler            | 🟡 Stages 0–4 implemented                                 | Stage 5 is the next major step.                                               |
| Runtime             | ✅ Complete for the current host heap and runtime objects | `crates/yarrow-core/src/runtime.rs`; host fns carry `Safety` metadata.        |
| Std library         | ⬜ Migration to pure-Yarrow source files pending          | Stage 5                                                                       |
| Unsafe memory model | ✅ Completed (syntax + compiler enforcement)              | Stage 4                                                                       |

---

# Part 1 — Architecture

## 1.1 Values and memory

Scalars live in registers.

Heap values are opaque `u64` handles pointing at runtime headers tagged by a kind code:

```text
KIND_STRING = 0x10
KIND_LIST   = 0x20
KIND_MAP    = 0x30
KIND_STRUCT = 0x40
KIND_PTR    = 0x50
KIND_ARRAY  = 0x60
KIND_UNION  = 0x70
```

Structs, arrays, lists, maps, and unions are heap-managed values.

`pointer<T>` is a typed raw address. The pointer value itself is non-owning and does not cause automatic destruction.

## 1.2 Ownership model

Yarrow's safe memory model is based on **ownership scopes rather than explicit lifetime parameters**.

- Stack values are dropped when popped.
- Variables own their values and release them at scope exit.
- `move` transfers ownership.
- `borrow` creates a `reference<T>` tied to the owning value's validity.
- A value cannot be popped or mutated in ways prohibited while it is borrowed.
- `defer` runs in reverse order.
- Heap regions own registered values and free them as a unit.
- Structs, arrays, unions, and other owning containers release their owned contents recursively.
- Safe references cannot outlive their owning value or region.
- The compiler should infer validity from these ownership relationships rather than expose Rust-style lifetime parameters.

### Important principle

Yarrow does **not** introduce explicit user-visible lifetime syntax such as:

```text
reference<'a, T>
```

unless a future concrete use case proves that the structural ownership/region model is insufficient.

## 1.3 Safe references vs raw pointers

These are deliberately different concepts.

### `reference<T>` — a safe borrow

The compiler tracks:

```text
owner
  ↓
borrow
  ↓
reference<T>
```

and ensures the reference cannot outlive the owner or its region.

### `pointer<T>` — a raw, non-owning address

The compiler knows its static pointee type but does **not** promise that the address:

- is valid,
- is aligned,
- points to a live object,
- is within its original allocation,
- does not alias another pointer,
- or remains valid after the allocation is freed.

Those guarantees are the programmer's responsibility inside an `unsafe` context.

This deliberately avoids recreating Rust's complete lifetime/provenance system for raw pointers.

## 1.4 Modules and `require` syntax

### Spec syntax (current)

`require` imports a dotted module path. The keyword comes **last**; an optional scope name sits between the path and the keyword:

```yarrow
"std.io" io require        # Import everything from io into a scope named io
"std.math.sqrt" require    # Import a function into the main scope
"std.math" require         # Import everything from math into main scope
```

General form:

```text
"<dotted.path>" [<scope>] require
```

| Form                      | Meaning                                         | Call site            |
| ------------------------- | ----------------------------------------------- | -------------------- |
| `"std.io" io require`     | Module imported into a named scope              | `io.write_line call` |
| `"std.math.sqrt" require` | A single item (function) imported by plain name | `sqrt call`          |
| `"std.math" require`      | Whole module imported into the main scope       | `sqrt call`          |

A `require` may appear at top level or inside a function body (then scoped to that function). Modules load depth-first into the same JIT module, in dependency order, so `require` really imports code, not just symbols.

### Path resolution rules

1. `a.b.c` resolves module `a.b` first; if it defines function `c`, only `c` is imported (by plain name, or under the given scope). If both `a/b/c.yar` and function `c` exist, the **function wins** and the compiler warns about the ambiguity.
2. Otherwise `a/b/c.yar` is imported as a module.
3. If there is no parent module file (e.g. `std.io`), the full path is a module file (`std/io.yar`).

### Implementation status

All three forms now work as specified. The compiler side (`crates/yarrow-core/src/compiler/mod.rs`) records `RequiredModule { path, alias, item, program }` and binds functions under the alias (`scope.func`), by plain name for whole-module imports, or as a single item for `"std.math.sqrt" require` (parent-first resolution, per [5.3](#53-item-import-resolution-completes-rule-1-of-require)).

## 1.5 The resulting memory architecture

The key design decision behind the plan:

```text
                         Yarrow memory
                              │
              ┌───────────────┴───────────────┐
              │                               │
          SAFE MEMORY                     UNSAFE MEMORY
              │                               │
      ┌───────┴────────┐             ┌────────┴─────────┐
      │                │             │                  │
    Stack            Regions     pointer<T>         raw memory
      │                │             │                  │
    Own/Borrow       Own/Drop     non-owning        programmer
      │                │             address         responsibility
      └───────┬────────┘             │                  │
              │                      └────────┬─────────┘
        reference<T>                          │
              │                          unsafe block
              │                              │
       compiler proves                 compiler checks
          validity                     acknowledgement
```

This is the **central architectural principle**: Yarrow's compiler does the hard lifetime reasoning for structured ownership, while raw memory is deliberately moved behind an explicit unsafe boundary rather than attempting to reproduce Rust's general lifetime machinery.

---

# Part 2 — The unsafe memory model

Yarrow is safe by default. Operations that bypass the normal ownership/borrow guarantees require an explicit unsafe context.

## 2.1 Two separate concepts

The language uses two distinct constructs with different meanings:

```yarrow
foo unsafe function   # API contract: callers must use an unsafe block
unsafe ... end        # lexical unsafe region inside a body
```

- `unsafe function` marks an API as requiring explicit acknowledgement from its caller.
- `unsafe ... end` creates a lexical unsafe context for the operations in its body.

## 2.2 `unsafe function`

Marks an API as requiring explicit acknowledgement from its caller.

An unsafe function may only be called while compiling inside an `unsafe` block.

```yarrow
pointer_function unsafe function do
    ...
end
```

A normal call:

```yarrow
pointer_function call
```

is a compile-time error. The caller must write:

```yarrow
unsafe
    pointer_function call
end
```

## 2.3 `unsafe ... end`

Creates a lexical unsafe context.

```yarrow
unsafe
    p load
    p 42 store
end
```

Leaving `end` immediately restores the previous safe context. `unsafe` is not a runtime operation.

## 2.4 Unsafe does not disable safety checking

An unsafe block **must not** disable:

- type checking,
- stack-effect checking,
- ownership checking,
- borrow checking,
- move checking,
- region checking for safe references,
- normal control-flow validation.

It only permits operations explicitly classified as unsafe.

Therefore:

```text
unsafe
    raw pointer operation
```

is allowed, but:

```text
unsafe
    invalid safe borrow
```

is still rejected.

The meaning of `unsafe` is:

> The programmer accepts responsibility for guarantees that the compiler cannot establish for the unsafe operation.

It does **not** mean:

> Turn off Yarrow's safety checker.

## 2.5 Unsafe operations — safety metadata

Unsafe operations are represented internally as operations/functions requiring an unsafe context.

Conceptually:

```text
Operation {
    name
    signature
    safety: Safe | Unsafe
}
```

or equivalent metadata.

The compiler maintains an unsafe context while compiling a function body:

```text
unsafe_depth == 0  → safe context
unsafe_depth > 0   → unsafe context
```

An unsafe operation encountered while `unsafe_depth == 0` produces a compile-time error.

## 2.6 Unsafe functions — calls

Function metadata gains an unsafe flag:

```text
Function {
    ...
    is_unsafe: bool
}
```

At every call:

```text
callee.is_unsafe && !unsafe_context  → compile-time error
```

The unsafe requirement does **not** automatically propagate to the caller:

```yarrow
foo unsafe function do
    ...
end

bar function do
    unsafe
        foo call
    end
end
```

`bar` remains a normal safe function. This keeps unsafe API boundaries explicit at call sites.

## 2.7 Unsafe function bodies

Declaring a function `unsafe` does not automatically make its entire body unsafe.

These are independent:

```text
unsafe function
    → unsafe API contract

unsafe
    ...
end
    → unsafe implementation region
```

Therefore an unsafe function may contain both safe and unsafe regions:

```yarrow
foo unsafe function do
    # safe operations

    unsafe
        # raw operations
    end

    # safe operations
end
```

This keeps unsafe implementation regions auditable.

## 2.8 Raw pointers

`pointer<T>` remains a typed raw address.

Properties:

- represented as an address;
- carries compile-time pointee type information;
- is non-owning;
- is not itself a heap-managed object;
- is not automatically freed;
- may be stored, copied, and passed as a value;
- using it for raw memory operations requires `unsafe`.

The compiler does **not** attempt to make arbitrary raw pointers participate in the safe borrow/lifetime system.

## 2.9 Unsafe pointer operations

The following are unsafe:

- raw pointer dereference;
- `load` through a raw pointer;
- `store` through a raw pointer;
- pointer arithmetic;
- integer ↔ pointer conversion where applicable;
- raw memory access;
- manually freeing memory;
- manually allocating untyped/raw memory;
- unsafe functions exposing these capabilities.

Pointer member access and write-through operations through `pointer<T>` are also unsafe:

```yarrow
unsafe
    cp.value
    cp.value 123 set
end
```

## 2.10 Allocation and deallocation

Raw allocation and deallocation are unsafe:

```text
alloc → UNSAFE
free  → UNSAFE
```

The compiler must not allow ordinary safe code to manually allocate/free raw memory. For example:

```yarrow
32 mem.alloc
```

outside an unsafe block is a compile-time error.

This does not prevent safe allocation APIs from being built later. A safe abstraction may internally use unsafe memory management and expose only a safe interface.

## 2.11 Raw memory host primitives

The compiler-level primitives:

```text
@alloc
@free
@load
@store
```

are themselves unsafe. They cannot be used to bypass the unsafe mechanism.

For example:

```yarrow
foo function do
    32 @alloc
end
```

must fail, whereas:

```yarrow
foo function do
    unsafe
        32 @alloc
    end
end
```

is permitted.

The eventual standard library should hide these compiler-level primitives from normal user code where practical.

## 2.12 Host registry

The host registry remains generic:

```text
{
    name,
    signature,
    extern "C" fn,
    safety
}
```

The `safety` metadata allows the generic host-call path to enforce unsafe requirements without adding per-function compiler logic.

For example:

```text
alloc:
    signature: u64 -> pointer<void>
    safety: Unsafe

free:
    signature: pointer<void> -> void
    safety: Unsafe
```

Safe host functions remain `Safe`.

---

# Part 3 — Implementation stages

## Stage 0 — Restore the build ✅

Port the compiler to the new AST.

**Done.** No changes required.

## Stage 1 — New operators ✅

**Done.**

- `unrot`
- `typeof`
- `borrow`
- `move`
- ownership/borrow tracking
- use-after-move tracking

The existing ownership infrastructure remains the basis for safe references.

## Stage 2 — Control flow ✅

**Done.** No changes required.

## Stage 3 — Unions ✅

**Done.** No changes required.

Union member borrows continue to use the normal safe `reference<T>` model.

## Stage 4 — Memory access and unsafe operations ✅

**Done.**

- `unsafe` keyword, `unsafe ... end` blocks, `name unsafe function` modifier (`Function.is_unsafe`, `FnState.unsafe_depth`).
- `Safety { Safe, Unsafe }` metadata on host functions; `@alloc`/`@free` are unsafe.
- Unsafe operations — typed `load`/`store`, `@load`/`@store`, pointer arithmetic, member access/set through `pointer<T>`, raw alloc/free, calls to unsafe functions — are rejected outside an unsafe context with `E370`. Unsafe does not disable normal type/stack/ownership/borrow checking.
- Bonus fix: `with void` functions trapped at end of body (`Void` leaked into `st.returns`); now filtered to match `declare_function`.

Gate verified: `pointer_function unsafe function` and `unsafe ... end` compile and run; bare unsafe operations fail with `E370` outside an unsafe context.

## Stage 5 — Std library in pure Yarrow

Move the std library to:

```text
crates/yarrow-core/lib/std/
```

using the `build.rs`/`STD_MODULES` architecture.

### 5.1 Build infrastructure ✅

> **Done.** `crates/yarrow-core/build.rs` recursively globs `lib/std/**/*.yar`, maps each file to its dotted module name (`io.yar` → `std.io`, `a/b.yar` → `std.a.b`), and emits `STD_MODULES` into `OUT_DIR` using `include_str!`. `compiler/modules.rs` now `include!`s that generated table; the hand-written `const STD_IO`/`STD_MATH_SQRT`/etc. literals are deleted. The std modules (`io`, `math` with `sqrt` inside, `string`, `list`, `map`) are authored as flat `.yar` files under `lib/std/`. `cargo:rerun-if-changed` rebuilds on std-source edits. Verified: `"std.io" io require`, `"std.string" str require`, `"std.math" require` all load and run end-to-end.
>
> Note: `std.math.sqrt` (the old single-function item-import approximation) is now the module `std.math` containing `sqrt`; true item-import resolution is 5.3.

### 5.2 `std.mem` ✅

> **Done.** `crates/yarrow-core/lib/std/mem.yar` exposes `alloc`, `free`, `load`, `store` as `unsafe function`s, each wrapping the compiler-level primitive (`@alloc`/`@free`/`@load`/`@store`) inside an explicit `unsafe ... end` region. Callers must enter an unsafe block — `unsafe 32 mem.alloc call end` — and every function is gated by the Stage 4 machinery (`E370 'call to 'std.mem::alloc'' requires an unsafe context` outside unsafe).
>
> Parser change: `load`/`store` are keywords (the typed pointer words `p load` / `addr value store`), but `std.mem` needs them as function names and module member names. `parser/mod.rs` now disambiguates: a statement starting with `load`/`store` followed by `function`/`unsafe function` is a function declaration (via new `peek_two_ahead`/`peek_lexeme` helpers), and member names after `.` accept the `load`/`store` keyword lexemes (`expect_member_name`). So both `mem.load call` and `p load` (typed word) parse correctly.
>
> Verified: `mem.alloc`/`mem.store`/`mem.load`/`mem.free` round-trip (alloc → raw word store → raw word load → free) and typed `pointer<T>` access via an `alloc` result; all four functions reject safe-context calls with E370; existing typed-word and Stage 4 tests still pass.

### 5.3 Item-import resolution (completes rule 1 of `require`) ✅

**Done.** `RequiredModule` gained `item: Option<String>` (a single function imported from a parent module instead of a whole module file). `load_one` resolves via a parent-first resolver (`resolve_require`): `a.b.c` parses module `a.b` and, if it defines a top-level function `c`, imports only that function; otherwise the full path loads as a module file (`a/b/c.yar`). When both exist the function wins and the compiler warns (E-stderr) about the ambiguity. `register_module_bindings` binds only the item for item imports (plain name, or the single member under an item scope via `item_aliases`, which rejects other members with E330), and whole-module imports widen a previously item-only import. `ModuleLoader` gained `try_load` for probing. Both forms verified per [1.4](#14-modules-and-require-syntax): `"std.io" io require` (`io.write_line call`) and `"std.math.sqrt" require` (`sqrt call`); `std_smoke`, mem/gate, and Stage 4 regressions unchanged; clippy clean.

### 5.4 Other standard modules

| Module       | Safety                                                     | Contents                                                                                                                   |
| ------------ | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `std.io`     | Safe where the API can preserve Yarrow's safety guarantees | `write_line`, `print`, `print_int`, `print_float`; low-level OS pointer/syscall interfaces remain unsafe where appropriate |
| `std.string` | Safe                                                       | `len`, `join`, comparisons                                                                                                 |
| `std.list`   | Safe                                                       | `push`, `get`, `set`, `len`                                                                                                |
| `std.map`    | Safe                                                       | `get`, `set`, `len`                                                                                                        |
| `std.region` | Safe                                                       | `make_region`, `put_region`, `free_region` where the compiler can establish ownership                                      |
| `std.math`   | Safe                                                       | `sqrt` and friends (pure arithmetic)                                                                                       |
| `std.fs`     | Safe high-level file ops                                   | `open_file`, `close_file`, `read_line`; low-level raw OS interfaces may be unsafe internally                               |

### 5.5 Safe abstractions over unsafe code

The standard library and future libraries should be encouraged to use this pattern:

```text
unsafe implementation
        ↓
establish invariant
        ↓
safe public API
```

For example:

```text
raw allocator
    ↓
unsafe arena implementation
    ↓
safe arena API
```

The user should not need `unsafe` merely because an implementation internally uses raw memory.

This is an important part of keeping the language pleasant while still allowing systems-level programming.

### 5.6 Host surface cleanup

- Delete `emit_builtin` and the now-redundant runtime container/string/print helpers where the pure-Yarrow std replaces them; keep only the tiny host surface.
- The raw memory words (`@alloc`/`@free`, raw `@load`/`@store`) are **not** deleted: `std.mem` is built on them, and they are the only `@`-builtins that survive in user-visible form behind `std.mem`.
- Honor `require` alias-vs-main-scope semantics everywhere.

### Stage 5 files

```text
crates/yarrow-core/build.rs          (new)
crates/yarrow-core/lib/std/          (new .yar sources)
crates/yarrow-core/src/compiler/modules.rs
crates/yarrow-core/src/compiler/mod.rs
crates/yarrow-core/src/runtime.rs
```

### Stage 5 gate

`docs/syntax.yar` compiles and runs end-to-end, including the `unsafe` regions and `mem.*` calls in `pointer_function`.

## Stage 6 — Remaining spec conformance

Continue with:

- smallest-fit literal typing;
- `drop` semantics;
- 128-bit numbers;
- float `%`/`^`;
- `run_main` coverage;
- remaining spec conformance;
- full `docs/syntax.yar` execution.

Also verify that unsafe constructs are included in the final conformance suite.

## Stage 6 — Ownership/borrow validation

Before calling the ownership system complete, explicitly test:

### Use-after-move

```text
move
use moved value
```

must produce a compile-time error.

The current plan contains a contradiction here: some sections say this is enforced while another says the compile-time error is still missing. That should be resolved in favor of the language specification: **safe use-after-move must be rejected at compile time.**

### Borrowed mutation

Mutation that violates the borrow rules must fail.

### Borrowed pop

Popping an owned value while it has an active safe borrow must fail.

### Region escape

A safe reference must not survive the region/owner from which it originates.

### Unsafe pointer escape

Raw pointer validity is not checked by the safe borrow system; misuse is the responsibility of the unsafe programmer.

---

# Part 4 — Open design questions

The previous pointer alias/lifetime question is **resolved**:

> Raw pointers are unsafe and are not subject to Rust-style lifetime/alias checking.

Remaining questions should be limited to implementation/spec details such as:

- exact diagnostics for unsafe operations → **resolved**: `E370 "'{what}' requires an unsafe context"`.
- whether nested `unsafe` blocks are allowed → **resolved**: yes, harmless (depth counter).
- whether `unsafe` is permitted in all block positions where ordinary blocks are permitted → **resolved**: `unsafe` is a statement form usable anywhere a statement is.
- exact metadata representation for `Safe | Unsafe` → **resolved**: `Safety { Safe, Unsafe }` on `HostFn`; `unsafe_depth: u32` on `FnState`; `is_unsafe: bool` on `Function`.
- whether low-level OS functions should be unsafe or whether safe wrappers should be the only public interface;
- whether `pointer<void>` may be used with raw word operations → **resolved**: `@load`/`@store` take plain `i64` addresses, so `pointer<void>` is not needed for raw words.
- exact integer/pointer conversion rules.

These should not expand into a general raw-pointer lifetime system.

---

# Part 5 — Definition of done

The compiler is feature-complete when:

1. `docs/syntax.yar` is the language source of truth.
2. The new tokenizer/parser/compiler pipeline is complete.
3. Stack ownership and safe borrowing are compile-time checked.
4. Regions provide structured heap lifetime management.
5. Safe references cannot escape their owner/region.
6. No user-visible explicit lifetime parameters are required for the normal ownership model.
7. `pointer<T>` provides raw-memory capability.
8. Raw memory operations require `unsafe`.
9. `unsafe function` declarations require callers to enter an unsafe block.
10. Unsafe blocks use the syntax:

```yarrow
unsafe
    ...
end
```

11. Unsafe does not disable normal type, stack, ownership, or borrow checking.
12. Raw pointers remain non-owning and are not subjected to a Rust-style lifetime/alias checker.
13. The std library is pure Yarrow except for the deliberately tiny host surface.
14. Unsafe host/compiler primitives cannot bypass the unsafe boundary.
15. Safe abstractions can encapsulate unsafe implementations.
16. `require` uses the current syntax `"<path>" [<scope>] require` and its resolution rules (see 1.4).
17. `docs/syntax.yar` compiles and runs.
18. `cargo check` and `cargo clippy` remain green.

---

# Part 6 — Building and running

```text
cargo check
cargo clippy
cargo run -- <file.yar>
```

must remain green after every stage.

The driver lives at `src/main.rs` (the `yarrow-cli` crate is an empty stub); modules required from user files resolve relative to the source file's directory.
