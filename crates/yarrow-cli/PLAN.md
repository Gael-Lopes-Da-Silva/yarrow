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

| Piece              | Status | Notes                                                              |
| ------------------ | ------ | ------------------------------------------------------------------ |
| `yarrow-cli` crate | ✅     | Argument parsing + command dispatch                                |
| Root binary        | ✅     | `src/main.rs` delegates to `yarrow_cli::run`                       |
| Commands           | ✅     | `run`, `check`, `compile`, `dump`, `explain`, `version`            |
| Flags              | ✅     | `--color`, `--error-limit`, `-L`, `-q`, `-v`, `--target`, `--main` |

**Today’s UX:** `cargo run -- <file.yar>` tokenizes, parses, JIT-compiles, runs `main`, prints a supported return value. Exit `0` ok, `1` compile/parse, `2` usage / read failure. `compile --target jit|object` and `run --target` are available; `run --target object` exits `2` until Stage 8.

**Target UX (this plan):** `check` / `run` / `compile` / `interpret`, with `--target jit|object` on `run` and `compile`, and `--main <NAME>` to pick the program entry (default `main`). See [Command set](#command-set).

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

| Value    | Meaning                                                                                   | Core mode               |
| -------- | ----------------------------------------------------------------------------------------- | ----------------------- |
| `jit`    | Cranelift in-process machine code (default). Whole program lowered, then used in-process. | `ExecutionMode::Jit`    |
| `object` | Native relocatable object (AOT). Write an artifact; linking/exec is CLI-side when ready.  | `ExecutionMode::Object` |

Interpretation is **not** a `--target`. It is its own command (`interpret`) so the UX stays: compile backends vs evaluate in the VM.

There is no separate `build` command; native emit is `compile --target object` (and optionally `run --target object` once link+exec exists).

---

## Command set

Modeled on common compiler / language CLIs. Yarrow is a single-file-plus-`require` language for now, so there is no package manager (`new`/`add`/`vendor`) in this plan.

### Intended UX

```text
yarrow check <file>                         # type / ownership / stack check only
yarrow run <file>                           # --target jit (default): JIT + execute entry
yarrow run --target jit <file>              # same, explicit
yarrow run --target object <file>           # native: compile (+ link when ready) and execute
yarrow run --main start <file>              # execute top-level `start` instead of `main`
yarrow compile <file>                       # --target jit (default): lower/codegen, do not run
yarrow compile --target jit <file>          # same, explicit
yarrow compile --target object <file>       # emit native object (-o when implemented)
yarrow compile --main start --target object <file>  # object binds process main → `start`
yarrow interpret <file>                     # interpreter; execute entry
```

**Default:** `yarrow <file.yar>` remains sugar for `yarrow run <file.yar>` (JIT + run). Entry function is `main` unless `--main` is set.

### Ship in this phase (landed or in progress)

| Command   | Analog                          | Behavior                                                                   |
| --------- | ------------------------------- | -------------------------------------------------------------------------- |
| `run`     | `cargo run`, `go run`           | Check + codegen per `--target` + execute entry (`main` or `--main`).       |
| `compile` | `rustc`, `go build -o`          | Check + codegen per `--target`; do not run. Object writes `-o` / `stem.o`. |
| `check`   | `cargo check`, `tsc --noEmit`   | Tokenize, parse, semantic check. Print diagnostics. **Do not** run entry.  |
| `dump`    | `rustc --emit`, `zig ast-check` | Print an intermediate (`tokens`, `ast`, `ir`) and exit. No run.            |
| `explain` | `rustc --explain E0308`         | Print the long form of a diagnostic code (`E308`, …).                      |
| `version` | `rustc -V`                      | Crate / git version string.                                                |

### Next (CLI stages below)

| Command               | Behavior                                                                   | Core need                              |
| --------------------- | -------------------------------------------------------------------------- | -------------------------------------- |
| `interpret`           | Check + interpret the entry (no machine code).                             | Stage 13b interpret API ✅             |
| `run --target object` | Native compile, then link + execute. Stage 6 stubs with exit `2`.          | Object emit + link (core 13c/19 ✅)    |
| `--main` on interpret | Already parsed on `run` / `check` / `compile`; Stage 8 finishes interpret. | `CompileOptions::entry_name` (core 17) |

### Later (other crates or language)

| Command | Notes                                                                            |
| ------- | -------------------------------------------------------------------------------- |
| `fmt`   | Delegate to `yarrow-fmt` when that crate exists.                                 |
| `lsp`   | Usually a separate binary; optional `yarrow lsp` later.                          |
| `test`  | No test runner in the language yet.                                              |
| `repl`  | Interactive shell on core `EvalContext`; not required for `interpret` file mode. |
| `clean` | Only meaningful once `compile --target object` writes artifacts.                 |

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

| Command     | Flags                                                                                                                   |
| ----------- | ----------------------------------------------------------------------------------------------------------------------- |
| `run`       | `<FILE>`; `--target jit\|object` (default `jit`); `--main <NAME>` (default `main`). Optional `--` + program args later. |
| `check`     | `<FILE>`; `--main <NAME>` (default `main`) for the required entry.                                                      |
| `compile`   | `<FILE>`; `--target jit\|object` (default `jit`); `--main <NAME>`; `-o <path>` when object emit lands.                  |
| `interpret` | `<FILE>`; `--main <NAME>` (default `main`).                                                                             |
| `dump`      | `<FILE>` plus `--emit tokens\|ast\|ir` (default `ir`).                                                                  |
| `explain`   | `<CODE>` e.g. `E308`.                                                                                                   |

---

## Exit codes

Keep the current driver convention and align with rustc-ish practice:

| Code  | Meaning                                                                                          |
| ----- | ------------------------------------------------------------------------------------------------ |
| `0`   | Success (`check` clean, `run`/`interpret` finished, `compile`/`dump` done, `explain` found).     |
| `1`   | User program failed to tokenize, parse, or compile (diagnostics printed).                        |
| `2`   | Usage, missing file, I/O, or unimplemented target/command (`object` / `interpret` before ready). |
| `101` | Internal compiler error if we later distinguish ICE from user errors (optional).                 |

`run` / `interpret` of a well-typed program that returns an integer: **print** the value (current behavior). Mapping the entry’s integer to the process exit code is grammar-optional for JIT/`interpret`; native process `main` (core Stage 18) does map it for `--target object` executables.

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

### Stage 6 — `compile` + `--target` ✅

Align the driver with the target UX. Prefer parsing the full surface early; stub backends that core has not finished.

1. Add `TargetKind` (`jit`, `object`) and `--target` on `run` and `compile` (default `jit`).
2. Add `compile` subcommand:
   - `--target jit`: check + JIT lower/finalize; **do not** call `run_main`. Exit `0` if codegen succeeds.
   - `--target object`: call `Session::compile_object_source`; write bytes to `-o` (default `stem.o`).
3. `run --target jit`: today’s behavior (default).
4. `run --target object`: until link+exec works (Stage 8), exit `2` with a clear message. Do not silently fall back to JIT. Core object emit is available (Stage 13c).
5. Keep `yarrow <file>` as sugar for `run --target jit <file>`.
6. Accept `--main <NAME>` on `run` / `compile` / `check`. Forward as `CompileOptions::entry_name` (core Stage 17 ✅).

**Gate:** `--help` lists `compile`, `run --target`, and `compile --target`. `yarrow compile --target jit docs/examples/valid/01_hello.yar` exits `0` without printing `main`’s return value. `yarrow compile --target object …` writes a non-empty `.o` (core 13c). `yarrow run --target object …` exits `2` until link+exec exists.

---

### Stage 7 — `interpret` ⬜

Core Stage 13b interpret API is available (`Session::interpret_source`).

1. `yarrow interpret <file>`: check + interpret the entry (`main` or `--main`); print `RunResult` like `run`.
2. No `--target` on `interpret`.
3. Optional later: `yarrow repl` on `EvalContext` (separate stage; not required here).

**Gate:** `yarrow interpret docs/examples/valid/01_hello.yar` matches `yarrow run` output for that file once core interpret MVP exists. Before that: command may be stubbed with exit `2`.

---

### Stage 8 — Native exec + `--main` ⬜

Depends on core Stages 17–19 (`entry_name`, Cranelift process `main`, `compile_executable_source`).

1. Wire `--main <NAME>` into `CompileOptions::entry_name` for `run`, `check`, `compile`, and `interpret`.
2. `compile --target object` still writes a `.o` unless later we grow a “full executable” `-o` mode; process `main` inside the object honors `--main`.
3. `run --target object`: link (core 19) and exec. Honor `--main`. Do not fall back to JIT.
4. Unknown entry: same E360 path as JIT, exit `1`.

**Gate:** `yarrow run --main main docs/examples/valid/01_hello.yar` matches default `run`. `yarrow run --main does_not_exist …` exits `1` with E360. `run --target object` runs `01_hello.yar` once core 19 is ready.

---

## Definition of done (this crate)

1. Root binary does not import `yarrow_core`.
2. `run` (default / `--target jit`) matches today’s compile-and-execute behavior for the example corpus.
3. `check` is usable for CI (`exit 0` / `1` only from program validity).
4. Help/version/color/error-limit behave as specified.
5. `dump` / `explain` land as core APIs allow.
6. `compile` and `--target` exist; `object` and `interpret` are either real or explicit exit `2`, never a silent no-op.
7. `--main <NAME>` is parsed on `run` / `check` / `compile` / `interpret` and forwarded as `CompileOptions::entry_name` (core Stage 17 ✅).

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands.
- Do not put tokenizer/parser/compiler logic here beyond calling `yarrow-core`.
- Do not invent language features or diagnostic codes in the CLI.
