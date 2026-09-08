# Yarrow Core Implementation Plan

Compiler library: tokenize → parse → check → `{ jit | object | executable | interpret }`.

`yarrow-core` is the **API**. Drivers (`yarrow-cli`, later `yarrow-fmt` / `yarrow-lsp`) call it; they do not reimplement the pipeline. CLI UX lives in [`crates/yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md).

## Source of truth

| Role                 | Path                                                                                                                               |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Language tour        | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md)                                                                                         |
| Formal syntax        | [`docs/SYNTAX.md`](../../docs/SYNTAX.md)                                                                                           |
| AST / types / memory | [`docs/AST.md`](../../docs/AST.md), [`TYPE_SYSTEM.md`](../../docs/TYPE_SYSTEM.md), [`MEMORY_MODEL.md`](../../docs/MEMORY_MODEL.md) |
| Runtime / modules    | [`docs/RUNTIME.md`](../../docs/RUNTIME.md)                                                                                         |
| Corpus               | [`docs/examples/`](../../docs/examples/README.md)                                                                                  |
| Agent rules          | [`AGENTS.md`](../../AGENTS.md)                                                                                                     |

Prefer the docs when code and docs disagree. Do not invent language features absent from the docs.

---

## Landed

| Component   | Notes                                                                                                      |
| ----------- | ---------------------------------------------------------------------------------------------------------- |
| Frontend    | Tokenizer + parser (flat postfix `Apply*`); rustc-style diagnostics; `Comment` tokens                      |
| Checking    | Types, ownership, borrow, regions, unsafe; stack-effect notes; `LowerKind::Check` (no JIT install)         |
| Warnings    | `W401` / `W402` / `W403` (unused binding / require / dead stack); `CheckedProgram::warnings`               |
| Session API | `check` / `compile` (JIT) / `compile_object` / `compile_executable` / `interpret`                          |
| AOT         | Runtime archive + Cranelift process `main` + `ld`/`lld` link (linux-gnu; no `cc` compile step)             |
| Runtime/std | Host heap, regions, lists/maps/strings; `std.io` / `std.string` / `std.fs` host wrappers                   |
| Interpret   | Stage 21 subset of `docs/examples/valid/**` (stdout matches JIT); structs / errors / regions still E393    |

**Gates:** `docs/examples/valid/**` compile and run (JIT); `invalid/**` fail for the stated reason; `warnings/**` check with `Ok` + warnings; `cargo fmt && cargo check && cargo clippy` green.

Phases A–E (Stages 0–24) are complete. Historical stage write-ups were removed; git history keeps them.

---

## Known gaps

| Area        | Gap                                                                                                           |
| ----------- | ------------------------------------------------------------------------------------------------------------- |
| AOT         | linux-gnu host only; no DWARF, `-O` tiers, or cross-compile (Phase F Stages 25–26)                            |
| Interpret   | No structs, unions, regions, unsafe, errors/`unwrap`, lists/maps (E393); not full JIT corpus parity           |
| Warnings    | Only unused / dead-stack; more lints later                                                                    |
| Projects    | Single-file + `require` only; no multi-root project graph (Stage 28)                                          |
| Default     | Session / CLI still default to JIT; product switch to `object` is Stage 29                                    |
| Linker      | System `ld`/`lld` only; bundled linker only if that becomes too painful (Stage 27)                            |
| LSP assist  | No typed-at-span / require-path index API yet; server uses `check_source` + AST (see `yarrow-lsp`)             |
| Formatter   | Whitespace still rebuilt by printer (`yarrow-fmt`)                                                            |

---

## Next (Phase F)

Focus: AOT polish on linux-gnu first (debug + opts), then target / linker story, then project shape and default backend. Keep interpreter corpus growth opportunistic when it unblocks a gate; do not invent language features.

### Stage 25 - AOT DWARF and `-O` tiers

Everyday AOT on linux-gnu is stable; add debug info and controllable optimization.

1. Emit DWARF (or Cranelift’s supported debug info) into object / executable products so a host debugger can set breakpoints on Yarrow entry and see function names / line mappings when spans exist.
2. Expose opt tiers on [`CompileOptions`](src/session.rs) (e.g. none / speed / size) and thread them into Cranelift settings for `Object` / executable paths; JIT may honor the same knob or document that it stays debug-friendly.
3. Keep link surface unchanged: still system `ld`/`lld` + `linkable_archive()`, no `cc` as compile driver.
4. Document the flags / options in [`docs/RUNTIME.md`](../../docs/RUNTIME.md) (AOT section); CLI wiring stays in `yarrow-cli` once core exposes the options.

**Gate:** `compile_executable_source` (or `compile_object_source` + link) of a small valid example produces a binary with inspectable debug info (e.g. `llvm-dwarfdump` / `readelf` shows compilation units or function names). At least two opt tiers produce distinct Cranelift flags or measurable IR/object differences. `cargo clippy` green; JIT corpus gates unchanged.

### Stage 26 - Cross-compile triples

Host is linux-gnu only today (`link.rs` + runtime archive). Add a real target triple story.

1. Accept a target triple (or Cranelift `Isa` selection) on compile/object/executable options; reject unsupported triples with a clear diagnostic (not a panic).
2. Build or select a matching `yarrow_runtime_aot` archive and CRT objects for that triple; document the layout and how agents/CI obtain archives (no inventing a second runtime ABI).
3. Object emit must use the triple’s ISA; executable link must pass the right linker emulation / sysroot flags when linking on the host for a different target.
4. Prefer one additional triple first (e.g. another linux-gnu arch, or linux-musl) before a broad matrix. Mach-O / Windows stay later unless already cheap.

**Gate:** documented command or Session options produce a non-host object (and, if link is in scope, an executable) for one non-host triple; missing archive/CRT fails with `E394`-family diagnostics. Host linux-gnu path still passes existing AOT examples. Update Known gaps when the first triple lands.

### Stage 27 - Bundled linker (optional)

Only if Stage 25–26 show system `ld`/`lld` discovery is too fragile for everyday use. Skip this stage (mark cancelled / deferred in notes) if system linkers remain reliable.

1. Vendor or depend on a known linker (e.g. `lld` as a library or pinned binary) invoked from `link::link_executable` without shelling out to a random PATH `ld` when the bundle is enabled.
2. Keep the “no `cc` compile step” rule; CRT discovery may still use `cc -print-file-name` for paths only.
3. Feature-gate or option-gate the bundle so default builds do not force a huge download unless chosen.

**Gate:** with the bundle enabled on linux-gnu, `compile_executable_source` succeeds without requiring a system `ld`/`lld` on `PATH` (CRT still locatable). Document how to enable it. If skipped: one-line note here and leave Known gaps pointing at system linkers.

### Stage 28 - Multi-file project graph beyond `require`

Today: one root file + `"path" [scope] require` relative to that file / search paths. No multi-root project model.

1. **Decide the product shape first** (document in RUNTIME / a short project note): e.g. explicit project manifest vs directory of roots vs “check this set of files sharing search paths.” Do not invent lifetime- or package-manager syntax absent from the docs; extend only what GRAMMAR/RUNTIME already allow or what a minimal manifest needs.
2. Core API: build a graph of modules / roots, share checked definitions where safe, and surface cycle / missing-module diagnostics with spans.
3. Session entry points for “check/compile project” (names TBD) that drivers can call; single-file `*_source` APIs remain.
4. Coordinate with CLI / LSP later; this stage only lands the library graph + diagnostics.

**Gate:** a documented multi-file fixture (beyond nested `require` from one root) type-checks via the new API; cycles or missing roots fail with stable codes. Existing single-file + `require` corpus still passes. No CLI subcommand required in this stage.

### Stage 29 - Default backend `object` instead of `jit`

Product/CLI decision; core must expose a coherent default.

1. Agree with [`yarrow-cli/PLAN.md`](../yarrow-cli/PLAN.md) backlog (“Default `--target object`”): either flip [`ExecutionMode`](src/session.rs) default from `Jit` to `Object`, or keep core default and only change CLI defaults—pick one and document it in RUNTIME.
2. Ensure `compile` / session helpers that today assume JIT either take an explicit mode or follow the new default without breaking `run_main` callers (JIT-only APIs stay JIT).
3. Update driver-facing docs / help strings when CLI lands the flip; core stage is done when the library default and Session behavior match the decision.

**Gate:** new `CompileOptions::new` (or documented CLI default) matches the chosen backend; `docs/examples/valid/01_hello.yar` still runs under an explicit JIT path. No silent change to `interpret` / `check`.

---

## Later (backlog)

| Item                                      | Notes                                                                 |
| ----------------------------------------- | --------------------------------------------------------------------- |
| Interpreter corpus → JIT parity           | Structs, unions, regions, unsafe, errors, lists/maps (grow past E393) |
| Richer warning / lint catalog             | Beyond W401–W403                                                      |
| Typed-at-span / signature probe API       | For `yarrow-lsp` Stage 9 hover / inlay                                |
| Broader cross-compile matrix              | After Stage 26’s first triple                                         |
| Mach-O / Windows AOT link                 | After linux cross story is real                                       |

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- When renaming std or syntax, update compiler and `lib/std` together.
- Do not add tests unless explicitly asked; use `docs/examples/**` as gates.
- Update this file when a stage gate lands (mark ✅, short notes; do not re-expand history).
- Do not invent lifetime syntax; ownership and regions are the model.
- `unsafe` never disables type, stack, ownership, or borrow checking.
- No CLI parsing (`clap`, argv, exit codes) here; that is `yarrow-cli`.
- May invoke a system **linker** from a library helper; must not depend on a C compiler for CRT or codegen.
