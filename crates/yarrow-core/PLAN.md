# Yarrow Core Implementation Plan

Compiler library: tokenize → parse → check → `{ jit | object | executable | interpret }`.

`yarrow-core` is the **API**. Drivers (`yarrow-cli`, later `yarrow-fmt` / `yarrow-lsp`) call it; they do not reimplement the pipeline. CLI UX lives in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md).

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

| Component   | Notes                                                                                                      |
| ----------- | ---------------------------------------------------------------------------------------------------------- |
| Frontend    | Tokenizer + parser (flat postfix `Apply*`); rustc-style diagnostics; `Comment` tokens                      |
| Checking    | Types, ownership, borrow, regions, unsafe; stack-effect notes; `LowerKind::Check` (no JIT install)         |
| Warnings    | `W401`–`W407` (unused / dead stack / never-written mutable / redundant `copy` / require ambiguity / unreachable); `CheckedProgram::warnings` |
| Session API | `check` / `compile` (JIT, explicit mode) / `compile_object` / `compile_executable` / `interpret`; default `ExecutionMode::Object`; `CheckedProgram::type_at` (Stage 30) |
| AOT         | Runtime archive + Cranelift process `main` + `ld`/`lld` link (linux-gnu + static musl); DWARF + `OptLevel`; cross object emit (`x86_64` / `aarch64` linux-gnu and linux-musl) |
| Projects    | `ProjectOptions` / `check_project` / `ModuleGraph`; `E382` cycles; `E383` missing roots (`docs/examples/project/`) |
| Runtime/std | Host heap, regions, lists/maps/strings; `std.io` / `std.string` / `std.fs` host wrappers                   |
| Interpret   | Stage 32 gate of `docs/examples/valid/**` (stdout matches JIT): Stage 21 plus structs/enums/methods, unions, errors/`unwrap`/`handle`, lists/maps; regions / unsafe still E393 |

**Gates:** `docs/examples/valid/**` compile and run (JIT); `invalid/**` fail for the stated reason; `warnings/**` check with `Ok` + warnings; `cargo fmt && cargo check && cargo clippy` green.

Phases A–E (Stages 0–24) and Phase F (Stages 25–26, 28–29) are complete. Stage 27 (bundled linker) stays deferred. Historical stage write-ups were removed; git history keeps them.

---

## Known gaps

| Area        | Gap                                                                                                           |
| ----------- | ------------------------------------------------------------------------------------------------------------- |
| AOT         | Cross link needs matching archive + CRT / linker emulation; Mach-O / Windows later (Stage 34)               |
| Interpret   | No regions / defer, unsafe / raw pointers, field `set`, or full `valid/**` parity (remaining E393 after Stage 32) |
| Warnings    | Unused / dead-stack / never-written mutable / redundant `copy` / require ambiguity / unreachable (`W401`–`W407`); more lints later |
| Projects    | Multi-root check via `check_project`; no CLI project driver yet                                               |
| Linker      | System `ld`/`lld` only; Stage 27 bundled linker deferred (discovery remains reliable)                         |
| LSP assist  | Typed-at-span via `type_at`; no require-path index API yet (navigation stays LSP AST)                      |
| Formatter   | Whitespace rebuilt by printer (`yarrow-fmt`); incomplete parse → hygiene via `parse_recovering` |

---

## Next (Phase G)

Focus: Mach-O / Windows AOT (Stage 34). Keep Stage 27 deferred unless system linkers become fragile. Do not invent language features.

### Stage 27 - Bundled linker (optional) ⏭️ deferred

Only if everyday AOT shows system `ld`/`lld` discovery is too fragile. Skip (keep deferred) if PATH linkers stay reliable.

1. Vendor or depend on a known linker (e.g. `lld` as a library or pinned binary) invoked from `link::link_executable` without shelling out to a random PATH `ld` when the bundle is enabled.
2. Keep the “no `cc` compile step” rule; CRT discovery may still use `cc -print-file-name` for paths only.
3. Feature-gate or option-gate the bundle so default builds do not force a huge download unless chosen.

**Gate:** with the bundle enabled on linux-gnu, `compile_executable_source` succeeds without requiring a system `ld`/`lld` on `PATH` (CRT still locatable). Document how to enable it.

**Deferred:** Host/cross AOT linking stays on PATH `ld`/`lld` with clear `E394`/`E395` diagnostics; no everyday fragility that justifies vendoring a linker.

### Stage 30 - Typed-at-span / signature probe API ✅

Unblocks [`yarrow-lsp` Stage 9](../yarrow-lsp/PLAN.md) typed hover / inlay. Today `CheckedProgram` is AST-only; the LSP must not invent types.

1. After a successful `check_source` (or from a retained check artifact), expose a stable probe: given a byte offset or `Span` in the root file, return the binding / expression type string and, when on a call or function name, a signature / stack-effect summary.
2. Prefer data already computed during checking (do not re-run full lower or JIT). Reuse stack-effect note formatting where it already exists for diagnostics.
3. Session surface: e.g. methods on `CheckedProgram` or a small `Analysis` / probe type returned alongside check. Keep the API usable without `ExecutionMode::Jit` / object emit.
4. Optional stretch (same stage only if cheap): require-path or definition span for an identifier at offset, enough for hover “defined in …” without a full project index. Otherwise leave navigation to LSP AST walks.
5. Document the probe in [`docs/RUNTIME.md`](../../docs/RUNTIME.md) or a short Session API note; coordinate names with `yarrow-lsp` Stage 9.

**Gate:** documented Session/`CheckedProgram` probe on a typed `const` / `mutable` in `docs/examples/valid/03_variables_and_typeof.yar` (or equivalent) returns a non-empty type string matching the checker. Probe on empty / non-code span returns none / clear miss. `check_source` latency and corpus gates unchanged; `cargo clippy` green. No fake types in the LSP.

**Done:** `TypeIndex` / `TypeProbe` + `CheckedProgram::type_at(offset)` filled during check-only lower (root-file bindings and function signatures with stack-effect lines); miss on non-code offsets. Documented under RUNTIME Session probes. LSP Stage 9 consumes it for typed hover.

---

### Stage 31 - Richer warning / lint catalog ✅

Extend beyond `W401`–`W403` without turning warnings into hard errors.

1. Inventory high-value, low-noise lints from the language docs (e.g. unused `mutable` that is never written, redundant `copy`, unreachable after divergent path, suspicious empty `match` arm, item-vs-module require ambiguity already printed as text → promote to a stable `W` code if not already).
2. Assign stable `W4xx` codes; add `explain_code` entries; emit only on successful check (same policy as Stage 20).
3. Add at least two new fixtures under `docs/examples/warnings/` that check with `Ok` and assert the new codes; keep `warnings/01_unused.yar` behavior.
4. Document codes in the diagnostics explain table / RUNTIME warnings blurb if one exists; do not invent style-guide-only nits that belong in `yarrow-fmt`.

**Gate:** new warning fixtures check successfully and surface the new codes via `CheckedProgram::warnings`. Existing `valid/**` / `invalid/**` gates unchanged. `cargo clippy` green.

**Done:** `W404` never-written scalar/`enum` `mutable`; `W405` redundant `copy` on non-heap params; `W406` require item-vs-module ambiguity (replaces `eprintln`); `W407` unreachable after divergent flow. Fixtures `02`–`04` plus `examples/check_warnings` gate. Empty `match` arm left for a later lint pass.

### Stage 32 - Interpreter corpus toward JIT parity ✅

Grow past Stage 21 / `E393` so more of `docs/examples/valid/**` interpret with stdout matching JIT `run --target jit`.

1. Extend the tree-walk interpreter for, in priority order that unblocks the most examples: structs / enums / `implement` methods; unions + type-dispatch `match`; lists / maps; `error` / `|T Err|` / `unwrap` / `handle`; regions / `defer`; then unsafe / pointers only if host helpers already cover the example.
2. Keep unsupported ops as clear `E393` (no panics, no silent wrong answers). Prefer host runtime calls over reimplementing heap layouts in the interpreter.
3. Update the corpus list in `interpreter/mod.rs` module docs as each example lands; do not claim full parity until every `valid/**` file that JIT runs also interprets (or is explicitly documented as interpret-out-of-scope with reason).
4. Coordinate with CLI `interpret` / REPL only if a new Session knob is required; default remains `interpret_source`.

**Gate:** at least `06_structs_and_enums.yar`, `07_unions.yar`, `10_errors.yar`, and `13_containers.yar` interpret with stdout matching JIT. Remaining E393 surface listed in Known gaps / module docs. JIT and object gates unchanged.

**Done:** structs / `implement` / enums; named unions + type-dispatch `match`; lists / hashmaps + `std.list` intrinsics; custom `error`, fallible `|T Err|`, `unwrap`, `handle` + fallback. Gate example `check_interpret`. Still E393: regions / defer, unsafe / pointers, field `set`, remaining `valid/**`.

### Stage 33 - Broader cross-compile matrix (linux) ✅

After Stage 26’s first non-host linux-gnu arch: widen linux targets before Mach-O / Windows.

1. Add at least one more supported triple class: prefer `*-linux-musl` (static-friendly) or document why another linux-gnu variant is chosen first.
2. Wire archive lookup (`linkable_archive_for` / env table) and CRT / linker emulation for that triple; reject half-supported configs with `E397` / `E394` / `E396` as today (no panic).
3. Object emit must use the triple’s ISA; executable link only when archive + CRT are actually available in CI or documented agent setup (`YARROW_BUILD_CROSS_AOT`, sysroot env).
4. Update [`docs/RUNTIME.md`](../../docs/RUNTIME.md) Cross-compile section and Known gaps; keep Mach-O / Windows for Stage 34.

**Gate:** documented Session options (or env) produce a non-host object for the new triple; missing pieces fail with stable diagnostics. Existing host + Stage 26 cross path still pass. `cargo clippy` green.

**Done:** `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl` accepted for object emit; static musl link when CRT + archive exist (`YARROW_AOT_SYSROOT` / `YARROW_AOT_CRT_DIR`); `build.rs` optionally records musl archives under `YARROW_BUILD_CROSS_AOT`; gate example `check_cross`. Mach-O / Windows remain Stage 34.

### Stage 34 - Mach-O / Windows AOT link

Platform object formats beyond ELF, once the linux cross story is real.

1. Emit Mach-O and/or COFF/PE objects via Cranelift’s object backend for a documented host or cross triple (`x86_64-apple-darwin`, `x86_64-pc-windows-msvc` / `gnu`, or the cheapest first win).
2. Link with the platform linker (`ld64` / `link.exe` / `lld` flavor) without introducing a `cc` compile step; CRT / import libs discovery must be explicit and documented.
3. Runtime archive must build for that target (`yarrow_runtime_aot` + `aot-exports`); document how agents obtain it. Unsupported host/target pairs stay `E397`.
4. Prefer object-only first if full executable link is blocked on CI; say so in RUNTIME and Known gaps.

**Gate:** at least one non-ELF object (and executable if in scope) builds for a documented triple; host linux-gnu path unchanged. RUNTIME documents the matrix; Known gaps updated. `cargo clippy` green.

---

## Later (backlog)

| Item                         | Notes                                              |
| ---------------------------- | -------------------------------------------------- |
| Require-path / def index API | If Stage 30 stretch is skipped; fuller LSP navigate |
| Interpreter full `valid/**`  | Finish remaining E393 after Stage 32 gate           |
| Bundled linker               | Revisit Stage 27 only if PATH `ld`/`lld` is fragile |
| Project CLI driver           | Lives in `yarrow-cli`; core graph already landed    |

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
