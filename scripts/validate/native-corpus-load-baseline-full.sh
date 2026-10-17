#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

readonly NODE_HEAP_MB=512
export NODE_OPTIONS="--max-old-space-size=${NODE_HEAP_MB}"
export TSPARSE="${CARGO_TARGET_DIR:-target}/release/tsparse"
run_quiet node tools/native-corpus-load-baseline.mjs
