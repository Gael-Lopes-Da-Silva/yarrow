# Multi-root project fixture (Stage 28 / CLI Stage 13 / LSP Stage 21).
#
# Product shape: an explicit set of root `.yar` files sharing module search
# paths. No manifest or package-manager syntax. Each root is a normal
# compilation unit; `require` works as in RUNTIME Modules.
#
# CLI:
#
# ```text
# yarrow check docs/examples/project/root_a.yar docs/examples/project/root_b.yar
# ```
#
# Library API:
#
# ```rust
# use yarrow_core::{ProjectOptions, check_project};
# let mut opts = ProjectOptions::from_root_paths([
#     "docs/examples/project/root_a.yar",
#     "docs/examples/project/root_b.yar",
# ])?;
# // parent dirs of each root are search paths; shared/ resolves as shared.util
# let checked = check_project(&opts)?;
# assert_eq!(checked.roots.len(), 2);
# assert!(checked.graph.modules.iter().any(|m| m == "shared.util"));
# ```
#
# LSP: pass the same paths in `initializationOptions.projectRoots` (see
# `crates/yarrow-lsp/scripts/harness.mjs` scenarios `project-roots` /
# `project-missing-root`).
#
# | File | Role |
# | --- | --- |
# | [`root_a.yar`](root_a.yar) | Root A |
# | [`root_b.yar`](root_b.yar) | Root B (no require of A) |
# | [`shared/util.yar`](shared/util.yar) | Shared helper via `"shared.util"` |
