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

# A module filter applies to every selected Cargo binary. Admit this one
# explicit file only on its own, so mixed paths cannot silently lose tests.
if (( $# == 1 )) && [[ "$1" == src/project/semantic/resources_tests.rs ]] && [[ -f "$1" ]]; then
  run_rust_tests "$TEST_WORKERS" --require-executed --lib 'project::semantic::resources_tests::'
  exit 0
fi
if (( $# == 1 )) && [[ "$1" =~ ^src/project/semantic/resources_tests/([A-Za-z_][A-Za-z0-9_]*)\.rs$ ]] && [[ -f "$1" ]]; then
  resource_module="${BASH_REMATCH[1]}"
  run_rust_tests "$TEST_WORKERS" --require-executed --lib "project::semantic::resources_tests::${resource_module}::"
  exit 0
fi

test_targets=()
for file in "$@"; do
  if [[ "$file" == src/lib.rs ]] && [[ -f "$file" ]]; then
    test_targets+=(--lib)
  elif [[ "$file" == src/main.rs ]] && [[ -f "$file" ]]; then
    test_targets+=(--bin tsparse)
  elif [[ "$file" =~ ^tests/([A-Za-z0-9_-]+)\.rs$ ]] && [[ -f "$file" ]]; then
    test_targets+=(--test "${BASH_REMATCH[1]}")
  else
    printf 'validation: expected existing src/lib.rs, src/main.rs or integration-test file tests/<target>.rs; got %s\n' "$file" >&2
    exit 2
  fi
done

run_rust_tests "$TEST_WORKERS" "${test_targets[@]}"
