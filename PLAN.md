# Yarrow Implementation Plan

Status of the language as specified in `docs/syntax.yar` versus the implemented pipeline in `crates/yarrow-core` (tokenizer → parser → compiler → Cranelift JIT).

`docs/syntax.yar` is the source of truth.

The compiler uses a stack/ownership/region model rather than explicit user-visible lifetime parameters. Safe references are validated through ownership, borrow, scope, and region information. Raw pointers are explicitly unsafe and do not participate in the safe borrow/lifetime model.

---

## Pipeline status

- **Tokenizer** — complete for the current spec.
- **Parser** — complete for the current AST.
- **Compiler** — Stages 0–4 implemented; Stage 5 is the next major step.
- **Runtime** — complete for the current host heap and runtime objects.
- **Std library** — migration to pure-Yarrow source files is pending.
- **Unsafe memory model** — syntax and compiler enforcement must be completed as part of the memory/stdlib work described below.

---

# Architecture notes

## Values and memory

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

---

# Ownership model

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

---

# Safe references vs raw pointers

These are deliberately different concepts.

## `reference<T>`

A safe borrow.

The compiler tracks:

```text
owner
  ↓
borrow
  ↓
reference<T>
```

and ensures the reference cannot outlive the owner or its region.

## `pointer<T>`

A raw, non-owning address.

The compiler knows its static pointee type but does **not** promise that the address:

- is valid,
- is aligned,
- points to a live object,
- is within its original allocation,
- does not alias another pointer,
- or remains valid after the allocation is freed.

Those guarantees are the programmer's responsibility inside an `unsafe` context.

This deliberately avoids recreating Rust's complete lifetime/provenance system for raw pointers.

---

# Unsafe memory model

Yarrow is safe by default.

Operations that bypass the normal ownership/borrow guarantees require an explicit unsafe context.

The language uses two separate concepts:

```yarrow
foo unsafe function
```

and:

```yarrow
unsafe
    ...
end
```

They have different meanings.

## `unsafe function`

Marks an API as requiring explicit acknowledgement from its caller.

An unsafe function may only be called while compiling inside an `unsafe` block.

Example:

```yarrow
pointer_function unsafe function do
    ...
end
```

A normal call:

```yarrow
pointer_function call
```

is a compile-time error.

The caller must write:

```yarrow
unsafe
    pointer_function call
end
```

## `unsafe ... end`

Creates a lexical unsafe context.

Example:

```yarrow
unsafe
    p load
    p 42 store
end
```

Leaving `end` immediately restores the previous safe context.

`unsafe` is not a runtime operation.

---

# Unsafe does not disable safety checking

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

is allowed.

But:

```text
unsafe
    invalid safe borrow
```

is still rejected.

The meaning of `unsafe` is:

> The programmer accepts responsibility for guarantees that the compiler cannot establish for the unsafe operation.

It does **not** mean:

> Turn off Yarrow's safety checker.

---

# Unsafe operations

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

The compiler maintains an unsafe context while compiling a function body.

Conceptually:

```text
unsafe_depth == 0
    → safe context

unsafe_depth > 0
    → unsafe context
```

An unsafe operation encountered while `unsafe_depth == 0` produces a compile-time error.

---

# Unsafe functions

Function metadata gains an unsafe flag:

```text
Function {
    ...
    is_unsafe: bool
}
```

A call to an unsafe function is rejected unless the current compilation context is unsafe.

The unsafe requirement does **not** automatically propagate to the caller.

For example:

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

`bar` remains a normal safe function.

This keeps unsafe API boundaries explicit at call sites.

---

# Unsafe function bodies

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

---

# Raw pointers

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

---

# Unsafe pointer operations

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

Pointer member access and write-through operations through `pointer<T>` are also unsafe.

Example:

```yarrow
unsafe
    cp.value
    cp.value 123 set
end
```

---

# Allocation and deallocation

Raw allocation and deallocation are unsafe.

Therefore:

```text
alloc → UNSAFE
free  → UNSAFE
```

The compiler must not allow ordinary safe code to manually allocate/free raw memory.

For example:

```yarrow
32 mem.alloc
```

outside an unsafe block is a compile-time error.

This does not prevent safe allocation APIs from being built later. A safe abstraction may internally use unsafe memory management and expose only a safe interface.

---

# Raw memory host primitives

The compiler-level primitives:

```text
@alloc
@free
@load
@store
```

are themselves unsafe.

They cannot be used to bypass the unsafe mechanism.

For example:

```yarrow
foo function do
    32 @alloc
end
```

must fail.

Whereas:

```yarrow
foo function do
    unsafe
        32 @alloc
    end
end
```

is permitted.

The eventual standard library should hide these compiler-level primitives from normal user code where practical.

---

# Host registry

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

# Stage 0 — Restore the build ✅

Port the compiler to the new AST.

**Done.**

No changes required.

---

# Stage 1 — New operators ✅

**Done.**

- `unrot`
- `typeof`
- `borrow`
- `move`
- ownership/borrow tracking
- use-after-move tracking

The existing ownership infrastructure remains the basis for safe references.

---

# Stage 2 — Control flow ✅

**Done.**

No changes required.

---

# Stage 3 — Unions ✅

**Done.**

No changes required.

Union member borrows continue to use the normal safe `reference<T>` model.

---

# Stage 4 — Memory access and unsafe operations

This stage establishes the complete boundary between Yarrow's safe ownership model and its raw memory facilities.

## 4.1 `pointer<T>`

Enable:

```text
pointer<T>
```

as a typed raw address.

`Ty::Ptr(...)` carries compile-time pointee information.

`pointer<void>` / unknown pointee representations cannot be loaded/stored through typed operations unless explicitly using the permitted raw-memory form.

---

## 4.2 Unsafe context

Add parser/compiler support for:

```yarrow
unsafe
    ...
end
```

Implementation:

- add an unsafe context to `FnState` / compiler context;
- enter unsafe context when compiling an `unsafe` block;
- restore the previous context after the block;
- nested unsafe blocks are harmless;
- unsafe context is lexical;
- unsafe context never survives the block's `end`.

---

## 4.3 Unsafe function modifier

Add:

```yarrow
foo unsafe function
```

to the parser/AST/function representation.

Function metadata records:

```text
is_unsafe
```

At every call:

```text
callee.is_unsafe && !unsafe_context
    → compile-time error
```

The caller must explicitly enter:

```yarrow
unsafe
    foo call
end
```

---

## 4.4 Unsafe operation metadata

Introduce a safety classification for compiler operations and host functions:

```text
Safe
Unsafe
```

Unsafe operations are rejected outside an unsafe context.

This must be implemented centrally so that future unsafe primitives cannot accidentally bypass the check.

---

## 4.5 Pointer load/store

Implement:

```yarrow
p load
addr value store
```

and pointer member auto-dereference/write-through:

```yarrow
p.field
p.field value set
```

These operations require unsafe context.

---

## 4.6 Pointer arithmetic

Implement:

```text
pointer + int
```

as byte-offset arithmetic while preserving the pointer type.

Pointer arithmetic requires unsafe context.

---

## 4.7 Raw memory access

Raw operations:

```text
@load
@store
```

require unsafe context.

They must not provide a safe-code escape hatch.

---

## 4.8 Allocation and free

Expose:

```text
@alloc
@free
```

through the generic host bridge.

Both require unsafe context.

Pointers themselves remain non-owning values and are excluded from automatic scope destruction.

---

## 4.9 Raw pointer lifetime/alias rules

Do **not** implement a Rust-style raw-pointer lifetime/alias checker.

Specifically, do not require the compiler to prove:

```text
pointer does not alias borrowed value
pointer cannot outlive allocation
pointer remains inside region
```

for arbitrary raw pointers.

Those guarantees belong to unsafe code.

The compiler continues to enforce the normal ownership/borrow/region rules for **safe references**.

This is a deliberate design decision, not a deferred feature.

---

## 4.10 Safety invariants

The following must hold:

### Safe code

Cannot:

```text
raw load
raw store
pointer dereference
pointer arithmetic
raw alloc
raw free
call unsafe function
```

### Unsafe code

Can perform those operations.

### Both safe and unsafe code

Must still obey:

```text
type checking
stack checking
ordinary ownership checking
ordinary borrow checking
ordinary move checking
control-flow checking
```

---

## Stage 4 files

Expected files:

```text
docs/syntax.yar
tokenizer/
parser/
ast.rs
compiler/mod.rs
compiler/types.rs
runtime.rs
```

Potentially:

```text
compiler/functions.rs
compiler/context.rs
```

depending on the final compiler organization.

---

## Stage 4 gate

The following must compile:

```yarrow
pointer_function unsafe function do
    unsafe
        ...
    end
end
```

and:

```yarrow
unsafe
    pointer_function call
end
```

The following must fail at compile time:

```yarrow
pointer_function call
```

and:

```yarrow
p load
```

when outside an unsafe block.

An unsafe block must **not** allow an otherwise-invalid borrow or ownership operation.

---

# Stage 5 — Std library in pure Yarrow

Move the std library to:

```text
crates/yarrow-core/lib/std/
```

using the existing `build.rs`/`STD_MODULES` architecture.

---

## `std.mem`

`std.mem` exposes the raw memory API, but its functions are explicitly unsafe.

Conceptually:

```yarrow
alloc unsafe function
free unsafe function
load unsafe function
store unsafe function
```

Their implementation uses the compiler-level raw primitives inside unsafe blocks.

For example:

```yarrow
alloc unsafe function do
    unsafe
        ...
    end
end
```

End users therefore must write:

```yarrow
unsafe
    32 mem.alloc call
end
```

instead of silently entering manual memory management.

The compiler-level raw primitives remain an implementation substrate and cannot bypass unsafe checking.

---

## Other standard modules

### `std.io`

Safe where the API can preserve Yarrow's safety guarantees.

Low-level OS pointer/syscall interfaces remain unsafe where appropriate.

### `std.string`

Safe abstractions over strings.

### `std.list`

Safe list operations.

### `std.map`

Safe map operations.

### `std.region`

Safe region-management abstractions where the compiler can establish ownership.

### `std.math`

Safe arithmetic.

### `std.fs`

Safe high-level file operations; low-level raw OS interfaces may be unsafe internally.

---

# Safe abstractions over unsafe code

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

---

# Stage 6 — Remaining spec conformance

Continue with:

- smallest-fit literal typing;
- `drop` semantics;
- 128-bit numbers;
- float `%`/`^`;
- `run_main` coverage;
- remaining spec conformance;
- full `docs/syntax.yar` execution.

Also verify that unsafe constructs are included in the final conformance suite.

---

# Stage 6 ownership/borrow validation

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

# Open design questions

The previous pointer alias/lifetime question is **resolved**:

> Raw pointers are unsafe and are not subject to Rust-style lifetime/alias checking.

Remaining questions should be limited to implementation/spec details such as:

- exact diagnostics for unsafe operations;
- whether nested `unsafe` blocks are allowed;
- whether `unsafe` is permitted in all block positions where ordinary blocks are permitted;
- exact metadata representation for `Safe | Unsafe`;
- whether low-level OS functions should be unsafe or whether safe wrappers should be the only public interface;
- whether `pointer<void>` may be used with raw word operations;
- exact integer/pointer conversion rules.

These should not expand into a general raw-pointer lifetime system.

---

# Definition of done

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
16. `docs/syntax.yar` compiles and runs.
17. `cargo check` and `cargo clippy` remain green.

---

# Building and running

```text
cargo check
cargo clippy
cargo run -- <file.yar>
```

must remain green after every stage.

---

## The resulting memory architecture

The key design decision behind the updated plan is:

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

This is the part I would consider the **central architectural principle of the implementation plan**: Yarrow's compiler does the hard lifetime reasoning for structured ownership, while raw memory is deliberately moved behind an explicit unsafe boundary rather than attempting to reproduce Rust's general lifetime machinery.
