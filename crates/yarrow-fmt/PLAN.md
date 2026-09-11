# Yarrow Formatter Implementation Plan

Library and binary that rewrite `.yar` source to match [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md).

`yarrow-fmt` is a **tooling crate**. It may depend on `yarrow-core` for tokenize / parse / diagnostics. It does **not** type-check, borrow-check, or codegen. CLI wiring (`yarrow fmt`) lives in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md). Editors format via [`yarrow-lsp`](../yarrow-lsp/PLAN.md) calling the same `format_source` API.

## Source of truth

| Role                        | Path                                                                                                             |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **Layout / idiomatic form** | [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md)                                                               |
| Language syntax             | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md), [`SYNTAX.md`](../../docs/SYNTAX.md)                                  |
| Intended AST                | [`docs/AST.md`](../../docs/AST.md)                                                                               |
| Corpus (format gates)       | `docs/examples/{valid,warnings,project}/**`, `crates/yarrow-core/lib/std/**`, `fixtures/stage19_ignore_regions.yar` ([`scripts/fmt-check.sh`](../../scripts/fmt-check.sh)) |
| Compiler API                | [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md)                                                           |
| Agent rules                 | [`AGENTS.md`](../../AGENTS.md)                                                                                   |

When the style guide and the formatter disagree, **change the formatter** (or amend the guide deliberately). Do not invent layout rules absent from the style guide.

---

## Scope

### In scope

Mechanical rewrite of parseable source:

- Source file hygiene: UTF-8, LF, no trailing whitespace, final newline
- Indent: **one tab per nesting level**; no space indent
- Soft wrap target: **100 columns** (break before consuming words); `FormatOptions::max_width` / `--max-width`
- Blank-line rules between top-level items and inside bodies
- Construct layout: `require`, types, `implement`, functions, `if`/`match`/`for`/`defer`/`unsafe`, calls, containers
- Comment text preserved; spacing around `#` normalized where the guide is explicit (`# ` after hash; one space before trailing `#`)
- Opt-in file-layout reorder; require sorting on by default (`--no-sort-requires` to disable)
- Best-effort hygiene on incomplete parse; selective construct reprint when recovery spans are trustworthy (Stage 18)
- Diff-friendly ignore regions via `# yarrow-fmt-ignore-begin` / `# yarrow-fmt-ignore-end` (Stage 19)
- Parallel multi-file driver (Stage 20); per-file bytes unchanged vs sequential
- Widened fmt-check corpus (Stage 21): `warnings/**`, `project/**`, stage19 fixture

### Out of scope

| Concern                         | Why                                                            |
| ------------------------------- | -------------------------------------------------------------- |
| Renaming (`PascalCase`, etc.)   | Naming is style/lint, not rewrite; core warnings / future lint |
| Idiom rewrites (`+`→`~`, etc.)  | Semantic / teachable; not silent format                        |
| Type-check / borrow fixes       | Compiler / `yarrow check`                                      |
| Tabs vs spaces as a config knob | Style guide fixes tabs; formatter always emits tabs            |
| Editor format-on-save wiring    | Editor / LSP client; server already exposes full-doc format    |

**Idempotence:** `format(format(src)) == format(src)` for accepted inputs (fully parsed subset only when best-effort).

---

## Architecture

```text
source (.yar)
  → comment tokens (yarrow-core; whitespace rebuilt in printer)
  → parse / parse_recovering (yarrow-core)
  → format IR (AST + TriviaMap)
  → printer (STYLE_GUIDE rules: construct → phrase wrap → indent → blanks → hygiene)
    or hygiene-only / selective reprint when parse incomplete (`format_source_best_effort`);
      ignore regions restored from hygiened original after layout (Stage 19)
  → UTF-8 string / write back

multi-file `run_fmt` (Stage 20): independent paths formatted in parallel (rayon);
  reporting / writes stay sorted-path order; stdin and single-file stay sequential
```

Public surface:

```rust
pub struct FormatOptions {
    pub max_width: usize,       // default 100; CLI rejects below MIN_MAX_WIDTH (20); library clamps
    pub sort_requires: bool,    // default true; --no-sort-requires to preserve order
    pub reorder_layout: bool,   // default false (opt-in; Stage 13)
}

pub const MIN_MAX_WIDTH: usize = 20;
pub const DEFAULT_MAX_WIDTH: usize = 100;

pub enum FormatError { /* Io | NotUtf8 | Parse */ }

pub struct FormattedSource { pub text: String, pub best_effort: bool }

pub fn format_source(source: &str, options: &FormatOptions) -> Result<String, FormatError>;
pub fn format_source_best_effort(source: &str, options: &FormatOptions) -> Result<FormattedSource, FormatError>;
pub fn format_file(path: &Path, options: &FormatOptions) -> Result<String, FormatError>;

pub struct FmtInput { /* options, check, stdin, best_effort, paths */ }
pub fn run_fmt(program: &str, input: FmtInput) -> ExitCode;

pub struct ByteRange { pub start: usize, pub end: usize }
pub struct FormatRangeEdit { pub range: ByteRange, pub new_text: String, pub expanded: bool }
pub fn format_range(source: &str, span: ByteRange, options: &FormatOptions) -> Result<FormatRangeEdit, FormatError>;
```

CLI (`yarrow-fmt` and `yarrow fmt`):

| Mode                 | Behavior                                         |
| -------------------- | ------------------------------------------------ |
| default              | Format files in place                            |
| `--check`            | Exit non-zero if any file would change           |
| `--stdin`            | Read stdin, write formatted stdout               |
| `--max-width N`      | Soft wrap width (default 100; min 20)            |
| `--sort-requires`    | Force-on require sorting (default already on)    |
| `--no-sort-requires` | Keep top-level require source order              |
| `--reorder-layout`   | Opt-in top-level file-layout reorder             |
| `--best-effort`      | On parse failure, hygiene (and Stage 18 reprint) |
| paths / dirs         | `.yar` files; recurse directories                |

Exit codes: `0` ok / already formatted (`--check`), `1` would reformat or parse/format failure, `2` usage / I/O.

---

## Landed (v1)

Stages 0–21 are complete. Historical stage write-ups were removed; git history keeps them.

| Piece                        | Notes                                                                          |
| ---------------------------- | ------------------------------------------------------------------------------ |
| Core comment tokens          | `TokenKind::Comment`; parser skips; printer rebuilds whitespace                |
| Format IR                    | `FormatIr` + `TriviaMap` (leading / trailing / file trailing)                  |
| Hygiene                      | LF, strip trailing WS, final newline; `NotUtf8` on bad files                   |
| Indent / blanks              | Tab nesting; aligned `end`; top-level blanks; collapse doubles                 |
| Construct + control layout   | Requires, types, functions, `if`/`match`/`for`/`defer`/`unsafe`/`handle`       |
| Phrase wrap                  | Soft wrap before consuming words; continuation +1 tab                          |
| Comments                     | Preserve text; `# ` / ` #` spacing                                             |
| Require sort (default on)    | `sort_requires` / `--no-sort-requires` (Stage 14)                              |
| Library + binary             | `format_source` / `format_file`; `yarrow-fmt` `--check` / `--stdin`            |
| Shared driver                | `run_fmt` / `FmtInput` for binary and CLI                                      |
| `yarrow fmt`                 | In-process wrapper ([`yarrow-cli` Stage 12](../yarrow-cli/PLAN.md))            |
| Corpus gate                  | `valid` / `warnings` / `project` + `lib/std` + stage19 fixture; CI `fmt-check` (Stages 16, 21) |
| LSP full-document format     | [`yarrow-lsp` Stage 8](../yarrow-lsp/PLAN.md) uses `format_source_best_effort` |
| File layout reorder (opt-in) | Stage 13: `reorder_layout` / `--reorder-layout`                                |
| Defaults polish              | Stage 14: sort on by default; `MIN_MAX_WIDTH`; no spaces-indent                |
| Range / span format API      | Stage 15: `format_range` / `FormatRangeEdit` (LSP Stage 17)                    |
| Stdlib + CI `--check`        | Stage 16: `scripts/fmt-check.sh` / `.github/workflows/fmt-check.yml`           |
| Best-effort incomplete parse | Stage 17: hygiene subset + `Parser::parse_recovering`                          |
| Selective construct reprint  | Stage 18: recovered top-level decls reprint when spans clear errors            |
| Ignore regions               | Stage 19: `# yarrow-fmt-ignore-begin` / `# yarrow-fmt-ignore-end`              |
| Parallel multi-file driver   | Stage 20: rayon over independent paths; sorted reporting                       |
| Widened fmt-check corpus     | Stage 21: `warnings/**`, `project/**`, stage19 fixture on the CI gate          |

**Gates:** `./scripts/fmt-check.sh` (or `yarrow fmt --check` on the Stage 21 path set) exits `0`; `cargo fmt && cargo check && cargo clippy` green for `yarrow_fmt` / `yarrow_cli`.

---

## Next

Focus: optional CLI range surface. Do not invent layout rules absent from the style guide. Naming stays out of silent format (core / future lint).

### Stage 21 - Widen fmt-check corpus ✅

**Done:** Bootstrap-formatted and gated `docs/examples/warnings/**`, `docs/examples/project/**`, and `crates/yarrow-fmt/fixtures/stage19_ignore_regions.yar` alongside `valid/**` + `lib/std`. Restored [`scripts/fmt-check.sh`](../../scripts/fmt-check.sh) / [`.github/workflows/fmt-check.yml`](../../.github/workflows/fmt-check.yml) with that set. Still excludes `docs/examples/invalid/**` and `fixtures/stage18_selective_reprint.yar` (intentional incomplete parse).

---

### Stage 22 - CLI range mode (optional)

Stage 15 kept range format library-only for LSP. Scripts may want the same without the language server.

1. Add a narrow CLI surface, e.g. `yarrow-fmt --range START:END` (byte offsets) or `--range-start` / `--range-end`, usable with `--stdin` or a single file.
2. Print the replacement text or a documented edit encoding; prefer matching `FormatRangeEdit` semantics (expansion included).
3. On parse failure: exit `1` with a clear message (same as full format); never half-write the file.
4. Wire through `yarrow fmt` the same flags. Document that editors should keep using LSP `rangeFormatting`.

**Gate:** formatting a span of a messy fixture via CLI yields the same `new_text` as `format_range` for that span; full-file default path unchanged. `cargo clippy` green.

**Skip if unused:** if no agent / script need appears after Stage 18–21, leave this in Later and mark Next accordingly.

---

## Mapping: style guide → stages

| Style guide section                    | Stages                                    |
| -------------------------------------- | ----------------------------------------- |
| Principles                             | Design only                               |
| Source files                           | Landed (3)                                |
| Indentation and line width             | Landed (4, 8, 14)                         |
| Blank lines                            | Landed (5)                                |
| Comments                               | Landed (1, 9); ignore markers Stage 19 ✅ |
| Naming                                 | Out of scope (core / lint)                |
| File layout (order)                    | Landed (13, opt-in)                       |
| Modules and `require`                  | Landed (6, 10, 14)                        |
| Visibility                             | Landed (print as written); Stage 13 order |
| Types / Functions / Variables          | Landed (6)                                |
| Stack phrases and operators            | Landed (8)                                |
| Literals and containers                | Landed (6)                                |
| Control flow / Defer / Unsafe / Errors | Landed (7)                                |
| Ownership / Stack hygiene              | Out of scope (semantics)                  |
| Checklist                              | Landed layout rows; naming rows ignored   |
| Tooling / ignore                       | Stage 19 ✅                               |

---

## Later (backlog)

| Item                    | Notes                                                                |
| ----------------------- | -------------------------------------------------------------------- |
| Naming lints            | Belong in core warnings or a future `yarrow lint`, not silent format |
| On-type format helpers  | LSP Stage 23: local indent after `end`+`\\n` (no fmt API yet) |
| Format config file      | Only if multi-flag defaults become painful; guide must define it     |
| Incremental / cached IR | After Stage 20 if parse dominates wall time (Stage 20 landed parallel files only) |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use fixtures + `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark ✅, short notes; do not re-expand history).
- Do not reimplement the language grammar in this crate; parse via `yarrow-core`.
- Do not type-check or run programs as part of format.
- In comments and documentation, never use `—` (em dash); use ASCII hyphen or rephrase.
- Format only through this crate’s API from CLI / LSP; never a second pretty-printer.
- If core needs trivia / recovery / API changes, land them in `yarrow-core` with a note here and in the core plan as needed.
