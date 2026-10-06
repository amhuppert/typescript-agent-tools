#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_WORKERS=2
readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"
export RUST_TEST_THREADS="$TEST_WORKERS"

# Without explicit targets, source changes can affect the entire suite.
if (( $# == 0 )); then
  run_rust_tests "$TEST_WORKERS"
  exit 0
fi

test_targets=()
for file in "$@"; do
  if [[ "$file" =~ ^tests/([A-Za-z0-9_-]+)\.rs$ ]] && [[ -f "$file" ]]; then
    test_targets+=(--test "${BASH_REMATCH[1]}")
  else
    printf 'validation: expected an existing integration-test file tests/<target>.rs; got %s\n' "$file" >&2
    exit 2
  fi
done

run_rust_tests "$TEST_WORKERS" "${test_targets[@]}"
