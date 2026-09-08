# yarrow-lsp

Language server for `.yar` files. Speaks LSP over **stdio** and calls `yarrow-core` / `yarrow-fmt` in-process.

## Run

```bash
# Dedicated binary
cargo run -p yarrow_lsp -- --stdio

# Via the main CLI (same process)
cargo run -- lsp
# or, after install: yarrow lsp
```

Flags (binary and `yarrow lsp`):

| Flag | Meaning |
| ---- | ------- |
| `--stdio` | Transport (default; only transport in v1) |
| `-L` / `--search-path DIR` | Extra module search root (repeatable) |
| `--main NAME` | Entry function name (default `main`) |
| `--no-format` | Do not advertise document formatting |
| `--log-level off\|error\|warn\|info\|debug` | stderr process logs (default `info`) |

## Editor setup

Point the editor's language server command at `yarrow lsp` or the `yarrow_lsp` binary. No extra args required for a basic handshake.

Optional `initializationOptions` (JSON, camelCase):

```json
{
  "searchPaths": ["/extra/modules"],
  "entryName": "main",
  "format": true
}
```

Workspace folders from `initialize` are appended as search paths. Client init options merge on top of process flags.

## Code actions

On a diagnostic whose code is in the `yarrow explain` catalog, the server offers **Explain Exxx** (no edits). Choosing it runs workspace command `yarrow.explain` with the code; the server shows the same long-form text as `yarrow explain`. Hover over a labeled span also appends that explain section.

## Signature help

On a postfix call site (`name call` or `a.b call`), `textDocument/signatureHelp` returns the callee signature (AST, enriched with `type_at` when check succeeds). Trigger character is space. Outside a resolved call site the response is null.
