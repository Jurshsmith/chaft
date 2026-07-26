#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
. "$script_dir/common.sh"

chaft_desktop_add_tool_paths

profile="${1:-debug}"
case "$profile" in
  debug)
    rust_target_dir=debug
    cargo_profile=
    ;;
  release)
    rust_target_dir=release
    cargo_profile=--release
    ;;
  *)
    printf 'usage: %s [debug|release]\n' "$0" >&2
    exit 2
    ;;
esac

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

python_bin="${CHAFT_PYTHON_BIN:-}"
if [ -z "$python_bin" ]; then
  if [ -x /usr/bin/python3 ]; then
    python_bin=/usr/bin/python3
  else
    python_bin=python3
  fi
fi

json_field() {
  file="$1"
  path="$2"
  "$python_bin" - "$file" "$path" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    value = json.load(handle)

for part in sys.argv[2].split("."):
    value = value[int(part)] if part.isdigit() else value[part]

print(value)
PY
}

assert_json() {
  file="$1"
  expression="$2"
  message="$3"
  "$python_bin" - "$file" "$expression" "$message" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

scope = {"data": data, "len": len, "any": any, "all": all}
if not eval(sys.argv[2], {"__builtins__": {}}, scope):
    raise SystemExit(sys.argv[3])
PY
}

assert_no_default_peer_config() {
  config_file="$1"
  message="$2"
  # A runtime that has never persisted desktop preferences has no
  # desktop.json; that is equivalent to an empty default peer endpoint.
  if [ ! -f "$config_file" ]; then
    return 0
  fi
  assert_json "$config_file" \
    'not data.get("defaultPeerEndpoint", "").strip()' \
    "$message"
}

unused_port() {
  "$python_bin" - <<'PY'
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
  "$python_bin" - "$port" "$log_file" <<'PY'
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

wait_for_ready_file() {
  ready_file="$1"
  timeout_ms="$2"
  "$python_bin" - "$ready_file" "$timeout_ms" <<'PY'
import os
import sys
import time

ready_file = sys.argv[1]
deadline = time.time() + (int(sys.argv[2]) / 1000)
while time.time() < deadline:
    try:
        if os.path.getsize(ready_file) > 0:
            raise SystemExit(0)
    except OSError:
        pass
    time.sleep(0.05)

print(f"timed out waiting for desktop readiness file: {ready_file}", file=sys.stderr)
raise SystemExit(1)
PY
}

smoke_timeout_ms() {
  value="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-18000}"
  case "$value" in
    ''|*[!0-9]*) value=18000 ;;
  esac
  if [ "$value" -lt 8000 ]; then
    value=8000
  elif [ "$value" -gt 60000 ]; then
    value=60000
  fi
  printf '%s\n' "$value"
}

cleanup() {
  for pid in "${continue_pid:-}" "${watchdog_pid:-}" \
    "${desktop_pid:-}" "${node_pid:-}"
  do
    if [ -n "$pid" ]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
    fi
  done

  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept desktop live-sync smoke directory: %s\n' "$smoke_dir"
  fi
}

print_failure_logs() {
  if [ -s "$desktop_stderr" ]; then
    printf '%s\n' 'desktop stderr:' >&2
    sed -n '1,240p' "$desktop_stderr" >&2
  fi
  if [ -s "$desktop_stdout" ]; then
    printf '%s\n' 'desktop stdout:' >&2
    sed -n '1,240p' "$desktop_stdout" >&2
  fi
  if [ -s "$node_log" ]; then
    printf '%s\n' 'peer node log:' >&2
    sed -n '1,240p' "$node_log" >&2
  fi
}

require_tool cargo
require_tool "$python_bin"

"$script_dir/build.sh" "$profile"

ffi_library="$repo_root/target/$rust_target_dir/$(chaft_desktop_ffi_library_name)"
desktop_binary="$(chaft_desktop_find_binary "$repo_root" "desktop-$profile" || true)"
source_qml_root="$repo_root/apps/desktop-qt/qml"

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) binary_suffix=.exe ;;
  *) binary_suffix= ;;
esac

cd "$repo_root"
cargo build -p chaft-cli -p chaft-node $cargo_profile
cli_bin="$repo_root/target/$rust_target_dir/chaft-cli$binary_suffix"
node_bin="$repo_root/target/$rust_target_dir/chaft-node$binary_suffix"

if [ ! -x "$desktop_binary" ]; then
  printf 'desktop binary not found for %s build\n' "$profile" >&2
  exit 1
fi
if [ ! -f "$ffi_library" ]; then
  printf 'FFI library not found: %s\n' "$ffi_library" >&2
  exit 1
fi
if [ ! -x "$cli_bin" ] || [ ! -x "$node_bin" ]; then
  printf 'missing Chaft CLI or node binary under %s\n' \
    "$repo_root/target/$rust_target_dir" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-desktop-live-sync.XXXXXX")"
trap cleanup EXIT INT TERM

owner_runtime="$smoke_dir/owner-runtime"
joiner_runtime="$smoke_dir/joiner-runtime"
node_runtime="$smoke_dir/node-runtime"
mkdir -p "$owner_runtime" "$joiner_runtime" "$node_runtime"

desktop_launch_binary="$(
  chaft_desktop_prepare_smoke_binary "$desktop_binary" "$smoke_dir"
)"
desktop_stdout="$smoke_dir/desktop.stdout"
desktop_stderr="$smoke_dir/desktop.stderr"
node_log="$smoke_dir/node.log"

port="$(unused_port)"
"$node_bin" --data-dir "$node_runtime" serve \
  --listen "127.0.0.1:$port" >"$node_log" 2>&1 &
node_pid=$!
wait_for_tcp "$port" "$node_log"

created_json="$smoke_dir/workspace-created.json"
"$cli_bin" --data-dir "$owner_runtime" init-workspace \
  --name "Desktop Live Sync Smoke" \
  --channel general >"$created_json"
workspace_id="$(json_field "$created_json" workspaceId)"
channel_id="$(json_field "$created_json" channelId)"

joiner_device_id="$("$cli_bin" --data-dir "$joiner_runtime" device-id)"
"$cli_bin" --data-dir "$owner_runtime" invite-member \
  --workspace-id "$workspace_id" \
  --device-id "$joiner_device_id" \
  --role member >"$smoke_dir/member-invite.json"
"$cli_bin" --data-dir "$owner_runtime" export-workspace-key \
  --workspace-id "$workspace_id" >"$smoke_dir/workspace-key.json"

baseline_text="desktop live sync baseline"
remote_marker="desktop live sync remote marker $workspace_id"
"$cli_bin" --data-dir "$owner_runtime" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$channel_id" \
  --text "$baseline_text" >"$smoke_dir/baseline-message.json"
"$cli_bin" --data-dir "$owner_runtime" publish-workspace \
  --workspace-id "$workspace_id" \
  --peer "127.0.0.1:$port" >"$smoke_dir/baseline-publish.json"
"$cli_bin" --data-dir "$joiner_runtime" pull-workspace \
  --workspace-id "$workspace_id" \
  --peer "127.0.0.1:$port" >"$smoke_dir/baseline-pull.json"
"$cli_bin" --data-dir "$joiner_runtime" import-workspace-key \
  --key-file "$smoke_dir/workspace-key.json" >"$smoke_dir/key-import.json"
"$cli_bin" --data-dir "$joiner_runtime" snapshot \
  --workspace-id "$workspace_id" \
  --decrypt >"$smoke_dir/joiner-before.json"
assert_json "$smoke_dir/joiner-before.json" \
  'any(item.get("body") == "desktop live sync baseline" for item in data["timeline"])' \
  'joiner baseline did not contain the seeded message'
assert_json "$smoke_dir/joiner-before.json" \
  'not any(item.get("body", "").startswith("desktop live sync remote marker") for item in data["timeline"])' \
  'remote-only marker was present before the desktop started'

case "$(uname -s)" in
  Linux|Darwin|MINGW*|MSYS*|CYGWIN*)
    export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"
    ;;
esac

timeout_ms="$(smoke_timeout_ms)"
endpoint="direct+tcp://127.0.0.1:$port"
ready_file="$smoke_dir/desktop-ready"
env \
  CHAFT_FFI_LIBRARY="$ffi_library" \
  CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
  CHAFT_RUNTIME_DIR="$joiner_runtime" \
  CHAFT_WORKSPACE_ID="$workspace_id" \
  CHAFT_PEER_ENDPOINT="$endpoint" \
  CHAFT_DESKTOP_SMOKE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_TEXT="$remote_marker" \
  CHAFT_DESKTOP_SMOKE_EXPECT_CHANNEL_ID="$channel_id" \
  CHAFT_DESKTOP_SMOKE_READY_FILE="$ready_file" \
  CHAFT_DESKTOP_SMOKE_READY_TEXT="$baseline_text" \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="$timeout_ms" \
  CHAFT_DESKTOP_ALLOW_LOOPBACK_FALLBACK=1 \
  CHAFT_IROH_ALLOW_PUBLIC_RELAYS=0 \
  CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY=0 \
  "$desktop_launch_binary" >"$desktop_stdout" 2>"$desktop_stderr" &
desktop_pid=$!

watchdog_seconds=$(( (timeout_ms * 2 + 15000 + 999) / 1000 ))
watchdog_marker="$smoke_dir/watchdog-fired"
(
  sleep "$watchdog_seconds"
  if kill -0 "$desktop_pid" 2>/dev/null; then
    : >"$watchdog_marker"
    kill "$desktop_pid" >/dev/null 2>&1 || true
  fi
) &
watchdog_pid=$!

continue_pid=
if [ "$(uname -s)" = "Darwin" ]; then
  (
    while kill -0 "$desktop_pid" 2>/dev/null; do
      kill -CONT "$desktop_pid" >/dev/null 2>&1 || true
      sleep 1
    done
  ) &
  continue_pid=$!
fi

# Do not publish the marker until the desktop proves that one full network sync
# and the baseline room-history load have both settled. The marker must then be
# discovered by a later automatic sync, which is the regression under test.
if ! wait_for_ready_file "$ready_file" "$timeout_ms"; then
  print_failure_logs
  printf '%s\n' 'desktop did not reach the pre-marker ready state' >&2
  exit 1
fi
if ! kill -0 "$desktop_pid" 2>/dev/null; then
  desktop_status=0
  wait "$desktop_pid" || desktop_status=$?
  desktop_pid=
  print_failure_logs
  printf 'desktop exited before the remote marker was published (status %s)\n' \
    "$desktop_status" >&2
  exit 1
fi

"$cli_bin" --data-dir "$owner_runtime" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$channel_id" \
  --text "$remote_marker" >"$smoke_dir/remote-message.json"
"$cli_bin" --data-dir "$owner_runtime" publish-workspace \
  --workspace-id "$workspace_id" \
  --peer "127.0.0.1:$port" >"$smoke_dir/remote-publish.json"
assert_json "$smoke_dir/remote-publish.json" \
  'data["publishedEventCount"] >= 1' \
  'owner did not publish the post-start marker'

desktop_status=0
wait "$desktop_pid" || desktop_status=$?
desktop_pid=

kill "$watchdog_pid" >/dev/null 2>&1 || true
wait "$watchdog_pid" >/dev/null 2>&1 || true
watchdog_pid=
if [ -n "$continue_pid" ]; then
  kill "$continue_pid" >/dev/null 2>&1 || true
  wait "$continue_pid" >/dev/null 2>&1 || true
  continue_pid=
fi

if [ -f "$watchdog_marker" ]; then
  print_failure_logs
  printf 'desktop live-sync smoke harness timed out after %ss\n' \
    "$watchdog_seconds" >&2
  exit 124
fi
if [ "$desktop_status" -ne 0 ]; then
  print_failure_logs
  printf 'desktop live-sync smoke failed with status %s\n' \
    "$desktop_status" >&2
  exit "$desktop_status"
fi

"$cli_bin" --data-dir "$joiner_runtime" snapshot \
  --workspace-id "$workspace_id" \
  --decrypt >"$smoke_dir/joiner-after.json"
assert_json "$smoke_dir/joiner-after.json" \
  'any(item.get("body", "").startswith("desktop live sync remote marker ") for item in data["timeline"])' \
  'joiner did not persist the remotely synchronized marker'

# A hosted desktop can receive peer writes directly into its open SQLite store
# without an outbound sync result to refresh the UI. Exercise that topology with
# no configured default peer: mutate the open runtime from another process and
# require the already-selected room snapshot to observe it without a local send.
host_runtime="$smoke_dir/host-runtime"
mkdir -p "$host_runtime"
host_created_json="$smoke_dir/host-workspace-created.json"
"$cli_bin" --data-dir "$host_runtime" init-workspace \
  --name "Desktop Hosted Reconcile Smoke" \
  --channel general >"$host_created_json"
host_workspace_id="$(json_field "$host_created_json" workspaceId)"
host_channel_id="$(json_field "$host_created_json" channelId)"
host_baseline_text="desktop hosted reconcile baseline"
host_external_marker="desktop hosted external marker $host_workspace_id"
"$cli_bin" --data-dir "$host_runtime" send-message \
  --workspace-id "$host_workspace_id" \
  --channel-id "$host_channel_id" \
  --text "$host_baseline_text" >"$smoke_dir/host-baseline-message.json"
"$cli_bin" --data-dir "$host_runtime" snapshot \
  --workspace-id "$host_workspace_id" \
  --decrypt >"$smoke_dir/host-before.json"
assert_json "$smoke_dir/host-before.json" \
  'any(item.get("body") == "desktop hosted reconcile baseline" for item in data["timeline"])' \
  'host baseline did not contain the seeded message'
assert_json "$smoke_dir/host-before.json" \
  'not any(item.get("body", "").startswith("desktop hosted external marker") for item in data["timeline"])' \
  'host external marker was present before the desktop started'

desktop_stdout="$smoke_dir/host-desktop.stdout"
desktop_stderr="$smoke_dir/host-desktop.stderr"
ready_file="$smoke_dir/host-desktop-ready"
env \
  CHAFT_FFI_LIBRARY="$ffi_library" \
  CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
  CHAFT_RUNTIME_DIR="$host_runtime" \
  CHAFT_WORKSPACE_ID="$host_workspace_id" \
  CHAFT_PEER_ENDPOINT= \
  CHAFT_DESKTOP_SMOKE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_TEXT="$host_external_marker" \
  CHAFT_DESKTOP_SMOKE_EXPECT_CHANNEL_ID="$host_channel_id" \
  CHAFT_DESKTOP_SMOKE_EXPECT_REACHABLE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_ROUTE=iroh-direct \
  CHAFT_DESKTOP_SMOKE_READY_FILE="$ready_file" \
  CHAFT_DESKTOP_SMOKE_READY_TEXT="$host_baseline_text" \
  CHAFT_DESKTOP_SMOKE_READY_REQUIRES_SYNC=0 \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="$timeout_ms" \
  CHAFT_DESKTOP_BACKGROUND_REACHABILITY=1 \
  CHAFT_DESKTOP_ALLOW_LOOPBACK_FALLBACK=0 \
  CHAFT_IROH_ALLOW_PUBLIC_RELAYS=0 \
  CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY=0 \
  "$desktop_launch_binary" >"$desktop_stdout" 2>"$desktop_stderr" &
desktop_pid=$!

watchdog_marker="$smoke_dir/host-watchdog-fired"
(
  sleep "$watchdog_seconds"
  if kill -0 "$desktop_pid" 2>/dev/null; then
    : >"$watchdog_marker"
    kill "$desktop_pid" >/dev/null 2>&1 || true
  fi
) &
watchdog_pid=$!

continue_pid=
if [ "$(uname -s)" = "Darwin" ]; then
  (
    while kill -0 "$desktop_pid" 2>/dev/null; do
      kill -CONT "$desktop_pid" >/dev/null 2>&1 || true
      sleep 1
    done
  ) &
  continue_pid=$!
fi

if ! wait_for_ready_file "$ready_file" "$timeout_ms"; then
  print_failure_logs
  printf '%s\n' 'hosted desktop did not reach the pre-write ready state' >&2
  exit 1
fi
if ! kill -0 "$desktop_pid" 2>/dev/null; then
  desktop_status=0
  wait "$desktop_pid" || desktop_status=$?
  desktop_pid=
  print_failure_logs
  printf 'hosted desktop exited before the external write (status %s)\n' \
    "$desktop_status" >&2
  exit 1
fi

"$cli_bin" --data-dir "$host_runtime" send-message \
  --workspace-id "$host_workspace_id" \
  --channel-id "$host_channel_id" \
  --text "$host_external_marker" >"$smoke_dir/host-external-message.json"

desktop_status=0
wait "$desktop_pid" || desktop_status=$?
desktop_pid=

kill "$watchdog_pid" >/dev/null 2>&1 || true
wait "$watchdog_pid" >/dev/null 2>&1 || true
watchdog_pid=
if [ -n "$continue_pid" ]; then
  kill "$continue_pid" >/dev/null 2>&1 || true
  wait "$continue_pid" >/dev/null 2>&1 || true
  continue_pid=
fi

if [ -f "$watchdog_marker" ]; then
  print_failure_logs
  printf 'desktop hosted-reconcile smoke timed out after %ss\n' \
    "$watchdog_seconds" >&2
  exit 124
fi
if [ "$desktop_status" -ne 0 ]; then
  print_failure_logs
  printf 'desktop hosted-reconcile smoke failed with status %s\n' \
    "$desktop_status" >&2
  exit "$desktop_status"
fi

"$cli_bin" --data-dir "$host_runtime" snapshot \
  --workspace-id "$host_workspace_id" \
  --decrypt >"$smoke_dir/host-after.json"
assert_json "$smoke_dir/host-after.json" \
  'any(item.get("body", "").startswith("desktop hosted external marker ") for item in data["timeline"])' \
  'host runtime did not persist the external marker'
assert_no_default_peer_config "$host_runtime/desktop.json" \
  'hosted reconciliation unexpectedly depended on a default peer endpoint'

# Finally exercise the real hosted-peer write path. A distinct member runtime
# publishes a new event to the endpoint exposed by the running desktop. The
# recipient desktop still performs no local action; its hosted-store
# reconciliation must notice the server-side SQLite write and refresh the room.
host_peer_client_runtime="$smoke_dir/host-peer-client-runtime"
mkdir -p "$host_peer_client_runtime"
host_peer_client_device_id="$("$cli_bin" --data-dir "$host_peer_client_runtime" device-id)"
"$cli_bin" --data-dir "$host_runtime" invite-member \
  --workspace-id "$host_workspace_id" \
  --device-id "$host_peer_client_device_id" \
  --role member >"$smoke_dir/host-peer-client-invite.json"
"$cli_bin" --data-dir "$host_runtime" export-workspace-key \
  --workspace-id "$host_workspace_id" \
  >"$smoke_dir/host-peer-workspace-key.json"
"$cli_bin" --data-dir "$host_runtime" publish-workspace \
  --workspace-id "$host_workspace_id" \
  --peer "127.0.0.1:$port" >"$smoke_dir/host-peer-seed-publish.json"
"$cli_bin" --data-dir "$host_peer_client_runtime" pull-workspace \
  --workspace-id "$host_workspace_id" \
  --peer "127.0.0.1:$port" >"$smoke_dir/host-peer-client-pull.json"
"$cli_bin" --data-dir "$host_peer_client_runtime" import-workspace-key \
  --key-file "$smoke_dir/host-peer-workspace-key.json" \
  >"$smoke_dir/host-peer-client-key-import.json"

host_peer_marker="desktop hosted peer marker $host_workspace_id"
desktop_stdout="$smoke_dir/host-peer-desktop.stdout"
desktop_stderr="$smoke_dir/host-peer-desktop.stderr"
ready_file="$smoke_dir/host-peer-desktop-ready.json"
env \
  CHAFT_FFI_LIBRARY="$ffi_library" \
  CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
  CHAFT_RUNTIME_DIR="$host_runtime" \
  CHAFT_WORKSPACE_ID="$host_workspace_id" \
  CHAFT_PEER_ENDPOINT= \
  CHAFT_DESKTOP_SMOKE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_TEXT="$host_peer_marker" \
  CHAFT_DESKTOP_SMOKE_EXPECT_CHANNEL_ID="$host_channel_id" \
  CHAFT_DESKTOP_SMOKE_EXPECT_REACHABLE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_ROUTE=iroh-direct \
  CHAFT_DESKTOP_SMOKE_READY_FILE="$ready_file" \
  CHAFT_DESKTOP_SMOKE_READY_TEXT="$host_external_marker" \
  CHAFT_DESKTOP_SMOKE_READY_REQUIRES_SYNC=0 \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="$timeout_ms" \
  CHAFT_DESKTOP_BACKGROUND_REACHABILITY=1 \
  CHAFT_DESKTOP_ALLOW_LOOPBACK_FALLBACK=0 \
  CHAFT_IROH_ALLOW_PUBLIC_RELAYS=0 \
  CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY=0 \
  "$desktop_launch_binary" >"$desktop_stdout" 2>"$desktop_stderr" &
desktop_pid=$!

watchdog_marker="$smoke_dir/host-peer-watchdog-fired"
(
  sleep "$watchdog_seconds"
  if kill -0 "$desktop_pid" 2>/dev/null; then
    : >"$watchdog_marker"
    kill "$desktop_pid" >/dev/null 2>&1 || true
  fi
) &
watchdog_pid=$!

continue_pid=
if [ "$(uname -s)" = "Darwin" ]; then
  (
    while kill -0 "$desktop_pid" 2>/dev/null; do
      kill -CONT "$desktop_pid" >/dev/null 2>&1 || true
      sleep 1
    done
  ) &
  continue_pid=$!
fi

if ! wait_for_ready_file "$ready_file" "$timeout_ms"; then
  print_failure_logs
  printf '%s\n' 'hosted peer desktop did not reach the ready state' >&2
  exit 1
fi
if ! kill -0 "$desktop_pid" 2>/dev/null; then
  desktop_status=0
  wait "$desktop_pid" || desktop_status=$?
  desktop_pid=
  print_failure_logs
  printf 'hosted peer desktop exited before the inbound publish (status %s)\n' \
    "$desktop_status" >&2
  exit 1
fi

hosted_peer_endpoint="$(json_field "$ready_file" hostedPeerEndpoint)"
case "$hosted_peer_endpoint" in
  iroh://*) ;;
  *)
    print_failure_logs
    printf 'hosted desktop readiness did not expose Iroh: %s\n' \
      "$hosted_peer_endpoint" >&2
    exit 1
    ;;
esac
"$cli_bin" --data-dir "$host_peer_client_runtime" send-message \
  --workspace-id "$host_workspace_id" \
  --channel-id "$host_channel_id" \
  --text "$host_peer_marker" >"$smoke_dir/host-peer-message.json"
"$cli_bin" --data-dir "$host_peer_client_runtime" publish-workspace \
  --workspace-id "$host_workspace_id" \
  --peer "$hosted_peer_endpoint" >"$smoke_dir/host-peer-publish.json"
assert_json "$smoke_dir/host-peer-publish.json" \
  'data["publishedEventCount"] >= 1' \
  'member runtime did not publish the hosted-peer marker'

desktop_status=0
wait "$desktop_pid" || desktop_status=$?
desktop_pid=

kill "$watchdog_pid" >/dev/null 2>&1 || true
wait "$watchdog_pid" >/dev/null 2>&1 || true
watchdog_pid=
if [ -n "$continue_pid" ]; then
  kill "$continue_pid" >/dev/null 2>&1 || true
  wait "$continue_pid" >/dev/null 2>&1 || true
  continue_pid=
fi

if [ -f "$watchdog_marker" ]; then
  print_failure_logs
  printf 'desktop hosted-peer smoke timed out after %ss\n' \
    "$watchdog_seconds" >&2
  exit 124
fi
if [ "$desktop_status" -ne 0 ]; then
  print_failure_logs
  printf 'desktop hosted-peer smoke failed with status %s\n' \
    "$desktop_status" >&2
  exit "$desktop_status"
fi

"$cli_bin" --data-dir "$host_runtime" snapshot \
  --workspace-id "$host_workspace_id" \
  --decrypt >"$smoke_dir/host-peer-after.json"
assert_json "$smoke_dir/host-peer-after.json" \
  'any(item.get("body", "").startswith("desktop hosted peer marker ") for item in data["timeline"])' \
  'host runtime did not persist the inbound hosted-peer marker'
assert_no_default_peer_config "$host_runtime/desktop.json" \
  'hosted-peer reconciliation unexpectedly stored a default peer endpoint'

printf 'desktop live-sync smoke passed: workspace=%s channel=%s hosted=%s hosted-channel=%s hosted-peer=%s\n' \
  "$workspace_id" "$channel_id" "$host_workspace_id" "$host_channel_id" \
  "$hosted_peer_endpoint"
