# Runtime

How a Yarrow program executes: evaluation stack, calls, errors, and modules. Complements [`TYPE_SYSTEM.md`](TYPE_SYSTEM.md) and [`MEMORY_MODEL.md`](MEMORY_MODEL.md). Surface forms come from [`GRAMMAR.md`](GRAMMAR.md) and [`SYNTAX.md`](SYNTAX.md).

```
Runtime
├── Stack
├── Functions
├── Errors
└── Modules
```

## Execution model

Pipeline:

```text
source .yar  →  tokenize  →  parse (AST)  →  check  →  { jit | object | interpret }
```

| Backend     | Role                                                                |
| ----------- | ------------------------------------------------------------------- |
| `check`     | Type / ownership / stack / region analysis; no machine code         |
| `jit`       | Cranelift in-process machine code; driver may run `main`            |
| `object`    | Relocatable native object (ELF / Mach-O / COFF); link stays outside |
| `interpret` | Tree-walk interpreter over the checked AST (file / future REPL)     |

- User modules resolve relative to the source file’s directory (`"a.b"` → `a/b.yar`).
- The standard library is embedded and imported the same way as user code (`"std.io"`, …).
- Compiled and interpreted code talks to a small **host runtime** for heap headers (strings, lists, maps, regions, free) and raw `alloc` / `free`. Object emit leaves those symbols as imports for a later link.
- Heap values are opaque handles; scalars and addresses are machine words. Kind codes describe how to free nested heap data.

### AOT link surface (host runtime)

Object emit (`Session::compile_object_source`) lowers `@name` / host calls to **`Linkage::Import`** symbols. Names and C ABIs come from the [`HOST_FNS`](../../crates/yarrow-runtime/src/lib.rs) table in `yarrow_runtime` (single source of truth with JIT `install_runtime`).

| Layer          | Crate / API                        | Role                                                                                 |
| -------------- | ---------------------------------- | ------------------------------------------------------------------------------------ |
| Implementation | `yarrow_runtime` (rlib)            | Heap, `@print_*`, regions, `HOST_FNS`                                                |
| JIT            | `runtime::install_runtime`         | Registers `HOST_FNS` names → addresses in the in-process JIT linker                  |
| AOT archive    | `yarrow_runtime_aot` (`staticlib`) | Same code, `aot-exports` feature adds linker-visible names (`alloc`, `print_str`, …) |
| Library access | `yarrow_core::linkable_archive()`  | Reads `libyarrow_runtime_aot.a` (path from build) for Stage 19 link                  |

Build the archive: `cargo build -p yarrow_runtime_aot`. Program `.o` (with Cranelift process `main`, Stage 18) + runtime `.a` link with a system linker (Stage 19). No C CRT and no `cc` compile step.

**Imports in a typical program object** (all defined by the runtime archive):

| Symbol                                                              | Role                                  |
| ------------------------------------------------------------------- | ------------------------------------- |
| `alloc`, `free`                                                     | Raw heap (`@alloc` / `@free`, unsafe) |
| `str_*`, `list_*`, `map_*`                                          | Container helpers                     |
| `print_str`, `print_int`, `print_float`, `print_newline`, `print_*` | Std I/O backing                       |
| `free_value`, `register_struct_descs`, `register_union_descs`       | Drop / layout registration            |
| `region_new`, `region_register`, `region_free`                      | Region lifetime                       |

Do not export those names from the JIT driver binary (`aot-exports` is AOT-only); a global `alloc` symbol would clash with the host allocator.

### Program entry / process `main`

A Yarrow program entry (default name `main`, override via `CompileOptions::entry_name`) is **not** the same symbol as the host process entry.

| Piece         | API / symbol                                                 | Role                                                                                                       |
| ------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Entry name    | `CompileOptions::entry_name` (default `main`)                | Which top-level Yarrow function is the program entry. CLI `--main` maps here (CLI plan).                   |
| Require / run | Session require-entry, JIT `run_main`, interpret `run_entry` | All honor `entry_name`. Missing entry is `E360` naming that function.                                      |
| Object export | `PROCESS_MAIN_SYMBOL` (`main`)                               | Cranelift-emitted process entry (`() -> i32`): calls the Yarrow entry and maps the return to an exit code. |

Exit mapping (process `main` trampoline):

- void / non-integer single return → `0`
- integer (including bool / enum) → value as exit status
- fallible envelope → `1` on error tag, else `0`

**Link target (Stage 19):** program `.o` + `libyarrow_runtime_aot.a` only, via `ld`/`lld` (not `cc`). No C CRT source and no `cc` compile step.

The language model is stack-based regardless of backend. JIT and object backends lower each function to Cranelift IR with an explicit compile-time operand stack that becomes SSA values. The interpreter keeps an explicit runtime operand stack instead.

Entry: every runnable program has a top-level entry (default `main`; override via `CompileOptions::entry_name` / CLI `--main`). The driver runs it after JIT or interpret (object emit does not execute). Optional numeric return from the entry is the process exit code for native binaries; the current CLI also prints supported single return values (`void`, integer, float, bool, string).

---

## Stack

The **evaluation stack** is the primary place values live between words.

### Effects

| Kind                                               | Typical effect                                                     |
| -------------------------------------------------- | ------------------------------------------------------------------ |
| Literal, container, name, type value               | push                                                               |
| Binary op, `store`, `move`                         | pop 2 (+ push result or nothing)                                   |
| Unary op, `typeof`, `borrow`, `load`, `dup`, `pop` | pop 1 (+ push)                                                     |
| `call` / `unwrap`                                  | pop callee (+ use preceding args); push returns / project envelope |
| `drop`                                             | clear stack; release borrows                                       |
| `swap` / `rot` / `unrot`                           | rearrange                                                          |

Declarations and `set` pop the value being stored. Control keywords (`if`, `for`, `match`) expect their subject or condition already on top.

### Ownership on the stack

- Stack **owns** temporary non-copy values it creates.
- `pop` / consume / `drop` free owned heap slots (see [`MEMORY_MODEL.md`](MEMORY_MODEL.md)).
- Variable reads of non-copy types push **borrows**, not second owners.
- `return` takes the return payload(s) and drops the rest of the frame’s stack.

### Control flow and the stack

- **`if`**: condition bool consumed; then/else must leave compatible stacks at join.
- **`match`**: subject borrowed for the duration; original stack restored after `end`.
- **`for`**: condition form rechecks a bool; iterable form walks a container (helpers from `std.loop`).
- **`defer`**: bodies run at scope exit in reverse order; they see the exiting scope’s bindings.
- **`unsafe`**: does not change stack discipline; only permits unsafe ops inside the block.

Stack height and types are checked at compile time; a runtime stack underflow in generated code indicates a compiler bug, not a user-recoverable error.

---

## Functions

### Definition

```text
name [visibility] [unsafe] function { parameter } do { statement } end [with type]
```

- Parameters are the **input stack**: moved onto the local stack in declaration order (first = deepest). Body bindings pop from the top, so the last parameter binds first.
- `with T` is the return type; omit for `void`.
- Nested functions are allowed; they are only callable from the enclosing body.
- Methods live in `Type implement … end` and usually take `reference<T>` (optionally `mutable`) as the first parameter.

### Call convention (language)

```yarrow
arg1 arg2 callee call
```

1. Push arguments (deepest first, matching parameter declaration order).
2. Push the callee (name or qualified name).
3. `call` pops the callee, consumes arguments per the signature, transfers or borrows per parameter rules, runs the body, pushes return value(s).

Methods:

```yarrow
point borrow
point.distance call
```

The receiver is a `reference<T>` on the stack before `call` (often from `borrow` or a non-copy read).

### `unsafe` functions

- Marked `unsafe function`: may contain unsafe operations; **call sites** must be inside `unsafe … end`.
- Even inside an unsafe function, unsafe ops are wrapped in `unsafe … end` so the site is visible.
- Borrow, ownership, and stack checks still apply.

### Host and builtins

A thin host surface backs heap ops and raw memory (`alloc`, `free`, string/list/map helpers, region register/free, …). Host entries are marked **Safe** or **Unsafe**; unsafe host calls require an unsafe context.

Std modules (`std.mem`, `std.io`, …) wrap host behavior in Yarrow where possible so user code stays in the safe model.

### `main` (program entry)

- Required for runnable sessions (`require_main`); public by default; no parameter list in surface syntax.
- Default name is `main`; `CompileOptions::entry_name` (CLI `--main`) may select another top-level function.
- Return optional; numeric return may set the process exit code (native process `main` / grammar).
- Fallible entry (`with |T Err|`) is not part of the supported driver print surface yet; AOT trampoline maps error tags to exit `1`.

---

## Errors

Errors are first-class values, declared like specialized enums:

```yarrow
MyCustomErrors error
	MY_CUSTOM_ERROR
end
```

Optional injection copies members from another error type:

```yarrow
MyCustomErrors error.Error error
	# ...
end
```

### Fallible returns

A function that may fail returns a **union literal** of success and error types:

```yarrow
end with |i32 MyCustomErrors|
```

At the ABI level this is an **error envelope**:

| Slot    | Success                        | Failure                    |
| ------- | ------------------------------ | -------------------------- |
| env     | `0`                            | error tag (program-unique) |
| payload | success value (or `0` if void) | unused / zero              |

Returning an error member sets the env tag. Returning a success value sets env to `0` and places the value in the payload (heap ownership transfers to the caller).

### `unwrap`

```yarrow
fallible call unwrap
```

- Success: push the payload (`T`).
- Failure: if the **caller** can error, propagate (return the envelope); otherwise trap / rejected at compile time when the caller cannot error.
- On a non-envelope value, `unwrap` is a no-op (identity).

### `handle`

```yarrow
fallible call handle
	match
		error.MY_CUSTOM_ERROR case
			# ...
		end
		else
			# ...
		end
	end
	0 fallback
end
```

- Success: keep the payload; skip the handler body.
- Failure: run the handler (often a `match` on the error); then push the **fallback** word as the result of the whole `handle`.
- Short form: `call handle 0 fallback end`.

Fallback must be usable at the success type (coercion allowed).

### Error `match`

Inside `handle`, `match` with no prior subject dispatches on the error the same way union `match` dispatches on member types (grammar: cases compare or name error members). Elsewhere, ordinary value `match` uses bool conditions.

Built-in and std error members (e.g. `error.OUT_OF_MEMORY`) are comparable tags across the program.

---

## Modules

### `require`

```text
"path" [alias] require
```

| Form                      | Meaning                                                    |
| ------------------------- | ---------------------------------------------------------- |
| `"std.io" io require`     | Import module into scope `io` (`io.write_line`)            |
| `"std.math" require`      | Import module bindings into the **current** scope          |
| `"std.math.sqrt" require` | **Item import**: only that function into the current scope |

- Keyword last.
- `require` is allowed at top level and inside function / method bodies (scoped imports).
- Private entities in a module file are not exported.

### Resolution

1. **Std**: dotted path matches an embedded `std.*` module (from `lib/std/**/*.yar` at build time).
2. **User**: under each search path, `"a.b.c"` → `a/b/c.yar`. The CLI adds the source file’s directory.
3. **Item import**: parent-first: `"a.b.c" require` may mean function `c` in module `a.b` rather than a nested module file; function wins over module when ambiguous (with a warning).

Imported modules are parsed and compiled into the **same** JIT module as the program, so `require` imports code, not only symbols.

### Visibility and names

- Default visibility for declarations and fields is private; `public` exports.
- `main` is public by default.
- Qualified names: `alias.entity`, `Type.MEMBER`, `error.TAG`, `module.fn`.

### Compilation unit

A complete runnable unit is:

1. The entry `.yar` with `main`
2. Every module reached by `require` (transitively)
3. Linked host imports declared by the runtime registry

---

## Interaction with memory and types

| Concern                                           | Doc                                  |
| ------------------------------------------------- | ------------------------------------ |
| What may sit on the stack / coerce                | [`TYPE_SYSTEM.md`](TYPE_SYSTEM.md)   |
| Who frees handles; borrows; regions; `pointer<T>` | [`MEMORY_MODEL.md`](MEMORY_MODEL.md) |
| AST shape of `call`, `handle`, `require`          | [`AST.md`](AST.md)                   |

Runtime invariants the implementation must keep:

- Kind codes and free recursion stay in sync between compiler and host.
- Error tags are interned per program so `==` and envelope propagation agree.
- Double free of a handle (region free then variable drop) is a no-op at the host.
- Unsafe host functions are unreachable from safe contexts.

When spec and code disagree, follow the grammar and these docs; close gaps via the crate `PLAN.md` stages.
