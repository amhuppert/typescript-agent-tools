#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

target_branch="${TARGET_BRANCH:-main}"
merge_base="$(git merge-base "$target_branch" HEAD 2>/dev/null || true)"
if [[ -z "$merge_base" ]]; then
  run_quiet cargo fmt --all -- --check --color never
  exit 0
fi

rust_files=()
full_check=0
while IFS= read -r -d '' file; do
  case "$file" in
    Cargo.toml | */Cargo.toml | rustfmt.toml | .rustfmt.toml | .cargo/config.toml | vendor/*)
      full_check=1 ;;
    src/*.rs | tests/*.rs | examples/*.rs | benches/*.rs)
      if [[ -f "$file" ]]; then rust_files+=("$file"); fi ;;
    *.rs) full_check=1 ;;
  esac
done < <(
  git diff --name-only -z --diff-filter=ACMR "$merge_base" --
  git ls-files -z --others --exclude-standard
)

if (( full_check )); then
  run_quiet cargo fmt --all -- --check --color never
elif (( ${#rust_files[@]} > 0 )); then
  run_quiet rustfmt --check --edition 2024 --config skip_children=true --color never "${rust_files[@]}"
fi
