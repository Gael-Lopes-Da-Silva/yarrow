# Yarrow Formatter Implementation Plan

Library (and later binary) that rewrites `.yar` source to match [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md).

`yarrow-fmt` is a **tooling crate**. It may depend on `yarrow-core` for tokenize / parse / diagnostics. It does **not** type-check, borrow-check, or codegen. CLI wiring (`yarrow fmt`) lives in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md) Stage 12 once this plan has a usable library API.

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

### In scope (formatter)

Mechanical rewrite of valid (or parseable) source:

- Source file hygiene: UTF-8, LF, no trailing whitespace, final newline
- Indent: **one tab per nesting level**; no space indent
- Soft wrap target: **100 columns** (break before consuming words)
- Blank-line rules between top-level items and inside bodies
- Construct layout: `require`, types, `implement`, functions, `if`/`match`/`for`/`defer`/`unsafe`, calls, containers
- Comment text preserved; spacing around `#` normalized where the guide is explicit (`# ` after hash; one space before trailing `#`)

### Out of scope (v1)

| Concern                         | Why                                                              |
| ------------------------------- | ---------------------------------------------------------------- |
| Renaming (`PascalCase`, etc.)   | Naming is style/lint, not rewrite; core warnings may cover later |
| Idiom rewrites (`+`→`~`, etc.)  | Semantic / teachable; not silent format                          |
| Reordering functions / types    | File layout order is guidance; optional later flag               |
| Type-check / borrow fixes       | Compiler / `yarrow check`                                        |
| Partial format of invalid files | May best-effort later; v1 requires a successful parse            |

**Idempotence:** `format(format(src)) == format(src)` for accepted inputs.

---

## Architecture

```text
source (.yar)
  → trivia-aware tokens (core; see Stage 1)
  → parse (yarrow-core Parser / Session::parse_source)
  → format IR (CST-ish or AST + attached trivia)
  → printer (STYLE_GUIDE rules)
  → UTF-8 string / write back
```

Public surface (target):

```rust
pub struct FormatOptions {
    pub max_width: usize,      // default 100
    // later: require_sort: bool, etc.
}

pub struct FormatError { /* path, diagnostics from parse, or I/O */ }

pub fn format_source(source: &str, options: &FormatOptions) -> Result<String, FormatError>;
pub fn format_file(path: &Path, options: &FormatOptions) -> Result<String, FormatError>;
```

CLI / binary (this crate or thin `yarrow fmt` wrapper):

| Mode         | Behavior                                         |
| ------------ | ------------------------------------------------ |
| default      | Format files in place                            |
| `--check`    | Exit non-zero if any file would change           |
| `--stdin`    | Read stdin, write formatted stdout               |
| paths / dirs | `.yar` files; recurse directories when requested |

Exit codes (align with CLI): `0` ok / already formatted (`--check`), `1` parse/format failure, `2` usage / I/O.

---

## Current state

| Piece              | Status | Notes                                          |
| ------------------ | ------ | ---------------------------------------------- |
| `yarrow-fmt` crate | ⬜     | Empty `lib.rs`; no deps yet                    |
| Style guide        | ✅     | Authoritative layout doc                       |
| Core tokenize      | ⚠      | Comments are **skipped** (`#` not tokens)      |
| Core parse         | ✅     | Enough structure to reprint once trivia exists |

---

## Stages

### Stage 0 - Crate skeleton and API stubs

1. Depend on `yarrow_core` (path).
2. Add `FormatOptions`, `FormatError`, `format_source` stub that returns the input unchanged (or errors clearly if not implemented).
3. Optional: `src/main.rs` binary later; library-first is enough here.
4. Document crate role in this file’s Current state.

**Gate:** `cargo check -p yarrow_fmt` green. Public API compiles.

---

### Stage 1 - Comment / trivia tokens in `yarrow-core`

Today the tokenizer drops `# …` comments. A formatter cannot preserve them without trivia.

1. In `yarrow-core`, emit line-comment tokens (lexeme + span) and optionally newline / whitespace trivia **or** attach leading/trailing trivia to tokens (pick one model; prefer comment tokens + rebuild whitespace in the printer if simpler).
2. Parser ignores comment tokens the same way it ignores nothing today (skip in the token cursor).
3. JIT / check / examples unchanged.
4. Document the trivia contract briefly in [`docs/RUNTIME.md`](../../docs/RUNTIME.md) or a short note in [`docs/AST.md`](../../docs/AST.md) only if the AST gains comment nodes; prefer keeping comments off the semantic AST.

**Gate:** round-trip test at the token level: source with `#` comments yields tokens that still carry comment text; `Session::parse_source` / valid corpus still parses. `cargo clippy` green for core + fmt.

**Blocked:** Stages 5+ that claim comment preservation. Stages 2–4 may proceed on comment-free inputs.

---

### Stage 2 - Parse + format IR

1. `format_source`: tokenize (with trivia when Stage 1 landed) → parse → build a format IR.
2. Prefer **AST + trivia map** (span → comments) over a full CST unless CST becomes necessary.
3. On parse failure: return `FormatError` with core diagnostics (reuse render later in CLI). Do not invent fmt-only syntax errors.
4. No semantic analysis.

**Gate:** IR can represent `01_hello.yar` (requires, function, body, string call). Unit-style probe or `cargo run` example optional; corpus gate comes later.

---

### Stage 3 - Source file hygiene

Implement style-guide **Source files** + checklist basics:

1. Normalize to LF (`\n`).
2. Strip trailing whitespace on every line.
3. Ensure exactly one final newline.
4. Do not rewrite non-UTF-8 (error clearly).

**Gate:** dirty fixture with trailing spaces / missing final newline / CRLF → clean output matching those three rules. Idempotent.

---

### Stage 4 - Indent and `end` alignment

Style-guide **Indentation** + **Visible structure**:

1. One tab per nesting level for bodies of `do` / `if` / `else` / `match` / `case` / `for` / `defer` / `unsafe` / type / `implement`.
2. `end` at the same indent as the opener keyword’s line.
3. Reject or rewrite leading space-indent to tabs for indented lines (formatter output always tabs).

**Gate:** a nested `if` / `function` example formats with tab indent and aligned `end`. Matches the shape of style-guide control-flow snippets.

---

### Stage 5 - Blank lines

Style-guide **Blank lines**:

1. Single blank line between top-level items (requires block, types, implement, functions).
2. No more than one consecutive blank line anywhere.
3. No blank line immediately after `do` / `if` / `else` / `case` / `for` / `defer` / `unsafe` or immediately before matching `end` (unless a later exception for dense multi-branch match is needed; start strict).
4. Inside functions: do not insert a blank line after every statement; preserve or apply only coarse grouping if cheap (v1 may only normalize consecutive blanks and top-level separation).

**Gate:** multi-item file (struct + implement + function + `main`) gets single blank lines between items; double blanks collapse to one.

---

### Stage 6 - Requires, types, functions (construct layout)

Map these style-guide sections into printer rules:

1. **Modules / `require`:** one require per line; `"path" [alias] require`; group at file top when they appear there.
2. **Types:** one field / member / union arm per line; `end` placement.
3. **Functions:** each parameter type on its own line between `function` and `do`; `name function do` when no params; `end with Type` on the same line as `end`; omit `with` for void.
4. **Calls:** keep `name call` with the last argument on one line when under `max_width`.
5. **Variables:** `<value> <name> (mutable|const|static) <Type>` on one line.
6. **Containers:** spaces between elements; no commas.

**Gate:** format the style-guide function / struct / require snippets (as fixtures under e.g. `crates/yarrow-fmt/fixtures/` or `docs/examples`) so output matches the guide’s “prefer” shape for those constructs. Comment-free fixtures OK if Stage 1 incomplete.

---

### Stage 7 - Control flow, defer, unsafe, errors

Layout from style-guide **Control flow**, **Defer**, **Unsafe**, **Errors**:

1. `condition if` / `else` / `end` blocks.
2. `match` with indented `case` … `end`; blank line between multi-line cases when easy.
3. `for` bodies.
4. One-line `defer … end` when the body is a single short phrase; else block form.
5. `unsafe … end` kept tight.
6. `call unwrap` / `call handle … end` spacing.

**Gate:** fixtures derived from the guide’s `match` / `defer` / `unsafe` / `handle` examples format stably and idempotently.

---

### Stage 8 - Line width and stack phrases

Style-guide **Indentation and line width** + **Stack phrases**:

1. Default `max_width = 100`.
2. Space-separate tokens; never jam (`1 2 +`, not `1 2+`).
3. Prefer one primary effect per line; allow short pure arithmetic on one line.
4. When over width, break **before** a consuming word (`call`, operator, `if`, …), continuation indented one tab deeper.

**Gate:** a deliberately long call phrase wraps before `call` like the guide example. Width-100 content stays single-line when it fits.

---

### Stage 9 - Comments

Requires Stage 1.

1. Preserve comment text (no em-dash rewriting beyond leaving text alone; guide forbids writing `-` in new comments, not stripping existing Unicode).
2. Own-line comments: single space after `#`.
3. Trailing comments: one space before `#`.
4. Keep own-line comments above the code they document when attachment is unambiguous; if ambiguous, keep relative order to the following token.

**Gate:** `01_hello.yar`-style file with a file comment and a trailing comment round-trips comment text; spacing matches the guide. Idempotent.

---

### Stage 10 - Require sorting (optional flag)

Style-guide: std requires first, then local; alphabetical within groups.

1. Default **on** for `format` once stable, or default **off** with `--sort-requires` / `FormatOptions::sort_requires` (prefer **opt-in** first to avoid noisy diffs, then flip default if desired).
2. Only reorder top-level requires; do not move function-local requires to file top.

**Gate:** fixture with shuffled `"std.…"` / local requires sorts as documented when the option is enabled; disabled path preserves order.

---

### Stage 11 - Library finish + check mode + binary

1. Stabilize `format_source` / `format_file`.
2. `--check`: compare formatted vs input; exit `1` if different (or CLI-aligned code).
3. In-place write; stdin/stdout mode.
4. Recurse directories for `*.yar` when given a directory.

**Gate:** `cargo run -p yarrow_fmt -- --check docs/examples/valid/01_hello.yar` exits `0` after a bootstrap format (or documents that corpus is not yet fully styled). Formatting twice does not change bytes.

---

### Stage 12 - CLI `yarrow fmt` + corpus gate

1. Implement [`yarrow-cli` Stage 12](../yarrow-cli/PLAN.md) wrapper: `yarrow fmt -- …` delegates to this crate’s API (same process; do not shell out).
2. Run formatter over `docs/examples/valid/**` (and optionally `lib/std/**`): either commit formatted results or keep `--check` green in CI later.
3. Update style-guide one-liner if needed: “Tools and formatters should target this guide” remains true.

**Gate:** `yarrow fmt --check` on `docs/examples/valid/01_hello.yar` (and a small set listed in notes) exits `0`. `cargo fmt && cargo check && cargo clippy` green.

---

## Mapping: style guide → stages

| Style guide section                    | Stages                                 |
| -------------------------------------- | -------------------------------------- |
| Principles                             | Design only                            |
| Source files                           | 3                                      |
| Indentation and line width             | 4, 8                                   |
| Blank lines                            | 5                                      |
| Comments                               | 1, 9                                   |
| Naming                                 | Out of scope                           |
| File layout (order)                    | Later / opt                            |
| Modules and `require`                  | 6, 10                                  |
| Visibility                             | 6 (print as written; no insert/remove) |
| Types / Functions / Variables          | 6                                      |
| Stack phrases and operators            | 8                                      |
| Literals and containers                | 6                                      |
| Control flow / Defer / Unsafe / Errors | 7                                      |
| Ownership / Stack hygiene              | Out of scope (semantics)               |
| Checklist                              | Split across 3–9; naming rows ignored  |

---

## Later (backlog)

| Item                                | Notes                                              |
| ----------------------------------- | -------------------------------------------------- |
| Reorder file layout to guide order  | High churn; opt-in only                            |
| Format invalid / partial parse      | Needs error-tolerant parser recovery policy        |
| Naming lints                        | Belong in core warnings or a future `yarrow lint`  |
| `yarrow-lsp` format-on-save         | See `yarrow-lsp` Stage 8; same `format_source` API |
| Configurable width / tabs vs spaces | Style guide fixes tabs; width may stay option only |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use fixtures + `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark done, short notes; do not re-expand history).
- Do not reimplement the language grammar in this crate; parse via `yarrow-core`.
- Do not type-check or run programs as part of format.
- Never use `-` in comments or docs added by this work.
- If core needs trivia/API changes, land them in `yarrow-core` with a note here and in the core plan Known gaps / Next as needed.
