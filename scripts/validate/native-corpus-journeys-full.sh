#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly BUILD_JOBS=2
readonly NODE_HEAP_MB=512
export NODE_OPTIONS="--max-old-space-size=${NODE_HEAP_MB}"
export TSPARSE="${CARGO_TARGET_DIR:-target}/release/tsparse"

node tools/native-corpus-journeys.mjs --capture-build-inputs
run_quiet cargo build --offline --release --jobs "$BUILD_JOBS" --bin tsparse
status=0
run_quiet node tools/native-corpus-journeys.mjs --resources || status=$?
run_quiet node tools/native-corpus-journeys.mjs --resources --service || status=$?
exit "$status"
