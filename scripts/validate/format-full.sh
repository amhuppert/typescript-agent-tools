#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

run_quiet --label format cargo fmt --all -- --check --color never
