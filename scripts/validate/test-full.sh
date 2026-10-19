#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"

# A passing run is recorded, so the changed wrapper's next selection starts
# from this tree.
tree="$(working_tree || true)"
fingerprint="$(rust_toolchain)"
run_rust_tests
record_pass rust-tests "$tree" "$fingerprint"
