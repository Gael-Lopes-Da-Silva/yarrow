# yarrow-lsp

Language server for Yarrow (`.yar`) editors. Speaks LSP over stdio (default) or
TCP (`--listen`), and delegates analysis to `yarrow-core` / formatting to
`yarrow-fmt`.

## Run

```bash
yarrow lsp
# or
cargo run -p yarrow_lsp --
cargo run -p yarrow_lsp -- --listen 127.0.0.1:0
```

Protocol harness (from repo root):

```bash
node crates/yarrow-lsp/scripts/harness.mjs
node crates/yarrow-lsp/scripts/harness.mjs virtual-std
```

## Virtual `yarrow-std:` URIs

Std modules are embedded in `yarrow-core`. Go-to-definition prefers on-disk
`lib/std/**` when that tree is reachable from the LSP build layout or the open
file. When no file exists (packaged binary without sources, or
`YARROW_LSP_FORCE_VIRTUAL_STD=1`), definition / hover resolve to a virtual URI:

| Module   | URI                     |
| -------- | ----------------------- |
| `std.io` | `yarrow-std:/io.yar`    |
| `std.math` | `yarrow-std:/math.yar` |

Clients fetch text with `workspace/textDocumentContent` `{ "uri": "…" }` (also
advertised under `initialize` → `capabilities.experimental.textDocumentContent`).
Virtual buffers are read-only: `didChange`, format, and rename are ignored or
refused. Diagnostics are not published for virtual std URIs.
