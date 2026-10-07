#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

run_quiet cargo build --offline --release --jobs 2 --bins --examples --benches
