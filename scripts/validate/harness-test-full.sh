#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_WORKERS=2
readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"

# A passing run is recorded, so the changed wrapper's next selection starts
# from this tree.
tree="$(working_tree || true)"
fingerprint="$(harness_toolchain)"
run_harness_tests "$TEST_WORKERS" tools/*.test.mjs
record_pass harness-tests "$tree" "$fingerprint"
