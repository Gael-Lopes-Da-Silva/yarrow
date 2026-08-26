<p align="center">
  <img src="assets/logo.jpg" alt="Yarrow" width="480">
</p>

<p align="center">
  <b>A typed, stack-based language with ownership, regions, and explicit unsafe</b>
</p>

<p align="center">
  Safe by default · No lifetime annotations · Cranelift JIT, object emit, and host link
</p>

---

Yarrow is postfix and stack-oriented: you push values, then the word that consumes them. Heap data is owned; `borrow` creates safe `reference<T>` values; regions free groups of allocations together. Raw pointers stay behind a visible `unsafe` boundary that does **not** turn off ownership or borrow checking.

## Quick look

```yarrow
"std.io" io require

greet function
	string
do
	"Hello, " swap ~ io.write_line call
end

main function do
	"Yarrow!" greet call
end
```

```bash
cargo run -- docs/examples/valid/01_hello.yar
# Hello, Yarrow!
```

## What you get

- **Stack-oriented** - postfix words, explicit stack effects, composable phrases
- **Ownership and regions** - no user-visible lifetime parameters on types
- **Safe vs unsafe** - `reference<T>` in safe code; `pointer<T>` and `mem.*` only in `unsafe`
- **Modules** - `"path" [alias] require` (keyword last); std is embedded
- **Tooling today** - `run` (JIT or native), `compile`, `check`, `interpret`, `dump`, `explain`
- **In progress** - formatter (`yarrow-fmt`) and language server (`yarrow-lsp`)

## Build and run

Requires a Rust toolchain (edition 2024) and, for native `object` / executable linking on Linux, a system linker (`ld` or `lld`).

```bash
cargo build --release
cargo run -- docs/examples/valid/01_hello.yar
```

Useful commands:

```bash
cargo run -- check docs/examples/valid/01_hello.yar
cargo run -- compile --target object -o /tmp/hello.o docs/examples/valid/01_hello.yar
cargo run -- run --target object docs/examples/valid/01_hello.yar
cargo run -- interpret docs/examples/valid/01_hello.yar
cargo run -- dump --emit ast docs/examples/valid/01_hello.yar
cargo run -- explain E360
```

`cargo run -- <file.yar>` is the same as `run --target jit` with entry `main`. Use `--main` to pick another entry name.

## Repository layout

- [`crates/yarrow-core`](crates/yarrow-core) - tokenizer, parser, checker, JIT / object / executable / interpret (`Session` API)
- [`crates/yarrow-runtime`](crates/yarrow-runtime) - host heap and `HOST_FNS`; [`aot/`](crates/yarrow-runtime/aot) staticlib for linking
- [`crates/yarrow-cli`](crates/yarrow-cli) - Clap driver used by the root `yarrow` binary
- [`crates/yarrow-fmt`](crates/yarrow-fmt) / [`crates/yarrow-lsp`](crates/yarrow-lsp) - formatter and LSP (planned)
- [`docs/`](docs/README.md) - language reference, style guide, and examples
- [`crates/yarrow-core/lib/std`](crates/yarrow-core/lib/std) - standard library written in Yarrow

Roadmaps and gates: `crates/**/PLAN.md`. Agent conventions: [`AGENTS.md`](AGENTS.md).

## Status

Core and CLI through the planned compile / check / interpret / native-run stages are landed. The language docs and [`docs/examples`](docs/examples/README.md) describe the intended shape; prefer the grammar when an example and the unfinished compiler disagree. Formatter and LSP are staged next in their crate plans.

## Learn more

- [Documentation index](docs/README.md) - grammar, syntax, types, memory, runtime, style
- [Grammar tour](docs/GRAMMAR.md) - language by annotated example
- [Examples](docs/examples/README.md) - valid and invalid programs
- [Style guide](docs/STYLE_GUIDE.md) - layout for human code and the future formatter
