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
  "$python_bin" - "$file" "$path" <<'PY'
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
  "$python_bin" - "$file" "$expression" "$message" <<'PY'
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
PY
}

expect_failure_contains() {
  pattern="$1"
  shift
  stdout_file="$smoke_dir/expected-failure.out"
  stderr_file="$smoke_dir/expected-failure.err"
  rm -f "$stdout_file" "$stderr_file"

  if "$@" >"$stdout_file" 2>"$stderr_file"; then
    printf 'expected command to fail: %s\n' "$*" >&2
    exit 1
  fi

  if ! grep -Eiq "$pattern" "$stdout_file" "$stderr_file"; then
    printf 'expected failure matching %s, got:\n' "$pattern" >&2
    cat "$stdout_file" "$stderr_file" >&2
    exit 1
  fi
}

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

require_tool cargo
require_tool grep
require_tool "$python_bin"

cargo_args="$*"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
case "$target_dir" in
  /*) ;;
  *) target_dir="$repo_root/$target_dir" ;;
esac

cd "$repo_root"
cargo build -p chaft-cli $cargo_args

cli_bin="$target_dir/debug/chaft-cli"
if [ ! -x "$cli_bin" ]; then
  printf 'missing expected smoke binary under %s/debug\n' "$target_dir" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-lifecycle-smoke.XXXXXX")"
trap cleanup EXIT INT TERM

runtime_dir="$smoke_dir/runtime"
mkdir -p "$runtime_dir"

owner_cli() {
  "$cli_bin" --data-dir "$runtime_dir" "$@"
}

admin_identity="$smoke_dir/admin-device.json"
member_identity="$smoke_dir/member-device.json"
guest_identity="$smoke_dir/guest-device.json"
candidate_identity="$smoke_dir/candidate-device.json"
declined_identity="$smoke_dir/declined-device.json"
outside_identity="$smoke_dir/outside-device.json"

admin_cli() {
  "$cli_bin" --data-dir "$runtime_dir" --identity-file "$admin_identity" "$@"
}

member_cli() {
  "$cli_bin" --data-dir "$runtime_dir" --identity-file "$member_identity" "$@"
}

guest_cli() {
  "$cli_bin" --data-dir "$runtime_dir" --identity-file "$guest_identity" "$@"
}

identity_device_id() {
  identity_file="$1"
  "$cli_bin" --data-dir "$runtime_dir" --identity-file "$identity_file" device-id
}

owner_device_id="$(owner_cli device-id)"
admin_device_id="$(identity_device_id "$admin_identity")"
member_device_id="$(identity_device_id "$member_identity")"
guest_device_id="$(identity_device_id "$guest_identity")"
candidate_device_id="$(identity_device_id "$candidate_identity")"
declined_device_id="$(identity_device_id "$declined_identity")"
outside_device_id="$(identity_device_id "$outside_identity")"

created_json="$smoke_dir/created.json"
owner_cli init-workspace \
  --name "Chaft Lifecycle Smoke" \
  --channel general \
  --access-policy invite-only > "$created_json"

workspace_id="$(json_field "$created_json" workspaceId)"
channel_id="$(json_field "$created_json" channelId)"

owner_cli invite-member \
  --workspace-id "$workspace_id" \
  --device-id "$admin_device_id" \
  --role admin > "$smoke_dir/invite-admin.json"
owner_cli invite-member \
  --workspace-id "$workspace_id" \
  --device-id "$member_device_id" \
  --role member > "$smoke_dir/invite-member.json"
owner_cli invite-member \
  --workspace-id "$workspace_id" \
  --device-id "$guest_device_id" \
  --role guest > "$smoke_dir/invite-guest.json"

admin_cli update-workspace-access-policy \
  --workspace-id "$workspace_id" \
  --access-policy request-access > "$smoke_dir/admin-access-policy.json"

expect_failure_contains 'insufficient role|manage_workspace_settings' \
  member_cli update-workspace-access-policy \
  --workspace-id "$workspace_id" \
  --access-policy discoverable

expect_failure_contains 'insufficient role|invite_member' \
  member_cli invite-member \
  --workspace-id "$workspace_id" \
  --device-id "$outside_device_id" \
  --role member

admin_cli update-member-role \
  --workspace-id "$workspace_id" \
  --device-id "$guest_device_id" \
  --role member > "$smoke_dir/admin-promote-guest.json"

expect_failure_contains 'insufficient role|manage_privileged_roles' \
  admin_cli update-member-role \
  --workspace-id "$workspace_id" \
  --device-id "$member_device_id" \
  --role admin

owner_cli update-member-role \
  --workspace-id "$workspace_id" \
  --device-id "$member_device_id" \
  --role admin > "$smoke_dir/owner-promote-member.json"

expect_failure_contains 'insufficient role|manage_privileged_roles' \
  admin_cli update-member-role \
  --workspace-id "$workspace_id" \
  --device-id "$member_device_id" \
  --role member

owner_cli update-member-role \
  --workspace-id "$workspace_id" \
  --device-id "$member_device_id" \
  --role member > "$smoke_dir/owner-demote-member.json"

expect_failure_contains 'root.*owner|must remain an owner' \
  owner_cli update-member-role \
  --workspace-id "$workspace_id" \
  --device-id "$owner_device_id" \
  --role admin

admin_cli record-join-request \
  --workspace-id "$workspace_id" \
  --request-id req_lifecycle_candidate \
  --device-id "$candidate_device_id" \
  --display-name "Rina Candidate" \
  --note "requesting access from smoke" \
  --source-type workspace_card \
  --source-display-name "Chaft Lifecycle Smoke" \
  --source-approval-policy admin_required > "$smoke_dir/join-request-approved.json"

admin_cli record-invite \
  --workspace-id "$workspace_id" \
  --invite-id inv_lifecycle_candidate \
  --device-id "$candidate_device_id" \
  --display-name "Rina Candidate" \
  --role member \
  --request-id req_lifecycle_candidate \
  --approval-policy admin_required \
  --sync-expectation needs_reachable_teammate > "$smoke_dir/record-invite-approved.json"

admin_cli record-join-request \
  --workspace-id "$workspace_id" \
  --request-id req_lifecycle_declined \
  --device-id "$declined_device_id" \
  --display-name "Drew Declined" \
  --note "declined from smoke" \
  --source-type workspace_card \
  --source-display-name "Chaft Lifecycle Smoke" \
  --source-approval-policy admin_required > "$smoke_dir/join-request-declined.json"

admin_cli resolve-join-request \
  --workspace-id "$workspace_id" \
  --request-id req_lifecycle_declined \
  --resolution declined > "$smoke_dir/resolve-join-request-declined.json"

admin_cli remove-member \
  --workspace-id "$workspace_id" \
  --device-id "$guest_device_id" > "$smoke_dir/admin-remove-guest.json"

expect_failure_contains 'missing role|insufficient role|not.*member|workspace root' \
  guest_cli send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$channel_id" \
  --text "removed member should not publish"

snapshot_json="$smoke_dir/snapshot.json"
owner_cli snapshot \
  --workspace-id "$workspace_id" > "$snapshot_json"

assert_json "$snapshot_json" \
  'data["accessPolicy"] == "request_access"' \
  'workspace access policy was not updated by an admin'
assert_json "$snapshot_json" \
  'any(member.get("deviceId") == "'"$owner_device_id"'" and member.get("role") == "owner" for member in data["members"])' \
  'owner was not retained as owner'
assert_json "$snapshot_json" \
  'any(member.get("deviceId") == "'"$admin_device_id"'" and member.get("role") == "admin" for member in data["members"])' \
  'admin invite did not produce an admin member'
assert_json "$snapshot_json" \
  'any(member.get("deviceId") == "'"$member_device_id"'" and member.get("role") == "member" for member in data["members"])' \
  'owner demotion did not restore the member role'
assert_json "$snapshot_json" \
  'not any(member.get("deviceId") == "'"$guest_device_id"'" for member in data["members"])' \
  'admin removal did not remove the regular member'
assert_json "$snapshot_json" \
  'any(request.get("requestId") == "req_lifecycle_candidate" and request.get("status") == "approved" for request in data["joinRequests"])' \
  'linked admin invite did not approve the join request'
assert_json "$snapshot_json" \
  'any(request.get("requestId") == "req_lifecycle_declined" and request.get("status") == "declined" for request in data["joinRequests"])' \
  'admin decline did not mark the join request declined'
assert_json "$snapshot_json" \
  'any(invite.get("inviteId") == "inv_lifecycle_candidate" and invite.get("requestId") == "req_lifecycle_candidate" and invite.get("role") == "member" for invite in data["invites"])' \
  'approved request invite was not recorded in snapshot'

printf 'workspace lifecycle smoke passed: workspace=%s\n' "$workspace_id"
