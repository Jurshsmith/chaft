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
  "$python_bin" -c '
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
' "$file" "$path"
}

assert_json() {
  file="$1"
  expression="$2"
  message="$3"
  "$python_bin" -c '
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

if not eval(
    sys.argv[2],
    {"__builtins__": {}},
    {"data": data, "len": len, "any": any, "all": all},
):
    raise SystemExit(sys.argv[3])
' "$file" "$expression" "$message"
}

validate_snapshot() {
  snapshot_file="$1"
  parent_message_id="$2"
  reply_message_id="$3"
  attachment_message_id="$4"
  deleted_message_id="$5"
  desktop_expected_text="$6"
  "$python_bin" - "$snapshot_file" "$parent_message_id" "$reply_message_id" \
    "$attachment_message_id" "$deleted_message_id" "$desktop_expected_text" <<'PY'
import json
import sys

snapshot_file = sys.argv[1]
parent_message_id = sys.argv[2]
reply_message_id = sys.argv[3]
attachment_message_id = sys.argv[4]
deleted_message_id = sys.argv[5]
desktop_expected_text = sys.argv[6]

with open(snapshot_file, encoding="utf-8") as handle:
    data = json.load(handle)

def fail(message):
    raise SystemExit(message)

if data.get("name") != "Chaft Visual Smoke":
    fail("snapshot did not preserve the visual smoke workspace name")

channels = data.get("channels", [])
for name in ("general", "product", "design", "p2p-lab", "vault"):
    if not any(channel.get("name") == name for channel in channels):
        fail(f"snapshot did not include channel {name!r}")

if not any(channel.get("name") == "vault" and channel.get("isPrivate") for channel in channels):
    fail("snapshot did not mark the vault channel as private")

members = data.get("members", [])
if not any(member.get("deviceId") == "dev_visual_smoke_admin" and member.get("role") == "admin" for member in members):
    fail("snapshot did not include the visual smoke admin member")
if not any(member.get("deviceId") == "dev_visual_smoke_member" and member.get("role") == "member" for member in members):
    fail("snapshot did not include the visual smoke regular member")

join_requests = data.get("joinRequests", [])
if not any(
    request.get("requestId") == "req_visual_smoke_joiner"
    and request.get("requesterDeviceId") == "dev_visual_smoke_joiner"
    and request.get("requesterDisplayName") == "Sam Rivera"
    and request.get("sourceType") == "workspace_card"
    and request.get("sourceDisplayName") == "Mira Chen"
    and request.get("status") == "waiting"
    for request in join_requests
):
    fail("snapshot did not include the visual smoke waiting join request")

timeline = data.get("timeline", [])
if data.get("gapCount") != 0:
    fail("visual workspace should not contain missing-history gaps")
if data.get("invalidSignatureCount") != 0:
    fail("visual workspace should not contain invalid signatures")
if data.get("timelineWindow", {}).get("totalCount", 0) < len(timeline):
    fail("timeline window metadata is smaller than the emitted timeline")

by_message = {
    item.get("messageId"): item
    for item in timeline
    if item.get("messageId")
}

parent = by_message.get(parent_message_id)
if parent is None:
    fail("snapshot did not include the edited parent message")
if parent.get("body") != "deterministic launch board ready, edited":
    fail("edited parent message did not render the latest body")
if parent.get("reactions", {}).get("+1") != 1:
    fail("edited parent message did not render the +1 reaction count")
if "+1" not in parent.get("myReactions", []):
    fail("edited parent message did not render the local +1 reaction")
if parent.get("threadReplyCount", 0) < 1:
    fail("edited parent message did not render reply thread metadata")

reply = by_message.get(reply_message_id)
if reply is None:
    fail("snapshot did not include the reply message")
if reply.get("replyToMessageId") != parent_message_id:
    fail("reply message did not point at the parent message")
if not reply.get("replyPreview"):
    fail("reply message did not include a reply preview")

attachment = by_message.get(attachment_message_id)
if attachment is None:
    fail("snapshot did not include the attachment message")
if attachment.get("attachmentCount") != 1:
    fail("attachment message did not render one attachment")
attachments = attachment.get("attachments", [])
if len(attachments) != 1:
    fail("attachment message did not expose one attachment row")
if not attachments[0].get("encrypted"):
    fail("attachment row was not marked encrypted")
if attachments[0].get("localBlobAvailable") is False:
    fail("attachment row reported the local blob as unavailable")

deleted = by_message.get(deleted_message_id)
if deleted is None:
    fail("snapshot did not include the deleted message")
if deleted.get("body") != "Message deleted" or not deleted.get("deleted"):
    fail("deleted message did not render the tombstone state")

if not any(item.get("body") == desktop_expected_text for item in timeline):
    fail("snapshot did not include the desktop smoke expected message")
PY
}

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] &&
    [ -z "${CHAFT_VISUAL_SMOKE_DIR:-}" ] &&
    [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -z "${CHAFT_VISUAL_SMOKE_DIR:-}" ] && [ -n "${smoke_dir:-}" ]; then
    printf 'kept visual smoke directory: %s\n' "$smoke_dir" >&2
  fi
}

require_tool "$python_bin"

cargo_args="$*"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
case "$target_dir" in
  /*) ;;
  *) target_dir="$repo_root/$target_dir" ;;
esac

cli_bin="${CHAFT_CLI_BIN:-}"
if [ -z "$cli_bin" ]; then
  require_tool cargo
  cd "$repo_root"
  cargo build -p chaft-cli $cargo_args
  cli_bin="$target_dir/debug/chaft-cli"
fi

if [ ! -x "$cli_bin" ]; then
  printf 'missing chaft-cli binary: %s\n' "$cli_bin" >&2
  exit 1
fi

if [ -n "${CHAFT_VISUAL_SMOKE_DIR:-}" ]; then
  smoke_dir="$CHAFT_VISUAL_SMOKE_DIR"
  rm -rf "$smoke_dir"
  mkdir -p "$smoke_dir"
else
  smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-visual-smoke.XXXXXX")"
fi
trap cleanup EXIT INT TERM

runtime_dir="$smoke_dir/runtime"
artifacts_dir="$smoke_dir/artifacts"
mkdir -p "$runtime_dir" "$artifacts_dir"

workspace_name="Chaft Visual Smoke"
desktop_expected_text="desktop visual smoke ready"
workspace_access_policy="${CHAFT_VISUAL_SMOKE_ACCESS_POLICY:-invite-only}"
smoke_expires_at="$($python_bin -c '
from datetime import datetime, timedelta, timezone
print((datetime.now(timezone.utc) + timedelta(days=30)).isoformat().replace("+00:00", "Z"))
')"

created_json="$artifacts_dir/created.json"
"$cli_bin" --data-dir "$runtime_dir" init-workspace \
  --name "$workspace_name" \
  --channel general \
  --access-policy "$workspace_access_policy" > "$created_json"

workspace_id="$(json_field "$created_json" workspaceId)"
general_channel_id="$(json_field "$created_json" channelId)"

"$cli_bin" --data-dir "$runtime_dir" update-device-profile \
  --workspace-id "$workspace_id" \
  --display-name "Ayo" > "$artifacts_dir/profile.json"

"$cli_bin" --data-dir "$runtime_dir" invite-member \
  --workspace-id "$workspace_id" \
  --device-id dev_visual_smoke_admin \
  --role admin > "$artifacts_dir/invite-admin.json"

"$cli_bin" --data-dir "$runtime_dir" invite-member \
  --workspace-id "$workspace_id" \
  --device-id dev_visual_smoke_member \
  --role member > "$artifacts_dir/invite-member.json"

"$cli_bin" --data-dir "$runtime_dir" record-invite \
  --workspace-id "$workspace_id" \
  --invite-id inv_visual_smoke_revoked \
  --device-id dev_visual_smoke_member \
  --display-name "Taylor Kim" \
  --role member \
  --expires-at "$smoke_expires_at" \
  --approval-policy preapproved \
  --sync-expectation needs_reachable_teammate \
  > "$artifacts_dir/record-revoked-invite.json"

"$cli_bin" --data-dir "$runtime_dir" resolve-invite \
  --workspace-id "$workspace_id" \
  --invite-id inv_visual_smoke_revoked \
  --resolution revoked > "$artifacts_dir/resolve-revoked-invite.json"

if [ "${CHAFT_VISUAL_SMOKE_LOST_INVITE:-0}" = "1" ]; then
  "$cli_bin" --data-dir "$runtime_dir" record-invite \
    --workspace-id "$workspace_id" \
    --invite-id inv_visual_smoke_lost \
    --device-id dev_visual_smoke_lost \
    --display-name "Jordan Lee" \
    --role member \
    --expires-at "$smoke_expires_at" \
    --approval-policy preapproved \
    --sync-expectation needs_reachable_teammate \
    > "$artifacts_dir/record-lost-invite.json"
fi

"$cli_bin" --data-dir "$runtime_dir" record-join-request \
  --workspace-id "$workspace_id" \
  --request-id req_visual_smoke_joiner \
  --device-id dev_visual_smoke_joiner \
  --display-name "Sam Rivera" \
  --note "Joining the product team workspace" \
  --source-type workspace_card \
  --source-display-name "Mira Chen" \
  > "$artifacts_dir/join-request.json"

if [ "${CHAFT_VISUAL_SMOKE_REQUEST_LOST_INVITE:-0}" = "1" ]; then
  "$cli_bin" --data-dir "$runtime_dir" record-join-request \
    --workspace-id "$workspace_id" \
    --request-id req_visual_smoke_request_lost \
    --device-id dev_visual_smoke_request_lost \
    --display-name "Riley Chen" \
    --note "Needs new link" \
    --source-type workspace_card \
    --source-display-name "Mira Chen" \
    > "$artifacts_dir/request-lost-invite-request.json"

  "$cli_bin" --data-dir "$runtime_dir" record-invite \
    --workspace-id "$workspace_id" \
    --invite-id inv_visual_smoke_request_lost \
    --device-id dev_visual_smoke_request_lost \
    --display-name "Riley Chen" \
    --role member \
    --request-id req_visual_smoke_request_lost \
    --expires-at "$smoke_expires_at" \
    --approval-policy preapproved \
    --sync-expectation needs_reachable_teammate \
    > "$artifacts_dir/request-lost-invite-record.json"
fi

if [ "${CHAFT_VISUAL_SMOKE_REINVITE_REQUEST:-0}" = "1" ]; then
  "$cli_bin" --data-dir "$runtime_dir" record-join-request \
    --workspace-id "$workspace_id" \
    --request-id req_visual_smoke_reinvite \
    --device-id dev_visual_smoke_reinvite \
    --display-name "Mina Park" \
    --note "Needs new invite" \
    --source-type approval_invite \
    --source-invite-id inv_visual_smoke_reinvite_revoked \
    --source-display-name "Mira Chen" \
    --source-approval-policy admin_required \
    > "$artifacts_dir/reinvite-request.json"

  "$cli_bin" --data-dir "$runtime_dir" record-invite \
    --workspace-id "$workspace_id" \
    --invite-id inv_visual_smoke_reinvite_revoked \
    --device-id dev_visual_smoke_reinvite \
    --display-name "Mina Park" \
    --role member \
    --request-id req_visual_smoke_reinvite \
    --expires-at "$smoke_expires_at" \
    --approval-policy preapproved \
    --sync-expectation needs_reachable_teammate \
    > "$artifacts_dir/record-reinvite-revoked.json"

  "$cli_bin" --data-dir "$runtime_dir" resolve-invite \
    --workspace-id "$workspace_id" \
    --invite-id inv_visual_smoke_reinvite_revoked \
    --resolution revoked > "$artifacts_dir/resolve-reinvite-revoked.json"
fi

product_json="$artifacts_dir/product-channel.json"
"$cli_bin" --data-dir "$runtime_dir" create-channel \
  --workspace-id "$workspace_id" \
  --name product > "$product_json"
product_channel_id="$(json_field "$product_json" channelId)"

design_json="$artifacts_dir/design-channel.json"
"$cli_bin" --data-dir "$runtime_dir" create-channel \
  --workspace-id "$workspace_id" \
  --name design > "$design_json"
design_channel_id="$(json_field "$design_json" channelId)"

p2p_json="$artifacts_dir/p2p-lab-channel.json"
"$cli_bin" --data-dir "$runtime_dir" create-channel \
  --workspace-id "$workspace_id" \
  --name p2p-lab > "$p2p_json"
p2p_channel_id="$(json_field "$p2p_json" channelId)"

vault_json="$artifacts_dir/vault-channel.json"
"$cli_bin" --data-dir "$runtime_dir" create-channel \
  --workspace-id "$workspace_id" \
  --name vault \
  --private > "$vault_json"
vault_channel_id="$(json_field "$vault_json" channelId)"

parent_json="$artifacts_dir/parent-message.json"
"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$general_channel_id" \
  --text "deterministic launch board ready" > "$parent_json"
parent_message_id="$(json_field "$parent_json" messageId)"

reply_json="$artifacts_dir/reply-message.json"
"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$general_channel_id" \
  --reply-to "$parent_message_id" \
  --text "replying with local-first context" > "$reply_json"
reply_message_id="$(json_field "$reply_json" messageId)"

product_expected_text="product room tracks fast native chat"
design_expected_text="design room keeps dense timeline polish"
p2p_expected_text="p2p lab expects partial replica recovery"
vault_expected_text="vault note stays local to authorized devices"

"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$product_channel_id" \
  --text "$product_expected_text" > "$artifacts_dir/product-message.json"

"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$design_channel_id" \
  --text "$design_expected_text" > "$artifacts_dir/design-message.json"

"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$p2p_channel_id" \
  --text "$p2p_expected_text" > "$artifacts_dir/p2p-lab-message.json"

"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$vault_channel_id" \
  --text "$vault_expected_text" > "$artifacts_dir/vault-message.json"

attachment_file="$artifacts_dir/launch-checklist.txt"
cat > "$attachment_file" <<'EOF'
Chaft visual smoke checklist
- local-first send
- reply threading
- encrypted attachment
- private room
- desktop hydration
EOF

attachment_json="$artifacts_dir/attachment-message.json"
"$cli_bin" --data-dir "$runtime_dir" send-attachment \
  --workspace-id "$workspace_id" \
  --channel-id "$general_channel_id" \
  --text "attached launch checklist" \
  --file "$attachment_file" \
  --media-type text/plain > "$attachment_json"
attachment_message_id="$(json_field "$attachment_json" messageId)"

"$cli_bin" --data-dir "$runtime_dir" edit-message \
  --workspace-id "$workspace_id" \
  --message-id "$parent_message_id" \
  --text "deterministic launch board ready, edited" > "$artifacts_dir/edit-parent.json"

"$cli_bin" --data-dir "$runtime_dir" add-reaction \
  --workspace-id "$workspace_id" \
  --message-id "$parent_message_id" \
  --reaction "+1" > "$artifacts_dir/add-reaction.json"

delete_target_json="$artifacts_dir/delete-target-message.json"
"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$general_channel_id" \
  --text "delete me after smoke validation" > "$delete_target_json"
deleted_message_id="$(json_field "$delete_target_json" messageId)"

"$cli_bin" --data-dir "$runtime_dir" delete-message \
  --workspace-id "$workspace_id" \
  --message-id "$deleted_message_id" > "$artifacts_dir/delete-message.json"

"$cli_bin" --data-dir "$runtime_dir" mark-channel-read \
  --workspace-id "$workspace_id" \
  --channel-id "$design_channel_id" > "$artifacts_dir/mark-design-read.json"

desktop_message_json="$artifacts_dir/desktop-message.json"
"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$general_channel_id" \
  --text "$desktop_expected_text" > "$desktop_message_json"
desktop_message_id="$(json_field "$desktop_message_json" messageId)"

if [ "${CHAFT_VISUAL_SMOKE_ARCHIVE_DESIGN:-0}" = "1" ]; then
  "$cli_bin" --data-dir "$runtime_dir" archive-channel \
    --workspace-id "$workspace_id" \
    --channel-id "$design_channel_id" > "$artifacts_dir/archive-design-channel.json"
fi

snapshot_json="$artifacts_dir/snapshot-decrypted.json"
"$cli_bin" --data-dir "$runtime_dir" snapshot \
  --workspace-id "$workspace_id" \
  --decrypt > "$snapshot_json"

validate_snapshot \
  "$snapshot_json" \
  "$parent_message_id" \
  "$reply_message_id" \
  "$attachment_message_id" \
  "$deleted_message_id" \
  "$desktop_expected_text"

if [ "${CHAFT_VISUAL_SMOKE_ARCHIVE_DESIGN:-0}" = "1" ]; then
  assert_json "$snapshot_json" \
    'any(channel.get("name") == "design" and channel.get("archived") is True for channel in data.get("channels", []))' \
    'visual smoke did not archive the design room'
fi

if [ "${CHAFT_VISUAL_SMOKE_LOST_INVITE:-0}" = "1" ]; then
  assert_json "$snapshot_json" \
    'any(invite.get("inviteId") == "inv_visual_smoke_lost" and invite.get("status") == "invited" for invite in data.get("invites", []))' \
    'visual smoke did not include the lost still-open invite'
fi

search_json="$artifacts_dir/search-deterministic.json"
"$cli_bin" --data-dir "$runtime_dir" search-workspace \
  --workspace-id "$workspace_id" \
  --query "deterministic" > "$search_json"
assert_json "$search_json" \
  'len(data["hits"]) >= 1 and any("deterministic launch board" in item.get("body", "") for item in data["hits"])' \
  'visual smoke search did not find the deterministic edited message'

status_json="$artifacts_dir/storage-health.json"
"$cli_bin" --data-dir "$runtime_dir" storage-health \
  --workspace-id "$workspace_id" > "$status_json"
assert_json "$status_json" \
  'data["corruptEventCount"] == 0 and data["nonServableParseableEventCount"] == 0' \
  'visual smoke storage health reported corrupt or non-servable events'

manifest_json="$smoke_dir/manifest.json"
"$python_bin" - "$manifest_json" "$runtime_dir" "$artifacts_dir" "$workspace_id" \
  "$general_channel_id" "$product_channel_id" "$design_channel_id" \
  "$p2p_channel_id" "$vault_channel_id" "$parent_message_id" \
  "$reply_message_id" "$attachment_message_id" "$deleted_message_id" \
  "$desktop_message_id" "$desktop_expected_text" "$product_expected_text" \
  "$design_expected_text" "$p2p_expected_text" "$vault_expected_text" \
  "$snapshot_json" "$search_json" "$status_json" <<'PY'
import json
import sys

(
    manifest_path,
    runtime_dir,
    artifacts_dir,
    workspace_id,
    general_channel_id,
    product_channel_id,
    design_channel_id,
    p2p_channel_id,
    vault_channel_id,
    parent_message_id,
    reply_message_id,
    attachment_message_id,
    deleted_message_id,
    desktop_message_id,
    desktop_expected_text,
    product_expected_text,
    design_expected_text,
    p2p_expected_text,
    vault_expected_text,
    snapshot_json,
    search_json,
    status_json,
) = sys.argv[1:]

manifest = {
    "runtimeDir": runtime_dir,
    "artifactsDir": artifacts_dir,
    "workspaceId": workspace_id,
    "workspaceName": "Chaft Visual Smoke",
    "desktopExpectedText": desktop_expected_text,
    "channels": {
        "general": general_channel_id,
        "product": product_channel_id,
        "design": design_channel_id,
        "p2pLab": p2p_channel_id,
        "vault": vault_channel_id,
    },
    "channelExpectedText": {
        "general": desktop_expected_text,
        "product": product_expected_text,
        "design": design_expected_text,
        "p2pLab": p2p_expected_text,
        "vault": vault_expected_text,
    },
    "messages": {
        "editedParent": parent_message_id,
        "reply": reply_message_id,
        "attachment": attachment_message_id,
        "deleted": deleted_message_id,
        "desktopExpected": desktop_message_id,
    },
    "snapshotJson": snapshot_json,
    "searchJson": search_json,
    "storageHealthJson": status_json,
}

with open(manifest_path, "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")

print(json.dumps(manifest, indent=2))
PY

printf 'visual smoke workspace passed: workspace=%s runtime=%s\n' \
  "$workspace_id" "$runtime_dir" >&2
