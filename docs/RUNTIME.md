# Runtime

How a Yarrow program executes: evaluation stack, calls, errors, and modules. Complements [`TYPE_SYSTEM.md`](TYPE_SYSTEM.md) and [`MEMORY_MODEL.md`](MEMORY_MODEL.md). Surface forms come from [`GRAMMAR.md`](GRAMMAR.md) and [`SYNTAX.md`](SYNTAX.md).

Contents: [Execution model](#execution-model), [Stack](#stack), [Functions](#functions), [Errors](#errors), [Modules](#modules).

## Execution model

Pipeline: source `.yar` is tokenized, parsed to an AST, checked, then run or emitted via one of `check`, `jit`, `object`, `executable`, or `interpret`.

**Trivia:** The tokenizer emits `Comment` tokens for `#` … end of line (lexeme includes `#` and comment text; the terminating newline is not part of the lexeme). Whitespace and newlines are not tokens. The parser skips `Comment` tokens the same way it ignores whitespace; comments are not AST nodes. Tools that need comment text (formatters) should read the token stream before parse.

| Backend      | Role                                                                |
| ------------ | ------------------------------------------------------------------- |
| `check`      | Type / ownership / stack / region analysis; CLIF lower without JIT/object product |
| `jit`        | Cranelift in-process machine code; driver may run `main`            |
| `object`     | Relocatable native object (ELF / Mach-O / COFF); link stays outside |
| `executable` | Object emit + system `ld`/`lld` link with the runtime archive       |
| `interpret`  | Tree-walk interpreter over the checked AST (file / future REPL)     |

- User modules resolve relative to the source file’s directory (`"a.b"` → `a/b.yar`).
- The standard library is embedded and imported the same way as user code (`"std.io"`, …).
- Compiled and interpreted code talks to a small **host runtime** for heap headers (strings, lists, maps, regions, free) and raw `alloc` / `free`. Object emit leaves those symbols as imports for a later link.
- Heap values are opaque handles; scalars and addresses are machine words. Kind codes describe how to free nested heap data.

### AOT link surface (host runtime)

Object emit (`Session::compile_object_source`) lowers `@name` / host calls to **`Linkage::Import`** symbols. Names and C ABIs come from the [`HOST_FNS`](../../crates/yarrow-runtime/src/lib.rs) table in `yarrow_runtime` (single source of truth with JIT `install_runtime`).

| Layer          | Crate / API                          | Role                                                                                 |
| -------------- | ------------------------------------ | ------------------------------------------------------------------------------------ |
| Implementation | `yarrow_runtime` (rlib)              | Heap, `@print_*`, regions, `HOST_FNS`                                                |
| JIT            | `runtime::install_runtime`           | Registers `HOST_FNS` names → addresses in the in-process JIT linker                  |
| AOT archive    | `yarrow_runtime_aot` (`staticlib`)   | Same code, `aot-exports` feature adds linker-visible names (`alloc`, `print_str`, …) |
| Library access | `yarrow_core::linkable_archive` / `linkable_archive_for` | Reads `libyarrow_runtime_aot.a` (host or per-triple; see Stage 26) for AOT link |
| Executable     | `Session::compile_executable_source` | Object emit + `link::link_executable` via system `ld`/`lld` (not `cc`)               |

Build the archive: `cargo build -p yarrow_runtime_aot`. Program `.o` (with Cranelift process `main`) + runtime `.a` are linked by `compile_executable_source`. No C CRT source and no `cc` compile step; CRT object paths may be discovered with `cc -print-file-name` when present.

**Imports in a typical program object** (all defined by the runtime archive):

| Symbol                                                              | Role                                  |
| ------------------------------------------------------------------- | ------------------------------------- |
| `alloc`, `free`                                                     | Raw heap (`@alloc` / `@free`, unsafe) |
| `str_new`, `str_len`, `str_join`, `str_cmp`                         | String heap helpers (`std.string`)    |
| `fs_open`, `fs_close`, `fs_read`, `fs_write`, `fs_last_error`         | File open / close / read / write (`std.fs`) |
| `list_*`, `map_*`                                                   | List / hashmap helpers                |
| `print_str`, `print_int`, `print_float`, `print_newline`            | `std.io` write / write_line / newline |
| `print_array`, `print_list`, `print_hashmap`                        | Container debug print                 |
| `free_value`, `register_struct_descs`, `register_union_descs`       | Drop / layout registration            |
| `region_new`, `region_register`, `region_free`                      | Region lifetime                       |

**Std wrappers (safe Yarrow):**

| Module       | API                                                                 | Host / builtin                                      |
| ------------ | ------------------------------------------------------------------- | --------------------------------------------------- |
| `std.io`     | `write`, `write_line`, `write_int`, `write_float`, `newline`        | `@print` / `@print_*` → `print_str` / `print_*`     |
| `std.string` | `len`, `concat`, `join` (left, right, sep), `compare` (−1 / 0 / 1) | `@string_len`, `~`, `@string_join`, `@str_cmp`      |
| `std.fs`     | `open_file`, `close_file`, `read_file`, `write_file`                | `@fs_open` / `@fs_close` / `@fs_read` / `@fs_write` / `@fs_last_error` |

Prefer alias `str` for `"std.string"` (`string` is a type keyword and cannot be a require scope name).

`std.fs` modes: `'r'` read, `'w'` write+create+truncate, `'a'` append+create. Host status codes: `0` ok, `1` IO, `2` not found, `3` invalid argument (`fs_open` returns the negated code on failure). Fallible wrappers map those to `error.IO_ERROR` / `NOT_FOUND` / `INVALID_ARGUMENT`.

`free` is exported under that linker name for object imports; on glibc the implementation forwards to `__libc_free` so it does not recurse into itself when other translation units call `free`.

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

**Link:** `Session::compile_executable_source` links program `.o` + `libyarrow_runtime_aot.a` with `ld`/`lld` (not `cc`). Diagnostics: `E394` linker/CRT missing, `E395` link failed, `E396` runtime archive unavailable, `E397` unsupported target. Default target is the host linux-gnu triple; see [Cross-compile triples](#cross-compile-triples-stage-26).

### AOT debug info and optimization (Stage 25)

Object and executable products honor two [`CompileOptions`](../../crates/yarrow-core/src/session.rs) knobs (CLI wiring comes later):

| Option | Default | Effect |
| ------ | ------- | ------ |
| `opt_level` | `OptLevel::None` | Cranelift `opt_level`: `none` / `speed` / `speed_and_size` (`OptLevel::Size`) |
| `debug_info` | `true` | Emit DWARF (`.debug_info` / `.debug_line` / …) into the program object |

DWARF includes a compilation unit for the source path, `DW_TAG_subprogram` entries for defined functions (Yarrow names plus process `main`), and coarse line mappings at function entries when spans exist. Inspect with `llvm-dwarfdump` or `readelf --debug-dump=info`.

JIT uses the same `opt_level` (default stays debug-friendly `None`). JIT does not emit DWARF.

### Cross-compile triples (Stage 26)

Object emit and executable link take an optional [`CompileOptions::target`](../../crates/yarrow-core/src/session.rs) ([`TargetTriple`](../../crates/yarrow-core/src/target.rs)). `None` means the host.

| Triple | Role |
| ------ | ---- |
| Host (`x86_64-unknown-linux-gnu` or `aarch64-unknown-linux-gnu`) | Default object / executable path |
| The other of those two | First cross target: object emit always; link when archive + CRT are available |

Unsupported triples (musl, Mach-O, Windows, …) fail with `E397` (no panic). JIT rejects a non-host `target` with `E397`. Object emit for aarch64 requires the `arm64` feature on `cranelift-codegen` (enabled by `yarrow_core`).

**Session example** (non-host object; no CLI flag yet):

```rust
use yarrow_core::{CompileOptions, ExecutionMode, Session, TargetTriple};

let mut opts = CompileOptions::new("hello.yar");
opts.mode = ExecutionMode::Object;
opts.target = Some(TargetTriple::parse("aarch64-unknown-linux-gnu").expect("supported"));
let session = Session::new(opts);
let artifact = session.compile_object_source(source)?;
// artifact.target is aarch64-unknown-linux-gnu; bytes are AArch64 ELF
```

Inspect: `readelf -h hello.o` should show `Machine: AArch64` when targeting aarch64 from an x86_64 host (or the reverse).

**Runtime archive layout** (same ABI / `HOST_FNS` as the host runtime; do not invent a second runtime):

| How | Path / env |
| --- | ---------- |
| Host (always) | Built by `yarrow-core`’s `build.rs` → `YARROW_RUNTIME_AOT_ARCHIVE` |
| Optional cross at build | Set `YARROW_BUILD_CROSS_AOT=1` when building `yarrow-core` after installing the Rust target (`rustup target add …` or Nix equivalent). Successful builds appear in `YARROW_RUNTIME_AOT_ARCHIVE_TABLE` as `triple=path;…` |
| Manual / CI override | `YARROW_RUNTIME_AOT_ARCHIVE_<triple_with_underscores>` → `libyarrow_runtime_aot.a` from `cargo build -p yarrow_runtime_aot --target <triple>` |

Example override for aarch64:

```bash
cargo build -p yarrow_runtime_aot --target aarch64-unknown-linux-gnu
export YARROW_RUNTIME_AOT_ARCHIVE_aarch64_unknown_linux_gnu=$PWD/target/aarch64-unknown-linux-gnu/debug/libyarrow_runtime_aot.a
```

**CRT / linker for cross executables:** host `ld` must support the target’s `-m` emulation (`elf_x86_64` / `aarch64linux`). CRT objects come from the usual `cc -print-file-name` path on the host, or for a non-host triple:

| Env | Meaning |
| --- | ------- |
| `YARROW_AOT_CRT_DIR` | Directory containing `Scrt1.o` / `crti.o` / … for the target |
| `YARROW_AOT_SYSROOT` | Sysroot searched under `usr/lib`, `lib`, and multiarch subdirs |

Missing archive → `E396`. Missing CRT / linker → `E394`. Broader matrices (musl, Mach-O, Windows) stay later.

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
