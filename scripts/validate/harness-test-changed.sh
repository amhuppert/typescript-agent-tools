#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_WORKERS=2
readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"

# Without explicit test files, changes can affect the entire harness suite.
if (( $# == 0 )); then
  run_harness_tests "$TEST_WORKERS" tools/*.test.mjs
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
