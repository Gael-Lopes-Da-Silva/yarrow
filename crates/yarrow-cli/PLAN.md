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
| `compile`   | Codegen only; default `object` writes `-o` / `stem.o`; `--emit exe` linked binary     |
| `check`     | Semantic check; one file → `check_source`; two+ roots → `check_project` |
| `interpret` | Stack VM via `interpret_source`; `--main`; args after `--` (rejected until core argv) |
| `repl`      | Line-oriented `EvalContext` loop; wraps snippets as `main`; EOF/`exit`/`quit`         |
| `lsp`       | Language server stdio / `--listen` (`yarrow_lsp`)                                     |
| `fmt`       | Format `.yar` in-process via `yarrow_fmt::run_fmt`                                    |
| `dump`      | `--emit tokens\|ast\|ir`                                                              |
| `explain`   | Long form for a diagnostic code                                                       |
| `version`   | Crate version (`-V` too)                                                              |

**Defaults:** `yarrow <file.yar>` → `run --target object` (matches `CompileOptions` / `ExecutionMode::Object`). Entry name `main` unless `--main` is set. Use `--target jit` for in-process run.

**Global flags:** `--color`, `--error-limit`, `-L` / `--search-path`, `-q`, `-v`.

**Exit codes:** `0` ok, `1` program diagnostics (incl. link), `2` usage / I/O / signal. Native `run --target object` propagates the child exit status when in `0..=255`.

Stages 1–13 are complete. Historical stage write-ups were removed; git history keeps them.

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

### Stage 14 - Color / quiet / env polish

Flags exist (`--color`, `-q`, `-v`) but policy is uneven across commands.

1. Document and implement one quiet policy: `-q` suppresses driver chatter (`wrote …`, repl banners, verbose progress) and **never** suppresses diagnostics or `explain` / `dump` payload on stdout.
2. Document verbose: `-v` may add progress on stderr; it must not change exit codes or hide errors.
3. Honor common env for auto color when `--color auto`: `NO_COLOR` forces never; optional `CLICOLOR_FORCE` / `FORCE_COLOR` forces always only if already cheap with the core `ColorChoice` path. Explicit `--color always|never` always wins over env.
4. Ensure `fmt` / `lsp` wrappers respect global `--color` / `-q` where those crates expose an equivalent (pass through or no-op with a short note if the child API has no color knob).
5. Touch `--help` / this plan’s Landed exit-code / flag blurbs if behavior changes.

**Gate:** `yarrow check -q docs/examples/valid/01_hello.yar` prints nothing on success; a deliberate invalid file still prints diagnostics on stderr. `NO_COLOR=1 yarrow check …` (with `--color auto`) produces uncolored diagnostic text. `cargo clippy` green.

---

### Stage 15 - Artifact convention + `yarrow clean`

Stage 12 skipped `clean`: default `stem.o` / bare `stem` in cwd is unsafe to delete without a documented convention.

1. Document the compile output convention in `--help` and a short RUNTIME or CLI blurb:
   - default object: `./<stem>.o` next to cwd (current behavior)
   - default exe: `./<stem>`
   - `-o PATH` overrides; only that path is the artifact
2. Prefer a **manifest sidecar** or **known-safe delete list** over “rm every `*.o` in cwd”:
   - e.g. `compile` writes `.yarrow-build/<stem>.o` (or records paths in `.yarrow-build/artifacts`) **or**
   - `clean` only deletes paths listed in a manifest produced by this CLI’s `compile`, never recursive glob of the user’s tree.
3. Add `yarrow clean` that removes only those documented artifacts; missing artifacts → exit `0` (idempotent); refuse unsafe globs.
4. If changing default output dirs, keep a migration note: old cwd `stem.o` is not auto-deleted by `clean` unless it appears in the manifest.
5. Do not invent a package manager or project file; this is compile-output hygiene only.

**Gate:** `yarrow compile --target object docs/examples/valid/01_hello.yar` then `yarrow clean` removes the documented artifact and exits `0`; a second `clean` exits `0`. `yarrow --help` lists `clean`. No deletion of unrelated `*.o` outside the convention. `cargo fmt && cargo check && cargo clippy` green.

**Blocked alternative:** if a manifest / build-dir change is too invasive, keep `clean` omitted and leave a Done note pointing here; do not ship a dangerous cwd wipe.

---

### Stage 16 - ICE → exit `101`

Today panics / bugs surface as process abort or generic failure. Rustc-style tools use `101` for internal compiler errors so scripts can distinguish ICE from user diagnostics (`1`) and usage (`2`).

1. Install a process-level catch around `yarrow_cli::run`’s command dispatch (or the root binary): `std::panic::catch_unwind` (UnwindSafe boundaries as needed) → print a short “internal compiler error” message + panic payload to stderr → `ExitCode::from(101)`.
2. Do **not** map ordinary `SessionDiagnostics` or I/O errors to `101`.
3. Optional stretch only if [`yarrow-core` Stage 40](../yarrow-core/PLAN.md) already tags ICE: if a diagnostic code / severity means ICE, map that batch to `101` as well; otherwise keep this CLI-only panic catch and leave tagging to core.
4. Document exit codes in Landed: `0` / `1` / `2` / `101`.
5. No new language diagnostics invented in the CLI.

**Gate:** a deliberate `panic!` behind a `#[cfg(test)]` or documented debug hook (or a one-off `RUST_BACKTRACE` agent check) yields exit `101` and a clear stderr line; normal `check` on `invalid/**` still exits `1`. `cargo clippy` green.

---

### Stage 17 - Corpus / `test` driver (no language invent)

There is no language-level `test` / `assert` story yet. Do not invent one in the CLI.

1. Add a thin driver, named either `yarrow test` **or** `yarrow check --corpus DIR` (pick one; prefer a name that does not imply unit-test syntax):
   - Walk a directory of `.yar` files (non-recursive or recursive; document which).
   - For each file: `check_source` (or `check_project` when multiple roots are passed explicitly).
   - Aggregate: exit `0` if all succeed; `1` if any program diagnostics; print a one-line summary (`N ok, M failed`) unless `-q`.
2. Default corpus for the gate: `docs/examples/valid` (must all pass). Do **not** require `invalid/**` to pass; optional `--expect-fail` / separate mode is out of scope unless trivial.
3. Skip or document non-`.yar` files; honor `-L`, `--error-limit`, `--main` as for `check`.
4. When a real language test story lands in core / GRAMMAR, replace or extend this stage’s driver; until then the command is a **corpus check**, not a test framework.
5. Update examples README with the one-liner agents should run.

**Gate:** `yarrow test docs/examples/valid` (or the chosen alias) exits `0`. Pointing at a tree that includes a known-bad file exits `1` with at least one rendered diagnostic. `cargo fmt && cargo check && cargo clippy` green.

---

## Later (backlog)

| Item                     | Notes                                                                   |
| ------------------------ | ----------------------------------------------------------------------- |
| Language-level tests     | After Stage 17 corpus driver; needs GRAMMAR / core, not CLI invention   |
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
