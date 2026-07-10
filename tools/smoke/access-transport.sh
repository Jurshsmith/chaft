#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

CARGO="${CARGO:-cargo}"

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

run_cargo_test() {
  package="$1"
  filter="$2"
  shift 2
  printf 'access transport smoke: cargo test -p %s %s\n' "$package" "$filter"
  "$CARGO" test -p "$package" "$filter" "$@" -- --nocapture
}

require_tool "$CARGO"

cd "$repo_root"

run_cargo_test chaft-ffi runtime_direct_peer_ffi_submits_and_persists_join_requests "$@"
run_cargo_test chaft-ffi runtime_pull_join_requests_direct_ffi_imports_known_peer_inbox "$@"
run_cargo_test chaft-ffi runtime_join_request_outbox_ffi_ "$@"
run_cargo_test chaft-ffi runtime_join_response_outbox_ffi_ "$@"
run_cargo_test chaft-ffi runtime_pull_join_responses_direct_ffi_imports_known_peer_inbox "$@"
run_cargo_test chaft-net-direct submit_join "$@"
run_cargo_test chaft-net-direct fetch_join "$@"

printf 'access transport smoke passed\n'
