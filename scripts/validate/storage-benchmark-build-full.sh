#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

# Explicit targets avoid the implicit release-mode library test harness that
# `--benches` adds. This profile measures project storage and CLI journeys.
run_quiet cargo build --offline --release --jobs 2 --bin tsparse --bench references
