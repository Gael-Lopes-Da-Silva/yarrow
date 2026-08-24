# Examples

Language illustrations for Yarrow. They follow [`GRAMMAR.md`](../GRAMMAR.md) and
[`SYNTAX.md`](../SYNTAX.md), not the current compiler snapshot. The implementation
is still catching up; treat these as the intended language shape.

```
docs/examples
├── valid/      # well-formed programs
│   └── helpers/
└── invalid/    # should be rejected by a conforming checker / compiler
```

## How to read them

- Each `.yar` file is a small, focused program with a short header comment.
- `valid/` shows idiomatic use of a feature.
- `invalid/` states the rule being broken in a `# ERROR:` comment near the bad line.
- Prefer the grammar when an example and the unfinished compiler disagree.

## Valid programs

| File                                                                       | Topic                                                                      |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| [`valid/00_grammar_tour.yar`](valid/00_grammar_tour.yar)             | Full illustrative program (twin of [`GRAMMAR.md`](../GRAMMAR.md)) |
| [`valid/01_hello.yar`](valid/01_hello.yar)                                 | Entry point, `require`, `io.write_line`                                    |
| [`valid/02_arithmetic_and_stack.yar`](valid/02_arithmetic_and_stack.yar)   | Operators, `dup` / `swap` / `rot`, `drop`                                  |
| [`valid/03_variables_and_typeof.yar`](valid/03_variables_and_typeof.yar)   | `mutable` / `const` / `static`, coercion, `typeof`                         |
| [`valid/04_functions.yar`](valid/04_functions.yar)                         | Nested function, `copy` parameter, `call`                                  |
| [`valid/05_control_flow.yar`](valid/05_control_flow.yar)                   | `if`, value `match`, `for` (condition and iterable)                        |
| [`valid/06_structs_and_enums.yar`](valid/06_structs_and_enums.yar)         | Struct, `implement`, `borrow`, enum `match`                                |
| [`valid/07_unions.yar`](valid/07_unions.yar)                               | Union store and `Type case` match                                          |
| [`valid/08_ownership_borrow_move.yar`](valid/08_ownership_borrow_move.yar) | Stack/variable ownership, `borrow`, `move`                                 |
| [`valid/09_regions_and_defer.yar`](valid/09_regions_and_defer.yar)         | `region.put` / `free`, `defer`                                             |
| [`valid/10_errors.yar`](valid/10_errors.yar)                               | `error`, `\|T Err\|`, `unwrap`, `handle`                                   |
| [`valid/11_unsafe_pointers.yar`](valid/11_unsafe_pointers.yar)             | `unsafe function`, `pointer<T>`, `std.mem`                                 |
| [`valid/12_modules.yar`](valid/12_modules.yar)                             | Aliased / bare / item `require` ([`helpers/greet.yar`](valid/helpers/greet.yar)) |
| [`valid/13_containers.yar`](valid/13_containers.yar)                       | Array, list, hashmap literals                                              |

## Invalid programs

| File                                                                                     | Expected rejection                 |
| ---------------------------------------------------------------------------------------- | ---------------------------------- |
| [`invalid/01_use_after_move.yar`](invalid/01_use_after_move.yar)                         | Use after `move`                   |
| [`invalid/02_mutate_while_borrowed.yar`](invalid/02_mutate_while_borrowed.yar)           | Mutate while borrowed              |
| [`invalid/03_pop_owner_while_borrowed.yar`](invalid/03_pop_owner_while_borrowed.yar)     | Drop owner while borrow live       |
| [`invalid/04_unsafe_call_outside_block.yar`](invalid/04_unsafe_call_outside_block.yar)   | Unsafe call outside `unsafe`       |
| [`invalid/05_if_non_bool.yar`](invalid/05_if_non_bool.yar)                               | Non-bool `if` condition            |
| [`invalid/06_bad_coercion.yar`](invalid/06_bad_coercion.yar)                             | Illegal declaration coercion       |
| [`invalid/07_second_borrow.yar`](invalid/07_second_borrow.yar)                           | Second overlapping borrow          |
| [`invalid/08_union_bad_case.yar`](invalid/08_union_bad_case.yar)                         | Union case not a member type       |
| [`invalid/09_unwrap_non_fallible_caller.yar`](invalid/09_unwrap_non_fallible_caller.yar) | `unwrap` where caller cannot error |
| [`invalid/10_missing_main.yar`](invalid/10_missing_main.yar)                             | Missing `main`                     |
| [`invalid/11_region_escape.yar`](invalid/11_region_escape.yar)                           | Free region while borrow live      |
| [`invalid/12_multi_error.yar`](invalid/12_multi_error.yar)                               | ≥2 compile diagnostics in one run  |
| [`invalid/13_multi_parse.yar`](invalid/13_multi_parse.yar)                               | ≥2 parse diagnostics in one run    |

## Related docs

- [`GRAMMAR.md`](../GRAMMAR.md) - full annotated tour
- [`STYLE_GUIDE.md`](../STYLE_GUIDE.md) - formatting and naming
- [`TYPE_SYSTEM.md`](../TYPE_SYSTEM.md) - types, coercion, checking
- [`MEMORY_MODEL.md`](../MEMORY_MODEL.md) - ownership, borrow, regions, unsafe
- [`RUNTIME.md`](../RUNTIME.md) - stack, calls, errors, modules
