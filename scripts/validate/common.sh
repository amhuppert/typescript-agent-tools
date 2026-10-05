#!/usr/bin/env bash
set -euo pipefail

export FORCE_COLOR=0
export NO_COLOR=1

run_quiet() {
  local output_file status
  output_file="$(mktemp)"
  if "$@" >"$output_file" 2>&1; then
    rm -f "$output_file"
    return 0
  else
    status=$?
  fi
  cat "$output_file" >&2
  rm -f "$output_file"
  return "$status"
}
