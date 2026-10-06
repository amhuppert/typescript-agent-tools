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

# Compiler oracles. TYPE_SCRIPT_API is the TypeScript 6.0.3 syntax API
# (lib/typescript.js) behind the AST runners. TYPE_SCRIPT_SEMANTIC_API and
# TYPE_SCRIPT_NATIVE are the TypeScript 7.0.2 semantic API package and native
# compiler behind the reference oracle. A variable that is already set wins;
# an unset one is looked up in the canonical checkout's .cache/compiler-oracles,
# found from any linked worktree. Each find_* function exports what it finds
# and returns 0, or returns 1 with the reason in compiler_missing.
compiler_missing=""

compiler_cache() {
  local common_dir
  common_dir="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 1
  [[ -n "$common_dir" ]] || return 1
  printf '%s/.cache/compiler-oracles\n' "$(dirname "$common_dir")"
}

find_syntax_api() {
  local cache api
  [[ -z "${TYPE_SCRIPT_API:-}" ]] || return 0
  if ! cache="$(compiler_cache)"; then
    compiler_missing="TYPE_SCRIPT_API is unset and no Git checkout was found"
    return 1
  fi
  api="$cache/typescript-6.0.3/package/lib/typescript.js"
  if [[ ! -f "$api" ]]; then
    compiler_missing="no syntax API at $api"
    return 1
  fi
  export TYPE_SCRIPT_API="$api"
}

find_semantic_compilers() {
  local version=7.0.2 cache platform candidates
  local api="${TYPE_SCRIPT_SEMANTIC_API:-}" native="${TYPE_SCRIPT_NATIVE:-}"
  if [[ -z "$api" || -z "$native" ]]; then
    if ! cache="$(compiler_cache)"; then
      compiler_missing="TYPE_SCRIPT_SEMANTIC_API or TYPE_SCRIPT_NATIVE is unset and no Git checkout was found"
      return 1
    fi
    if [[ -z "$api" ]]; then
      api="$cache/typescript-$version/package"
      if [[ ! -f "$api/package.json" ]]; then
        compiler_missing="no semantic API package at $api"
        return 1
      fi
    fi
    if [[ -z "$native" ]]; then
      platform="$(node -p 'process.platform + "-" + process.arch')"
      native="$cache/typescript-$platform-$version/package/lib/tsc"
      if [[ ! -x "$native" ]]; then
        shopt -s nullglob
        candidates=("$cache"/typescript-*-"$version"/package/lib/tsc)
        shopt -u nullglob
        if (( ${#candidates[@]} == 1 )); then
          native="${candidates[0]}"
        elif (( ${#candidates[@]} == 0 )); then
          compiler_missing="no native compiler at $native"
          return 1
        else
          compiler_missing="no native compiler for $platform at $native, and several others: ${candidates[*]}"
          return 1
        fi
      fi
    fi
  fi
  export TYPE_SCRIPT_SEMANTIC_API="$api" TYPE_SCRIPT_NATIVE="$native"
}

# Run Rust tests on WORKERS jobs and threads: run_rust_tests WORKERS [CARGO ARG...]
# A test that cannot run on this platform, such as a permission test run as
# the superuser, passes after printing a `SKIP name: reason` line. Failure
# replays the complete output; success prints one line naming any skips.
run_rust_tests() {
  local workers="$1" output status=0 skips count
  shift
  output="$(mktemp)"
  cargo test --offline --jobs "$workers" "$@" -- --test-threads="$workers" --show-output \
    >"$output" 2>&1 || status=$?
  if (( status != 0 )); then
    cat "$output" >&2
    rm -f "$output"
    return "$status"
  fi
  skips="$(grep -E '^SKIP ' "$output" || true)"
  rm -f "$output"
  if [[ -z "$skips" ]]; then
    printf 'rust tests: passed, 0 skipped\n'
  else
    count="$(printf '%s\n' "$skips" | wc -l | tr -d ' ')"
    printf 'rust tests: passed, %s skipped on this platform:\n%s\n' "$count" "$skips"
  fi
}

# Run Node harness test files on WORKERS workers: run_harness_tests WORKERS FILE...
# Compilers come from find_*; a test whose compiler is absent skips with its
# reason. Success prints one line of counts and failure replays the complete
# spec output. HARNESS_REQUIRE_COMPILERS=1 is strict mode: every compiler must
# be found (exit 2 before any test runs otherwise) and any skip fails the run.
# A run in which no test executed fails in either mode.
run_harness_tests() {
  local workers="$1" strict=0 output summary status=0 line
  local tests executed passed skipped todo skips
  local reporter
  shift
  reporter="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../tools" && pwd)/test-summary-reporter.mjs"
  case "${HARNESS_REQUIRE_COMPILERS:-}" in
    "" | 0) ;;
    1) strict=1 ;;
    *)
      printf 'harness tests: HARNESS_REQUIRE_COMPILERS must be 1, 0, or unset; got %s\n' "$HARNESS_REQUIRE_COMPILERS" >&2
      return 2 ;;
  esac
  local missing=()
  find_syntax_api || missing+=("$compiler_missing")
  find_semantic_compilers || missing+=("$compiler_missing")
  if (( strict && ${#missing[@]} > 0 )); then
    printf 'harness tests: strict mode (HARNESS_REQUIRE_COMPILERS=1) requires every compiler; no tests ran.\n' >&2
    printf 'harness tests: %s\n' "${missing[@]}" >&2
    return 2
  fi

  output="$(mktemp)"
  summary="$(mktemp)"
  node --test --test-concurrency="$workers" \
    --test-reporter=spec --test-reporter-destination=stdout \
    --test-reporter="$reporter" --test-reporter-destination="$summary" \
    -- "$@" >"$output" 2>&1 || status=$?
  if (( status != 0 )); then
    cat "$output" >&2
    rm -f "$output" "$summary"
    return "$status"
  fi
  if ! read -r tests executed passed skipped todo <"$summary"; then
    printf 'harness tests: the test runner exited 0 without a summary\n' >&2
    cat "$output" >&2
    rm -f "$output" "$summary"
    return 1
  fi
  skips="$(tail -n +2 "$summary")"
  rm -f "$output" "$summary"

  line="harness tests: $tests total, $executed executed, $passed passed, $skipped skipped"
  if (( todo > 0 )); then line+=", $todo todo"; fi
  if (( executed == 0 )); then
    printf '%s; no test executed\n' "$line" >&2
    [[ -z "$skips" ]] || printf '%s\n' "$skips" >&2
    return 1
  fi
  if [[ -n "$skips" ]]; then
    if (( strict )); then
      printf '%s; strict mode (HARNESS_REQUIRE_COMPILERS=1) fails on skips:\n%s\n' "$line" "$skips" >&2
      return 1
    fi
    line+=" (HARNESS_REQUIRE_COMPILERS=1 fails on skips)"
  fi
  printf '%s\n' "$line"
}
