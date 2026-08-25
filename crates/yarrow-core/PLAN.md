# Yarrow Core Implementation Plan

Compiler library for Yarrow: tokenizer, parser, type/ownership checking, Cranelift lowering, host runtime, and diagnostics.

`yarrow-core` is the **API**. Other crates (`yarrow-cli`, later `yarrow-fmt` / `yarrow-lsp`) call it. They do not reimplement the pipeline. CLI flags, subcommands, and process exit codes live in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md).

## Source of truth

| Role                          | Path                                                 |
| ----------------------------- | ---------------------------------------------------- |
| Language tour (authoritative) | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md)           |
| Formal syntax                 | [`docs/SYNTAX.md`](../../docs/SYNTAX.md)             |
| Intended AST shape            | [`docs/AST.md`](../../docs/AST.md)                   |
| Types / checking              | [`docs/TYPE_SYSTEM.md`](../../docs/TYPE_SYSTEM.md)   |
| Ownership / unsafe            | [`docs/MEMORY_MODEL.md`](../../docs/MEMORY_MODEL.md) |
| Runtime / errors / modules    | [`docs/RUNTIME.md`](../../docs/RUNTIME.md)           |
| Source style                  | [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md)   |
| Conformance corpus            | [`docs/examples/`](../../docs/examples/README.md)    |
| Agent rules                   | [`AGENTS.md`](../../AGENTS.md)                       |

Prefer the docs when code and docs disagree. Do not invent language features absent from the docs.

---

## Current state

| Component   | Status | Notes                                                                                      |
| ----------- | ------ | ------------------------------------------------------------------------------------------ |
| Tokenizer   | ✅     | Full doc surface; UTF-8-safe                                                               |
| Parser/AST  | ✅     | Flat postfix `Apply*`; `Program::entry_function`; `StructLit` / `Map` / `EmptyMapOrStruct` |
| Compiler    | ✅     | JIT + check-only + object emit; ownership / borrow / region / unsafe                       |
| Diagnostics | ✅     | Rustc-style spans; stack-effect notes on underflow / join / return                         |
| Library API | ✅     | `Session`: check, JIT, object emit, interpret MVP                                          |
| AOT / link  | 🟡     | Runtime archive + `entry_name`; Cranelift `main` (18) then link without `cc` (19)          |
| Runtime     | 🟡     | Host heap, regions, lists, maps, strings; JIT via `install_runtime`; AOT via staticlib     |
| Std library | 🟡     | Core modules + intrinsics; `std.fs` stub; partial `io` / `string`                          |

**Gates today:** all `docs/examples/valid/**` compile and run (JIT); all `docs/examples/invalid/**` fail for the stated reason; `cargo fmt --all && cargo check && cargo clippy` green.

**Execution backends (landed):** `Check`, `Jit`, `Object` (relocatable bytes), `Interpret` (MVP). **Next:** Phase D turns object emit into a runnable host binary.

---

## Completed phases (summary)

### Phase A: Language surface (Stages 0–7) ✅

Tokenizer, parser, and compiler aligned with the current docs: `~` concat, `|T Err|` fallible returns, `error` types, visibility, `copy` params, std module names, ownership/borrow/region checks, grammar tour conformance.

### Phase B: Diagnostics (Stages 8–10) ✅

Full rustc-style rendering (`Span`, labels, notes, help), teachable error catalog (E373–E376, E370, E308, …), parser recovery and compiler multi-error collection with output cap.

### Phase C: Session API, backends, and frontend polish (Stages 11–15) ✅

| Stage | What landed                                                                                                 |
| ----- | ----------------------------------------------------------------------------------------------------------- |
| 11    | `CompileOptions` / `Session`; tokenize, parse, check, compile without duplicating the root driver           |
| 12    | Stack-effect notes on underflow / branch join / return imbalance                                            |
| 13a   | `ExecutionMode` + `CheckedProgram`; `check_source` without JIT install                                      |
| 13b   | Tree-walk `interpret_source` / `EvalContext` MVP (`01_hello`, `02_arithmetic_and_stack`)                    |
| 13c   | `cranelift-object` via `CodeModule`; `Session::compile_object_source` → `ObjectArtifact { bytes, ir }`      |
| 14    | Float smallest-fit (`float_literal_kind`); teachable `E334` for float `%` / `^`; `f16` as `f32` CLIF on x64 |
| 15    | Flat postfix `Apply*` / flat `Seq`; `Program::entry_function`; `StructLit` / `Map` / `EmptyMapOrStruct`     |

Pipeline: `tokenize → parse → check → { jit | object | interpret }` (see [`docs/RUNTIME.md`](../../docs/RUNTIME.md)). Object emit leaves host runtime symbols as `Linkage::Import`; linking and a runnable executable are Phase D.

---

## Known gaps (compiler scope)

These are remaining mismatches or thin areas inside this crate, not CLI work.

| Area        | Gap                                                                                               |
| ----------- | ------------------------------------------------------------------------------------------------- |
| AOT         | Program `.o` (incl. process `main`) + runtime `.a`; host link without `cc` remains (Stages 18–19) |
| Entry       | `CompileOptions::entry_name` landed; C CRT source to be replaced by Cranelift `main` (Stage 18)   |
| Backends    | Check still uses Cranelift as analysis vehicle; interpret MVP only; no DWARF / cross-compile      |
| Warnings    | No unused-binding / dead-stack / unused-require diagnostics                                       |
| Std/runtime | `std.fs` has no host I/O; `std.io` / `std.string` partial (runtime, not blocking compiler stages) |

---

## Phase D: Object / AOT to executable

Stage 13c emits a relocatable host object. Phase D makes that path produce a **real runnable binary** the CLI can write and exec (`compile --target object`, later `run --target object`).

Boundary:

| Layer         | Owns                                                                              |
| ------------- | --------------------------------------------------------------------------------- |
| `yarrow-core` | Runtime definitions, process entry in object emit, link helper → executable bytes |
| `yarrow-cli`  | Paths (`-o`), invoking the link API, process exec, exit codes, clap               |

Do **not** put clap or process exit codes in this crate. Phase D must **not** depend on a C compiler (`cc` / `gcc` / `clang` as a compile driver) or on compiling C CRT source. Invoking a system **linker** (`ld` / `lld` / …) from a library helper is allowed for Stage 19.

### Stage 16 — Linkable host runtime ✅

Today `HOST_FNS` are installed only into the JIT builder (`install_runtime`). Object files import the same names but nothing defines them for AOT.

1. Expose a **linkable** artifact for the host runtime (same symbols / `extern "C"` ABI as `HOST_FNS`): static archive, relocatable object(s), or equivalent bytes the linker can consume.
2. Single source of truth: symbol names and signatures stay aligned with `HOST_FNS` (no second handwritten export list that can drift).
3. Document the AOT link surface in [`docs/RUNTIME.md`](../../docs/RUNTIME.md) (what object emit imports; what the runtime provides).
4. Keep JIT `install_runtime` working unchanged.

**Gate:** runtime artifact is non-empty and exports the host symbols used by `01_hello.yar` (e.g. print helpers). `compile_object_source` still produces a valid `.o`. `cargo clippy` green.

**Notes (landed):** `yarrow_runtime` (rlib) holds implementation + `HOST_FNS`. `yarrow_runtime_aot` (`staticlib`, `aot-exports`) exports linker names without polluting the JIT binary. `yarrow_core::linkable_archive()` reads `libyarrow_runtime_aot.a`.

---

### Stage 17 — Program entry name ✅

Default entry name is `main`. Callers (CLI `--main`) may choose another top-level function. Core owns the name as data, not argv parsing.

1. **`CompileOptions::entry_name`** (default `"main"`):
   - `require_main` looks up this name (E360 names the chosen entry, not a hardcoded `main`).
   - JIT `run_main`, interpret `run_entry`, and object emit all use the same name.
2. Document the entry-name contract in [`docs/RUNTIME.md`](../../docs/RUNTIME.md).

**Gate:** `entry_name` is on `CompileOptions` and used by require-entry / JIT / interpret / object lower. Missing entry fails with E360 naming the requested function.

**Notes (landed):** `entry_name` wired end-to-end. A transitional C CRT (`entry_crt_source` + `yarrow_entry` trampoline) also landed; **Stage 18 removes that C path** and emits process `main` with Cranelift instead.

---

### Stage 18 — Cranelift process `main` (no C CRT) ⬜

Yarrow object emit already uses Cranelift. The process entry shim must live in that same object so AOT never compiles C and never needs `cc`.

1. In object emit, define and **export** linker symbol `main` (host C ABI for process entry, typically `() -> i32`) in Cranelift IR that:
   - calls the configured Yarrow entry (`CompileOptions::entry_name`, body kept non-exported / local so it does not clash with process `main`);
   - maps returns to an exit code: void / non-integer → `0`; integer → truncated/widened value; fallible error tag → `1`.
2. Collapse the transitional `yarrow_entry` + C CRT into this single Cranelift `main` (remove `entry_crt_source` / `EntryCrt` / `Session::entry_crt` and related docs).
3. JIT and interpret keep calling the Yarrow entry by name; they do not use process `main`.
4. Update [`docs/RUNTIME.md`](../../docs/RUNTIME.md): process entry is Cranelift-exported `main`; link is program `.o` + runtime `.a` only (no CRT `.c` / `.o`).

**Gate:** `compile_object_source` on `01_hello.yar` yields an object whose `nm` shows a global `main` and does not require any C source. No public API returns C text for CRT. `cargo clippy` green. Still no requirement that core shells out to a linker.

---

### Stage 19 — Link to host executable (no C compiler) ⬜

Wire Stages 13c + 16 + 18 into one library API that produces a runnable host binary **without** a C compiler.

1. **`Session::compile_executable_source`** (name can adjust) — check, emit program object (already contains process `main`), link with the runtime archive via a system **linker** (`ld` / `lld` / equivalent), not `cc` as a compile driver.
2. Return a structured artifact (e.g. `ExecutableArtifact { file, bytes }` or path + bytes); include IR when useful for dump parity.
3. Clear diagnostics when the linker is missing or link fails (`E39x`), with help pointing at needing a system linker (not a C toolchain).
4. No silent fallback to JIT.
5. CLI consumes this for `compile --target object` (write `-o`) and later `run --target object` (write temp + exec); that wiring is CLI-plan work.

**Gate:** `01_hello.yar` via the new API yields a host executable that runs and prints the same lines as JIT `run`, then exits cleanly. Invalid programs still fail before link. Core never invokes `cc`/`gcc`/`clang` to compile CRT or user code. `cargo clippy` green.

#### Out of scope for Phase D (backlog)

- Cross-compilation ISA / triple selection.
- DWARF / debug info, optimized AOT tiers (`-O`).
- Shipping a bundled linker (system `ld` / `lld` is enough; do not require `cc`).
- Switching the default `run` / `compile` target from `jit` to `object`.
- Full interpret parity with every JIT intrinsic.
- REPL UI (`yarrow-cli`).

---

## Backlog (compiler, after current stages or in parallel if small)

| Item                                                         | Value                     | Effort                              |
| ------------------------------------------------------------ | ------------------------- | ----------------------------------- |
| Warning catalog (unused `const`/`mutable`, unused `require`) | Teachable compiler        | Medium                              |
| Interpreter corpus parity + REPL `EvalContext` increments    | `interpret`, later `repl` | Medium–high after Stage 13b         |
| Multi-file project graph (beyond `require`)                  | Real programs             | Medium                              |
| `std.fs` host I/O, richer `io` / `string`                    | Runtime / std             | Medium; keep out of compiler stages |
| IR dump with source line comments                            | Nicer `dump --emit ir`    | Low                                 |
| DWARF / AOT optimization tiers                               | Production native builds  | High; after Stage 19                |
| Cross-compile targets                                        | Non-host objects          | High; after Stage 19                |

---

## Definition of done

### Phases A + B (done)

1. Docs authoritative; compiler matches `GRAMMAR.md` / `SYNTAX.md` for the example corpus.
2. Ownership, borrow, regions, unsafe enforced as documented.
3. Failures render rustc-style with teachable notes; multi-error when recovery applies.
4. `cargo fmt --all`, `cargo check`, `cargo clippy` green.

### Phase C (done)

5. Session API is what callers use; root driver no longer duplicates tokenize/parse/compile setup.
6. Stack-effect notes on high-traffic stack errors.
7. Execution backends: check / JIT / object emit / interpret MVP.
8. Float smallest-fit and teachable `E334`; flat postfix AST and struct/map disambiguation.

### Phase D (open)

9. Host runtime is linkable for AOT (Stage 16 ✅).
10. `CompileOptions::entry_name` selects the program entry (Stage 17 ✅).
11. Object emit exports Cranelift process `main` (no C CRT) (Stage 18).
12. `Session` (or equivalent) can produce a runnable host executable for `01_hello.yar` without a C compiler (Stage 19).

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- When renaming std or syntax, update compiler and `lib/std` together.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands.
- Do not invent lifetime syntax; ownership and regions are the model.
- `unsafe` never disables type, stack, ownership, or borrow checking.
- Do not add CLI parsing (`clap`, argv, exit codes) here. Those belong in `yarrow-cli`.
- Phase D may invoke a system **linker** from a library helper; it must not become a CLI driver and must not depend on a C compiler for CRT or codegen.
