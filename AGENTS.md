# AGENTS.md

Yarrow is a typed, stack-based programming language with ownership, borrow checking, regions, and an explicit `unsafe` boundary. Implemented in Rust; compiles to Cranelift JIT.

## Source of truth

- Language spec: `docs/GRAMMAR.md`
- Implementation roadmap + gates: `crates/**/PLAN.md`

## Layout

- `crates/yarrow-core`: tokenizer → parser → compiler → runtime (Cranelift JIT)
- `crates/yarrow-cli`: CLI driver made with Clap (modules resolve relative to the source file)
- `src/main.rs`: entry point of the project, linked to the CLI
- `docs/`: documentation of the project, from grammar and syntax to memory model and type system or style guide

## Agent rules

- Do not write tests unless the user explicitly asks. Tests come later.
- Treat `docs/GRAMMAR.md` as authoritative.
- Update `PLAN.md` status and gates when a stage is finished.
- When spec and code diverge, follow the plan’s tasks to close the gap.
- Keep the safe vs unsafe boundary strict:
  - Safe: ownership, borrow, regions, stack effects, types.
  - `unsafe function` / `unsafe … end` only allow explicitly unsafe operations (raw pointers, etc.). They do not disable borrow or ownership checking.
- Prefer minimal, focused changes that pass the relevant PLAN gate.
- Do not invent lifetime syntax (`reference<'a, T>`). Ownership and regions are the model.
- Module imports use the form `"path" [scope] require` (keyword last).
- In comments and documentation, never use `—`.

## Build & run

```bash
cargo fmt --all
cargo check
cargo clippy
cargo run -- <file.yar>
```
