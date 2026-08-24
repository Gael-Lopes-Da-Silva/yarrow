# Yarrow Core Implementation Plan

Tracks bringing `crates/yarrow-core` in line with the **current** language docs, then deepening the compiler (diagnostics, UX).

## Source of truth

| Role                          | Path                                                 |
| ----------------------------- | ---------------------------------------------------- |
| Language tour (authoritative) | [`docs/GRAMMAR.md`](../../docs/GRAMMAR.md)           |
| Formal syntax                 | [`docs/SYNTAX.md`](../../docs/SYNTAX.md)             |
| Intended AST shape            | [`docs/AST.md`](../../docs/AST.md)                   |
| Types / checking              | [`docs/TYPE_SYSTEM.md`](../../docs/TYPE_SYSTEM.md)   |
| Ownership / unsafe            | [`docs/MEMORY_MODEL.md`](../../docs/MEMORY_MODEL.md) |
| Runtime / errors / modules    | [`docs/RUNTIME.md`](../../docs/RUNTIME.md)           |
| Source style                  | [`docs/STYLE_GUIDE.md`](../../docs/STYLE_GUIDE.md)   |
| Conformance corpus            | [`docs/examples/`](../../docs/examples/README.md)    |
| Agent rules                   | [`AGENTS.md`](../../AGENTS.md)                       |

**Phase A (Stages 0–7)** brought the tokenizer, parser, compiler, and std surface in line with the docs. Prefer the docs when code and docs disagree. Do not invent language features that are absent from the docs.

**Phase B (Stages 8+)** focuses on compiler UX and depth: rustc-style diagnostics first, then multi-error recovery and related tooling. Language surface changes stay doc-driven.

---

## Pipeline status (honest)

| Component   | Status      | Reality                                                                          |
| ----------- | ----------- | -------------------------------------------------------------------------------- |
| Tokenizer   | 🟢 Stage 1  | Docs surface tokens (`~`, `\|`, visibility, `error`, `copy`); UTF-8-safe lexing  |
| Parser/AST  | 🟢 Stage 2  | Parses `docs/examples/valid/**`; AST gains visibility, params, `error`, `Concat` |
| Compiler    | 🟢 Stage 7  | JIT; grammar tour + examples; ownership/borrow/region; unsafe (`E370`)           |
| Diagnostics | 🟢 Stage 10 | Rustc-style; teachable notes; multi-error recovery (cap 20)                      |
| Runtime     | 🟡 Partial  | Host heap, regions, lists, maps, strings, `Safety` metadata exist                |
| Std library | 🟢 Stage 5  | `public` APIs; `region`/`loop`/`error`/`fs` present; list/map polymorphic        |

Nothing below is “done” relative to the current docs unless its gate passes against those docs.

---

## What changed since the old plan

The previous plan tracked `docs/syntax.yar` and treated stages 0–4 as complete. The language surface has moved. Important deltas:

| Topic            | Old / current code                         | Current docs                                       |
| ---------------- | ------------------------------------------ | -------------------------------------------------- |
| Concatenation    | `+` on strings                             | `~` only; `+` is numeric / pointer offset          |
| Fallible returns | `with T or Error` (keyword `or`)           | `with \|T Err\|` union type literal                |
| Errors           | Ad hoc `error.*` tags / `Primitive::Error` | `Name error … end` (+ optional injection)          |
| Visibility       | Not parsed                                 | `public` / `private` on types, fields, functions   |
| Params           | Bare types only                            | `type [copy \| mutable]`                           |
| Loop control     | Keywords `break` / `continue`              | `std.loop` helpers (`loop.break`, `loop.value`, …) |
| Iterable `for`   | Optional binder names before `for`         | Iterable on stack; value/index via `std.loop`      |
| `std.mem`        | `alloc`                                    | `allocate`                                         |
| `std.list`       | `push`                                     | `push_last`                                        |
| `std.region`     | `make_region` / `put_region` / …           | `create` / `put` / `free`                          |
| Gate program     | `docs/syntax.yar`                          | `docs/GRAMMAR.md` tour + `docs/examples/**`        |
| 128-bit types    | Present in AST primitives                  | Not in `SYNTAX.md` primitives (drop or defer)      |

Keep useful runtime/compiler machinery from the old stages (ownership sets, unsafe contexts, module loader, Cranelift pipeline). Re-aim it at the new surface.

---

## Gap inventory

### Tokenizer (`src/tokenizer/`)

**Present and still valid:** arithmetic and comparison ops, `//`, logical/bitwise words, stack ops (`drop`/`dup`/`swap`/`rot`/`unrot`/`pop`), `typeof`, `borrow`/`move`, `load`/`store`, control keywords (`if`/`else`/`for`/`match`/`case`/`defer`/`handle`/`fallback`/`unwrap`), `function`/`do`/`with`/`end`/`call`/`return`, mutability (`mutable`/`const`/`static`/`set`), `struct`/`implement`/`enum`/`union`, `require`, `unsafe`, literals, `@` builtins, comments `#`.

**Missing vs docs:**

- `~` → concatenation operator
- `|` → union type literal delimiters (`|i32 AppError|`)
- `public` / `private`
- `error` (error-type declaration keyword)
- `copy` (parameter modifier)

**Extra vs docs (decide in Stage 1):**

- `break` / `continue` as language keywords (docs put these in `std.loop`)

### Parser / AST (`src/parser/`)

**Roughly working:** program of statements; functions (`unsafe function`); structs/enums/unions/implement; require; var decl / set; if/else; match (value + type case); for (condition + iterable, with old binder forms); defer; handle/fallback; move; unwrap/call; containers; generics `array`/`list`/`hashmap`/`pointer`/`reference`; nested functions.

**Diverges from `AST.md` / `SYNTAX.md`:**

| Spec                                     | Code today                                                     |
| ---------------------------------------- | -------------------------------------------------------------- |
| Flat stack `word`s (`ApplyBin`, …)       | Often builds nested `Expr::Binary` trees from an operand stack |
| `Program { items, main }`                | Flat `items: Vec<Stmt>`; `main` not distinguished              |
| `Visibility` on functions/structs/fields | Absent                                                         |
| `Error` top-level / `error_decl`         | Absent                                                         |
| `ParamModifier` (`copy` / `mutable`)     | Params are bare `Type` only                                    |
| `Operator::Concat` / `~`                 | Absent; string join via `BinOp::Plus`                          |
| `\|T U\|` union literals                 | `with` parsed as `T or U or …` via keyword `or`                |
| Enum optional underlying type            | `EnumDecl` has no carrier type                                 |
| Struct vs hashmap `{}` by key shape      | Map-oriented; struct literals incomplete relative to grammar   |
| Iterable `for` without binders           | Still accepts `iterable name [index] for`                      |

### Compiler (`src/compiler/`)

**Working enough to build on:** JIT module, function signatures, many operators, calls, unwrap/handle envelope (old shape), borrow/move tracking seeds, unsafe gate (`E370`), typed pointer load/store + arithmetic, regions via host builtins, list/map/string host ops, module/`require` resolution (std embed + item import), enums, some union match, structs/methods, defer.

**Known holes / mismatches:**

- String `+` concat → must become `~` only
- Anonymous / `|T U|` union types still rejected in places (`E308`)
- Field `set` unsupported (`E301`)
- Float `%` / `^` unsupported (`E334`)
- Fallible ABI still “success or `Error`”, not named `error` types + `|T Err|`
- Visibility / export checks missing
- `copy` parameters missing
- Smallest-fit integer/float literal typing incomplete vs type system
- Ownership gates not fully aligned with `docs/examples/invalid/*`
- Region escape not fully checked
- `run_main` limited return types (`E360`)
- Internals still expose old builtin names (`make_region`, `list_push`, `alloc`, …)

### Std (`lib/std/`)

| Module       | Spec expectation                      | Status                                              |
| ------------ | ------------------------------------- | --------------------------------------------------- |
| `std.io`     | `write_line`, …                       | Partial (`write_line`)                              |
| `std.mem`    | `allocate`, `free`, `load`, `store`   | ✅ `public` wrappers over `@alloc` / …              |
| `std.list`   | `push_last`, `get`, `put`, `len`, …   | ✅ intrinsics (any `list<T>`)                       |
| `std.map`    | hashmap helpers                       | ✅ intrinsics (any `hashmap<K V>`)                  |
| `std.string` | `len`, join helpers                   | Partial; join overlaps `~`                          |
| `std.math`   | `sqrt`, …                             | Partial                                             |
| `std.region` | `create`, `put`, `free`               | ✅ intrinsics over host region ops                  |
| `std.loop`   | `break`, `continue`, `value`, `index` | ✅ compiler intrinsics                              |
| `std.error`  | shared error members                  | ✅ `Error` + common tags                            |
| `std.fs`     | file open/close/…                     | Stub (`open_file` returns `NOT_FOUND`; no host I/O) |

### Runtime (`src/runtime.rs`)

Keep the host surface small. Prefer renaming/wrapping through std rather than growing new raw builtins. Host may keep `@alloc` while Yarrow exposes `mem.allocate`.

---

## Phase A: Language surface (Stages 0–7)

Gates should prefer `docs/examples/valid/*` and `docs/examples/invalid/*`, then larger slices of the grammar tour. Do not mark a stage ✅ until its gate commands succeed.

### Stage 0: Rebaseline ✅

- Point this plan (and comments) at `docs/GRAMMAR.md`, not `docs/syntax.yar`
- Inventory stays as above; fix only blockers for `cargo check` / `cargo clippy`
- Agree: 128-bit primitives (`i128`/`u128`/`f128`) are **out of spec**; removed from `Primitive` (Ty scalar slots kept for encoding stability)

**Gate:** `cargo fmt --all && cargo check && cargo clippy` green; plan matches docs.

---

### Stage 1: Tokenizer surface sync ✅

Bring lexing in line with `SYNTAX.md`.

1. Add `TokenKind` + lexing for `~` (concat)
2. Add `|` (pipe) for union type literals
3. Add keywords: `public`, `private`, `error`, `copy`
4. Keep `break` / `continue` as temporary language keywords until `std.loop` (Stage 5)
5. Fix UTF-8 source scanning (byte vs char cursor) so grammar comments with `→` tokenize

**Gate:** tokenizer accepts every token appearing in `docs/GRAMMAR.md` and `docs/examples/**/*.yar`; rejects unknown sigils cleanly.

---

### Stage 2: Parser / AST toward the docs ✅

Update `ast.rs` and `parser/mod.rs` to match `AST.md` / `SYNTAX.md` closely enough that the compiler can be honest.

Priority order:

1. **Concat** - parse `~` as `BinOp::Concat`
2. **Visibility** - optional `public`/`private` on functions, structs, fields
3. **Param modifiers** - `type copy` / `type mutable` on parameters
4. **Union type literals** - `|T U …|` in `with` and type positions
5. **`error` declarations** - `Name [QualifiedName] error { Ident } end` (`error` soft with `.` / `require`)
6. **Enum underlying type** - `Name [type] enum`
7. **`for` surface** - condition and iterable forms only (no binder names before `for`)
8. **Structural cleanup (can span stages)** - flatten toward stack words; distinguish `main`; struct vs hashmap literals by key shape — deferred

**Gate:** parse all of `docs/examples/valid/*.yar` into AST without error (compile may still fail). Invalid examples that are purely syntactic still fail at parse when appropriate.

---

### Stage 3: Core compiler semantics sync ✅

Align lowering and type checking with `TYPE_SYSTEM.md` / `MEMORY_MODEL.md` / `RUNTIME.md`.

1. **`~` concat** - ✅ string join via `str_join`; reject string `+` (`E335`)
2. **Smallest-fit literals** - ✅ integers pick smallest unsigned/signed; floats still `f64` for Cranelift
3. **`copy` / `mutable` params** - ✅ `copy` deep-copies strings (other heap types still `E336`); scalars trivial
4. **Visibility** - ✅ non-`public` functions do not export across `require` (`E381`); std APIs marked `public`
5. **Field `set`** - already implemented for struct/pointer members; remaining E301 paths unchanged
6. **Numeric gaps** - float `%` / `^` still `E334` (documented reject)
7. **Drop / stack hygiene** - ✅ `drop` clears the whole stack; `pop` removes one
8. **Gate support** - `std.loop` intrinsics (`value`/`index`/`break`/`continue`); `list.push_last`; Seq-trailing container initializers

**Gate:** `docs/examples/valid/01`–`06`, `13` compile and run; string examples use `~`.

---

### Stage 4: Unions, errors, unwrap/handle ✅

Replace the old `Error` / `or` envelope story with the documented model.

1. Named `error` types + member tags - ✅ `ErrorDecl` registration; `AppError.NOT_FOUND`
2. Optional injection (`Name other.error error … end`) - ✅ copies tags from inject source
3. Fallible returns `|T Err|` / `|void Err|` as union literals - ✅ expands to envelope ABI
4. Envelope ABI as in `RUNTIME.md` (env tag + payload) - ✅ unchanged `(env, payload)`
5. `unwrap` propagate vs reject when caller cannot error - ✅ compile error (`E308`) if caller cannot fail
6. `handle` + `fallback` + error `match` inside handle - ✅ `AppError.MEMBER case` tag dispatch
7. Named union values + `Type case` arms yielding `reference<Member>` - ✅ (also fixed `~` on strings)
8. Wire `std.error` members - ✅ `lib/std/error.yar` (`OUT_OF_MEMORY`, …)

**Gate:** `docs/examples/valid/07_unions.yar`, `10_errors.yar`, and invalid `08`/`09` behave as documented.

---

### Stage 5: Std library to match the grammar ✅

Pure-Yarrow modules under `lib/std/`, names exactly as in `GRAMMAR.md`.

1. Rename APIs: `mem.allocate`, `list.push_last`, … - ✅
2. Add `std.region` (`create` / `put` / `free`) wrapping host region ops - ✅ intrinsics
3. Add `std.loop` (`break` / `continue` / `value` / `index`) - ✅ (Stage 3/5)
4. Add `std.error` baseline members - ✅ (Stage 4)
5. Add / extend `std.fs` as required by the grammar tour - ✅ stub (`File`, `open_file`/`close_file`; open returns `NOT_FOUND` until host I/O)
6. Generalize list/map beyond single hard-coded element types where the compiler allows - ✅ `std.list` / `std.map` as polymorphic intrinsics
7. Prefer: unsafe host → invariant → safe Yarrow API - ✅ mem wrappers; region/list/map intrinsics
8. Shrink direct `@`-use in user-facing examples; keep host surface tiny - ✅ examples use std names + `call`

Also fixed for the gate: multi-module string data (`yarrow.str.N` dedupe), parse-time `drop` no longer discarding calls, nested function compilation, docs/`call` forms for `mem.*` and region binding.

**Gate:** examples `08`, `09`, `11`, `12` plus grammar snippets that `require` these modules compile against the new names.

---

### Stage 6: Ownership, regions, unsafe conformance ✅

Finish compile-time checks; keep raw pointer validity programmer-owned inside `unsafe`.

1. Use-after-move → error (`invalid/01`) - ✅ `moved_vars` + `E373`
2. Mutate / consume while borrowed → error (`invalid/02`) - ✅ mutating builtins / `set` + `E374`
3. Pop/drop owner while borrowed → error (`invalid/03`) - ✅ `consume` / `emit_drop` + `E374`
4. Second overlapping borrow → error (`invalid/07`) - ✅ `E375`
5. Region escape → error - ✅ free while borrow live / use after free (`invalid/11`, `E376`)
6. Unsafe call / op outside `unsafe` → `E370` (`invalid/04`) - ✅
7. `pointer<T>` path matches grammar (`valid/11`) - ✅

Also: `if` / conditional `for` require `bool` (`invalid/05`).

**Gate:** all `docs/examples/invalid/*.yar` fail as annotated; all `valid/*` that exercise memory succeed.

---

### Stage 7: Full grammar-surface conformance ✅

1. Compile and run the illustrative program in `docs/GRAMMAR.md` - ✅ twin at `docs/examples/valid/00_grammar_tour.yar`
2. Close remaining `run_main` / driver gaps needed for demos - ✅ void/`i*`/`f*`/bool/string/`|T Err|` main
3. Module resolution edge cases (alias vs bare require, item import) per `RUNTIME.md` - ✅ (`12_modules.yar`)
4. Remove dead compatibility paths - ✅ language `break`/`continue` removed (`std.loop` only); legacy `for` binders removed; comments updated off `with T or Error`

Also fixed for the gate: method-call borrow release; else-only `match` CFG; soft `error.TAG` cases inside `handle`; `std.fs.open_file` stub success; item import after aliased require.

**Gate:** grammar tour + full `docs/examples/valid/**` run; `cargo check` / `clippy` green.

---

## Phase B: Compiler UX and depth

Language surface from Phase A is the baseline. Phase B improves how the compiler explains failures and how resilient the front end is. Target presentation is **full rustc-style** (not a single-line summary): primary span, secondary labels, source snippets with underlines, and `note` / `help` text.

Stage 8–10 landed rustc-style diagnostics, teachable notes, and multi-error recovery.

### Stage 8: Diagnostic infrastructure (full rustc-style) ✅

Build the plumbing so every failure can render like rustc from day one (multi-label + notes), not a minimal “file:line” stub.

1. **`Span`** - byte range (and cached line/column) on tokens; `Location` kept as the token start
2. **AST spans** - `Stmt { kind, span }`; operand-stack coverage for expression statements
3. **`Diagnostic`** - `code`, `message`, primary span, `labels[]` (span + message), `notes[]`, `helps[]`
4. **Renderer** - rustc layout: `error[E…]: …`, `--> path:line:col`, source line(s), `^` / `-` underlines, `= note:` / `= help:`
5. **Source map** - `SourceFile` with line index; driver holds path + source
6. **Kill `Location::default()`** - compile errors use `st.current_span` / item spans (remaining `Span::default()` only where no source construct exists, e.g. JIT init)
7. **Wire driver** - `src/main.rs` prints via the renderer; respects `NO_COLOR`

**Gate:** all `docs/examples/invalid/*` render with a primary underline on the annotated region (not `1:1`); at least one diagnostic shows a **secondary** label (e.g. move site + use site); `cargo check` / `clippy` green. ✅

---

### Stage 9: Teachable error catalog ✅

Make high-traffic codes explain the language model the way rustc teaches ownership.

| Family               | Codes (approx) | Labels / notes                                                    |
| -------------------- | -------------- | ----------------------------------------------------------------- |
| Ownership / borrow   | E373–E376      | Secondary at move/borrow site; note + help                        |
| Unsafe               | E370           | Point at call; help to wrap in `unsafe` / `unsafe function`       |
| Types / stack        | E309, E324     | Expected vs found; bool-condition help                            |
| Modules / visibility | E381           | Suggest `public`                                                  |
| Fallible / unions    | E308           | Unwrap needs `\|T Err\|` / suggest `handle`; union members listed |

Also landed:

1. Per-op spans in `Expr::Seq` so primary underlines hit the bad word near `# ERROR:`
2. `borrow_sites` tracking for secondary labels on E374/E375/E376
3. Optional `--explain` deferred

**Gate:** each invalid example’s primary span hits the `# ERROR:` line region; ownership/unsafe/fallible cases include a useful `note` or `help`; explain path optional but codes remain stable. ✅

---

### Stage 10: Multi-error reporting and recovery ✅

1. **Parser recovery** - ✅ on statement errors, sync to the next boundary and keep parsing; return a `DiagnosticBatch`
2. **Compiler collection** - ✅ accumulate diagnostics across independent statements and function bodies; abandon IR for failed bodies
3. **Output cap** - ✅ stop after `DEFAULT_ERROR_LIMIT` (20); driver prints an abort line when capped

**Gate:** `invalid/12_multi_error.yar` and `invalid/13_multi_parse.yar` each report ≥2 distinct diagnostics; valid corpus still clean. ✅

---

### After Stage 10 (backlog, not scheduled)

Not required to close Phase B; pick when diagnostics feel solid:

- Stack-effect dumps on underflow / join mismatch (`[i32, string]` vs expected)
- `--emit-ir` / richer `YARROW_DBG_IR` with source comments
- Deferred Stage 2 cleanups (flatten stack words; `Program.main`; struct vs hashmap `{}` by key shape)
- Literal / numeric polish (`f16` smallest-fit; clearer float `%` / `^` rejects)
- Std depth (host `std.fs` I/O; richer `io` / `string`)
- CLI: `yarrow check`, exit codes, multi-file
- AOT / object emit (only after JIT demos and diagnostics feel solid)

---

## Mapping from the old stages

| Old stage                         | Disposition                                          |
| --------------------------------- | ---------------------------------------------------- |
| 0 Restore build                   | Absorbed into Stage 0                                |
| 1 New operators / ownership seeds | Keep machinery; re-verify under Stages 3 and 6       |
| 2 Control flow                    | Keep; adjust `for` / loop helpers in Stages 2 and 5  |
| 3 Unions                          | Reopen under Stage 4 (spec changed)                  |
| 4 Unsafe / pointers               | Keep; re-gate under Stage 6                          |
| 5 Pure-Yarrow std                 | Reopen as Stage 5 with **new** API names and modules |
| 6 Remaining conformance           | Split across Stages 3–7                              |

---

## Definition of done

### Phase A (language surface)

1. Docs remain authoritative; code matches `GRAMMAR.md` / `SYNTAX.md`.
2. Tokenizer, parser, and AST expose the documented surface (`~`, `\|T U\|`, visibility, `error`, `copy`, …).
3. Compiler enforces stack types, ownership, borrow, regions, and unsafe boundaries as documented.
4. Std module names used in the grammar exist and behave.
5. `docs/examples/valid/**` compile and run; `docs/examples/invalid/**` are rejected for the stated reason.
6. No user-visible lifetime parameters.
7. `unsafe` never disables type, stack, ownership, or borrow checking.
8. `cargo fmt --all`, `cargo check`, and `cargo clippy` stay green.

### Phase B (diagnostics)

9. Failures render rustc-style: path, primary and secondary spans, source underlines, `note` / `help`.
10. Compile errors carry real spans (no systematic `Location::default()` for user-facing codes).
11. Invalid examples point at the annotated bad line; ownership/unsafe/fallible codes teach the rule briefly.
12. Multi-error runs report more than one distinct diagnostic when recovery applies (Stage 10). ✅

---

## Working rules

- Prefer minimal diffs that pass the **current** stage gate.
- When renaming std or syntax, update compiler and `lib/std` in the same stage; do not leave the grammar examples stranded.
- Do not add tests unless explicitly asked (`AGENTS.md`); use example programs as gates instead.
- Update this file’s stage checkboxes and pipeline table when a gate lands.
- For Phase B, prefer improving diagnostic quality over new language features unless the docs change first.
