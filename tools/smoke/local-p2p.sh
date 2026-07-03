#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

json_field() {
  file="$1"
  path="$2"
  python3 - "$file" "$path" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    value = json.load(handle)

for part in sys.argv[2].split("."):
    value = value[int(part)] if part.isdigit() else value[part]

if isinstance(value, bool):
    print("true" if value else "false")
else:
    print(value)
PY
}

assert_json() {
  file="$1"
  expression="$2"
  message="$3"
  python3 - "$file" "$expression" "$message" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

if not eval(sys.argv[2], {"__builtins__": {}}, {"data": data, "len": len, "any": any}):
    raise SystemExit(sys.argv[3])
PY
}

unused_port() {
  python3 - <<'PY'
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
  python3 - "$port" "$log_file" <<'PY'
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
  if [ -n "${node_pid:-}" ]; then
    kill "$node_pid" >/dev/null 2>&1 || true
    wait "$node_pid" >/dev/null 2>&1 || true
  fi

  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

require_tool cargo
require_tool python3

cargo_args="$*"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
case "$target_dir" in
  /*) ;;
  *) target_dir="$repo_root/$target_dir" ;;
esac

cd "$repo_root"
cargo build -p chaft-cli -p chaft-node $cargo_args

cli_bin="$target_dir/debug/chaft-cli"
node_bin="$target_dir/debug/chaft-node"
if [ ! -x "$cli_bin" ] || [ ! -x "$node_bin" ]; then
  printf 'missing expected smoke binaries under %s/debug\n' "$target_dir" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-smoke.XXXXXX")"
trap cleanup EXIT INT TERM

app_dir="$smoke_dir/app"
peer_dir="$smoke_dir/peer"
node_dir="$smoke_dir/backup-node"
mkdir -p "$app_dir" "$peer_dir" "$node_dir"

created_json="$smoke_dir/created.json"
"$cli_bin" --data-dir "$app_dir" init-workspace \
  --name "Chaft Smoke" \
  --channel general > "$created_json"

workspace_id="$(json_field "$created_json" workspaceId)"
channel_id="$(json_field "$created_json" channelId)"

peer_device_id="$("$cli_bin" --data-dir "$peer_dir" device-id)"
invite_json="$smoke_dir/invite-peer.json"
"$cli_bin" --data-dir "$app_dir" invite-member \
  --workspace-id "$workspace_id" \
  --device-id "$peer_device_id" \
  --role member > "$invite_json"

message_json="$smoke_dir/message.json"
"$cli_bin" --data-dir "$app_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$channel_id" \
  --text "smoke encrypted hello" > "$message_json"

attachment_file="$smoke_dir/attachment.txt"
printf 'smoke attachment secret\n' > "$attachment_file"
attachment_json="$smoke_dir/attachment-message.json"
"$cli_bin" --data-dir "$app_dir" send-attachment \
  --workspace-id "$workspace_id" \
  --channel-id "$channel_id" \
  --text "smoke encrypted attachment" \
  --file "$attachment_file" \
  --media-type text/plain > "$attachment_json"

snapshot_json="$smoke_dir/app-snapshot.json"
"$cli_bin" --data-dir "$app_dir" snapshot \
  --workspace-id "$workspace_id" \
  --decrypt > "$snapshot_json"
assert_json "$snapshot_json" \
  'any(item.get("body") == "smoke encrypted hello" for item in data["timeline"])' \
  'local decrypted snapshot did not include the sent message'
assert_json "$snapshot_json" \
  'any(item.get("attachmentCount") == 1 for item in data["timeline"])' \
  'local decrypted snapshot did not include the attachment message'

search_json="$smoke_dir/app-search.json"
"$cli_bin" --data-dir "$app_dir" search-workspace \
  --workspace-id "$workspace_id" \
  --query "encrypted hello" > "$search_json"
assert_json "$search_json" \
  'len(data["hits"]) >= 1 and data["hits"][0]["body"] == "smoke encrypted hello"' \
  'local search did not find the encrypted message'

workspace_key_json="$smoke_dir/workspace-key.json"
"$cli_bin" --data-dir "$app_dir" export-workspace-key \
  --workspace-id "$workspace_id" > "$workspace_key_json"

port="$(unused_port)"
node_log="$smoke_dir/backup-node.log"
"$node_bin" --data-dir "$node_dir" serve --listen "127.0.0.1:$port" > "$node_log" 2>&1 &
node_pid="$!"
wait_for_tcp "$port" "$node_log"

endpoint_json="$smoke_dir/endpoint.json"
"$cli_bin" --data-dir "$app_dir" publish-peer-endpoint \
  --workspace-id "$workspace_id" \
  --endpoint-id backup-node \
  --endpoint "direct+tcp://127.0.0.1:$port" \
  --backup-peer > "$endpoint_json"

backup_json="$smoke_dir/backup.json"
"$cli_bin" --data-dir "$app_dir" backup-workspace \
  --workspace-id "$workspace_id" \
  --peer "127.0.0.1:$port" > "$backup_json"
assert_json "$backup_json" \
  'len(data["publishedEventIds"]) >= 2 and data["publishedBlobCount"] >= 1 and data["missingBlobCount"] == 0' \
  'backup node did not receive the workspace event slice'

pull_json="$smoke_dir/pull.json"
"$cli_bin" --data-dir "$peer_dir" pull-workspace \
  --workspace-id "$workspace_id" \
  --peer "127.0.0.1:$port" > "$pull_json"
assert_json "$pull_json" \
  'len(data["fetchedEventIds"]) >= 2 and data["missingBlobCount"] == 0' \
  'peer did not pull the workspace from the backup node'

peer_partial_snapshot_json="$smoke_dir/peer-partial-snapshot.json"
"$cli_bin" --data-dir "$peer_dir" snapshot \
  --workspace-id "$workspace_id" > "$peer_partial_snapshot_json"
assert_json "$pull_json" \
  'data["gapCount"] >= 1' \
  'partial backup pull did not report missing history gaps'
assert_json "$peer_partial_snapshot_json" \
  'data["gapCount"] >= 1 and any(item.get("kind") == "missing_history_gap" for item in data["timeline"])' \
  'partial backup snapshot did not expose bounded missing-history gaps'

full_publish_json="$smoke_dir/full-publish.json"
"$cli_bin" --data-dir "$app_dir" publish-workspace \
  --workspace-id "$workspace_id" \
  --peer "127.0.0.1:$port" > "$full_publish_json"
assert_json "$full_publish_json" \
  'len(data["publishedEventIds"]) >= 3 and data["missingBlobCount"] == 0' \
  'full workspace publish did not complete the backup node history'

full_pull_json="$smoke_dir/full-pull.json"
"$cli_bin" --data-dir "$peer_dir" pull-workspace \
  --workspace-id "$workspace_id" \
  --peer "127.0.0.1:$port" > "$full_pull_json"
assert_json "$full_pull_json" \
  'data["gapCount"] == 0 and data["fetchedBlobCount"] >= 1 and data["missingBlobCount"] == 0' \
  'peer did not repair partial history after full workspace publish'

peer_raw_snapshot_json="$smoke_dir/peer-raw-snapshot.json"
"$cli_bin" --data-dir "$peer_dir" snapshot \
  --workspace-id "$workspace_id" > "$peer_raw_snapshot_json"
assert_json "$peer_raw_snapshot_json" \
  'any(item.get("body") == "Encrypted message" for item in data["timeline"])' \
  'raw peer snapshot did not preserve encrypted message rows before key import'

import_json="$smoke_dir/import-key.json"
"$cli_bin" --data-dir "$peer_dir" import-workspace-key \
  --key-file "$workspace_key_json" > "$import_json"
assert_json "$import_json" \
  'data["workspaceId"]' \
  'workspace key import did not return a workspace ID'

peer_decrypted_snapshot_json="$smoke_dir/peer-decrypted-snapshot.json"
"$cli_bin" --data-dir "$peer_dir" snapshot \
  --workspace-id "$workspace_id" \
  --decrypt > "$peer_decrypted_snapshot_json"
assert_json "$peer_decrypted_snapshot_json" \
  'any(item.get("body") == "smoke encrypted hello" for item in data["timeline"])' \
  'peer decrypted snapshot did not include the replicated message'
assert_json "$peer_decrypted_snapshot_json" \
  'any(item.get("attachmentCount") == 1 for item in data["timeline"])' \
  'peer decrypted snapshot did not include the replicated attachment'

peer_search_json="$smoke_dir/peer-search.json"
"$cli_bin" --data-dir "$peer_dir" search-workspace \
  --workspace-id "$workspace_id" \
  --query "encrypted hello" > "$peer_search_json"
assert_json "$peer_search_json" \
  'len(data["hits"]) >= 1 and data["hits"][0]["body"] == "smoke encrypted hello"' \
  'peer search did not find the replicated decrypted message'

status_json="$smoke_dir/storage-health.json"
"$cli_bin" --data-dir "$peer_dir" storage-health \
  --workspace-id "$workspace_id" > "$status_json"
assert_json "$status_json" \
  'data["corruptEventCount"] == 0 and data["nonServableParseableEventCount"] == 0' \
  'peer storage health reported corrupt or non-servable events'

printf 'local P2P smoke passed: workspace=%s peer=127.0.0.1:%s\n' "$workspace_id" "$port"
