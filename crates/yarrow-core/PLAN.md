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

| Component   | Status | Notes                                                                     |
| ----------- | ------ | ------------------------------------------------------------------------- |
| Tokenizer   | ✅     | Full doc surface; UTF-8-safe                                              |
| Parser/AST  | ✅     | Parses `docs/examples/valid/**`; some internal AST shape debt remains     |
| Compiler    | ✅     | JIT only; ownership / borrow / region / unsafe; grammar tour              |
| Diagnostics | ✅     | Rustc-style spans, teachable notes, multi-error recovery (cap 20)         |
| Library API | 🟡     | Pieces exported (`Tokenizer`, `Parser`, `Compiler`, `render`); no session |
| Runtime     | 🟡     | Host heap, regions, lists, maps, strings; no real file I/O                |
| Std library | 🟡     | Core modules + intrinsics; `std.fs` stub; partial `io` / `string`         |

**Gates today:** all `docs/examples/valid/**` compile and run; all `docs/examples/invalid/**` fail for the stated reason; `cargo fmt --all && cargo check && cargo clippy` green.

The root binary still wires tokenize → parse → compile → `run_main` by hand (`src/main.rs`). That wiring should move behind a core session API, then into `yarrow-cli`.

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
| API         | Callers duplicate the pipeline; no `Session` / `CompileOptions`; `compile` always builds a JIT module           |
| Parser/AST  | Operand stack builds nested `Expr` trees; `Program` has no distinguished `main`; `{}` struct vs hashmap is weak |
| Types       | Float `%` / `^` rejected (`E334`); `f16` smallest-fit not implemented; floats default to `f64`                  |
| Diagnostics | Stack underflow / join / return errors do not dump expected vs found stack types                                |
| Codegen     | JIT only (`YARROW_DBG_IR` env dump); no `CodegenMode`, no object/AOT backend                                    |
| Warnings    | No unused-binding / dead-stack / unused-require diagnostics                                                     |
| Std/runtime | `std.fs` has no host I/O; `std.io` / `std.string` partial (runtime, not blocking compiler stages)               |

---

## Phase C: Compiler API and checking depth

Language surface and diagnostic baseline are done. Phase C makes the compiler a usable **library** (what `yarrow-cli` will call) and deepens checking/codegen inside the same crate.

CLI commands (`check`, `run`, `--emit`, …) are specified in the CLI plan. This crate exposes the operations those commands need.

### Stage 11 — Compile session API ✅

**Why first:** `yarrow-cli` must not copy `src/main.rs`. A single session type is the compiler-side contract for check, run, dump, and later build.

1. **`CompileOptions`** — source path, extra module search paths, diagnostic cap, whether to require `main`, codegen mode (see Stage 13; until then JIT is fine).
2. **`Session` (or `Frontend`)** — owns source text / `SourceFile`, search paths, collected diagnostics.
3. **Pipeline methods** (library, no process I/O beyond reading the entry file if the caller passes a path):
   - `tokenize` / `parse` / `compile`
   - `check(&Program) -> Result<(), DiagnosticBatch>` — same semantic checks as `compile`, **must not** call `run_main`
   - structured outcome: diagnostics + optional compiled artifact
4. **Public re-exports** from `yarrow_core` so `yarrow-cli` depends only on this crate for compile work.
5. Keep existing `Compiler::compile` / `run_main` working; session wraps them.

**Gate:** a small Rust caller (the current root driver, or a later CLI `check`) can tokenize, parse, and compile without duplicating diagnostic printing setup beyond `render` + `ColorChoice`. Valid examples still run; invalid examples still produce batches. No clap in this crate.

---

### Stage 12 — Stack-effect diagnostics ⬜

Teach the stack model on common failures (underflow, branch join mismatch, return stack imbalance).

1. Track expected vs found stack types in `pop_slot` / branch merge / `return`.
2. Append a note like `stack: [i32, string] → expected [bool]` on E324 / E328 / E362-style errors.
3. Format types the way users write them (`list<i32>`, `|i32 error.Error|`), not debug dumps.

**Gate:** at least two invalid examples (or new ones if needed) gain a stack note without regressing primary spans.

---

### Stage 13 — Codegen modes and IR dump API ⬜

Stop treating JIT + `YARROW_DBG_IR` as the only backend knob.

1. **`CodegenMode`**: `Jit` (current), `Check` (type/ownership/stack checks; skip native define/finalize if practical), later `Object`.
2. **`Compiler::emit_ir() -> String`** (or per-function map) for Cranelift IR. Do not key this on an env var as the only interface; env can remain a debug convenience. (`Compiler::emit_ir` / `SessionArtifact::emit_ir` landed for CLI `dump --emit ir`; `YARROW_DBG_IR` still prints the same text.)
3. Optional: source comments on IR blocks using `Span` line info.

**Gate:** after a successful compile (or check, if IR is still built), a library call returns readable Cranelift IR for at least one valid example function. `YARROW_DBG_IR` still works or is documented as wrapping the API.

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

| Item                                                         | Value                       | Effort                              |
| ------------------------------------------------------------ | --------------------------- | ----------------------------------- |
| Warning catalog (unused `const`/`mutable`, unused `require`) | Teachable compiler          | Medium                              |
| AOT / `cranelift-object` (`CodegenMode::Object`)             | `yarrow build` in the CLI   | High; needs Stage 13 modes first    |
| Error-code catalog data for `explain`                        | CLI `explain E308`          | Low once messages stabilize         |
| Multi-file project graph (beyond `require`)                  | Real programs               | Medium                              |
| `std.fs` host I/O, richer `io` / `string`                    | Runtime / std, not lowering | Medium; keep out of compiler stages |

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
7. Codegen mode + IR dump as library API (Stage 13).
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
