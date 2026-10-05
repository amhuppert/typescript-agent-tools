#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

run_quiet cargo clippy --offline --jobs 2 --all-targets --no-deps -- -D warnings
