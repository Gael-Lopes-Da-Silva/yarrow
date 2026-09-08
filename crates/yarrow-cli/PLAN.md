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

| Command     | Behavior                                                     |
| ----------- | ------------------------------------------------------------ |
| `run`       | `--target jit` (default) or `object` (link + exec); `--main`; args after `--` |
| `compile`   | Codegen only; `object` writes `-o` / `stem.o`; `--emit exe` linked binary |
| `check`     | Semantic check only                                          |
| `interpret` | Stack VM via `interpret_source`; `--main`; args after `--` (rejected until core argv) |
| `dump`      | `--emit tokens\|ast\|ir`                                     |
| `explain`   | Long form for a diagnostic code                              |
| `version`   | Crate version (`-V` too)                                     |

**Defaults:** `yarrow <file.yar>` → `run --target jit`. Entry name `main` unless `--main` is set.

**Global flags:** `--color`, `--error-limit`, `-L` / `--search-path`, `-q`, `-v`.

**Exit codes:** `0` ok, `1` program diagnostics (incl. link), `2` usage / I/O / signal. Native `run --target object` propagates the child exit status when in `0..=255`.

Stages 1–10 are complete. Historical stage write-ups were removed; git history keeps them.

---

## Targets (reference)

| Value    | `run`                  | `compile`              |
| -------- | ---------------------- | ---------------------- |
| `jit`    | JIT + execute entry    | JIT lower; do not run  |
| `object` | Link executable + exec | Write relocatable `.o` or linked `exe` via `--emit` |

`interpret` is not a `--target`.

---

## Next

Driver polish and optional tools. Prefer core Stages 20–23 before a heavy `repl`.

### Stage 9 - Executable emit from `compile` ✅

`--emit object|exe` on `compile` (default `object` when `--target object`). `exe` calls `Session::compile_executable_source` and sets execute permission. `run --target object` still compile-link-execs without keeping the binary.

**Gate:** `yarrow compile --target object --emit exe -o /tmp/hello docs/examples/valid/01_hello.yar` produces a runnable file; `--help` documents `--emit`.

### Stage 10 - Program arguments ✅

`run` / `interpret` take `ARGS` after `--` (`last = true`). `run --target object` forwards them as OS argv to the child. JIT / interpret reject non-empty args (exit `2`) until core exposes a language argv API. Native `object` sees OS argv regardless of language-level argv.

**Gate:** `yarrow run --target object docs/examples/valid/01_hello.yar --` still works with no program args. With args, the child receives them (`strace`/`/proc`). Missing core argv support does not break no-arg runs.

### Stage 11 - `repl` (blocked on core interpret depth)

Interactive loop on `EvalContext`.

1. Depends on useful Stage 21 interpret coverage.
2. Line-oriented: eval statements / expressions as the language allows; print `RunResult`.
3. No package manager, no multi-file project UI.

**Gate:** `yarrow repl` starts, evaluates a trivial snippet equivalent to printing a string or int, exits cleanly on EOF/`exit`.

### Stage 12 - Tooling subcommands (thin wrappers)

Only when the other crate can run.

1. `yarrow fmt -- …` delegates to `yarrow_fmt::format_*` in-process when [`yarrow-fmt/PLAN.md`](../yarrow-fmt/PLAN.md) Stage 11+ has landed (see that plan for style-guide stages).
2. Optional `yarrow lsp` similarly when [`yarrow-lsp/PLAN.md`](../yarrow-lsp/PLAN.md) Stage 10+ has landed (`yarrow_lsp::run_stdio`).
3. `yarrow clean` removes default `*.o` / known `-o` artifacts in the cwd if we document a convention; skip if still pointless.

**Gate:** each wired subcommand either works or is absent from `--help` (no stub that exits `2` pretending to be real). Prefer omitting until ready.

---

## Later (backlog)

| Item                      | Notes                                                    |
| ------------------------- | -------------------------------------------------------- |
| Default `--target object` | Product choice; keep `jit` default until AOT is the norm |
| `test` subcommand         | Needs a language-level test story                        |
| ICE exit `101`            | Optional once core distinguishes ICE                     |
| Color / quiet polish      | Only if real UX pain shows up                            |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark ✅, short notes; do not re-expand history).
- No tokenizer/parser/compiler logic beyond calling `yarrow-core`.
- Do not invent language features or diagnostic codes in the CLI.
