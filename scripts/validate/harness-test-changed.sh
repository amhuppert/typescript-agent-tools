#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_WORKERS=2
readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"

# Without explicit test files, run what the changes since the last passing
# run can affect (select_harness_tests), and record a pass for the next
# selection. Explicit files prove nothing about other changes and record none.
if (( $# == 0 )); then
  select_harness_tests
  case "$HARNESS_SELECTION" in
    none) printf 'harness tests: %s\n' "$HARNESS_SELECTION_REASON" ;;
    every) run_harness_tests "$TEST_WORKERS" tools/*.test.mjs ;;
    files)
      printf 'harness tests: selected %s of %s files (%s)\n' "${#HARNESS_SELECTED[@]}" \
        "$(ls tools/*.test.mjs | wc -l | tr -d ' ')" "$HARNESS_SELECTION_REASON"
      workers=${#HARNESS_SELECTED[@]}
      if (( workers > TEST_WORKERS )); then workers=$TEST_WORKERS; fi
      run_harness_tests "$workers" "${HARNESS_SELECTED[@]}" ;;
  esac
  record_pass harness-tests "$HARNESS_TREE" "$HARNESS_FINGERPRINT"
  exit 0
fi

# Only harness test files are run: a runner such as tools/robustness-corpus.mjs
# would execute as a corpus tool, and another module would pass trivially.
test_files=()
for file in "$@"; do
  if [[ "$file" =~ ^tools/[A-Za-z0-9_-]+\.test\.mjs$ ]] && [[ -f "$file" ]]; then
    test_files+=("$file")
  else
    printf 'validation: expected an existing harness test file tools/<name>.test.mjs; got %s\n' "$file" >&2
    exit 2
  fi
done

workers=${#test_files[@]}
if (( workers > TEST_WORKERS )); then
  workers=$TEST_WORKERS
fi
run_harness_tests "$workers" "${test_files[@]}"
