#!/usr/bin/env bash
# Links the tracked hooks into .git/hooks without replacing the Git LFS hooks there.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
hooks_dir="$(git rev-parse --git-path hooks)"
ln -sf "$root/scripts/hooks/pre-commit" "$hooks_dir/pre-commit"
echo "installed pre-commit -> scripts/hooks/pre-commit"
