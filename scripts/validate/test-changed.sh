#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"

# Without explicit targets, run what the changes since the last passing run
# can affect (select_rust_tests), and record a pass for the next selection.
# An explicit target proves nothing about other changes, so it records none.
if (( $# == 0 )); then
  select_rust_tests
  case "$RUST_SELECTION" in
    none) printf 'rust tests: %s\n' "$RUST_SELECTION_REASON" ;;
    every)
      printf 'rust tests: every target (%s)\n' "$RUST_SELECTION_REASON"
      run_rust_tests ;;
    targets)
      printf 'rust tests: selected %s (%s)\n' "${RUST_SELECTED[*]}" "$RUST_SELECTION_REASON"
      run_rust_tests "${RUST_SELECTED[@]}" ;;
  esac
  record_rust_tests_passed "$RUST_TREE"
  exit 0
fi

# A module filter applies to every selected Cargo binary. Admit each
# explicit module only on its own, so mixed paths cannot silently lose tests.
if (( $# == 1 )) && [[ "$1" == src/project/program/resolve/reuse_tests.rs ]] && [[ -f "$1" ]]; then
  run_rust_tests --require-executed --filter 'project::program::resolve::reuse_tests::' --lib
  exit 0
fi
if (( $# == 1 )) && [[ "$1" == src/project/semantic/resources_tests.rs ]] && [[ -f "$1" ]]; then
  run_rust_tests --require-executed --filter 'project::semantic::resources_tests::' --lib
  exit 0
fi
if (( $# == 1 )) && [[ "$1" =~ ^src/project/semantic/resources_tests/([A-Za-z_][A-Za-z0-9_]*)\.rs$ ]] && [[ -f "$1" ]]; then
  resource_module="${BASH_REMATCH[1]}"
  run_rust_tests --require-executed --filter "project::semantic::resources_tests::${resource_module}::" --lib
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

run_rust_tests "${test_targets[@]}"
