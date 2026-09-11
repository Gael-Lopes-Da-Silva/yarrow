#!/usr/bin/env bash
# Gate: yarrow fmt --check on the always-green corpus (Stage 16, widened Stage 21).
# Paths: docs/examples/valid, warnings, project; crates/yarrow-core/lib/std;
# crates/yarrow-fmt/fixtures/stage19_ignore_regions.yar.
# Excludes docs/examples/invalid (parse failures expected) and stage18 selective-
# reprint fixture (intentional incomplete parse for best-effort gates).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

exec cargo run -q -p yarrow -- fmt --check \
	docs/examples/valid \
	docs/examples/warnings \
	docs/examples/project \
	crates/yarrow-core/lib/std \
	crates/yarrow-fmt/fixtures/stage19_ignore_regions.yar
