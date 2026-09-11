# Yarrow CLI Implementation Plan

User-facing compiler driver. Talks to **`yarrow-core` only**. Owns clap, stdout/stderr, color, and process exit codes.

Root binary [`src/main.rs`](../../src/main.rs) stays a thin `yarrow_cli::run(args) -> ExitCode` wrapper.

Language and compiler work: [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md). Formatter / LSP stay separate crates (`yarrow-fmt`, `yarrow-lsp`) and may be invoked as subcommands later.

## Source of truth

| Role          | Path                                                   |
| ------------- | ------------------------------------------------------ |
| Compiler API  | [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md) |
| Language docs | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md)             |
| Corpus        | [`docs/examples/`](../../docs/examples/README.md)      |
| Agent rules   | [`AGENTS.md`](../../AGENTS.md)                         |

Do not reimplement checking or codegen here. If a command needs a missing core feature, add it to `yarrow-core` first (or mark the command blocked).

---

## Landed

```text
src/main.rs  →  yarrow_cli::run
                  ├── clap (commands + global flags)
                  └── yarrow_core::Session
```

| Command     | Behavior                                                                              |
| ----------- | ------------------------------------------------------------------------------------- |
| `run`       | `--target object` (default, link + exec) or `jit`; `--main`; args after `--`          |
| `compile`   | Codegen only; default `object` writes `-o` / `stem.o`; `--emit exe` linked binary     |
| `check`     | Semantic check only                                                                   |
| `interpret` | Stack VM via `interpret_source`; `--main`; args after `--` (rejected until core argv) |
| `repl`      | Line-oriented `EvalContext` loop; wraps snippets as `main`; EOF/`exit`/`quit`         |
| `lsp`       | Language server stdio (`yarrow_lsp::run_stdio_blocking`)                              |
| `fmt`       | Format `.yar` in-process via `yarrow_fmt::run_fmt`                                    |
| `dump`      | `--emit tokens\|ast\|ir`                                                              |
| `explain`   | Long form for a diagnostic code                                                       |
| `version`   | Crate version (`-V` too)                                                              |

**Defaults:** `yarrow <file.yar>` → `run --target object` (Stage 29; matches `CompileOptions` / `ExecutionMode::Object`). Entry name `main` unless `--main` is set. Use `--target jit` for in-process run.

**Global flags:** `--color`, `--error-limit`, `-L` / `--search-path`, `-q`, `-v`.

**Exit codes:** `0` ok, `1` program diagnostics (incl. link), `2` usage / I/O / signal. Native `run --target object` propagates the child exit status when in `0..=255`.

Stages 1–12 are complete. Historical stage write-ups were removed; git history keeps them.

---

## Targets (reference)

| Value    | `run`                  | `compile`                                           |
| -------- | ---------------------- | --------------------------------------------------- |
| `jit`    | JIT + execute entry    | JIT lower; do not run                               |
| `object` | Link executable + exec | Write relocatable `.o` or linked `exe` via `--emit` |

`interpret` is not a `--target`.

---

## Next

No further CLI stages. Optional polish lives in Later.

### Stage 9 - Executable emit from `compile` ✅

`--emit object|exe` on `compile` (default `object` when `--target object`). `exe` calls `Session::compile_executable_source` and sets execute permission. `run --target object` still compile-link-execs without keeping the binary.

**Gate:** `yarrow compile --target object --emit exe -o /tmp/hello docs/examples/valid/01_hello.yar` produces a runnable file; `--help` documents `--emit`.

### Stage 10 - Program arguments ✅

`run` / `interpret` take `ARGS` after `--` (`last = true`). `run --target object` forwards them as OS argv to the child. JIT / interpret reject non-empty args (exit `2`) until core exposes a language argv API. Native `object` sees OS argv regardless of language-level argv.

**Gate:** `yarrow run --target object docs/examples/valid/01_hello.yar --` still works with no program args. With args, the child receives them (`strace`/`/proc`). Missing core argv support does not break no-arg runs.

### Stage 11 - `repl` ✅

Interactive loop on `EvalContext`. Line-oriented: snippets without top-level `function` / `require` wrap as `main`; leftover stack values (W403) promote to `end with <type>` so expressions print as `RunResult`. No package manager / multi-file UI.

**Gate:** `yarrow repl` starts; `42` / `"hi"` print; clean exit on EOF / `exit` / `quit`.

### Stage 12 - Tooling subcommands (thin wrappers) ✅

1. `yarrow fmt` wired: in-process `yarrow_fmt::run_fmt` (`--check`, `--stdin`, `--max-width`, `--sort-requires` / `--no-sort-requires`, `--reorder-layout`, `--best-effort`, directory recurse). Same exit codes as `yarrow-fmt`.
2. `yarrow lsp` wired: in-process `yarrow_lsp::run_stdio_blocking` with `--stdio`, `-L`, `--main`, `--no-format`, `--log-level` (see [`yarrow-lsp` Stage 10](../yarrow-lsp/PLAN.md)).
3. `yarrow clean` skipped: no documented build-dir / artifact manifest; default `stem.o` / bare `stem` in cwd is not enough to clean safely.

**Gate:** `yarrow --help` lists `fmt` and `lsp` (not `clean`). `yarrow fmt --check docs/examples/valid/01_hello.yar` exits `0`. `cargo fmt && cargo check && cargo clippy` green.

**Done:** `Cmd::Fmt` → `commands::run_fmt_command`; `Cmd::Lsp` → `commands::run_lsp`.

---

## Later (backlog)

| Item                 | Notes                                             |
| -------------------- | ------------------------------------------------- |
| `yarrow clean`       | Only if a build-artifact convention is documented |
| `test` subcommand    | Needs a language-level test story                 |
| ICE exit `101`       | Optional once core distinguishes ICE              |
| Color / quiet polish | Only if real UX pain shows up                     |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark ✅, short notes; do not re-expand history).
- No tokenizer/parser/compiler logic beyond calling `yarrow-core`.
- Do not invent language features or diagnostic codes in the CLI.
