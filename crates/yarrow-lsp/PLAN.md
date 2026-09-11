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

### Landed (v1, Stages 0–19, 21–22)

- stdio Language Server Protocol (LSP 3.17-shaped)
- TCP `--listen host:port` transport (one client) + in-repo protocol harness
- Text document sync for `file://` `.yar` buffers
- Publish diagnostics from `Session::check_source` (and parse failures)
- Optional multi-root `Session::check_project` via init `projectRoots`
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
- Core `definition_at` probes for goto / hover (Stage 22; AST fallback on miss)

Stage 20 (editor extension packaging) was **canceled**; clients live in separate repos.

### Out of scope

| Concern                               | Why                                                         |
| ------------------------------------- | ----------------------------------------------------------- |
| Package-manager / manifest invent     | Language has no project file; roots are explicit paths only |
| Silent semantic rename across modules | Stage 15 stays conservative; never guess                    |
| Debug Adapter Protocol                | Separate product; AOT/JIT debug is not an LSP feature       |
| Snippet / AI rewrite actions          | Not mechanical language support                             |
| Non-`.yar` / markdown embedded        | Skip until requested                                        |
| Editor extensions (VS Code / Zed / …) | Live in separate repos later; not in the main compiler tree |

**Transport:** stdio is the default. TCP (`--listen`) is for tests / remote clients (Stage 19).

---

## Architecture

```text
editor  ←stdio JSON-RPC→  yarrow-lsp
                            ├── DocumentStore (uri → text + version)
                            ├── Analysis (Session::parse_source / check_source
                            │             / check_project when multi-root)
                            ├── PositionMap (LSP ↔ core Span / SourceFile)
                            ├── Features (diag, hover, def, refs, symbols, complete,
                            │             format, codeAction, signature, inlay, …)
                            ├── yarrow_core::Session
                            └── yarrow_fmt::format_source / format_range
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

| Piece     | Choice                                                 |
| --------- | ------------------------------------------------------ |
| LSP types | via `tower-lsp-server` (community fork of `tower-lsp`) |
| Runtime   | `tokio`                                                |
| Binary    | `crates/yarrow-lsp` + `yarrow lsp`                     |

Do **not** shell out to `yarrow check`; call `Session` in-process.

### Position mapping

LSP positions are UTF-16 code units by default (or UTF-8 if negotiated). Core `Span` / `SourceFile::location` use **byte offsets** and Unicode scalar columns.

1. `PositionMap` converts `Span` ↔ `lsp::Range`.
2. Prefer negotiating `positionEncoding = utf-8` when the client supports it; still support UTF-16 for VS Code-class clients.

### Analysis model

On `didOpen` / `didChange` (debounced):

1. Update buffer text.
2. Build `CompileOptions` with `source_path` from URI, search paths from init options / workspace folders.
3. Call `check_source` (or `check_project` when Stage 21 multi-root mode applies).
4. Map `DiagnosticBatch` → `PublishDiagnosticsParams` (and answer pull requests).
5. Cache last successful parse / check artifact for hover, navigation, inlays, tokens.

Open documents + transitive `require` resolution cover the single-file case. Multi-root project check is Stage 21 (core graph already exists).

---

## Current state

| Piece               | Status | Notes                                                         |
| ------------------- | ------ | ------------------------------------------------------------- |
| `yarrow-lsp` crate  | ✅     | Stages 0–19 + 21–22 landed; Stage 20 canceled                 |
| Core Session API    | ✅     | `parse_source` / `check_source` / `check_project` + spans     |
| Core diagnostics    | ✅     | `Diagnostic` / `Severity` / codes / explain table             |
| Typed hover data    | ✅     | `CheckedProgram::type_at` (core Stage 30)                     |
| Def / require probe | ✅     | Core Stage 35 + LSP Stage 22 consume `definition_at`          |
| Cross-file resolve  | ⚠      | Probe + AST / `require`; project multi-root in Stage 21       |
| `yarrow-fmt`        | ✅     | Full-doc best-effort + `format_range`; on-type still deferred |
| CLI `yarrow lsp`    | ✅     | In-process `run_stdio_blocking` / `--listen`                  |

---

## Landed (Stages 0–19, 21–22)

Stages 0–19 are complete. Historical stage write-ups were removed; git history keeps them. Stage 20 (editor extensions) was canceled. Stages 21–22 are landed.

| Stage | Capability                                                  |
| ----- | ----------------------------------------------------------- |
| 0     | Crate + stdio hello (`initialize` / `shutdown`)             |
| 1     | Document sync (`DocumentStore`, full sync)                  |
| 2     | Position map + publish diagnostics (debounce)               |
| 3     | Hierarchical `documentSymbol`                               |
| 4     | Same-file `definition`                                      |
| 5     | AST hover + explain blurb on diagnostic spans               |
| 6     | Completions (keywords, scoped names, `std.*` require)       |
| 7     | Same-file `references` + cross-file def via `require`       |
| 8     | Full-document `formatting` via `yarrow-fmt`                 |
| 9     | Typed hover via `CheckedProgram::type_at`                   |
| 10    | `LspConfig` / init options + `yarrow lsp` wrapper           |
| 11    | `codeAction` Explain Exxx + `yarrow.explain` command        |
| 12    | `signatureHelp` for postfix `name call`                     |
| 13    | Inlay hints from `TypeIndex` probes                         |
| 14    | Semantic tokens (full document) from tokenizer + AST decls  |
| 15    | File-local rename (`prepareRename` + `rename`)              |
| 16    | Workspace symbols (`workspace/symbol` over open + requires) |
| 17    | Range formatting via `format_range` (on-type deferred)      |
| 18    | Pull diagnostics (`textDocument/diagnostic`; workspace off) |
| 19    | TCP `--listen` + `scripts/harness.mjs`                      |
| 20    | Editor extensions - **canceled** (separate repos)           |
| 21    | Project-aware multi-root via `projectRoots` + `check_project` |
| 22    | Core `definition_at` for goto / hover; AST fallback on miss |

**Stage 21 notes:** Init option `projectRoots: string[]` (absolute or cwd-relative). Default remains single-file `check_source`. Open buffers overlay on-disk roots; missing roots publish `E383`. `interFileDependencies` is true in project mode. Stretch (graph → workspace symbols) deferred. Harness: `project-roots`, `project-missing-root`.

**Stage 22 notes:** Prefer `CheckedProgram::definition_at` (core Stage 35) for `textDocument/definition` and hover “defined in …” / module path. Misses keep AST / require resolution. Harness: `definition-require` on `12_modules.yar` `greet` alias.

---

## Next

Focus: on-type formatting / workspace pull / latency. Prefer harness scenarios over ad-hoc scripts. Do not invent language features or a package manifest.

### Stage 23 - On-type formatting

Stage 17 deferred on-type: mid-edit buffers often fail to parse, and top-level `format_range` expansion is too aggressive for a keystroke.

1. Prefer shipping after [`yarrow-fmt` Stage 18](../yarrow-fmt/PLAN.md) selective reprint (or prove a tiny safe subset without it).
2. Advertise `documentOnTypeFormattingProvider` only for triggers that stay mechanical and local (candidate: `\n` when the previous non-ws token is `end` and best-effort / range fmt can align without rewriting the whole file). Skip mid-token and mid-identifier triggers.
3. On parse / format failure: return null / empty edits; never corrupt the buffer.
4. Honor `--no-format` / init `format: false` (omit capability).
5. If no safe trigger exists, Done notes say deferred again with reason; do not ship a fighting formatter.

**Gate:** either one harness (or documented manual) on-type edit yields a safe indent/align edit, **or** Done explicitly keeps on-type deferred. Range + full format unchanged. `cargo clippy` green.

---

### Stage 24 - Workspace pull diagnostics

Stage 18 set `workspaceDiagnostics: false` and skipped `workspace/diagnostic`.

1. Enable workspace pull only for **open documents** and, when Stage 21 project mode is on, configured project roots (not an unbounded disk walk).
2. Advertise `workspaceDiagnostics: true` when implemented; answer `workspace/diagnostic` with per-document reports (or a documented partial report).
3. Reuse the uri+version cache from Stage 18; avoid double-flicker with push.
4. `interFileDependencies`: true only if Stage 21 actually rechecks related roots together; otherwise keep false and document.
5. Harness scenario: workspace pull returns diagnostics for an open invalid fixture without waiting on publish.

**Gate:** scripted `workspace/diagnostic` (or equivalent) sees `E373` (or known code) for the open invalid file. Text-document pull + push still work. `cargo clippy` green.

---

### Stage 25 - Analysis cancelation / latency polish

Re-check on every debounced change is enough until latency hurts; then harden the request path without inventing salsa.

1. Cancel or ignore stale in-flight checks when a newer `didChange` supersedes them (version-aware).
2. Keep debounce configurable or documented; do not block the LSP event loop on long checks (spawn / async as the stack already allows).
3. Optional: reuse module-load results across open files when Stage 21 project mode already shares a graph; do not add a parallel cache that disagrees with Session.
4. No salsa / incremental IR unless profiling shows a clear win after 1–3; if skipped, Done notes say so.
5. Harness or timing note optional; primary gate is correctness under rapid edits (no torn diagnostics for an older version after a newer check completes).

**Gate:** open a file, apply two quick full-document changes with increasing versions; the last published / pulled diagnostics match the latest version only. `cargo clippy` green.

---

### Stage 26 - Virtual `yarrow-std:` URIs (optional)

Only if embedded / packaged std has no reliable on-disk `lib/std` path for goto / hover.

1. Confirm need: if `lib/std` is always resolvable via search paths, **skip** and leave this in Later.
2. Otherwise map `std.*` require targets to a virtual URI scheme (e.g. `yarrow-std:io.yar`) and serve read-only content from the embedded sources for definition / hover.
3. Do not allow edits to virtual buffers (or mark them non-writable). Diagnostics stay on user files.
4. Document the scheme in the crate README.

**Gate:** goto on a `std.io` require opens a buffer (virtual or real) showing the std module text. If skipped for lack of need, Done notes say so. `cargo clippy` green.

---

## Mapping: LSP features → stages

| LSP capability                     | Stages         | Core / fmt dependency                      |
| ---------------------------------- | -------------- | ------------------------------------------ |
| initialize / shutdown              | 0 ✅           | -                                          |
| textDocument sync                  | 1 ✅           | -                                          |
| publishDiagnostics                 | 2 ✅           | `check_source`, spans                      |
| documentSymbol                     | 3 ✅           | AST spans                                  |
| definition                         | 4, 7, 22 ✅    | AST + require; core Stage 35 probes        |
| hover                              | 5, 9, 22 ✅    | AST; `type_at`; def/require probes         |
| completion                         | 6 ✅           | grammar keywords + AST names               |
| references                         | 7 ✅           | binding / name index                       |
| formatting                         | 8 ✅           | `yarrow-fmt`                               |
| codeAction / explain               | 11 ✅          | `explain_code`                             |
| signatureHelp                      | 12 ✅          | AST + `type_at`                            |
| inlayHint                          | 13 ✅          | `TypeIndex` probes                         |
| semanticTokens                     | 14 ✅          | tokens + AST                               |
| rename                             | 15 ✅          | references / resolve (file-local)          |
| workspaceSymbol                    | 16 ✅; 21 ✅   | open + requires; project roots via `check_project` |
| rangeFormatting / onTypeFormatting | 17 ✅; 23      | `format_range`; on-type after fmt Stage 18         |
| textDocument/diagnostic (pull)     | 18 ✅          | same as publish + uri/version cache                |
| workspace/diagnostic               | 24             | open / project roots only                          |
| TCP + test harness                 | 19 ✅          | transport only                                     |
| multi-root project check           | 21 ✅          | `check_project` (core Stage 28)                    |
| editor extensions                  | 20 ❌ canceled | separate repos later                       |
| DAP / debug                        | Out of scope   | AOT/JIT debug                              |

---

## Later (backlog)

| Item                        | Notes                                                 |
| --------------------------- | ----------------------------------------------------- |
| Cross-module silent rename  | Explicitly refused; Stage 15 stays conservative       |
| Salsa / full incremental IR | Only if Stage 25 still too slow                       |
| DAP / debug adapter         | Separate product                                      |
| Markdown / embedded `.yar`  | Skip until requested                                  |
| Editor extensions           | Separate repos (Stage 20 canceled here)               |
| Call / type hierarchy       | Needs richer core index; do not fake from names alone |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- Do not add tests unless explicitly asked; use scripted LSP messages + `docs/examples/**` as gates (prefer `scripts/harness.mjs` from Stage 19).
- Update this file when a stage gate lands (mark done, short notes; do not re-expand history). When a whole phase is done, collapse finished stages into **Landed** the same way Stages 0–19 were.
- No tokenizer / parser / typechecker logic here beyond calling `yarrow-core`.
- Format only through `yarrow-fmt`, never a second pretty-printer.
- In comments and documentation, never use `—` (em dash); use ASCII hyphen or rephrase.
- If core needs position helpers, richer probes, or require-path APIs, land them in `yarrow-core` and note the dependency here and in the core plan Known gaps / Next.
- Keep the safe vs unsafe boundary visible in hovers / inlays when relevant; do not imply `unsafe` turns off checking.
