#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly TEST_WORKERS=2
readonly TEST_HEAP_MB=2048
export NODE_OPTIONS="--max-old-space-size=${TEST_HEAP_MB}"

if (( $# == 0 )); then
  run_quiet node --test --test-concurrency="$TEST_WORKERS" tools/*.test.mjs
else
  workers=$#
  if (( workers > TEST_WORKERS )); then
    workers=$TEST_WORKERS
  fi
  run_quiet node --test --test-concurrency="$workers" -- "$@"
fi
