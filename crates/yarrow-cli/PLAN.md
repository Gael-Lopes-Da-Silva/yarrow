# Yarrow CLI Implementation Plan

User-facing compiler driver. Talks to **`yarrow-core` only**. Owns clap, stdout/stderr, color, and process exit codes.

Root binary [`src/main.rs`](../../src/main.rs) stays a thin `yarrow_cli::run(args) -> ExitCode` wrapper.

Language and compiler work: [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md). Formatter / LSP stay separate crates; CLI wires them as thin in-process subcommands (`fmt`, `lsp`).

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
| `compile`   | Codegen only; default `./<stem>.o` / `./<stem>`; `-o` overrides; records `.yarrow-build/artifacts` |
| `check`     | Semantic check; one file → `check_source`; two+ roots → `check_project`; `--corpus DIR` → non-recursive `*.yar` batch |
| `interpret` | Stack VM via `interpret_source`; `--main`; args after `--` (rejected until core argv) |
| `repl`      | Line-oriented `EvalContext` loop; wraps snippets as `main`; EOF/`exit`/`quit`         |
| `lsp`       | Language server stdio / `--listen` (`yarrow_lsp`)                                     |
| `fmt`       | Format `.yar` in-process via `yarrow_fmt::run_fmt`                                    |
| `dump`      | `--emit tokens\|ast\|ir`                                                              |
| `explain`   | Long form for a diagnostic code                                                       |
| `clean`     | Delete only paths listed in `.yarrow-build/artifacts` (idempotent; no `*.o` glob)     |
| `version`   | Crate version (`-V` too)                                                              |

**Defaults:** `yarrow <file.yar>` → `run --target object` (matches `CompileOptions` / `ExecutionMode::Object`). Entry name `main` unless `--main` is set. Use `--target jit` for in-process run.

**Global flags:** `--color` (`auto` honors `NO_COLOR` / `CLICOLOR_FORCE` / `FORCE_COLOR`; explicit wins), `--error-limit`, `-L` / `--search-path`, `-q` (suppress driver chatter, never diagnostics / explain / dump payload), `-v` (stderr progress; no-op under `-q`).

**Exit codes:** `0` ok, `1` program diagnostics (incl. link), `2` usage / I/O / signal, `101` internal compiler error (caught panic or `E999` / `SessionFailureKind::Ice`). Native `run --target object` propagates the child exit status when in `0..=255`.

Stages 1–17 are complete. Historical stage write-ups were removed; git history keeps them.

---

## Targets (reference)

| Value    | `run`                  | `compile`                                           |
| -------- | ---------------------- | --------------------------------------------------- |
| `jit`    | JIT + execute entry    | JIT lower; do not run                               |
| `object` | Link executable + exec | Write relocatable `.o` or linked `exe` via `--emit` |

`interpret` is not a `--target`.

---

## Next

Focus: UX / artifact polish. Do not invent language features or a package manifest in the CLI.

### Stage 13 - Multi-root project `check` ✅

Clap: `yarrow check FILE [FILE...]`. One path → `Session::check_source`; two or more → `ProjectOptions::from_root_paths` + `Session::check_project` (shared `-L`, `--main`, `--error-limit`; `require_main` true). Missing roots → rendered `E383`, exit `1`. `-v` lists `CheckedProject.graph.modules` on stderr. Docs: RUNTIME Projects, `docs/examples/project/README.md`.

**Gate:** `yarrow check docs/examples/project/root_a.yar docs/examples/project/root_b.yar` exits `0`; single-file check unchanged.

---

### Stage 14 - Color / quiet / env polish ✅

Quiet: `-q` suppresses `wrote …`, repl banners/prompts, and `-v` progress (`GlobalArgs::progress`); never diagnostics or `explain` / `dump` stdout. Verbose: progress only; no exit-code changes. Color: `--color auto` via core `ColorChoice` (`NO_COLOR` → never; non-empty `CLICOLOR_FORCE` / `FORCE_COLOR` → always); explicit `always`/`never` wins. `fmt` / `lsp`: no color API; `lsp` honors `-q` for startup banner; `fmt` quiet/color are documented no-ops.

**Gate:** `yarrow check -q` success is silent; invalid still prints diagnostics; `NO_COLOR=1` + `--color auto` → uncolored text.

---

### Stage 15 - Artifact convention + `yarrow clean` ✅

Defaults stay cwd: `./<stem>.o` (`--emit object`) and `./<stem>` (`--emit exe`); `-o PATH` overrides. Successful `compile` writes record the path in `.yarrow-build/artifacts`. `yarrow clean` deletes only those listed paths (never a recursive `*.o` wipe); missing files / missing manifest → exit `0`. Pre-manifest cwd objects are not auto-deleted.

**Gate:** `compile` then `clean` removes the recorded artifact; second `clean` exits `0`; unrelated `*.o` untouched.

---

### Stage 16 - ICE → exit `101` ✅

Command dispatch is wrapped in `std::panic::catch_unwind`: unexpected panics print an ICE banner to stderr and exit `101`. Session failures use `SessionFailureKind` ([`yarrow-core` Stage 40](../yarrow-core/PLAN.md)): `Ice` (`E999`) → `101`, `User` → `1`. I/O and clap usage stay `2`. Gate hooks: `YARROW_DEBUG_ICE=panic` (caught panic) and `YARROW_DEBUG_ICE=session` (`debug_trigger_ice`).

**Gate:** debug panic / session ICE → `101`; `check` on `invalid/**` still `1`.

---

### Stage 17 - Corpus / `test` driver (no language invent) ✅

`yarrow check --corpus DIR` walks immediate `*.yar` in `DIR` (non-recursive; nested helpers are not separate roots) and runs `check_source` on each. Aggregate: exit `0` if all succeed, `1` if any program diagnostics (`101` if any ICE); prints `N ok, M failed` unless `-q`. Honors `-L`, `--error-limit`, `--main`. Not a language-level test framework. Gate corpus: `docs/examples/valid`.

**Gate:** `yarrow check --corpus docs/examples/valid` exits `0`; a tree with a known-bad file exits `1` with diagnostics.

---

## Later (backlog)

| Item                     | Notes                                                                   |
| ------------------------ | ----------------------------------------------------------------------- |
| Language-level tests     | After Stage 17 corpus driver ✅; needs GRAMMAR / core, not CLI invention |
| JIT / interpret argv     | Wire when core exposes language argv; Stage 10 already forwards OS argv |
| Multi-root compile / run | After Stage 13 check-only; needs a defined multi-entry product story    |
| LSP project workspace    | [`yarrow-lsp` Stage 21](../yarrow-lsp/PLAN.md); depends on Stage 13 UX + core graph |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark ✅, short notes; do not re-expand history).
- No tokenizer/parser/compiler logic beyond calling `yarrow-core`.
- Do not invent language features or diagnostic codes in the CLI.
