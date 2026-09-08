# Multi-root project fixture (Stage 28).
#
# Product shape: an explicit set of root `.yar` files sharing module search
# paths. No manifest or package-manager syntax. Each root is a normal
# compilation unit; `require` works as in RUNTIME Modules.
#
# Check with the library API (no CLI subcommand in this stage):
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
# | File | Role |
# | --- | --- |
# | [`root_a.yar`](root_a.yar) | Root A |
# | [`root_b.yar`](root_b.yar) | Root B (no require of A) |
# | [`shared/util.yar`](shared/util.yar) | Shared helper via `"shared.util"` |
