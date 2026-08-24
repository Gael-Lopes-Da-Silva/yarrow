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

| Component   | Status | Notes                                                                         |
| ----------- | ------ | ----------------------------------------------------------------------------- |
| Tokenizer   | ✅     | Full doc surface; UTF-8-safe                                                  |
| Parser/AST  | ✅     | Parses `docs/examples/valid/**`; some internal AST shape debt remains         |
| Compiler    | ✅     | JIT + check-only path; ownership / borrow / region / unsafe                   |
| Diagnostics | ✅     | Rustc-style spans; stack-effect notes on underflow / join / return            |
| Library API | 🟡     | `ExecutionMode` / `check_source` / `CheckedProgram`; Object/Interpret stubbed |
| Runtime     | 🟡     | Host heap, regions, lists, maps, strings; no real file I/O                    |
| Std library | 🟡     | Core modules + intrinsics; `std.fs` stub; partial `io` / `string`             |

**Gates today:** all `docs/examples/valid/**` compile and run; all `docs/examples/invalid/**` fail for the stated reason; `cargo fmt --all && cargo check && cargo clippy` green.

Long-term execution story (planned in Stage 13+): **JIT** (`run` / `compile --target jit`), **native object** (`--target object`), **interpreter** (`interpret`). One frontend; backends plug in after checking.

---

## Completed phases (summary)

### Phase A: Language surface (Stages 0–7) ✅

Tokenizer, parser, and compiler aligned with the current docs: `~` concat, `|T Err|` fallible returns, `error` types, visibility, `copy` params, std module names, ownership/borrow/region checks, grammar tour conformance.

### Phase B: Diagnostics (Stages 8–10) ✅

Full rustc-style rendering (`Span`, labels, notes, help), teachable error catalog (E373–E376, E370, E308, …), parser recovery and compiler multi-error collection with output cap.

---

## Known gaps (compiler scope)

These are remaining mismatches or thin areas inside this crate, not CLI work.

| Area        | Gap                                                                                                             |
| ----------- | --------------------------------------------------------------------------------------------------------------- |
| API         | `ExecutionMode` + `check_source` landed; Object/Interpret not implemented yet                                   |
| Parser/AST  | Operand stack builds nested `Expr` trees; `Program` has no distinguished `main`; `{}` struct vs hashmap is weak |
| Types       | Float `%` / `^` rejected (`E334`); `f16` smallest-fit not implemented; floats default to `f64`                  |
| Backends    | Check + JIT; no object/AOT, no interpreter; check still drives Cranelift as analysis vehicle                    |
| Warnings    | No unused-binding / dead-stack / unused-require diagnostics                                                     |
| Std/runtime | `std.fs` has no host I/O; `std.io` / `std.string` partial (runtime, not blocking compiler stages)               |

---

## Phase C: Compiler API and checking depth

Language surface and diagnostic baseline are done. Phase C makes the compiler a usable **library** (what `yarrow-cli` will call) and deepens checking/codegen inside the same crate.

CLI commands (`check`, `run`, `--emit`, …) are specified in the CLI plan. This crate exposes the operations those commands need.

### Stage 11 — Compile session API ✅

**Why first:** `yarrow-cli` must not copy `src/main.rs`. A single session type is the compiler-side contract for check, run, compile, interpret, and dump.

1. **`CompileOptions`** — source path, extra module search paths, diagnostic cap, whether to require `main`, execution mode (see Stage 13; until then JIT is fine).
2. **`Session` (or `Frontend`)** — owns source text / `SourceFile`, search paths, collected diagnostics.
3. **Pipeline methods** (library, no process I/O beyond reading the entry file if the caller passes a path):
   - `tokenize` / `parse` / `compile`
   - `check(&Program) -> Result<(), DiagnosticBatch>` — same semantic checks as `compile`, **must not** call `run_main`
   - structured outcome: diagnostics + optional compiled artifact
4. **Public re-exports** from `yarrow_core` so `yarrow-cli` depends only on this crate for compile work.
5. Keep existing `Compiler::compile` / `run_main` working; session wraps them.

**Gate:** a small Rust caller (the current root driver, or a later CLI `check`) can tokenize, parse, and compile without duplicating diagnostic printing setup beyond `render` + `ColorChoice`. Valid examples still run; invalid examples still produce batches. No clap in this crate.

---

### Stage 12 — Stack-effect diagnostics ✅

Teach the stack model on common failures (underflow, branch join mismatch, return stack imbalance).

1. Track expected vs found stack types in `pop_slot` / branch merge / `return`.
2. Append a note like `stack: [i32, string] → expected [bool]` on E324 / E328 / E362-style errors.
3. Format types the way users write them (`list<i32>`, `|i32 error.Error|`), not debug dumps.

**Gate:** at least two invalid examples (or new ones if needed) gain a stack note without regressing primary spans.

---

### Stage 13 — Execution backends (JIT / object / interpret) 🟡

**Status:** 13a ✅; 13b / 13c still open.

Target backends (names can adjust; keep the split):

| Mode        | CLI consumer                                      | Role                                                               |
| ----------- | ------------------------------------------------- | ------------------------------------------------------------------ |
| `Check`     | `yarrow check`, IDE / LSP later                   | Full type / ownership / stack / region checks; **no** machine code |
| `Jit`       | `yarrow run` / `compile` (default `--target jit`) | Cranelift in-process machine code + optional `run_main`            |
| `Object`    | `yarrow run` / `compile --target object`          | Native relocatable object (AOT); link/exec stays CLI-side          |
| `Interpret` | `yarrow interpret` (and later `repl`)             | Stack VM over checked code; no machine code                        |

`emit_ir` / `SessionArtifact::emit_ir` already landed (CLI `dump --emit ir`). Keep that API; do not gate Stage 13 on it.

#### 13a — Pipeline split and mode knob ✅

Stop folding “check” into “always build a JIT module”.

1. **`ExecutionMode`** on `CompileOptions`: `Check`, `Jit`, `Object`, `Interpret`.
2. **`CheckedProgram`** handoff after successful `check_source` (file + program). Checking is shared with JIT via the same lower path; Check skips `define_function` / JIT finalize (`Compiler::set_check_only`).
3. **Session API:**
   - `check_source` — diagnostics only; no JIT install.
   - `compile_source` with `Jit` — today’s path.
   - `Object` / `Interpret` return clear E391 / E392 diagnostics until 13b/13c.
4. CLI `yarrow check` uses `ExecutionMode::Check` + `check_source`.

**Note:** Check still constructs a Cranelift `JITModule` as the vehicle for SSA/slot analysis. A backend-free checker can come later; the gate is no JIT _install_ / finalize for Check.

**Gate (13a):** `Session` + `ExecutionMode::Check` type-checks valid examples without installing JIT code. Invalid examples still fail with the same codes. Default `Jit` path still runs `01_hello.yar`. `cargo clippy` green.

#### 13b — Interpreter groundwork (required for this stage)

Interpretation is the path for `yarrow interpret` and a future REPL. Do **not** put REPL UI in this crate; expose an eval API the CLI can wrap later.

1. **Design choice (pick one and document in this PLAN when implemented):**
   - **Preferred:** lower checked functions to a small **stack bytecode** (ops mirror the language’s stack words), then interpret that. Natural fit for Yarrow; REPL can compile chunks to bytecode.
   - **Acceptable MVP:** tree-walk the checked AST with an explicit operand stack and the existing host runtime (`runtime.rs`). Faster to land; bytecode can replace it later without changing the Session surface.
2. **Library surface:**
   - `Session::interpret_source` (or `compile` + `InterpretArtifact::run_main`) returning the same `RunResult` shape as JIT where practical.
   - For REPL readiness: an **`EvalContext` / `Interpreter`** that can accept additional checked top-level items or expressions over a live heap (even if Stage 13 only supports whole-file `main` first).
3. **Semantics:** interpreter executes **already-checked** code. It does not re-do borrow checking at runtime. Reuse host runtime for handles, regions, lists, maps, strings.
4. **Parity target for the gate:** enough of the language to run a small set of `docs/examples/valid/**` (at least hello + one control-flow / stack example). Full corpus parity can follow; do not block the stage on 100% JIT feature coverage (e.g. every intrinsic).

**Gate (13b):** at least one valid example runs to the same printable `RunResult` via interpret as via JIT. API is callable without clap. Document which examples are in / out of interpret scope.

#### 13c — Object / AOT groundwork (required API; emit can be thin)

Native objects power `yarrow compile --target object` (and later `yarrow run --target object` after link+exec). Full link-to-executable stays CLI-side; core must emit something real or a deliberate stub.

1. Wire `ExecutionMode::Object` through Session.
2. **Minimum for Stage 13:** either
   - **(A) Preferred:** use `cranelift-object` (or equivalent) to emit a relocatable object for lowered functions into a buffer or path the caller chooses; host runtime / `main` calling convention documented as follow-up if linking is incomplete, **or**
   - **(B) Acceptable:** mode + Session entry point that returns a structured `ObjectEmitNotImplemented` / clear `CompileError` so CLI `compile --target object` is not a silent no-op; implement real emit in a follow-up stage once 13a is stable.
3. Share as much lowering as practical with JIT (Cranelift CLIF from the same checked input). Avoid a second ad hoc codegen.
4. Do **not** require producing a finished executable or system linker invocation inside `yarrow-core`.

**Gate (13c):** `ExecutionMode::Object` is selectable on `CompileOptions`. Either a non-empty object artifact is produced for a trivial function, or the API fails with an explicit not-implemented diagnostic/error the CLI can print. No silent success.

#### Out of scope for Stage 13 (backlog / CLI)

- REPL UI, line editing, multi-line paste (`yarrow-cli`).
- Linking objects into a standalone binary / choosing a system linker (`run`/`compile --target object` polish).
- Cross-compilation targets, DWARF, optimized AOT tiers.
- Full interpret parity with every JIT intrinsic and module edge case.
- Switching the default `run` / `compile` target from `jit` to `object` or interpret.

#### Implementation notes

- Keep `YARROW_DBG_IR` as a debug convenience over `emit_ir`, not the primary dump API.
- Optional later: source line comments on IR dumps via `Span` (nice-to-have, not a gate).
- Update [`docs/RUNTIME.md`](../../docs/RUNTIME.md) pipeline diagram when modes land (tokenize → parse → check → `{jit \| object \| interpret}`).
- CLI plan: `check` → `Check`; `run`/`compile --target jit|object` → `Jit`/`Object`; `interpret` → `Interpret`. No separate `build` command.

**Stage 13 gate (overall):** 13a landed; 13b runs ≥1 valid example; 13c has a real emit **or** an explicit not-implemented API; JIT regression-free on the example corpus used today; plans updated.

---

### Stage 14 — Numeric / literal polish ⬜

Close type-system gaps that are already in the docs.

1. Float literals: smallest-fit (`3.14` → `f16` when it fits), matching [`TYPE_SYSTEM.md`](../../docs/TYPE_SYSTEM.md).
2. Clearer reject for float `%` / `^` (`E334`): message + help that `%` is integer remainder and `^` is integer power, not float ops.
3. Keep integer smallest-fit behavior (already implemented).

**Gate:** grammar/examples that rely on float defaulting still compile or are updated with the docs; `E334` help is teachable; `cargo clippy` green.

---

### Stage 15 — AST / frontend cleanup ⬜

Internal compiler-facing AST, not new syntax.

1. Flatten postfix sequences where `Expr::Seq` nesting hides per-word spans.
2. Distinguish `main` on `Program` (or a resolve pass) so missing-`main` (`E360`) does not scan ad hoc.
3. Tighten `{}` struct vs hashmap disambiguation per grammar (typed empty `{}` vs struct literals).

**Gate:** example corpus unchanged in behavior; diagnostics do not get worse; no public language change.

---

### Backlog (compiler, after 11–15 or in parallel if small)

| Item                                                         | Value                           | Effort                                 |
| ------------------------------------------------------------ | ------------------------------- | -------------------------------------- |
| Warning catalog (unused `const`/`mutable`, unused `require`) | Teachable compiler              | Medium                                 |
| Full object emit + link story (`ExecutionMode::Object`)      | `compile`/`run --target object` | High; Stage 13c API first              |
| Interpreter corpus parity + REPL `EvalContext` increments    | `interpret`, later `repl`       | Medium–high after Stage 13b            |
| Error-code catalog data for `explain`                        | CLI `explain E308`              | Landed: `explain_code` / Phase B notes |
| Multi-file project graph (beyond `require`)                  | Real programs                   | Medium                                 |
| `std.fs` host I/O, richer `io` / `string`                    | Runtime / std, not lowering     | Medium; keep out of compiler stages    |
| IR dump with source line comments                            | Nicer `dump --emit ir`          | Low                                    |

---

## Definition of done

### Phases A + B (done)

1. Docs authoritative; compiler matches `GRAMMAR.md` / `SYNTAX.md` for the example corpus.
2. Ownership, borrow, regions, unsafe enforced as documented.
3. Failures render rustc-style with teachable notes; multi-error when recovery applies.
4. `cargo fmt --all`, `cargo check`, `cargo clippy` green.

### Phase C (in progress when started)

5. Session API is what callers use; root driver no longer duplicates tokenize/parse/compile setup (Stage 11).
6. Stack-effect notes on high-traffic stack errors (Stage 12).
7. Execution backends: check / JIT / object groundwork / interpret MVP (Stage 13).
8. Literal/float polish and AST cleanup (Stages 14–15) as scheduled.

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- When renaming std or syntax, update compiler and `lib/std` together.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands.
- Do not invent lifetime syntax; ownership and regions are the model.
- `unsafe` never disables type, stack, ownership, or borrow checking.
- Do not add CLI parsing (`clap`, argv, exit codes) here. Those belong in `yarrow-cli`.
