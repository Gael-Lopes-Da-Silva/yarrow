# Yarrow Core Implementation Plan

Compiler library: tokenize → parse → check → `{ jit | object | executable | interpret }`.

`yarrow-core` is the **API**. Drivers (`yarrow-cli`, `yarrow-fmt`, `yarrow-lsp`) call it; they do not reimplement the pipeline. CLI UX lives in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md).

## Source of truth

| Role                 | Path                                                                                                                               |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Language tour        | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md)                                                                                         |
| Formal syntax        | [`docs/SYNTAX.md`](../../docs/SYNTAX.md)                                                                                           |
| AST / types / memory | [`docs/AST.md`](../../docs/AST.md), [`TYPE_SYSTEM.md`](../../docs/TYPE_SYSTEM.md), [`MEMORY_MODEL.md`](../../docs/MEMORY_MODEL.md) |
| Runtime / modules    | [`docs/RUNTIME.md`](../../docs/RUNTIME.md)                                                                                         |
| Corpus               | [`docs/examples/`](../../docs/examples/README.md)                                                                                  |
| Agent rules          | [`AGENTS.md`](../../AGENTS.md)                                                                                                     |

Prefer the docs when code and docs disagree. Do not invent language features absent from the docs.

---

## Landed

| Component   | Notes                                                                                                                                                                           |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Frontend    | Tokenizer + parser (flat postfix `Apply*`); rustc-style diagnostics; `Comment` tokens                                                                                           |
| Checking    | Types, ownership, borrow, regions, unsafe; stack-effect notes; `LowerKind::Check` (no JIT install)                                                                              |
| Warnings    | `W401`–`W410` (unused / dead stack / never-written mutable / redundant `copy` / require ambiguity / unreachable / empty match arm / empty `if` then / empty `unsafe`); `CheckedProgram::warnings` |
| Session API | `check` / `compile` (JIT, explicit mode) / `compile_object` / `compile_executable` / `interpret`; default `ExecutionMode::Object`; `type_at` (30) + `definition_at` (35); ICE `E999` / `SessionFailureKind` (40) |
| AOT         | Runtime archive + Cranelift process `main` + `ld`/`lld` link (linux-gnu + static musl); DWARF + `OptLevel`; cross object emit (linux-gnu / linux-musl + Stage 34 COFF / Mach-O) |
| Projects    | `ProjectOptions` / `check_project` / `ModuleGraph`; `E382` cycles; `E383` missing roots (`docs/examples/project/`)                                                              |
| Runtime/std | Host heap, regions, lists/maps/strings; `std.io` / `std.string` / `std.fs` host wrappers                                                                                        |
| Interpret   | Stage 37 gate of `docs/examples/valid/**` (stdout matches JIT): Stage 36 plus unsafe/`pointer<T>`/`move`/runes/`std.fs`; `00_grammar_tour.yar` remains out of scope (E393) |
| Probes      | `TypeIndex` / `type_at`; `DefIndex` / `definition_at` (bindings + requires, root-only; Stage 35)                                                                               |

**Gates:** `docs/examples/valid/**` compile and run (JIT); `invalid/**` fail for the stated reason; `warnings/**` check with `Ok` + warnings; `cargo fmt && cargo check && cargo clippy` green.

Phases A–E (Stages 0–24) and Phase F–G (Stages 25–26, 28–34) are complete. Stage 27 (bundled linker) stays deferred. Historical stage write-ups were removed; git history keeps them.

---

## Known gaps

| Area       | Gap                                                                                                                      |
| ---------- | ------------------------------------------------------------------------------------------------------------------------ |
| AOT        | Cross link needs matching archive + CRT; Mach-O / Windows executable link is Stage 39 (**blocked**: needs Darwin or Windows-gnu native host / CI; no fake exe link on linux) |
| Interpret  | `00_grammar_tour.yar` stays out of scope (mixed surface / `loop.break` and further tour forms); Stage 37 landed unsafe / pointers / move / fs |
| Warnings   | `W401`–`W410` landed (Stage 38: empty match arm / empty `if` then / empty `unsafe`)                                      |
| ICE        | `E999` / `SessionFailureKind` landed (Stage 40); CLI exit `101` consumption is [`yarrow-cli` Stage 16](../yarrow-cli/PLAN.md) |
| Projects   | Multi-root check via `check_project`; CLI driver ✅ [`yarrow-cli` Stage 13](../yarrow-cli/PLAN.md)                    |
| Linker     | System `ld`/`lld` only; Stage 27 bundled linker deferred (discovery remains reliable)                                    |
| LSP assist | Typed-at-span + definition / require probes landed (Stage 35); LSP consumption is [`yarrow-lsp` Stage 22](../yarrow-lsp/PLAN.md) |
| Formatter  | Whitespace rebuilt by printer (`yarrow-fmt`); incomplete parse → hygiene + selective top-level reprint; ignore regions Stage 19; parallel multi-file Stage 20; widened fmt-check corpus Stage 21 ([`yarrow-fmt`](../yarrow-fmt/PLAN.md)) |

---

## Next (Phase H)

Focus: Mach-O / Windows executable link when a native Darwin or Windows-gnu host / CI agent is available (Stage 39, currently **blocked**). Stage 40 ICE tagging is done. Keep Stage 27 deferred unless PATH linkers become fragile. Do not invent language features. Project CLI is [`yarrow-cli` Stage 13](../yarrow-cli/PLAN.md) ✅. LSP Stage 22 consumes Stage 35 probes. CLI Stage 16 can consume `SessionFailureKind::Ice` → exit `101`.

### Stage 35 - Require-path / definition probe API - **done**

`CheckedProgram::definition_at` / `DefIndex` on the root file: binding and function definition spans (use sites point back), plus `require` alias / path string → resolved module path and optional on-disk `file_path`. Root-only (required-module bodies not indexed). Documented in [`docs/RUNTIME.md`](../../docs/RUNTIME.md). Stretch (multi-file index) deferred to LSP + project graph.

### Stage 36 - Interpreter: regions, defer, field `set` - **done**

Close the Stage 32 E393 gaps that unblock the next corpus files without taking on unsafe yet.

1. Implement region create / put / free and `defer` (reverse registration order at scope exit) via host runtime helpers already used by JIT; match `docs/examples/valid/09_regions_and_defer.yar` stdout to JIT.
2. Implement field `set` on structs (and any related mutable field stores the example needs) without panicking; keep unsupported shapes as clear `E393`.
3. Prefer host heap layouts over a second memory model in the interpreter.
4. Update `interpreter/mod.rs` corpus list and RUNTIME interpret blurb; Known gaps shrink accordingly.
5. Do not claim full `valid/**` parity until Stage 37.

**Gate:** `09_regions_and_defer.yar` interprets with stdout matching JIT `run --target jit`. Field `set` used by that path (or a minimal adjacent fixture) no longer returns E393. Stage 32 gate files still pass. `cargo run -p yarrow_core --example check_interpret` green (extended). `cargo clippy` green.

**Landed:** `std.region::{create,put,free}` intrinsics call `yarrow_region_*`; `defer` runs at function scope exit in reverse order; struct field `set` stores through member targets; interpreter registers `FieldDesc` tables so region free of structs is safe.

---

### Stage 37 - Interpreter: unsafe / pointers + remaining `valid/**` - **done**

Finish interpret parity for the rest of the JIT-runnable corpus, or document explicit out-of-scope files with reason.

1. Priority order: `11_unsafe_pointers.yar` (`unsafe` blocks / functions, `pointer<T>` load/store, `std.mem`); then `08_ownership_borrow_move.yar`; then `14_io_and_string.yar` / `15_fs.yar` if still E393; then `00_grammar_tour.yar` only where interpret can match JIT without inventing ops.
2. Keep unsupported ops as `E393` (no panics, no silent wrong answers). Prefer host calls for allocate / free / load / store.
3. Update corpus list in module docs; every `valid/**` file that JIT runs either interprets with matching stdout **or** is listed as interpret-out-of-scope with a one-line reason in Known gaps / module docs.
4. Coordinate with CLI `interpret` / REPL only if a new Session knob is required; default remains `interpret_source`.

**Gate:** at least `11_unsafe_pointers.yar` and `08_ownership_borrow_move.yar` interpret with stdout matching JIT. Remaining E393 surface (if any) listed in Known gaps. JIT and object gates unchanged. `check_interpret` extended. `cargo clippy` green.

**Landed:** `unsafe` bodies; `pointer<T>` load/store and field access/`set`; `@alloc`/`@free`/`@load`/`@store`; `move`; runes; `std.fs` host helpers; heap return ownership claimed from locals (matches JIT). Out of scope: `00_grammar_tour.yar` (`loop.break` and further tour forms stay E393).

---

### Stage 38 - Warning catalog follow-ups - **done**

Stage 31 left empty `match` arms and further low-noise lints for later.

1. Add `W408` (or next free code) for suspicious empty `match` arms when the language docs / style make that a clear signal; emit only on successful check.
2. Inventory at most 1–2 additional high-value lints from TYPE_SYSTEM / MEMORY_MODEL / GRAMMAR (not style-guide naming nits that belong in `yarrow-fmt`).
3. Assign stable `W4xx` codes; `explain_code` entries; fixtures under `docs/examples/warnings/`; keep existing `01`–`04` behavior.
4. Update RUNTIME warnings table; do not turn warnings into hard errors.

**Gate:** new fixtures check with `Ok` and surface the new codes via `CheckedProgram::warnings`. `cargo run -p yarrow_core --example check_warnings` green. Existing `valid/**` / `invalid/**` unchanged. `cargo clippy` green.

**Landed:** `W408` empty `match` case arm; `W409` empty `if` then branch; `W410` empty `unsafe` block; fixtures `05`–`06`; `check_warnings` gate restored.

---

### Stage 39 - Mach-O / Windows executable link - **blocked**

Stage 34 landed object-only COFF / Mach-O on linux hosts (`E397` for exe). Finish executable link where a real linker + CRT / import story exists.

1. On a documented **native** host (or CI image) for at least one of `x86_64-apple-darwin` / `aarch64-apple-darwin` or `x86_64-pc-windows-gnu` (MSVC later if needed), implement `compile_executable_source` via `ld64` / `link.exe` / appropriate `lld` flavor without a `cc` compile step.
2. Runtime archive + CRT / import libs discovery must be explicit (env / documented paths); unsupported host/target pairs stay `E397`.
3. Prefer shipping one working host→host exe path before cross-link from linux to Mach-O / PE.
4. Update [`docs/RUNTIME.md`](../../docs/RUNTIME.md) Cross-compile matrix and Known gaps; keep MSVC / other triples out of scope unless cheap.

**Gate:** on the documented host (or documented CI), `compile_executable_source` for that triple produces a runnable binary for `docs/examples/valid/01_hello.yar` (or equivalent). Host linux-gnu path unchanged. Object-only path for the other Stage 34 triples still works. `cargo clippy` green.

**Blocked:** no Darwin / Windows-gnu native agent in this environment (linux-gnu / NixOS host only; no `ld64` / MinGW / `link.exe`). Per stage notes, do not fake exe link on linux. Reopen when a macOS or Windows-gnu runner (or matching cross toolchain + CRT import story) is available.

---

### Stage 40 - ICE-tagged session failures - **done**

Helps [`yarrow-cli` Stage 16](../yarrow-cli/PLAN.md) distinguish internal bugs (`101`) from user diagnostics (`1`) without relying only on `catch_unwind`.

1. Define a narrow Session / diagnostic path for internal compiler errors (bug, invariant break caught at a boundary): stable code (e.g. `ICE` / `E999`; pick one and document) or a typed `SessionError::Ice` that drivers can map to exit `101`.
2. Convert selected `unreachable!` / expect sites at API boundaries to this path where recovery is possible; do **not** blanket-catch all panics inside the library (CLI may still `catch_unwind`).
3. Ordinary `SessionDiagnostics` batches stay exit-`1` material; never mark user programs as ICE.
4. Document in RUNTIME / explain table; CLI plan Stage 16 stretch can consume the tag.
5. No new language features.

**Gate:** a documented debug-only or example hook that triggers the ICE path returns the tagged error (not a silent `E3xx` user diagnostic). Normal `invalid/**` checks still produce ordinary diagnostics. `cargo clippy` green.

**Landed:** stable `E999` (`ICE_CODE`); `Diagnostic::ice` / `CompileError::ice`; `SessionDiagnostics::is_ice` / `failure_kind` (`SessionFailureKind`); `Session::debug_trigger_ice` gate hook; object-backend JIT pointer path returns ICE instead of panicking; `explain_code` + RUNTIME section; `check_ice` example.

---

## Later (backlog)

| Item                             | Notes                                                                              |
| -------------------------------- | ---------------------------------------------------------------------------------- |
| Bundled linker                   | Revisit Stage 27 only if PATH `ld`/`lld` is fragile                                |
| Program argv API                 | Needs GRAMMAR / std design first; CLI object path already forwards OS argv         |
| Language-level tests             | Needs GRAMMAR; CLI corpus driver is [`yarrow-cli` Stage 17](../yarrow-cli/PLAN.md) |
| MSVC / more AOT triples          | After Stage 39’s first native exe path                                             |
| Project compile / run multi-root | Check-only graph exists; multi-entry product story first                           |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- When renaming std or syntax, update compiler and `lib/std` together.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark ✅, short notes; do not re-expand history).
- Do not invent lifetime syntax; ownership and regions are the model.
- `unsafe` never disables type, stack, ownership, or borrow checking.
- No CLI parsing (`clap`, argv, exit codes) here; that is `yarrow-cli`.
- May invoke a system **linker** from a library helper; must not depend on a C compiler for CRT or codegen.
