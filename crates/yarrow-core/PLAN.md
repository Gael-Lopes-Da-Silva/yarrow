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

| Component   | Notes                                                                                          |
| ----------- | ---------------------------------------------------------------------------------------------- |
| Frontend    | Tokenizer + parser (flat postfix `Apply*`); rustc-style diagnostics                            |
| Checking    | Types, ownership, borrow, regions, unsafe; stack-effect notes                                  |
| Session API | `check` / `compile` (JIT) / `compile_object` / `compile_executable` / `interpret`              |
| AOT         | Runtime archive + Cranelift process `main` + `ld`/`lld` link (linux-gnu; no `cc` compile step) |
| Runtime/std | Host heap, regions, lists/maps/strings; std modules + intrinsics (`fs` / `io` / `string` thin) |

**Gates:** `docs/examples/valid/**` compile and run (JIT); `invalid/**` fail for the stated reason; `cargo fmt && cargo check && cargo clippy` green.

Phases A–D (Stages 0–19) are complete. Historical stage write-ups were removed; git history keeps them.

---

## Known gaps

| Area        | Gap                                                                                |
| ----------- | ---------------------------------------------------------------------------------- |
| AOT         | linux-gnu host only; no DWARF, `-O` tiers, or cross-compile                        |
| Backends    | Check still lowers via Cranelift; interpret is MVP (not full JIT intrinsic parity) |
| Warnings    | No unused-binding / dead-stack / unused-`require` diagnostics                      |
| Std/runtime | `std.fs` has no host I/O; `std.io` / `std.string` partial                          |
| Projects    | Single-file + `require` only; no multi-root project graph                          |
| Formatter   | Comments skipped in tokenize; `yarrow-fmt` needs trivia (see `yarrow-fmt` Stage 1) |
| LSP         | No typed-at-span / require-path index API yet; server uses `check_source` + AST (see `yarrow-lsp`) |

---

## Next (Phase E)

Focus: teachable diagnostics, interpreter depth, and std/runtime usefulness. Keep AOT polish (DWARF / opts / cross) for Phase F unless a small fix blocks CLI use.

### Stage 20 - Warning catalog

Unused `const` / `mutable`, unused `require`, and obvious dead stack values after check.

1. Define warning codes (do not reuse error numbers) and `Diagnostic` severity for warnings.
2. Emit after a successful check path; `--error-limit` does not drop warnings unless a separate cap is added later.
3. Document codes in the explain table.

**Gate:** at least one valid example (or a new `docs/examples/warnings/` file) produces a warning under `check` without failing exit status at the Session API level. `cargo clippy` green.

### Stage 21 - Interpreter corpus parity

Grow `interpret_source` / `EvalContext` toward the valid example corpus (not only `01_hello` / `02_arithmetic`).

1. Track which `docs/examples/valid/**` files interpret cleanly; expand support in priority of language surface used.
2. Keep the same `RunResult` shape as JIT `run_main`.
3. Do not block on REPL UI (CLI owns `repl`).

**Gate:** a documented subset of valid examples (listed in this stage’s notes when landed) matches JIT stdout for `interpret`. Prefer growing the subset over claiming full parity early.

### Stage 22 - Std / runtime: `io` + `string` depth

Fill gaps that real programs hit before filesystem work.

1. Align `lib/std/io.yar` / `string.yar` with host helpers already in `yarrow-runtime` where possible.
2. Add host functions only when the grammar/docs require them; update [`docs/RUNTIME.md`](../../docs/RUNTIME.md).
3. Add or extend corpus examples that exercise the new surface.

**Gate:** new or extended valid examples compile under JIT and (where applicable) interpret; docs list the new host symbols.

### Stage 23 - Std / runtime: `std.fs` host I/O

Replace the `std.fs` stub with real host file operations (read/write/open as documented).

1. Host ABI in `yarrow-runtime` + AOT exports.
2. Safe Yarrow wrappers in `lib/std/fs.yar`.
3. Fallible error mapping consistent with existing `|T Err|` conventions.

**Gate:** a valid example reads or writes a temp file via `std.fs` under JIT; AOT link still resolves the new symbols.

### Stage 24 - Check without full codegen (optional stretch)

Today check-only still rides Cranelift as an analysis vehicle. If Stage 20–23 do not need it, skip or defer.

1. Separate semantic analysis from `define_function` / module building where cheap.
2. Keep `ExecutionMode::Check` behavior and diagnostics identical for the corpus.

**Gate:** `check_source` on the valid corpus matches today’s success/failure set with no JIT install.

---

## Later (Phase F, backlog)

| Item                                      | Notes                                     |
| ----------------------------------------- | ----------------------------------------- |
| DWARF / AOT `-O` tiers                    | After everyday AOT is stable on linux-gnu |
| Cross-compile triples                     | High effort; needs runtime + CRT story    |
| Bundled linker                            | Only if system `ld`/`lld` is too painful  |
| Multi-file project graph beyond `require` | Language/product decision first           |
| Default backend `object` instead of `jit` | Product/CLI decision                      |

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
