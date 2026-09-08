# Yarrow Formatter Implementation Plan

Library and binary that rewrite `.yar` source to match [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md).

`yarrow-fmt` is a **tooling crate**. It may depend on `yarrow-core` for tokenize / parse / diagnostics. It does **not** type-check, borrow-check, or codegen. CLI wiring (`yarrow fmt`) lives in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md). Editors format via [`yarrow-lsp`](../yarrow-lsp/PLAN.md) calling the same `format_source` API.

## Source of truth

| Role                        | Path                                                                            |
| --------------------------- | ------------------------------------------------------------------------------- |
| **Layout / idiomatic form** | [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md)                              |
| Language syntax             | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md), [`SYNTAX.md`](../../docs/SYNTAX.md) |
| Intended AST                | [`docs/AST.md`](../../docs/AST.md)                                              |
| Corpus (format gates)       | [`docs/examples/`](../../docs/examples/README.md), style-guide snippets         |
| Compiler API                | [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md)                          |
| Agent rules                 | [`AGENTS.md`](../../AGENTS.md)                                                  |

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

### Out of scope

| Concern                         | Why                                                              |
| ------------------------------- | ---------------------------------------------------------------- |
| Renaming (`PascalCase`, etc.)   | Naming is style/lint, not rewrite; core warnings / future lint   |
| Idiom rewrites (`+`→`~`, etc.)  | Semantic / teachable; not silent format                          |
| Type-check / borrow fixes       | Compiler / `yarrow check`                                        |
| Tabs vs spaces as a config knob | Style guide fixes tabs; formatter always emits tabs              |
| Editor format-on-save wiring    | Editor / LSP client; server already exposes full-doc format      |

**Idempotence:** `format(format(src)) == format(src)` for accepted inputs.

---

## Architecture

```text
source (.yar)
  → comment tokens (yarrow-core; whitespace rebuilt in printer)
  → parse (yarrow-core Parser / Session::parse_source)
  → format IR (AST + TriviaMap)
  → printer (STYLE_GUIDE rules: construct → phrase wrap → indent → blanks → hygiene)
  → UTF-8 string / write back
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

pub fn format_source(source: &str, options: &FormatOptions) -> Result<String, FormatError>;
pub fn format_file(path: &Path, options: &FormatOptions) -> Result<String, FormatError>;

pub struct FmtInput { /* options, check, stdin, paths */ }
pub fn run_fmt(program: &str, input: FmtInput) -> ExitCode;

pub struct ByteRange { pub start: usize, pub end: usize }
pub struct FormatRangeEdit { pub range: ByteRange, pub new_text: String, pub expanded: bool }
pub fn format_range(source: &str, span: ByteRange, options: &FormatOptions) -> Result<FormatRangeEdit, FormatError>;
```

CLI (`yarrow-fmt` and `yarrow fmt`):

| Mode                  | Behavior                                       |
| --------------------- | ---------------------------------------------- |
| default               | Format files in place                          |
| `--check`             | Exit non-zero if any file would change         |
| `--stdin`             | Read stdin, write formatted stdout             |
| `--max-width N`       | Soft wrap width (default 100; min 20)          |
| `--sort-requires`     | Force-on require sorting (default already on)  |
| `--no-sort-requires`  | Keep top-level require source order            |
| `--reorder-layout`    | Opt-in top-level file-layout reorder           |
| paths / dirs          | `.yar` files; recurse directories               |

Exit codes: `0` ok / already formatted (`--check`), `1` would reformat or parse/format failure, `2` usage / I/O.

---

## Landed (v1)

Stages 0–12 are complete. Historical stage write-ups were removed; git history keeps them.

| Piece                         | Notes                                                                 |
| ----------------------------- | --------------------------------------------------------------------- |
| Core comment tokens           | `TokenKind::Comment`; parser skips; printer rebuilds whitespace       |
| Format IR                     | `FormatIr` + `TriviaMap` (leading / trailing / file trailing)         |
| Hygiene                       | LF, strip trailing WS, final newline; `NotUtf8` on bad files          |
| Indent / blanks               | Tab nesting; aligned `end`; top-level blanks; collapse doubles        |
| Construct + control layout    | Requires, types, functions, `if`/`match`/`for`/`defer`/`unsafe`/`handle` |
| Phrase wrap                   | Soft wrap before consuming words; continuation +1 tab                 |
| Comments                      | Preserve text; `# ` / ` #` spacing                                    |
| Require sort (default on)     | `sort_requires` / `--no-sort-requires` (Stage 14)                 |
| Library + binary              | `format_source` / `format_file`; `yarrow-fmt` `--check` / `--stdin`   |
| Shared driver                 | `run_fmt` / `FmtInput` for binary and CLI                             |
| `yarrow fmt`                  | In-process wrapper ([`yarrow-cli` Stage 12](../yarrow-cli/PLAN.md))   |
| Corpus gate                   | `docs/examples/valid/**` bootstrapped; `--check` exits `0`            |
| LSP full-document format      | [`yarrow-lsp` Stage 8](../yarrow-lsp/PLAN.md) uses `format_source`    |
| File layout reorder (opt-in)  | Stage 13: `reorder_layout` / `--reorder-layout`                       |
| Defaults polish               | Stage 14: sort on by default; `MIN_MAX_WIDTH`; no spaces-indent       |
| Range / span format API       | Stage 15: `format_range` / `FormatRangeEdit` (LSP Stage 17)           |

**Gates:** `yarrow fmt --check docs/examples/valid` exits `0`; `cargo fmt && cargo check && cargo clippy` green for `yarrow_fmt` / `yarrow_cli`.

---

## Next

Focus: widen the corpus gate, then (if core allows) best-effort invalid input. Do not invent layout rules absent from the style guide.

### Stage 13 - File layout reorder (opt-in) ✅

Style-guide **File layout**. High churn; keep **opt-in** so default format stays diff-quiet.

Recommended module order:

1. File comment (optional)
2. Top-level `require` lines (std then local; sorting gated by `sort_requires`, default on)
3. Type declarations (`struct` / `enum` / `union` / `error`)
4. `Type implement` blocks (prefer immediately after the type they extend when both move together)
5. Private helpers
6. Public API functions
7. `main` last (entry files only)

Tasks:

1. Add `FormatOptions::reorder_layout` (default **false**) and CLI `--reorder-layout`.
2. Reorder only **top-level** items; never pull function-local requires or nested decls to file scope.
3. Move each item with its attached leading own-line comments; preserve relative order inside the same category when the guide does not distinguish further (stable sort).
4. Keep a single blank line between top-level items after the move (reuse blank-line pass).
5. Document that visibility (`public` / private helpers) is inferred from existing AST flags / keywords, not guessed from names.

**Gate:** fixture with shuffled types / implements / helpers / `main` reorders to the guide sequence when the option is enabled; disabled path preserves order. Idempotent either way. `cargo fmt && cargo check && cargo clippy` green.

**Notes:** `layout` module (`layout_kind`, `reorder_toplevel_indices`); construct layout attaches leading comments in source order then emits guide order; matching `implement` follows its type; orphan implements after types; `Other` before `main`. CLI `--reorder-layout` on `yarrow-fmt` and `yarrow fmt`. Fixture `fixtures/stage13_layout.yar`. Default remains off.

---

### Stage 14 - Defaults and option polish ✅

Stage 10 left require sorting opt-in. Width is already configurable; tabs are not.

1. Flip `FormatOptions::sort_requires` default to **true** once Stage 13 (or corpus) shows diffs are acceptable; add `--no-sort-requires` (and keep `--sort-requires` as an explicit no-op / force-on for scripts).
2. Leave `max_width` default at 100; document that values below a small floor (e.g. 20) are clamped or rejected with a clear usage error.
3. Do **not** add a spaces-indent option. Reject or ignore any future indent-style config; printer always emits tabs.
4. Update CLI help, style-guide tooling blurb if defaults change, and this plan’s Architecture snippet.
5. Re-run `yarrow fmt` over `docs/examples/valid` (and Stage 15 corpus if already landed) so `--check` stays green under the new defaults.

**Gate:** default `format_source` sorts requires without a flag; `--no-sort-requires` preserves require order; `--max-width` still soft-wraps. Idempotent. `cargo fmt && cargo check && cargo clippy` green.

**Notes:** `sort_requires` default **true**; CLI `--sort-requires` / `--no-sort-requires`; `MIN_MAX_WIDTH` (20) rejected by `run_fmt`, clamped via `FormatOptions::effective_max_width` in the library; `DEFAULT_MAX_WIDTH` (100). No spaces-indent option. Style guide tooling blurb updated. Require-run printer keeps comments above the first require as a block header when there is no separating blank (so default sort does not bury file comments). Corpus re-bootstrapped under new defaults.

---

### Stage 15 - Range / span format API ✅

Unblocks [`yarrow-lsp` Stage 17](../yarrow-lsp/PLAN.md) (`rangeFormatting` / optional on-type). Full-document format stays the source of truth; do not ship a second pretty-printer.

1. Add a library entry point, e.g. `format_range(source, span, options) -> Result<FormatRangeEdit, FormatError>` (exact names flexible), that either:
   - formats the whole file via `format_source` and returns the rewritten slice / text edits intersecting `span`, **or**
   - expands `span` to enclosing top-level item boundaries when a naive intersect would break indent / blanks, and documents that expansion.
2. Return enough data for LSP `TextEdit`s (byte or line/column ranges in the original buffer). Prefer one contiguous replacement when simpler and still correct.
3. On parse failure: return `FormatError::Parse` (LSP maps to empty edits); never partially corrupt the buffer.
4. Keep the API usable without writing files; binary / `yarrow fmt` need not expose range mode in this stage.

**Gate:** fixture with a messy contiguous region; range format yields edits confined to (or documented expansion of) that region and matches full-doc format for the rewritten slice. Second call on the result is a no-op. `cargo fmt && cargo check && cargo clippy` green.

**Notes:** `ByteRange` + `FormatRangeEdit` (`range`, `new_text`, `expanded`, `apply` / `is_noop`); `format_range` runs full `format_source`, expands to enclosing top-level items (leading comments after the last blank; fills holes for one contiguous cover); require runs expand together when `sort_requires`; `reorder_layout` or hygiene-shifted buffers fall back to whole-file replace. Replacement text is the matching item cover in the formatted buffer (identity by require path / type / implement target / function name). No CLI range mode. Fixture `fixtures/stage15_range.yar`; example `examples/stage15_gate.rs`.

---

### Stage 16 - Stdlib corpus + CI `--check`

Stage 12 gated `docs/examples/valid`. Widen the always-green surface and make CI enforce it.

1. Bootstrap-format `crates/yarrow-core/lib/std/**/*.yar` (commit results).
2. Document the gate set in notes: at least `docs/examples/valid` and `lib/std`.
3. Add a CI step (or script invoked by CI) that runs `yarrow fmt --check` on that set and fails the job on exit `1`.
4. Do not silently format `docs/examples/invalid/**` (parse failures are expected).

**Gate:** `yarrow fmt --check docs/examples/valid crates/yarrow-core/lib/std` exits `0`. CI job fails if a `.yar` in that set drifts. `cargo fmt && cargo check && cargo clippy` green.

---

### Stage 17 - Best-effort format on partial parse

Today v1 requires a successful parse. Editors often want hygiene / indent on broken buffers.

1. Coordinate with `yarrow-core`: needs an error-tolerant / recovery parse policy that still yields a partial AST (or token stream with statement boundaries). Do not invent a second parser in this crate. If core has no recovery API yet, mark this stage **blocked** and stop after a short design note.
2. Define a safe subset when parse is incomplete: source hygiene always; indent / blanks only where structure is unambiguous; skip construct reprint for broken regions (leave original text).
3. Extend `FormatError` or return a structured “partial success” only if callers can distinguish full vs best-effort (LSP must not replace the buffer with a worse partial). Prefer: succeed with a flag, or fail closed like today until recovery is trustworthy.
4. Idempotence applies only to the fully-parsed subset; document limits.

**Gate:** one deliberately broken fixture gets LF / trailing-WS / final-newline cleanup without deleting the broken region; a fully valid file still fully formats. If core recovery is unavailable, Done notes say blocked and this stage stays open. `cargo fmt && cargo check && cargo clippy` green for whatever landed.

---

## Mapping: style guide → stages

| Style guide section                    | Stages                                      |
| -------------------------------------- | ------------------------------------------- |
| Principles                             | Design only                                 |
| Source files                           | Landed (3)                                  |
| Indentation and line width             | Landed (4, 8); width floor / defaults Stage 14 ✅ |
| Blank lines                            | Landed (5)                                  |
| Comments                               | Landed (1, 9)                               |
| Naming                                 | Out of scope (core / lint)                  |
| File layout (order)                    | Stage 13 ✅ (opt-in)                        |
| Modules and `require`                  | Landed (6, 10); default sort Stage 14 ✅    |
| Visibility                             | Landed (print as written); Stage 13 order   |
| Types / Functions / Variables          | Landed (6)                                  |
| Stack phrases and operators            | Landed (8)                                  |
| Literals and containers                | Landed (6)                                  |
| Control flow / Defer / Unsafe / Errors | Landed (7)                                  |
| Ownership / Stack hygiene              | Out of scope (semantics)                    |
| Checklist                              | Landed layout rows; naming rows ignored     |

---

## Later (backlog)

| Item                         | Notes                                                                 |
| ---------------------------- | --------------------------------------------------------------------- |
| Naming lints                 | Belong in core warnings or a future `yarrow lint`, not silent format  |
| Diff-friendly ignore regions | Only if real need (`yarrow-fmt-ignore` style); not in the guide today |
| Parallel / incremental fmt   | Premature until corpus + CI pain shows up                             |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use fixtures + `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark ✅, short notes; do not re-expand history).
- Do not reimplement the language grammar in this crate; parse via `yarrow-core`.
- Do not type-check or run programs as part of format.
- Never use `-` in comments or docs added by this work.
- Format only through this crate’s API from CLI / LSP; never a second pretty-printer.
- If core needs trivia / recovery / API changes, land them in `yarrow-core` with a note here and in the core plan as needed.
