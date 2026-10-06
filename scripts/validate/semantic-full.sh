#!/usr/bin/env bash
# Check fixtures/references against the pinned TypeScript 7.0.2 semantic
# oracle: marker agreement, then the Rust tool's answers to every position
# and name query. Compilers come from TYPE_SCRIPT_SEMANTIC_API and
# TYPE_SCRIPT_NATIVE, or else from the canonical checkout's
# .cache/compiler-oracles. Exit 2 means the oracle was not found and nothing
# was checked; that is never a pass.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly BUILD_JOBS=2
readonly NODE_HEAP_MB=2048
readonly VERSION=7.0.2
export NODE_OPTIONS="--max-old-space-size=${NODE_HEAP_MB}"

unavailable() {
  printf 'semantic oracle: %s\n' "$1" >&2
  printf 'semantic oracle: the TypeScript %s comparison did not run. Set TYPE_SCRIPT_SEMANTIC_API and TYPE_SCRIPT_NATIVE, or install the compilers under .cache/compiler-oracles in the canonical checkout.\n' "$VERSION" >&2
  exit 2
}

find_semantic_compilers || unavailable "$compiler_missing"
export REFERENCE_BATCH="${CARGO_TARGET_DIR:-target}/debug/examples/reference_batch"

run_quiet cargo build --offline --jobs "$BUILD_JOBS" --example reference_batch
run_quiet node tools/semantic-conformance.mjs markers
run_quiet node tools/semantic-conformance.mjs compare
