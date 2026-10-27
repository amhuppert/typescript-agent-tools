#!/usr/bin/env bash
set -euo pipefail

export FORCE_COLOR=0
export NO_COLOR=1

# Run a command, hiding its output unless it fails, when its output goes to
# stderr and its exit status is returned: run_quiet [--label LABEL] COMMAND...
# Given LABEL, a success prints the one-line verdict `LABEL: ok`.
run_quiet() {
  local output_file status label=""
  if [[ "${1:-}" == --label ]]; then
    label="$2"
    shift 2
  fi
  output_file="$(mktemp)"
  if "$@" >"$output_file" 2>&1; then
    rm -f "$output_file"
    if [[ -n "$label" ]]; then printf '%s: ok\n' "$label"; fi
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
# compiler behind the reference oracle. TYPE_SCRIPT_56_API is the TypeScript
# 5.6.3 package behind the TypeScript 5.6 dialect's tools (tools/ts56-*.mjs).
# A variable that is already set wins; an unset one is looked up in the
# canonical checkout's .cache/compiler-oracles, found from any linked
# worktree. Each find_* function exports what it finds and returns 0, or
# returns 1 with the reason in compiler_missing.
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

find_typescript_56() {
  local cache package="${TYPE_SCRIPT_56_API:-}"
  if [[ -z "$package" ]]; then
    if ! cache="$(compiler_cache)"; then
      compiler_missing="TYPE_SCRIPT_56_API is unset and no Git checkout was found"
      return 1
    fi
    package="$cache/typescript-5.6.3/package"
  fi
  if [[ ! -f "$package/package.json" ]]; then
    compiler_missing="no TypeScript 5.6.3 package at $package"
    return 1
  fi
  export TYPE_SCRIPT_56_API="$package"
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

# The resource profile of Rust test validation, which the test command's
# cost in CommandCenter.json reflects: build jobs, test executables run at
# once, and test threads in each.
export RUST_BUILD_JOBS=4 RUST_TEST_EXECUTABLES=4 RUST_TEST_THREADS=2

# The merge base of HEAD and the target branch (TARGET_BRANCH, default
# main) that changed-scope checks compare against. Fails without one.
target_merge_base() {
  local base
  base="$(git merge-base "${TARGET_BRANCH:-main}" HEAD 2>/dev/null)" || return 1
  [[ -n "$base" ]] || return 1
  printf '%s\n' "$base"
}

# The tree of every file in the working tree that Git does not ignore,
# committed or not, written to the object database through a copy of the
# index, so the index itself is untouched. The copy keeps the index's time:
# Git rehashes an entry no older than its index, so a file changed in the
# second the index was written, at the same size, still counts as changed.
# Fails outside a Git checkout.
working_tree() {
  local real index status=0
  real="$(git rev-parse --git-path index 2>/dev/null)" || return 1
  index="$(mktemp)" || return 1
  if [[ -f "$real" ]]; then
    cp -p "$real" "$index" || { rm -f "$index"; return 1; }
  else
    rm -f "$index"
  fi
  # A build directory that Git does not ignore would be hashed as source.
  # Naming an ignored one in a pathspec makes `git add` fail, so only an
  # unignored one is excluded.
  local skip=() target top relative
  top="$(git rev-parse --show-toplevel 2>/dev/null)" || { rm -f "$index"; return 1; }
  top="$(cd "$top" && pwd -P)"
  target="$(cd "${CARGO_TARGET_DIR:-target}" 2>/dev/null && pwd -P)" || target=""
  if [[ -n "$target" && "$target/" == "$top/"* ]]; then
    relative="${target#"$top/"}"
    if ! git check-ignore -q -- "$relative/" 2>/dev/null; then
      skip=(":(exclude)$relative")
    fi
  fi
  { GIT_INDEX_FILE="$index" git add -A -- . ${skip[@]+"${skip[@]}"} >/dev/null 2>&1 \
      && GIT_INDEX_FILE="$index" git write-tree; } || status=$?
  rm -f "$index"
  return "$status"
}

# Where test validation keeps its state: each Rust test executable's last
# duration, which orders the next run, and the last pass each check recorded.
rust_validation_dir() {
  printf '%s/validation\n' "${CARGO_TARGET_DIR:-target}"
}

# The toolchain a Rust test pass depended on besides the tree.
rust_toolchain() {
  rustc -Vv 2>/dev/null | cksum | cut -d ' ' -f 1
}

# The pass CHECK last recorded, as its tree and its toolchain fingerprint on
# two lines, or nothing.
recorded_pass() {
  local file
  file="$(rust_validation_dir)/$1-passed.tree"
  [[ -f "$file" ]] && cat "$file"
  return 0
}

# Record that CHECK passed for TREE under FINGERPRINT, so its next changed
# selection starts from TREE. Nothing is recorded when the working tree no
# longer holds TREE: a file that changed during the run may or may not have
# been read, so the run proves nothing about it.
record_pass() {
  local check="$1" tree="$2" fingerprint="$3" now directory
  [[ -n "$tree" ]] || return 0
  now="$(working_tree)" || return 0
  [[ "$now" == "$tree" ]] || return 0
  directory="$(rust_validation_dir)"
  mkdir -p "$directory"
  printf '%s\n%s\n' "$tree" "$fingerprint" >"$directory/$check-passed.tree.$$"
  mv "$directory/$check-passed.tree.$$" "$directory/$check-passed.tree"
}

# The base of a changed selection for CHECK under FINGERPRINT, printed as the
# base and what it is on two lines: the tree CHECK last recorded while Git
# still holds it and the fingerprint is unchanged, else the target branch's
# merge base. Prints `changed` alone when the fingerprint changed, and fails
# when there is no base.
selection_base() {
  local check="$1" fingerprint="$2" recorded base
  recorded="$(recorded_pass "$check")"
  base="$(printf '%s\n' "$recorded" | sed -n 1p)"
  if [[ -n "$base" ]] && git cat-file -e "$base^{tree}" 2>/dev/null; then
    if [[ "$(printf '%s\n' "$recorded" | sed -n 2p)" != "$fingerprint" ]]; then
      printf 'changed\n'
      return 0
    fi
    printf '%s\nthe last passing run\n' "$base"
    return 0
  fi
  base="$(target_merge_base)" || return 1
  printf '%s\n%s\n' "$base" "${TARGET_BRANCH:-main}"
}

# Whether the binary crate's own modules (src/main.rs, src/cli/,
# src/service/, src/process_limits.rs) reach only the binary's unit tests and
# the integration tests that run the binary: the library declares none of
# them, no `#[path]` reaches them or leaves its directory, and only the
# service support module runs the binary for other tests.
binary_crate_is_separate() {
  local file
  if grep -qE '^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+(cli|service|process_limits)[[:space:]]*[;{]' src/lib.rs 2>/dev/null; then
    return 1
  fi
  if grep -rhoE '#\[path[[:space:]]*=[[:space:]]*"[^"]*"' src 2>/dev/null | grep -qE '\.\.|cli/|service|process_limits|main\.rs'; then
    return 1
  fi
  for file in tests/support/*.rs; do
    [[ -f "$file" && "$file" != tests/support/service.rs ]] || continue
    if grep -q 'CARGO_BIN_EXE_' "$file"; then return 1; fi
  done
  return 0
}

# Choose the Rust test targets that the changes since the last passing run
# can affect. Sets RUST_SELECTION to `every`, `none`, or `targets`, with the
# Cargo target arguments in RUST_SELECTED; RUST_SELECTION_REASON for the
# report; RUST_TREE, the working tree selected for (empty outside Git); and
# RUST_FINGERPRINT, the toolchain, which record_pass takes once the run
# passes. The base comes from selection_base; a toolchain change since the
# recorded pass runs every target. Each changed path selects what can read it:
# - tests/NAME.rs: that test, or nothing once it is deleted;
# - the binary crate's own modules: the binary's unit tests and every test
#   that runs the binary (`CARGO_BIN_EXE_` or the service support module);
# - a file no Rust target compiles (docs/, tools/, scripts/, memory-bank/,
#   top-level documents and settings): every test, and the binary's unit
#   tests, whose source names it;
# - anything else, such as library sources, tests/support/, fixtures/,
#   vendor/, and build settings: every target.
select_rust_tests() {
  RUST_SELECTION=every
  RUST_SELECTED=()
  RUST_SELECTION_REASON="the changes since ${TARGET_BRANCH:-main} could not be determined"
  RUST_TREE=""
  local found base since list path name file mention every="" binary=0 tests=() mentions=()
  RUST_FINGERPRINT="$(rust_toolchain)"
  RUST_TREE="$(working_tree)" || { RUST_TREE=""; return 0; }
  found="$(selection_base rust-tests "$RUST_FINGERPRINT")" || return 0
  if [[ "$found" == changed ]]; then
    RUST_SELECTION_REASON="the Rust toolchain changed since the last passing run"
    return 0
  fi
  base="$(printf '%s\n' "$found" | sed -n 1p)"
  since="$(printf '%s\n' "$found" | sed -n 2p)"
  list="$(mktemp)"
  if ! git diff-tree -r --no-renames --name-only -z "$base" "$RUST_TREE" >"$list"; then
    rm -f "$list"
    return 0
  fi
  while IFS= read -r -d '' path; do
    case "$path" in
      tests/*/*) every="${every:-$path}" ;;
      tests/*.rs) if [[ -f "$path" ]]; then tests+=("$(basename "$path" .rs)"); fi ;;
      src/main.rs | src/cli/* | src/service/* | src/process_limits.rs) binary=1 ;;
      docs/* | tools/* | scripts/* | memory-bank/*) mentions+=("${path%%/*}/") ;;
      README.md | AGENTS.md | CommandCenter.json | LICENSE | .gitignore) mentions+=("$path") ;;
      *) every="${every:-$path}" ;;
    esac
  done <"$list"
  rm -f "$list"
  if [[ -z "$every" ]] && (( binary )) && ! binary_crate_is_separate; then
    every="a binary module that the library or another test helper shares"
  fi
  if [[ -n "$every" ]]; then
    RUST_SELECTION_REASON="$every changed since $since"
    return 0
  fi
  local run_binary=$binary
  if (( binary )); then
    for file in tests/*.rs; do
      [[ -f "$file" ]] || continue
      if grep -qE 'CARGO_BIN_EXE_|support::service|service::Service' "$file"; then
        tests+=("$(basename "$file" .rs)")
      fi
    done
  fi
  for mention in ${mentions[@]+"${mentions[@]}"}; do
    for file in tests/*.rs; do
      [[ -f "$file" ]] || continue
      if grep -qF -- "$mention" "$file"; then tests+=("$(basename "$file" .rs)"); fi
    done
    if grep -rqsF -- "$mention" src/main.rs src/cli src/service src/process_limits.rs; then
      run_binary=1
    fi
  done
  if (( run_binary )); then RUST_SELECTED+=(--bin tsparse); fi
  while IFS= read -r name; do
    [[ -n "$name" ]] && RUST_SELECTED+=(--test "$name")
  done < <(printf '%s\n' ${tests[@]+"${tests[@]}"} | LC_ALL=C sort -u)
  if (( ${#RUST_SELECTED[@]} == 0 )); then
    RUST_SELECTION=none
    RUST_SELECTION_REASON="no Rust test inputs changed since $since"
  else
    RUST_SELECTION=targets
    RUST_SELECTION_REASON="changes since $since"
  fi
}

# One test executable of run_rust_tests, given as INDEX<TAB>LABEL<TAB>EXECUTABLE:
# its output goes to RUST_TEST_WORK/INDEX.out, and STATUS<TAB>SECONDS to
# RUST_TEST_WORK/INDEX.status. The executable `doc` runs the doc tests.
run_rust_test_executable() {
  local index label executable started status=0
  IFS=$'\t' read -r index label executable <<<"$1"
  started=$SECONDS
  if [[ "$executable" == doc ]]; then
    cargo test --offline --jobs "$RUST_BUILD_JOBS" --doc -- --test-threads="$RUST_TEST_THREADS" --show-output \
      </dev/null >"$RUST_TEST_WORK/$index.out" 2>&1 || status=$?
  else
    "$executable" --test-threads="$RUST_TEST_THREADS" --show-output ${RUST_TEST_FILTER:+"$RUST_TEST_FILTER"} \
      </dev/null >"$RUST_TEST_WORK/$index.out" 2>&1 || status=$?
  fi
  printf '%s\t%s\n' "$status" "$(( SECONDS - started ))" >"$RUST_TEST_WORK/$index.status"
}

# Build the selected Rust test targets once, then run their test executables
# RUST_TEST_EXECUTABLES at a time with RUST_TEST_THREADS threads each, the
# longest first by their last durations; the build uses RUST_BUILD_JOBS jobs.
# An integration test whose file has a line beginning
# `// validation: run alone`, such as one that asserts wall-clock limits,
# runs after the others, one at a time:
# run_rust_tests [--require-executed] [--filter NAME] [CARGO TARGET ARG...]
# Without a target argument every target runs, with the library's doc tests,
# as `cargo test` does. NAME filters the tests each executable runs. A test
# that cannot run on this platform, such as a permission test run as the
# superuser, passes after printing a `SKIP name: reason` line. Every
# executable runs even after one fails. A failed build replays its
# diagnostics, and a failed executable its complete output; success prints
# one line naming any skips.
run_rust_tests() {
  local require_executed=0 filter="" work status=0 count index label executable
  local seconds durations failed=() failed_labels=() first_status=0 executed=0 skips line batch at_once
  while (( $# > 0 )); do
    case "$1" in
      --require-executed) require_executed=1; shift ;;
      --filter) filter="$2"; shift 2 ;;
      *) break ;;
    esac
  done
  work="$(mktemp -d)"
  cargo test --offline --jobs "$RUST_BUILD_JOBS" --no-run --message-format=json-render-diagnostics "$@" \
    >"$work/artifacts.json" 2>"$work/build.log" || status=$?
  if (( status != 0 )); then
    cat "$work/build.log" >&2
    rm -rf "$work"
    return "$status"
  fi
  # LABEL<TAB>EXECUTABLE for each test executable Cargo built, sorted by label.
  if ! {
    if (( $# == 0 )); then printf 'doc tests\tdoc\n'; fi
    node -e '
      const fs = require("node:fs");
      const path = require("node:path");
      for (const line of fs.readFileSync(process.argv[1], "utf8").split("\n")) {
        if (!line.startsWith("{")) continue;
        const message = JSON.parse(line);
        if (message.reason !== "compiler-artifact" || !message.executable || !message.profile?.test) continue;
        console.log(path.relative(process.cwd(), message.target.src_path) + "\t" + message.executable);
      }' "$work/artifacts.json"
  } | LC_ALL=C sort >"$work/jobs"; then
    printf 'validation: Cargo'"'"'s build messages could not be read\n' >&2
    cat "$work/build.log" >&2
    rm -rf "$work"
    return 1
  fi
  count="$(wc -l <"$work/jobs" | tr -d ' ')"
  if (( count == 0 )); then
    printf 'validation: the build produced no test executables\n' >&2
    cat "$work/build.log" >&2
    rm -rf "$work"
    return 1
  fi
  durations="$(rust_validation_dir)/rust-test-durations"
  awk -F '\t' -v OFS='\t' -v durations="$durations" '
    BEGIN { while ((getline line < durations) > 0) { split(line, field, "\t"); seconds[field[1]] = field[2] } }
    { print (($1 in seconds) ? seconds[$1] : 1000000), NR, $1, $2 }' "$work/jobs" \
    | sort -t "$(printf '\t')" -k1,1nr -k2,2n | cut -f 2- >"$work/order"
  : >"$work/together"
  : >"$work/alone"
  while IFS=$'\t' read -r index label executable; do
    if [[ "$label" == tests/*.rs ]] && grep -qE '^[[:space:]]*//!?[[:space:]]*validation: run alone' "$label" 2>/dev/null; then
      printf '%s\t%s\t%s\n' "$index" "$label" "$executable" >>"$work/alone"
    else
      printf '%s\t%s\t%s\n' "$index" "$label" "$executable" >>"$work/together"
    fi
  done <"$work/order"
  status=0
  for batch in together alone; do
    if [[ "$batch" == together ]]; then at_once="$RUST_TEST_EXECUTABLES"; else at_once=1; fi
    tr '\n' '\0' <"$work/$batch" \
      | RUST_TEST_WORK="$work" RUST_TEST_FILTER="$filter" CARGO_MANIFEST_DIR="$PWD" \
        xargs -0 -r -n 1 -P "$at_once" \
          bash -c "$(declare -f run_rust_test_executable)"'; run_rust_test_executable "$1"' run_rust_test_executable \
      || status=$?
  done
  if (( status != 0 )); then
    printf 'validation: the test executables could not be run (xargs exit %s)\n' "$status" >&2
    rm -rf "$work"
    return 1
  fi
  index=0
  while IFS=$'\t' read -r label executable; do
    index=$(( index + 1 ))
    if ! IFS=$'\t' read -r status seconds <"$work/$index.status" 2>/dev/null; then
      status=1 seconds=0
      printf '%s did not report a result\n' "$label" >>"$work/$index.out"
    fi
    printf '%s\t%s\n' "$label" "$seconds" >>"$work/durations"
    if (( status != 0 )); then
      failed+=("$index")
      failed_labels+=("$label")
      if (( first_status == 0 )); then first_status=$status; fi
    fi
    if grep -qE '^test result: ok\. [1-9][0-9]* passed;' "$work/$index.out" 2>/dev/null; then
      executed=1
    fi
  done <"$work/jobs"
  mkdir -p "$(dirname "$durations")"
  awk -F '\t' -v OFS='\t' -v previous="$durations" '
    BEGIN { while ((getline line < previous) > 0) { split(line, field, "\t"); seconds[field[1]] = field[2] } }
    { seconds[$1] = $2 }
    END { for (label in seconds) print label, seconds[label] }' "$work/durations" \
    | LC_ALL=C sort >"$durations.$$"
  mv "$durations.$$" "$durations"
  if (( ${#failed[@]} > 0 )); then
    line="$(printf '%s, ' "${failed_labels[@]}")"
    printf 'rust tests: %s of %s test executables failed: %s\n' "${#failed[@]}" "$count" "${line%, }" >&2
    for index in "${failed[@]}"; do
      printf '\n==== %s (exit %s)\n' "$(sed -n "${index}p" "$work/jobs" | cut -f 1)" \
        "$(cut -f 1 "$work/$index.status" 2>/dev/null || printf 'none')" >&2
      cat "$work/$index.out" >&2
    done
    rm -rf "$work"
    return "$first_status"
  fi
  if (( require_executed && ! executed )); then
    printf 'validation: no tests executed in the selected Rust module\n' >&2
    for (( index = 1; index <= count; index++ )); do cat "$work/$index.out" >&2; done
    rm -rf "$work"
    return 1
  fi
  skips="$(for (( index = 1; index <= count; index++ )); do grep -E '^SKIP ' "$work/$index.out" || true; done)"
  rm -rf "$work"
  if [[ -z "$skips" ]]; then
    printf 'rust tests: passed, 0 skipped\n'
  else
    printf 'rust tests: passed, %s skipped on this platform:\n%s\n' "$(printf '%s\n' "$skips" | wc -l | tr -d ' ')" "$skips"
  fi
}

# The Node and compilers a harness pass depended on besides the tree: the
# Node version and each compiler variable's value, size, and time, as the
# find_* functions resolve them.
harness_toolchain() {
  local name value
  find_syntax_api >/dev/null 2>&1 || true
  find_semantic_compilers >/dev/null 2>&1 || true
  find_typescript_56 >/dev/null 2>&1 || true
  {
    node -v 2>/dev/null
    for name in TYPE_SCRIPT_API TYPE_SCRIPT_SEMANTIC_API TYPE_SCRIPT_NATIVE TYPE_SCRIPT_56_API; do
      value="${!name:-}"
      printf '%s=%s\n' "$name" "$value"
      if [[ -n "$value" ]]; then ls -ld "$value" "$value/package.json" 2>/dev/null || true; fi
    done
  } | cksum | cut -d ' ' -f 1
}

# Choose the harness test files that the changes since the last passing run
# can affect (tools/harness-selection.mjs owns the rules). Sets
# HARNESS_SELECTION to `every`, `none`, or `files`, with the files in
# HARNESS_SELECTED; HARNESS_SELECTION_REASON for the report; and HARNESS_TREE
# and HARNESS_FINGERPRINT, which record_pass takes once the run passes. The
# base comes from selection_base; a Node or compiler change since the
# recorded pass runs every test. Fails when the selection itself fails.
select_harness_tests() {
  HARNESS_SELECTION=every
  HARNESS_SELECTED=()
  HARNESS_SELECTION_REASON="the changes since ${TARGET_BRANCH:-main} could not be determined"
  HARNESS_TREE=""
  local found base since list selection file
  HARNESS_FINGERPRINT="$(harness_toolchain)"
  HARNESS_TREE="$(working_tree)" || { HARNESS_TREE=""; return 0; }
  found="$(selection_base harness-tests "$HARNESS_FINGERPRINT")" || return 0
  if [[ "$found" == changed ]]; then
    HARNESS_SELECTION_REASON="Node or a compiler changed since the last passing run"
    return 0
  fi
  base="$(printf '%s\n' "$found" | sed -n 1p)"
  since="$(printf '%s\n' "$found" | sed -n 2p)"
  list="$(mktemp)"
  if ! git diff-tree -r --no-renames --name-only -z "$base" "$HARNESS_TREE" >"$list"; then
    rm -f "$list"
    return 0
  fi
  selection="$(mktemp)"
  if ! node "$(dirname "${BASH_SOURCE[0]}")/../../tools/harness-selection.mjs" "$list" >"$selection"; then
    printf 'validation: the harness test selection failed\n' >&2
    rm -f "$list" "$selection"
    return 1
  fi
  rm -f "$list"
  if [[ "$(head -c 6 "$selection")" == every$'\t' ]]; then
    HARNESS_SELECTION_REASON="$(cut -f 2 "$selection" | head -n 1) changed since $since"
    rm -f "$selection"
    return 0
  fi
  while IFS= read -r file; do
    [[ -n "$file" ]] && HARNESS_SELECTED+=("$file")
  done <"$selection"
  rm -f "$selection"
  if (( ${#HARNESS_SELECTED[@]} == 0 )); then
    HARNESS_SELECTION=none
    HARNESS_SELECTION_REASON="no harness test inputs changed since $since"
  elif (( ${#HARNESS_SELECTED[@]} < $(ls tools/*.test.mjs | wc -l) )); then
    HARNESS_SELECTION=files
    HARNESS_SELECTION_REASON="changes since $since"
  else
    HARNESS_SELECTED=()
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
  find_typescript_56 || missing+=("$compiler_missing")
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
