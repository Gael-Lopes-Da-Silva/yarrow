# yarrow-lsp

Language server for `.yar` files. Speaks LSP over **stdio** (default) or **TCP** (`--listen`) and calls `yarrow-core` / `yarrow-fmt` in-process.

## Run

```bash
# Dedicated binary (stdio)
cargo run -p yarrow_lsp -- --stdio

# Via the main CLI (same process)
cargo run -- lsp
# or, after install: yarrow lsp

# TCP: accept one client (port 0 = ephemeral; prints bound address on stderr)
cargo run -p yarrow_lsp -- --listen 127.0.0.1:0
# or: yarrow lsp --listen 127.0.0.1:0
```

Flags (binary and `yarrow lsp`):

| Flag | Meaning |
| ---- | ------- |
| `--stdio` | Transport (default when `--listen` is omitted) |
| `--listen HOST:PORT` | Accept one TCP client (port `0` picks an ephemeral port) |
| `-L` / `--search-path DIR` | Extra module search root (repeatable) |
| `--main NAME` | Entry function name (default `main`) |
| `--no-format` | Do not advertise document / range formatting |
| `--no-inlay` | Do not advertise inlay hints |
| `--log-level off\|error\|warn\|info\|debug` | stderr process logs (default `info`) |

## Protocol harness

Scripted LSP checks without an editor. From the repo root:

```bash
node crates/yarrow-lsp/scripts/harness.mjs
# same as: node crates/yarrow-lsp/scripts/harness.mjs pull-diagnostics
```

The harness starts `yarrow_lsp --listen 127.0.0.1:0`, connects over TCP, runs initialize → open fixture → assert (default: pull diagnostics `E373` on `docs/examples/invalid/01_use_after_move.yar`) → shutdown. Override the server command with `YARROW_LSP_BIN` (space-separated argv prefix).

## Editor setup

Point the editor's language server command at `yarrow lsp` or the `yarrow_lsp` binary. No extra args required for a basic handshake.

Optional `initializationOptions` (JSON, camelCase):

```json
{
  "searchPaths": ["/extra/modules"],
  "entryName": "main",
  "format": true,
  "inlayHints": true
}
```

Workspace folders from `initialize` are appended as search paths. Client init options merge on top of process flags.

## Code actions

On a diagnostic whose code is in the `yarrow explain` catalog, the server offers **Explain Exxx** (no edits). Choosing it runs workspace command `yarrow.explain` with the code; the server shows the same long-form text as `yarrow explain`. Hover over a labeled span also appends that explain section.

## Signature help

On a postfix call site (`name call` or `a.b call`), `textDocument/signatureHelp` returns the callee signature (AST, enriched with `type_at` when check succeeds). Trigger character is space. Outside a resolved call site the response is null.

## Inlay hints

After a successful check, `textDocument/inlayHint` places type annotations after binding names (for example `: i32` on `answer`) and short `stack: …` notes on function sites, using only core `TypeIndex` probes. Disable with `--no-inlay` or init option `inlayHints: false`.

## Semantic tokens

`textDocument/semanticTokens/full` classifies core tokenizer tokens (keywords, strings, numbers, comments, operators) and same-file resolved identifiers (`function` / `variable` / `type` / `property`). Unresolved names and brackets are left to the client grammar. Legend types: keyword, function, variable, type, parameter, property, string, number, comment, operator; modifier `declaration` on binding sites.

## Rename

`textDocument/prepareRename` and `textDocument/rename` rename local functions, variables, types, properties, and explicit require aliases in the current file (binding-accurate identifier edits only). Keywords, scope collisions, unresolved names, and implicit / cross-module require renames are rejected with a clear error; module path strings are not rewritten.

## Workspace symbols

`workspace/symbol` searches top-level functions, types, and `implement` methods in open `.yar` buffers and one-hop resolved `require` files (no full project crawl). Matching is case-insensitive substring (prefix matches sort first); results are capped at 100. Empty query returns a bounded list. Closed / unchecked trees stay invisible.

## Range formatting

`textDocument/rangeFormatting` formats via `yarrow_fmt::format_range`: the selection expands to enclosing top-level item boundaries (same style as full-document format). Parse failure returns null (buffer unchanged). A second request on an already-formatted cover yields empty edits. Disabled with `--no-format` / init `format: false` (omits the capability). On-type formatting is not advertised: mid-edit parse failures and top-level expansion are unsafe for keystroke triggers.

## Pull diagnostics

`textDocument/diagnostic` returns the same diagnostics as push (`check_document`), with identifier `yarrow`. Results are cached by URI + document version (`resultId` = `v{version}`); a matching `previousResultId` yields an unchanged report so dual push+pull clients avoid flicker. Push on `didOpen` / `didChange` is unchanged. Workspace-wide pull is not advertised.
