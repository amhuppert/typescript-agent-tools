#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_WORKERS=2
readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"
export RUST_TEST_THREADS="$TEST_WORKERS"

run_quiet cargo test --offline --jobs "$TEST_WORKERS" -- --test-threads="$TEST_WORKERS"
