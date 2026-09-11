# Yarrow LSP Implementation Plan

Language server for `.yar` editors. Talks to **`yarrow-core`** for tokenize / parse / check / diagnostics, and **`yarrow-fmt`** for document formatting. Owns LSP protocol, document sync, and editor UX. Does **not** reimplement the compiler.

CLI wiring: `yarrow lsp` delegates in-process via [`yarrow-cli` Stage 12](../yarrow-cli/PLAN.md).

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

### Landed (v1, Stages 0–19)

- stdio Language Server Protocol (LSP 3.17-shaped)
- TCP `--listen host:port` transport (one client) + in-repo protocol harness
- Text document sync for `file://` `.yar` buffers
- Publish diagnostics from `Session::check_source` (and parse failures)
- Pull diagnostics (`textDocument/diagnostic`) with uri+version cache shared with push
- Navigation: go-to-definition, find references (same file + `require` cross-file)
- Hover (AST + typed via `CheckedProgram::type_at`) and document symbols
- Completions: keywords + in-scope / imported names + `std.*` require paths
- Document formatting via `yarrow-fmt` (full document + range; on-type deferred)
- Code actions / hover that surface `explain_code` for diagnostic codes
- `LspConfig`, init options, `yarrow lsp` CLI wrapper
- Signature help at postfix `name call` sites
- Inlay hints from core type probes (`--no-inlay` / init `inlayHints`)
- Semantic tokens (full document) from tokenizer + AST decls
- File-local rename (`prepareRename` + `rename`; refuse unsafe cross-module edits)
- Workspace symbols (`workspace/symbol` over open buffers + resolved requires)

### In scope (next, Stages 20+)

- Thin VS Code / Zed extension packaging (server stays editor-agnostic)

### Out of scope

| Concern                        | Why                                                              |
| ------------------------------ | ---------------------------------------------------------------- |
| Full project / workspace index | Core is single-file + `require`; no multi-root project graph yet |
| Incremental / salsa analysis   | Premature; re-check open docs on change is enough for now        |
| Debug Adapter Protocol         | Separate product; AOT/JIT debug story is Phase F                 |
| Silent semantic rename across crates | Needs stable name resolution API; never guess               |
| Snippet / AI rewrite actions   | Not mechanical language support                                  |
| Non-`.yar` / markdown embedded | Skip until requested                                             |

**Transport:** stdio is the default. TCP (`--listen`) is for tests / remote clients (Stage 19).

---

## Architecture

```text
editor  ←stdio JSON-RPC→  yarrow-lsp
                            ├── DocumentStore (uri → text + version)
                            ├── Analysis (Session::parse_source / check_source)
                            ├── PositionMap (LSP ↔ core Span / SourceFile)
                            ├── Features (diag, hover, def, refs, symbols, complete,
                            │             format, codeAction, signature, inlay, …)
                            ├── yarrow_core::Session
                            └── yarrow_fmt::format_source
```

Public / binary surface:

```rust
pub async fn run_stdio() -> Result<(), LspError>;
pub async fn run_stdio_with(config: LspConfig) -> Result<(), LspError>;
pub fn run_stdio_blocking() -> Result<(), LspError>;
pub async fn run_tcp_with(addr, config, on_listen) -> Result<(), LspError>;
pub fn run_tcp_blocking(addr, config, on_listen) -> Result<(), LspError>;
/// Shared by stdio and TCP.
pub async fn run_with_streams(/* … */) -> Result<(), LspError>;
```

Protocol stack:

| Piece     | Choice                                                     |
| --------- | ---------------------------------------------------------- |
| LSP types | via `tower-lsp-server` (community fork of `tower-lsp`)     |
| Runtime   | `tokio`                                                    |
| Binary    | `crates/yarrow-lsp` + `yarrow lsp`                         |

Do **not** shell out to `yarrow check`; call `Session` in-process.

### Position mapping

LSP positions are UTF-16 code units by default (or UTF-8 if negotiated). Core `Span` / `SourceFile::location` use **byte offsets** and Unicode scalar columns.

1. `PositionMap` converts `Span` ↔ `lsp::Range`.
2. Prefer negotiating `positionEncoding = utf-8` when the client supports it; still support UTF-16 for VS Code-class clients.

### Analysis model

On `didOpen` / `didChange` (debounced):

1. Update buffer text.
2. Build `CompileOptions` with `source_path` from URI, search paths from init options / workspace folders.
3. Call `check_source` (or parse-only on failure path).
4. Map `DiagnosticBatch` → `PublishDiagnosticsParams` (and answer pull requests in Stage 18).
5. Cache last successful parse / check artifact for hover, navigation, inlays, tokens.

No background whole-workspace crawl. Open documents + transitive `require` resolution during check are enough until a real project index exists.

---

## Current state

| Piece              | Status | Notes                                              |
| ------------------ | ------ | -------------------------------------------------- |
| `yarrow-lsp` crate | ✅     | Stage 19: TCP + protocol harness                   |
| Core Session API   | ✅     | `parse_source` / `check_source` + spans            |
| Core diagnostics   | ✅     | `Diagnostic` / `Severity` / codes / explain table  |
| Typed hover data   | ✅     | `CheckedProgram::type_at` (core Stage 30)          |
| Cross-file resolve | ⚠      | Works via `require` paths; no project index API    |
| `yarrow-fmt`       | ✅     | Full-doc `format_source_best_effort`; `format_range` (fmt Stage 15/17) |
| CLI `yarrow lsp`   | ✅     | In-process `run_stdio_blocking`                    |

---

## Landed (Stages 0–17)

Stages 0–17 are complete. Historical stage write-ups for 0–16 were removed; git history keeps them. Stage 17 remains below for context until the next plan collapse.

| Stage | Capability |
| ----- | ---------- |
| 0 | Crate + stdio hello (`initialize` / `shutdown`) |
| 1 | Document sync (`DocumentStore`, full sync) |
| 2 | Position map + publish diagnostics (debounce) |
| 3 | Hierarchical `documentSymbol` |
| 4 | Same-file `definition` |
| 5 | AST hover + explain blurb on diagnostic spans |
| 6 | Completions (keywords, scoped names, `std.*` require) |
| 7 | Same-file `references` + cross-file def via `require` |
| 8 | Full-document `formatting` via `yarrow-fmt` |
| 9 | Typed hover via `CheckedProgram::type_at` |
| 10 | `LspConfig` / init options + `yarrow lsp` wrapper |
| 11 | `codeAction` Explain Exxx + `yarrow.explain` command |
| 12 | `signatureHelp` for postfix `name call` |
| 13 | Inlay hints from `TypeIndex` probes |
| 14 | Semantic tokens (full document) from tokenizer + AST decls |
| 15 | File-local rename (`prepareRename` + `rename`) |
| 16 | Workspace symbols (`workspace/symbol` over open + requires) |
| 17 | Range formatting via `format_range` (on-type deferred) |

---

## Stages

### Stage 17 - Range format and on-type format ✅

Narrow formatting after full-document format is solid (Stage 8).

1. `textDocument/rangeFormatting`: format the selected range. Prefer formatting the whole file via `format_source` and intersecting edits with the range, **or** a fmt API that accepts a span if one is added; do not ship a second pretty-printer.
2. If intersecting full-doc edits is lossy for indentation context, expand the range to enclosing top-level item boundaries and document that behavior.
3. Optional `textDocument/onTypeFormatting` for trigger characters that the style guide makes mechanical (e.g. `\n` after `end` alignment only if fmt can express it safely). Skip triggers that would fight the user mid-token.
4. On parse / format failure: return null / empty edits; never partially corrupt the buffer (same as Stage 8).
5. Honor existing `format` enable flag from `LspConfig`; when format is disabled, omit these capabilities too.

**Gate:** range format on a messy contiguous region in a parseable buffer yields edits confined to (or documented expansion of) that region and matching `format_source` for the rewritten slice. Idempotent second request yields empty. On-type either lands with one safe trigger or is explicitly deferred in the Done notes with reason.

**Done:** `textDocument/rangeFormatting` via `yarrow_fmt::format_range` (expands to enclosing top-level item cover; same style as full-doc). Parse failure → null. Idempotent second request → empty edits. `--no-format` / init `format: false` omits range formatting too. On-type deferred: mid-edit buffers often fail to parse, and top-level expansion is too aggressive for a keystroke.

---

### Stage 18 - Pull diagnostics (LSP 3.17) ✅

Support clients that prefer pull over (or in addition to) push.

1. Advertise `diagnosticProvider` (identifier e.g. `yarrow`) with inter-file support off unless cheap.
2. Implement `textDocument/diagnostic` using the same `check_document` path as publish; return `FullDocumentDiagnosticReport` (or unchanged related if version matches).
3. Keep existing push on open/change for editors that still expect it; avoid double-flicker when the client uses both (prefer answering pull from cache keyed by uri + version).
4. Workspace pull (`workspace/diagnostic`) is optional: only open documents, or skip and document.
5. Preserve diagnostic `code`, severity, and related information already mapped in Stage 2.

**Gate:** scripted client requests `textDocument/diagnostic` on `docs/examples/invalid/01_use_after_move.yar` and receives at least one diagnostic with code `E373` (or the file’s known code) without relying on a prior `publishDiagnostics` wait. Push path still works for open.

**Done:** `diagnosticProvider` identifier `yarrow`; `interFileDependencies` / `workspaceDiagnostics` false (workspace pull skipped). `textDocument/diagnostic` uses `check_document` and a uri+version cache shared with push (`result_id` = `v{version}`; matching `previousResultId` → unchanged). Push on open/change unchanged. Scripted gate: open fixture → pull → `E373` without waiting on publish.

---

### Stage 19 - TCP transport and protocol test harness ✅

Make automated LSP gates reliable without ad-hoc one-off scripts each stage.

1. Add a `--listen host:port` (or `--tcp`) mode alongside default `--stdio`; same `LspConfig` flags otherwise.
2. Factor JSON-RPC framing so stdio and TCP share one server backend.
3. Provide a small in-repo harness (Node, Rust, or shell + `nc`) under `crates/yarrow-lsp/` or `tools/` that: starts the server, runs initialize → open fixture → assert one capability → shutdown.
4. Migrate at least one existing gate (diagnostics or explain code action) onto the harness so future stages reuse it.
5. Document how to run the harness in the crate README (short). Do not require a real editor for CI-style checks.

**Gate:** `yarrow lsp --listen 127.0.0.1:0` (or documented flag) accepts one harness run that passes initialize + one feature assert. Stdio path unchanged. README blurb exists. `cargo clippy` green.

**Done:** `--listen HOST:PORT` on `yarrow-lsp` and `yarrow lsp` (port `0` = ephemeral; prints `yarrow-lsp: listening on …` then accepts one client). `run_tcp_with` / `run_tcp_blocking` share `run_with_streams` with stdio. Harness: `crates/yarrow-lsp/scripts/harness.mjs` (default scenario `pull-diagnostics` → `E373` on `01_use_after_move.yar`). Stdio unchanged.

---

### Stage 20 - Editor extension packaging (VS Code / Zed)

Thin client extensions that launch `yarrow lsp` / `yarrow-lsp`; server remains editor-agnostic.

1. VS Code: minimal extension (`activationEvents` on `.yar`, language id `yarrow`) that starts the server via `yarrow lsp` on `PATH` or a config `yarrow.lsp.path`.
2. Contribute language configuration (comments `#`, brackets) only; syntax highlighting may stay TextMate-basic or defer to semantic tokens (Stage 14).
3. Zed: equivalent language + LSP entry if packaging cost is low; otherwise VS Code first and note Zed as follow-up in Done.
4. Ship extension sources under something like `editors/vscode/` (or `crates/yarrow-lsp/editors/`); do not embed the Rust server inside the extension binary.
5. Document install / “set command path” in the extension README; keep `crates/yarrow-lsp/README.md` as the server source of truth.

**Gate:** documented steps open a `.yar` file in the packaged editor and see diagnostics from the language server (screenshot or scripted smoke optional). Extension does not vendor a second formatter or checker.

---

## Mapping: LSP features → stages

| LSP capability                         | Stages   | Core / fmt dependency              |
| -------------------------------------- | -------- | ---------------------------------- |
| initialize / shutdown                  | 0 ✅     | -                                  |
| textDocument sync                      | 1 ✅     | -                                  |
| publishDiagnostics                     | 2 ✅     | `check_source`, spans              |
| documentSymbol                         | 3 ✅     | AST spans                          |
| definition                             | 4, 7 ✅  | AST + require resolution            |
| hover                                  | 5, 9 ✅  | AST; `type_at`                     |
| completion                             | 6 ✅     | grammar keywords + AST names       |
| references                             | 7 ✅     | binding / name index               |
| formatting                             | 8 ✅     | `yarrow-fmt`                       |
| codeAction / explain                   | 11 ✅    | `explain_code`                     |
| signatureHelp                          | 12 ✅    | AST + `type_at`                    |
| inlayHint                              | 13 ✅    | `TypeIndex` probes                 |
| semanticTokens                         | 14 ✅    | tokens + AST                       |
| rename                                 | 15 ✅    | references / resolve               |
| workspaceSymbol                        | 16 ✅        | open buffers + require ASTs        |
| rangeFormatting / onTypeFormatting     | 17 ✅        | `yarrow-fmt` (`format_range`; on-type deferred) |
| textDocument/diagnostic (pull)         | 18 ✅        | same as publish + uri/version cache             |
| TCP + test harness                     | 19 ✅        | transport only                     |
| editor extensions                      | 20       | packaging                          |
| DAP / debug                            | Out of scope | AOT/JIT debug                  |

---

## Later (backlog)

| Item                              | Notes                                                      |
| --------------------------------- | ---------------------------------------------------------- |
| Full project / multi-root index   | Blocked on core project graph; do not fake in LSP          |
| Cross-crate silent rename         | Explicitly refused; Stage 15 stays conservative            |
| Incremental / salsa analysis      | Only if check latency becomes a real pain                  |
| Virtual `yarrow-std:` URIs        | Only if embedded std has no on-disk `lib/std` path         |
| DAP / debug adapter               | Separate product                                           |
| Markdown / embedded `.yar`        | Skip until requested                                       |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use scripted LSP messages + `docs/examples/**` as gates (prefer `scripts/harness.mjs` from Stage 19).
- Update this file when a stage gate lands (mark done, short notes; do not re-expand history). When a whole phase is done, collapse finished stages into **Landed** the same way Stages 0–11 were.
- No tokenizer / parser / typechecker logic here beyond calling `yarrow-core`.
- Format only through `yarrow-fmt`, never a second pretty-printer.
- In comments and documentation, never use `—` (em dash); use ASCII hyphen or rephrase.
- If core needs position helpers, richer probes, or require-path APIs, land them in `yarrow-core` and note the dependency here and in the core plan Known gaps / Next.
- Keep the safe vs unsafe boundary visible in hovers / inlays when relevant; do not imply `unsafe` turns off checking.
