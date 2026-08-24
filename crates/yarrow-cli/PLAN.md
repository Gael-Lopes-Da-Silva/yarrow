# Yarrow CLI Implementation Plan

User-facing compiler driver. This crate talks to **`yarrow-core` only** for tokenize / parse / check / compile / run / interpret / dump. It owns argument parsing, subcommands, stdout/stderr, color, and process exit codes.

The application entry point is the workspace binary [`src/main.rs`](../../src/main.rs), which should stay a thin `yarrow_cli::main()` (or `run(args) -> ExitCode`) wrapper.

Language and compiler work: [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md). Formatter / LSP crates stay separate (`yarrow-fmt`, `yarrow-lsp`) and may be invoked as subcommands later.

## Source of truth

| Role                 | Path                                                   |
| -------------------- | ------------------------------------------------------ |
| Compiler library API | [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md) |
| Language docs        | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md)             |
| Example corpus       | [`docs/examples/`](../../docs/examples/README.md)      |
| Agent rules          | [`AGENTS.md`](../../AGENTS.md)                         |

Do not reimplement checking or codegen in this crate. If a command needs a compiler feature that does not exist yet, add it to `yarrow-core` first (or mark the command blocked).

---

## Current state

| Piece              | Status | Notes                                        |
| ------------------ | ------ | -------------------------------------------- |
| `yarrow-cli` crate | ✅     | Argument parsing + `run`/`check` dispatch    |
| Root binary        | ✅     | `src/main.rs` delegates to `yarrow_cli::run` |
| Commands           | ✅     | `run`, `check`, `dump`, `explain`, `version` |
| Flags              | ✅     | `--color`, `--error-limit`, `-L`, `-q`, `-v` |

**Today’s UX:** `cargo run -- <file.yar>` tokenizes, parses, JIT-compiles, runs `main`, prints a supported return value. Exit `0` ok, `1` compile/parse, `2` usage / read failure.

**Target UX (this plan):** `check` / `run` / `compile` / `interpret`, with `--target jit|object` on `run` and `compile`. See [Command set](#command-set).

---

## Architecture

```text
src/main.rs          # binary crate `yarrow`
    └── yarrow_cli::run(args) -> ExitCode
            ├── clap (commands + global flags)
            └── yarrow_core::Session (check / compile / run / interpret / emit)
```

Workspace change when Stage 1 lands:

- Root `yarrow` package depends on `yarrow_cli`, not `yarrow_core`.
- `yarrow_cli` depends on `yarrow_core` and `clap`.

---

## Targets

Shared by `run` and `compile` via `--target`:

| Value    | Meaning                                                                 | Core mode              |
| -------- | ----------------------------------------------------------------------- | ---------------------- |
| `jit`    | Cranelift in-process machine code (default). Whole program lowered, then used in-process. | `ExecutionMode::Jit`   |
| `object` | Native relocatable object (AOT). Write an artifact; linking/exec is CLI-side when ready. | `ExecutionMode::Object` |

Interpretation is **not** a `--target`. It is its own command (`interpret`) so the UX stays: compile backends vs evaluate in the VM.

There is no separate `build` command; native emit is `compile --target object` (and optionally `run --target object` once link+exec exists).

---

## Command set

Modeled on common compiler / language CLIs. Yarrow is a single-file-plus-`require` language for now, so there is no package manager (`new`/`add`/`vendor`) in this plan.

### Intended UX

```text
yarrow check <file>                         # type / ownership / stack check only
yarrow run <file>                           # --target jit (default): JIT + execute main
yarrow run --target jit <file>              # same, explicit
yarrow run --target object <file>           # native: compile (+ link when ready) and execute
yarrow compile <file>                       # --target jit (default): lower/codegen, do not run main
yarrow compile --target jit <file>          # same, explicit
yarrow compile --target object <file>       # emit native object (-o when implemented)
yarrow interpret <file>                     # stack VM / interpreter; execute main
```

**Default:** `yarrow <file.yar>` remains sugar for `yarrow run <file.yar>` (JIT + run).

### Ship in this phase (landed or in progress)

| Command   | Analog                          | Behavior                                                                      |
| --------- | ------------------------------- | ----------------------------------------------------------------------------- |
| `run`     | `cargo run`, `go run`           | Check + codegen per `--target` + execute `main` (JIT today).                  |
| `check`   | `cargo check`, `tsc --noEmit`   | Tokenize, parse, semantic check. Print diagnostics. **Do not** run `main`.    |
| `dump`    | `rustc --emit`, `zig ast-check` | Print an intermediate (`tokens`, `ast`, `ir`) and exit. No run.               |
| `explain` | `rustc --explain E0308`         | Print the long form of a diagnostic code (`E308`, …).                         |
| `version` | `rustc -V`                      | Crate / git version string.                                                   |

### Next (CLI stages below; need core Stage 13)

| Command      | Behavior                                                                                          | Core need                                      |
| ------------ | ------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `compile`    | Codegen only (no `main`). Default `--target jit`. `--target object` writes a native artifact.     | `Jit` finalize without run; `Object` emit      |
| `--target`   | On `run` and `compile`: `jit` \| `object` (default `jit`).                                        | `ExecutionMode`                                |
| `interpret`  | Check + interpret `main` (no machine code).                                                       | Stage 13b interpret API                        |
| `run --target object` | Native compile, then execute (link step when available). Until ready: clear exit `2`.     | Object emit (+ link story)                     |

### Later (other crates or language)

| Command | Notes                                                            |
| ------- | ---------------------------------------------------------------- |
| `fmt`   | Delegate to `yarrow-fmt` when that crate exists.                 |
| `lsp`   | Usually a separate binary; optional `yarrow lsp` later.          |
| `test`  | No test runner in the language yet.                              |
| `repl`  | Interactive shell on core `EvalContext`; not required for `interpret` file mode. |
| `clean` | Only meaningful once `compile --target object` writes artifacts. |

---

## Global flags

Available on every command (clap `global = true` where it makes sense):

| Flag                          | Default                | Purpose                                                                        |
| ----------------------------- | ---------------------- | ------------------------------------------------------------------------------ |
| `--color always\|never\|auto` | `auto`                 | Maps to `yarrow_core::ColorChoice`. Honor `NO_COLOR` when `auto`.              |
| `--error-limit <n>`           | core default (20)      | Passed into `DiagnosticBatch` / session options.                               |
| `--search-path <dir>` / `-L`  | entry file’s directory | Extra `require` search paths (`Compiler::add_module_search_path`). Repeatable. |
| `-q` / `--quiet`              | off                    | Diagnostics only on failure; no “running …” chatter.                           |
| `-v` / `--verbose`            | off                    | Extra driver progress on stderr (not IR; use `dump ir`).                       |
| `-h` / `--help`               |                        | Clap generated.                                                                |
| `-V` / `--version`            |                        | Same as `version` subcommand.                                                  |

Command-specific:

| Command     | Flags                                                                                          |
| ----------- | ---------------------------------------------------------------------------------------------- |
| `run`       | `<FILE>`; `--target jit\|object` (default `jit`). Optional `--` + program args later.          |
| `check`     | `<FILE>` required.                                                                             |
| `compile`   | `<FILE>`; `--target jit\|object` (default `jit`); `-o <path>` when object emit lands.          |
| `interpret` | `<FILE>` required.                                                                             |
| `dump`      | `<FILE>` plus `--emit tokens\|ast\|ir` (default `ir`).                                         |
| `explain`   | `<CODE>` e.g. `E308`.                                                                          |

---

## Exit codes

Keep the current driver convention and align with rustc-ish practice:

| Code  | Meaning                                                                                          |
| ----- | ------------------------------------------------------------------------------------------------ |
| `0`   | Success (`check` clean, `run`/`interpret` finished, `compile`/`dump` done, `explain` found).     |
| `1`   | User program failed to tokenize, parse, or compile (diagnostics printed).                        |
| `2`   | Usage, missing file, I/O, or unimplemented target/command (`object` / `interpret` before ready). |
| `101` | Internal compiler error if we later distinguish ICE from user errors (optional).                 |

`run` / `interpret` of a well-typed program that returns an integer: **print** the value (current behavior). Mapping `main`’s integer to the process exit code is grammar-optional and is **not** the first CLI stage.

---

## Stages

### Stage 1 — Clap skeleton, `run`, thin `main` ✅

1. Add `clap` (derive) to `yarrow-cli`.
2. Implement `yarrow_cli::run<I, S>(args: I) -> ExitCode`.
3. Subcommands: `run` (plus default file positional).
4. Wire color, read file, call **core session or today’s pipeline** (if core Stage 11 is not landed, temporarily keep the pipeline in the CLI behind a private helper, then delete it when session exists). Prefer waiting on / pairing with core Stage 11.
5. Root `src/main.rs`: `std::process::ExitCode` from `yarrow_cli::run(std::env::args_os())`.
6. Root `Cargo.toml`: depend on `yarrow_cli` only.

**Gate:** `cargo run -- docs/examples/valid/01_hello.yar` still works. `cargo run -- --help` prints clap help. Invalid usage with no file exits `2`. `cargo fmt --all && cargo check && cargo clippy` green.

---

### Stage 2 — `check` ✅

1. `yarrow check <file>`: tokenize, parse, compile; print diagnostics; **do not** `run_main`.
2. Success: exit `0`, no program output.
3. Failure: same rendering as `run` (batches, error-limit abort line).

**Gate:** `yarrow check docs/examples/valid/*.yar` exits 0; `yarrow check docs/examples/invalid/*.yar` exits 1 with rustc-style diagnostics. Valid files still run via `yarrow run` / default.

---

### Stage 3 — Global polish ✅

1. `--color`, `NO_COLOR`, `--error-limit`, `-L` / `--search-path`, `-q` / `-v`.
2. `version` / `-V`.
3. Consistent diagnostic printing helper in this crate only (path fill, batch cap message). Core still owns `render`.

**Gate:** `YARROW_DBG_IR` is **not** required for CLI tests. `--color never` output has no ANSI. `--error-limit 1` on `invalid/12_multi_error.yar` prints the abort line.

---

### Stage 4 — `dump` ✅

Depends on core IR dump API (landed) for `ir`. Tokens/AST from tokenizer/parser.

1. `--emit tokens` — debug-friendly token list with spans.
2. `--emit ast` — Debug or a stable compact dump of `Program`.
3. `--emit ir` — Cranelift IR via core API.

**Gate:** `dump --emit tokens` on `01_hello.yar` exits 0 with tokens on stdout. `dump --emit ir` works once core exposes IR.

---

### Stage 5 — `explain` ✅

1. Static or core-provided table: code → paragraph (reuse teachable notes from Phase B).
2. Unknown code: message + exit `2`.

**Gate:** `yarrow explain E308` prints unwrap/fallible help; `yarrow explain E999` exits `2`.

---

### Stage 6 — `compile` + `--target` ⬜

Align the driver with the target UX. Prefer parsing the full surface early; stub backends that core has not finished.

1. Add `TargetKind` (`jit`, `object`) and `--target` on `run` and `compile` (default `jit`).
2. Add `compile` subcommand:
   - `--target jit`: check + JIT lower/finalize; **do not** call `run_main`. Exit `0` if codegen succeeds.
   - `--target object`: call core object emit; write `-o` when implemented. Until core is ready: stderr message, exit `2`.
3. `run --target jit`: today’s behavior (default).
4. `run --target object`: until link+exec works, exit `2` with a clear message (or compile-only note). Do not silently fall back to JIT.
5. Keep `yarrow <file>` as sugar for `run --target jit <file>`.

**Gate:** `--help` lists `compile`, `run --target`, and `compile --target`. `yarrow compile --target jit docs/examples/valid/01_hello.yar` exits `0` without printing `main`’s return value. `yarrow run --target object …` and `compile --target object …` exit `2` with an explicit not-implemented message until core Stage 13c lands.

---

### Stage 7 — `interpret` ⬜

Blocked on core Stage 13b.

1. `yarrow interpret <file>`: check + interpret `main`; print `RunResult` like `run`.
2. No `--target` on `interpret`.
3. Optional later: `yarrow repl` on `EvalContext` (separate stage; not required here).

**Gate:** `yarrow interpret docs/examples/valid/01_hello.yar` matches `yarrow run` output for that file once core interpret MVP exists. Before that: command may be stubbed with exit `2`.

---

## Definition of done (this crate)

1. Root binary does not import `yarrow_core`.
2. `run` (default / `--target jit`) matches today’s compile-and-execute behavior for the example corpus.
3. `check` is usable for CI (`exit 0` / `1` only from program validity).
4. Help/version/color/error-limit behave as specified.
5. `dump` / `explain` land as core APIs allow.
6. `compile` and `--target` exist; `object` and `interpret` are either real or explicit exit `2`, never a silent no-op.

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands.
- Do not put tokenizer/parser/compiler logic here beyond calling `yarrow-core`.
- Do not invent language features or diagnostic codes in the CLI.
