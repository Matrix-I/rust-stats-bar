#!/usr/bin/env bash
# The single verification entry point: formatting, lints, tests and the purity gate.
# CI and the "before reporting done" gate both run exactly this.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Without --all, cargo fmt checks the workspace members only. --all would also check local
# path dependencies, such as the vendored ksni, which keeps upstream's formatting.
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-core-purity.sh
./scripts/check-extension.sh
./scripts/test-install-extension.sh
