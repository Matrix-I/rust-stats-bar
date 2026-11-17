#!/usr/bin/env bash
# The single verification entry point: formatting, lints, tests and the purity gate.
# CI and the "before reporting done" gate both run exactly this.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-core-purity.sh
