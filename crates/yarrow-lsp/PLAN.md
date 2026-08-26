# Yarrow LSP Implementation Plan

Language server for `.yar` editors. Talks to **`yarrow-core`** for tokenize / parse / check / diagnostics, and optionally **`yarrow-fmt`** for document formatting. Owns LSP protocol, document sync, and editor UX. Does **not** reimplement the compiler.

CLI wiring (`yarrow lsp`) lives in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md) Stage 12 once this crate has a runnable stdio server.

## Source of truth

| Role              | Path                                                                              |
| ----------------- | --------------------------------------------------------------------------------- |
| Language syntax   | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md), [`SYNTAX.md`](../../docs/SYNTAX.md)   |
| AST / types       | [`docs/AST.md`](../../docs/AST.md), [`TYPE_SYSTEM.md`](../../docs/TYPE_SYSTEM.md) |
| Modules / runtime | [`docs/RUNTIME.md`](../../docs/RUNTIME.md)                                        |
| Style (format)    | [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md) via `yarrow-fmt`               |
| Compiler API      | [`crates/yarrow-core/PLAN.md`](../yarrow-core/PLAN.md)                            |
| Formatter API     | [`crates/yarrow-fmt/PLAN.md`](../yarrow-fmt/PLAN.md)                              |
| Corpus (smoke)    | [`docs/examples/`](../../docs/examples/README.md)                                 |
| Agent rules       | [`AGENTS.md`](../../AGENTS.md)                                                    |

Prefer core diagnostics and spans over inventing LSP-only error messages. When protocol and language disagree, follow the language docs and map carefully into LSP.

---

## Scope

### In scope (v1 through Stage 11)

- stdio Language Server Protocol (LSP 3.17-shaped; no need for every optional method)
- Text document sync for `file://` `.yar` buffers
- Publish diagnostics from `Session::check_source` (and parse failures)
- Navigation: go-to-definition, find references (same file first; `require` cross-file next)
- Hover and document symbols from AST (typed hover when core exposes enough)
- Completions: keywords + in-scope / imported names (best-effort)
- Document formatting when `yarrow-fmt` is ready
- `textDocument/codeAction` / hover help that surfaces `explain_code` for diagnostic codes

### Out of scope (v1)

| Concern                        | Why                                                              |
| ------------------------------ | ---------------------------------------------------------------- |
| Full project / workspace index | Core is single-file + `require`; no multi-root project graph yet |
| Incremental / salsa analysis   | Premature; re-check open docs on change is enough at first       |
| Debug Adapter Protocol         | Separate product; AOT/JIT debug story is Phase F                 |
| Semantic rename across crates  | Needs stable name resolution API; start file-local only          |
| Snippet / AI rewrite actions   | Not mechanical language support                                  |
| Non-`.yar` / markdown embedded | Skip until requested                                             |

**Transport:** stdio only for v1. TCP / socket later if useful for testing.

---

## Architecture

```text
editor  ←stdio JSON-RPC→  yarrow-lsp
                            ├── DocumentStore (uri → text + version)
                            ├── Analysis (Session::parse_source / check_source)
                            ├── PositionMap (LSP ↔ core Span / SourceFile)
                            ├── Features (diag, hover, def, refs, symbols, complete, format)
                            ├── yarrow_core::Session
                            └── yarrow_fmt::format_source   (when Stage 8+)
```

Public / binary surface (target):

```rust
/// Library entry used by the binary and by `yarrow lsp`.
pub async fn run_stdio() -> Result<(), LspError>;

/// Optional: run with explicit stdin/stdout for tests / embedding.
pub async fn run_with_transport(/* … */) -> Result<(), LspError>;
```

Protocol stack (pick one; prefer maintained crates):

| Piece     | Choice (default)                                       |
| --------- | ------------------------------------------------------ |
| LSP types | via `tower-lsp-server` (community fork of `tower-lsp`) |
| Runtime   | `tokio`                                                |
| Binary    | `crates/yarrow-lsp` `[[bin]]` or `src/main.rs`         |

Do **not** shell out to `yarrow check`; call `Session` in-process.

### Position mapping

LSP positions are UTF-16 code units by default (or UTF-8 if negotiated). Core `Span` / `SourceFile::location` use **byte offsets** and Unicode scalar columns.

1. Own a small `PositionMap` in this crate (or a tiny helper in core if reused by fmt).
2. Convert `Span` → `lsp::Range` and reverse for requests.
3. Prefer negotiating `positionEncoding = utf-8` when the client supports it; still support UTF-16 for VS Code-class clients.

### Analysis model (v1)

On `didOpen` / `didChange` (debounced):

1. Update buffer text.
2. Build `CompileOptions` with `source_path` from URI, search paths from init options / workspace folders.
3. Call `check_source` (or parse-only on failure path).
4. Map `DiagnosticBatch` → `PublishDiagnosticsParams`.
5. Cache last successful `Program` (and later typed info) for hover / navigation.

No background whole-workspace crawl in v1. Open documents + transitive `require` resolution already performed by core during check are enough.

---

## Current state

| Piece              | Status | Notes                                             |
| ------------------ | ------ | ------------------------------------------------- |
| `yarrow-lsp` crate | ⬜     | Empty `lib.rs`; no deps                           |
| Core Session API   | ✅     | `parse_source` / `check_source` + spans           |
| Core diagnostics   | ✅     | `Diagnostic` / `Severity` / codes / explain table |
| Typed hover data   | ⚠      | `CheckedProgram` is AST-only today                |
| Cross-file resolve | ⚠      | Works inside compile via `require`; no index API  |
| `yarrow-fmt`       | ⬜     | Format feature blocked on fmt Stage 11            |
| CLI `yarrow lsp`   | ⬜     | Thin wrapper after Stage 10                       |

---

## Stages

### Stage 0 - Crate skeleton and stdio hello

1. Depend on `yarrow_core`, `tower-lsp-server`, `tokio`, `serde` / `serde_json` as needed.
2. Binary that speaks LSP: `initialize` / `initialized` / `shutdown` / `exit`.
3. Advertise minimal capabilities (empty or sync-only).
4. `run_stdio` public API; `cargo run -p yarrow_lsp` starts the server.

**Gate:** `cargo check -p yarrow_lsp` green. A client (or scripted JSON-RPC) completes initialize handshake and shuts down cleanly.

---

### Stage 1 - Document sync

1. `textDocument/didOpen`, `didChange` (full or incremental; full is fine first), `didClose`.
2. In-memory `DocumentStore`: URI → `{ version, text }`.
3. Language id `yarrow`; file association `.yar`.
4. Ignore non-`.yar` unless opened with that language id.

**Gate:** open a buffer, apply a change, close it; store reflects text. No diagnostics required yet.

---

### Stage 2 - Position map + diagnostic publish

1. Implement LSP ↔ `Span` conversion against `SourceFile`.
2. On open/change: `Session::check_source` (or parse on earlier failure).
3. Map primary (+ secondary labels if cheap) to LSP diagnostics; include `code` and severity.
4. Clear diagnostics on close / when check succeeds with empty batch.
5. Debounce rapid `didChange` (e.g. 150–300 ms) so typing stays responsive.

**Gate:** opening `docs/examples/invalid/**` (or a known-bad snippet) publishes at least one diagnostic with a sensible range. Valid `01_hello.yar` publishes empty diagnostics after check.

---

### Stage 3 - Document symbols and folding-friendly outline

1. Walk top-level `Program` items: functions, types, `implement`, etc.
2. `textDocument/documentSymbol` (hierarchical if easy; flat SymbolInformation OK first).
3. Use item `span` from the AST; name from declaration identifiers.

**Gate:** outline for a multi-item valid example lists `main` and at least one type or helper function with non-empty ranges.

---

### Stage 4 - Go to definition (same file)

1. Resolve identifier / qualified name at position via AST walk + token fallback.
2. Jump to local declaration span (params, locals, top-level, type members when spans exist).
3. If unresolved, return empty (no fake locations).

**Gate:** in a file with `foo function` and a `foo call`, definition on the call name lands on `foo`. Missing name returns empty.

---

### Stage 5 - Hover (AST signatures)

1. Hover on declarations and references shows a short markdown string: kind + name + parameter / return shape from the AST when available.
2. On a diagnostic span, optionally append explain blurb via `explain_code` when the code is known.
3. Typed / ownership detail is **out of scope** until core exposes it (see Stage 9 / core backlog).

**Gate:** hover on `main` in `01_hello.yar` shows a non-empty signature-ish string. Hover on empty space returns none.

---

### Stage 6 - Completions (keywords + names)

1. Keyword list from grammar (`function`, `do`, `end`, `if`, `match`, `require`, …).
2. Completions from current file top-level names and, when parse succeeded, locals in the innermost span containing the cursor (best-effort).
3. After `"…"` / require context, suggest known `std.*` module paths if cheap (embedded list or static table); do not invent modules absent from `lib/std`.
4. No AI / fuzzy ranking beyond simple prefix filter.

**Gate:** in an empty-ish body, completing `fun` offers `function`. After defining `helper`, completing `hel` can offer `helper`.

---

### Stage 7 - References and cross-file definition via `require`

1. Find references in the current document (all name occurrences bound to the same decl when cheap; textual fallback only if binding data is missing, and document the limitation).
2. For `require`d modules: when core check already loaded them, use resolved module path + name to support go-to-definition into dependency files (open as `file://` path from search path / std embed materialization policy).
3. If std modules are embedded and have no on-disk path, either skip jump or expose a read-only virtual URI scheme; pick one and document it (prefer real `lib/std/**` paths from the repo / install layout when available).

**Gate:** definition on an imported `std` / local require name opens or returns a location for that module entity when a file path exists. Same-file find-all-references returns ≥1 location for a used function.

---

### Stage 8 - Formatting (depends on `yarrow-fmt`)

Blocked on [`yarrow-fmt` Stage 11+](../yarrow-fmt/PLAN.md).

1. `textDocument/formatting` (and optional range formatting later).
2. Call `yarrow_fmt::format_source` with default options.
3. Return a full-document `TextEdit` (or minimal diff if easy).
4. On format / parse failure: show diagnostic or return error; do not partially corrupt the buffer.

**Gate:** format request on a deliberately messy but parseable buffer returns edits that match `format_source`. Idempotent format yields empty edits.

---

### Stage 9 - Typed hover / richer analysis (core-assisted)

Blocked on a small core API if AST-only hover is insufficient.

1. Prefer a Session or check artifact that can answer `type_at(span)` / `signature_at` without full JIT.
2. If core Stage 24 (check without codegen) helps latency, use it.
3. Hover shows type / stack effect notes when available; fall back to Stage 5 AST hover.

**Gate:** documented probe where hover on a typed binding shows the type string from core. If core API is not ready, keep this stage ⬜ and do not fake types in the LSP.

---

### Stage 10 - Binary polish + `yarrow lsp` wrapper

1. Stable CLI flags if any (`--stdio` default; maybe log level to stderr).
2. Init options: search paths (`-L` equivalent), entry name default, format enable.
3. Wire [`yarrow-cli` Stage 12](../yarrow-cli/PLAN.md): `yarrow lsp` delegates in-process to `yarrow_lsp::run_stdio`.
4. Short editor setup note (command path) in this file’s notes or `docs/` only if the user asks for docs; otherwise README blurb in crate is enough.

**Gate:** `yarrow lsp` (or `cargo run -p yarrow_lsp`) initializes against a real editor or scripted client. `cargo fmt && cargo check && cargo clippy` green.

---

### Stage 11 - Code actions and explain

1. Code action or hover link: “Explain E3xx” using `format_explain` / `explain_code`.
2. Optional: “Open style guide” is out of scope; keep actions diagnostic-centric.
3. No auto-fix that changes semantics unless tied to a known safe rewrite (prefer none in v1).

**Gate:** a published diagnostic with a known code offers an action or hover section that includes the explain text.

---

## Mapping: LSP features → stages

| LSP capability                         | Stages       | Core / fmt dependency        |
| -------------------------------------- | ------------ | ---------------------------- |
| initialize / shutdown                  | 0            | -                            |
| textDocument sync                      | 1            | -                            |
| publishDiagnostics                     | 2            | `check_source`, spans        |
| documentSymbol                         | 3            | AST spans                    |
| definition                             | 4, 7         | AST + require resolution     |
| hover                                  | 5, 9         | AST; later typed API         |
| completion                             | 6            | grammar keywords + AST names |
| references                             | 7            | binding / name index         |
| formatting                             | 8            | `yarrow-fmt`                 |
| codeAction / explain                   | 11           | `explain_code`               |
| rename / workspaceSymbol / semanticTok | Later        | richer index                 |
| DAP / debug                            | Out of scope | AOT/JIT debug                |

---

## Later (backlog)

| Item                              | Notes                                           |
| --------------------------------- | ----------------------------------------------- |
| Semantic tokens                   | Needs token classification API; nice for themes |
| Rename (file then project)        | After stable resolve; never silent cross-module |
| Workspace symbols                 | Needs project index beyond open buffers         |
| Inlay hints (types / stack)       | After typed analysis API                        |
| Signature help                    | Call-site param hints from AST / types          |
| Range format / on-type format     | After full-doc format is solid                  |
| TCP transport / test harness      | Scripted protocol tests                         |
| VS Code / Zed extension packaging | Productizing; server stays editor-agnostic      |
| Pull diagnostics (LSP 3.17)       | Optional once push diagnostics are stable       |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use scripted LSP messages + `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark done, short notes; do not re-expand history).
- No tokenizer / parser / typechecker logic here beyond calling `yarrow-core`.
- Format only through `yarrow-fmt`, never a second pretty-printer.
- Never use `-` in comments or docs added by this work (ASCII hyphen only; no em dash).
- If core needs position helpers, typed hover, or require-path APIs, land them in `yarrow-core` and note the dependency here and in the core plan Known gaps / Next.
- Keep the safe vs unsafe boundary visible in hovers when relevant; do not imply `unsafe` turns off checking.
