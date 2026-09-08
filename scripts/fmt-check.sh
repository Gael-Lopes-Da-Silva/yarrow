#!/usr/bin/env bash
# Gate: yarrow fmt --check on the always-green corpus (Stage 16).
# Paths: docs/examples/valid and crates/yarrow-core/lib/std.
# Does not format docs/examples/invalid (parse failures expected).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

exec cargo run -q -p yarrow -- fmt --check \
	docs/examples/valid \
	crates/yarrow-core/lib/std
