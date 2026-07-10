#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

CARGO="${CARGO:-cargo}"
PYTHON="${CHAFT_PYTHON_BIN:-python3}"

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

assert_json() {
  file="$1"
  expression="$2"
  message="$3"
  "$PYTHON" - "$file" "$expression" "$message" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

if not eval(sys.argv[2], {"__builtins__": {}}, {"data": data, "len": len}):
    raise SystemExit(sys.argv[3])
PY
}

unused_port() {
  "$PYTHON" - <<'PY'
import socket

sock = socket.socket()
sock.bind(("127.0.0.1", 0))
print(sock.getsockname()[1])
sock.close()
PY
}

wait_for_tcp() {
  port="$1"
  log_file="$2"
  "$PYTHON" - "$port" "$log_file" <<'PY'
import socket
import sys
import time

port = int(sys.argv[1])
deadline = time.time() + 10
last_error = None

while time.time() < deadline:
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.25):
            raise SystemExit(0)
    except OSError as error:
        last_error = error
        time.sleep(0.1)

try:
    with open(sys.argv[2], encoding="utf-8", errors="replace") as handle:
        log = handle.read()
except OSError:
    log = ""

print(f"timed out waiting for 127.0.0.1:{port}: {last_error}", file=sys.stderr)
if log:
    print(log, file=sys.stderr)
raise SystemExit(1)
PY
}

cleanup() {
  for pid in ${node_pids:-}; do
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
  done
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

run_cargo_test() {
  package="$1"
  filter="$2"
  shift 2
  printf 'access transport smoke: cargo test -p %s %s\n' "$package" "$filter"
  "$CARGO" test -p "$package" "$filter" "$@" -- --nocapture
}

start_node() {
  data_dir="$1"
  port="$2"
  log_file="$3"
  "$node_bin" --data-dir "$data_dir" serve --listen "127.0.0.1:$port" >"$log_file" 2>&1 &
  node_pid="$!"
  node_pids="${node_pids:-} $node_pid"
  wait_for_tcp "$port" "$log_file"
}

stop_node() {
  pid="$1"
  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" >/dev/null 2>&1 || true
}

require_tool "$CARGO"
require_tool "$PYTHON"

cd "$repo_root"

run_cargo_test chaft-ffi runtime_direct_peer_ffi_submits_and_persists_join_requests "$@"
run_cargo_test chaft-ffi runtime_pull_join_requests_direct_ffi_imports_known_peer_inbox "$@"
run_cargo_test chaft-ffi runtime_join_request_outbox_ffi_ "$@"
run_cargo_test chaft-ffi runtime_join_response_outbox_ffi_ "$@"
run_cargo_test chaft-ffi runtime_pull_join_responses_direct_ffi_imports_known_peer_inbox "$@"
run_cargo_test chaft-net-direct submit_join "$@"
run_cargo_test chaft-net-direct fetch_join "$@"

target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
case "$target_dir" in
  /*) ;;
  *) target_dir="$repo_root/$target_dir" ;;
esac

"$CARGO" build -p chaft-cli -p chaft-node "$@"
cli_bin="$target_dir/debug/chaft-cli"
node_bin="$target_dir/debug/chaft-node"
if [ ! -x "$cli_bin" ] || [ ! -x "$node_bin" ]; then
  printf 'missing expected smoke binaries under %s/debug\n' "$target_dir" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-access-transport-smoke.XXXXXX")"
trap cleanup EXIT INT TERM

admin_node_dir="$smoke_dir/admin-node"
requester_node_dir="$smoke_dir/requester-node"
mkdir -p "$admin_node_dir" "$requester_node_dir"
admin_port="$(unused_port)"
requester_port="$(unused_port)"
start_node "$admin_node_dir" "$admin_port" "$smoke_dir/admin-node.log"
admin_node_pid="$node_pid"
start_node "$requester_node_dir" "$requester_port" "$smoke_dir/requester-node.log"
requester_node_pid="$node_pid"

workspace_id="wrk_access_transport_smoke"
other_workspace_id="wrk_access_transport_other"
empty_workspace_id="wrk_access_transport_empty"
request_file="$smoke_dir/join-request.json"
other_request_file="$smoke_dir/join-request-other.json"
response_file="$smoke_dir/join-response.json"
other_response_file="$smoke_dir/join-response-other.json"

cat > "$request_file" <<EOF
{
  "kind": "chaft.workspace-join-request.v1",
  "schemaVersion": 1,
  "workspaceId": "$workspace_id",
  "requestId": "req_access_transport_smoke",
  "deviceId": "dev_access_transport_requester",
  "displayName": "Access Transport Smoke",
  "note": "process-level access envelope smoke",
  "responsePeerEndpoint": "direct+tcp://127.0.0.1:$requester_port"
}
EOF

cat > "$other_request_file" <<EOF
{
  "kind": "chaft.workspace-join-request.v1",
  "schemaVersion": 1,
  "workspaceId": "$other_workspace_id",
  "requestId": "req_access_transport_smoke",
  "deviceId": "dev_access_transport_requester",
  "displayName": "Access Transport Smoke",
  "note": "same request id in a second workspace",
  "responsePeerEndpoint": "direct+tcp://127.0.0.1:$requester_port"
}
EOF

"$cli_bin" --data-dir "$smoke_dir/requester-runtime" submit-join-request \
  --peer "127.0.0.1:$admin_port" \
  --workspace-id "$workspace_id" \
  --request-file "$request_file" > "$smoke_dir/submit-request.json"
"$cli_bin" --data-dir "$smoke_dir/requester-runtime" submit-join-request \
  --peer "127.0.0.1:$admin_port" \
  --workspace-id "$workspace_id" \
  --request-file "$request_file" > "$smoke_dir/submit-request-duplicate.json"
"$cli_bin" --data-dir "$smoke_dir/requester-runtime" submit-join-request \
  --peer "127.0.0.1:$admin_port" \
  --workspace-id "$other_workspace_id" \
  --request-file "$other_request_file" > "$smoke_dir/submit-other-request.json"

stop_node "$admin_node_pid"
admin_port="$(unused_port)"
start_node "$admin_node_dir" "$admin_port" "$smoke_dir/admin-node-restarted.log"
admin_node_pid="$node_pid"

"$cli_bin" --data-dir "$smoke_dir/admin-runtime" fetch-join-requests \
  --peer "127.0.0.1:$admin_port" \
  --workspace-id "$workspace_id" > "$smoke_dir/fetch-requests.json"
"$cli_bin" --data-dir "$smoke_dir/admin-runtime" fetch-join-requests \
  --peer "127.0.0.1:$admin_port" \
  --workspace-id "$other_workspace_id" > "$smoke_dir/fetch-other-requests.json"
"$cli_bin" --data-dir "$smoke_dir/admin-runtime" fetch-join-requests \
  --peer "127.0.0.1:$admin_port" \
  --workspace-id "$empty_workspace_id" > "$smoke_dir/fetch-empty-requests.json"

assert_json "$smoke_dir/submit-request.json" \
  'data["submitted"] is True' \
  'join request direct submit did not report success'
assert_json "$smoke_dir/fetch-requests.json" \
  'data["count"] == 1 and "process-level access envelope smoke" in data["envelopes"][0]' \
  'known peer did not return submitted join request'
assert_json "$smoke_dir/fetch-other-requests.json" \
  'data["count"] == 1 and "same request id in a second workspace" in data["envelopes"][0]' \
  'same request id in another workspace did not return its own join request'
assert_json "$smoke_dir/fetch-empty-requests.json" \
  'data["count"] == 0' \
  'join request fetch leaked across workspace scope'

cat > "$response_file" <<EOF
{
  "kind": "chaft.workspace-join-response.v1",
  "schemaVersion": 1,
  "workspaceId": "$workspace_id",
  "requestId": "req_access_transport_smoke",
  "resolution": "declined",
  "createdAt": "2026-07-10T00:00:00.000Z",
  "responderDeviceId": "dev_access_transport_admin",
  "responderDisplayName": "Access Transport Admin"
}
EOF

cat > "$other_response_file" <<EOF
{
  "kind": "chaft.workspace-join-response.v1",
  "schemaVersion": 1,
  "workspaceId": "$other_workspace_id",
  "requestId": "req_access_transport_smoke",
  "resolution": "declined",
  "createdAt": "2026-07-10T00:00:00.000Z",
  "responderDeviceId": "dev_access_transport_admin",
  "responderDisplayName": "Access Transport Admin Other"
}
EOF

"$cli_bin" --data-dir "$smoke_dir/admin-runtime" submit-join-response \
  --peer "127.0.0.1:$requester_port" \
  --workspace-id "$workspace_id" \
  --response-file "$response_file" > "$smoke_dir/submit-response.json"
"$cli_bin" --data-dir "$smoke_dir/admin-runtime" submit-join-response \
  --peer "127.0.0.1:$requester_port" \
  --workspace-id "$workspace_id" \
  --response-file "$response_file" > "$smoke_dir/submit-response-duplicate.json"
"$cli_bin" --data-dir "$smoke_dir/admin-runtime" submit-join-response \
  --peer "127.0.0.1:$requester_port" \
  --workspace-id "$other_workspace_id" \
  --response-file "$other_response_file" > "$smoke_dir/submit-other-response.json"

stop_node "$requester_node_pid"
requester_port="$(unused_port)"
start_node "$requester_node_dir" "$requester_port" "$smoke_dir/requester-node-restarted.log"
requester_node_pid="$node_pid"

"$cli_bin" --data-dir "$smoke_dir/requester-runtime" fetch-join-responses \
  --peer "127.0.0.1:$requester_port" \
  --workspace-id "$workspace_id" > "$smoke_dir/fetch-responses.json"
"$cli_bin" --data-dir "$smoke_dir/requester-runtime" fetch-join-responses \
  --peer "127.0.0.1:$requester_port" \
  --workspace-id "$other_workspace_id" > "$smoke_dir/fetch-other-responses.json"
"$cli_bin" --data-dir "$smoke_dir/requester-runtime" fetch-join-responses \
  --peer "127.0.0.1:$requester_port" \
  --workspace-id "$empty_workspace_id" > "$smoke_dir/fetch-empty-responses.json"

assert_json "$smoke_dir/submit-response.json" \
  'data["submitted"] is True' \
  'join response direct submit did not report success'
assert_json "$smoke_dir/fetch-responses.json" \
  'data["count"] == 1 and "Access Transport Admin" in data["envelopes"][0]' \
  'known peer did not return submitted join response'
assert_json "$smoke_dir/fetch-other-responses.json" \
  'data["count"] == 1 and "Access Transport Admin Other" in data["envelopes"][0]' \
  'same request id in another workspace did not return its own join response'
assert_json "$smoke_dir/fetch-empty-responses.json" \
  'data["count"] == 0' \
  'join response fetch leaked across workspace scope'

printf 'access transport smoke passed\n'
