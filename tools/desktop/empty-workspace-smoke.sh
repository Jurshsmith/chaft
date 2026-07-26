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
    ;;
  release)
    rust_target_dir=release
    ;;
  *)
    printf 'usage: %s [debug|release] [output.png]\n' "$0" >&2
    exit 2
    ;;
esac

output_path="${2:-${CHAFT_DESKTOP_SMOKE_SCREENSHOT:-}}"

python_bin="${CHAFT_PYTHON_BIN:-}"
if [ -z "$python_bin" ]; then
  if [ -x /usr/bin/python3 ]; then
    python_bin=/usr/bin/python3
  else
    python_bin=python3
  fi
fi

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept empty workspace smoke directory: %s\n' "$smoke_dir"
  fi
}

smoke_timeout_ms() {
  value="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}"
  case "$value" in
    ''|*[!0-9]*)
      value=15000
      ;;
  esac
  if [ "$value" -lt 1000 ]; then
    value=1000
  elif [ "$value" -gt 60000 ]; then
    value=60000
  fi
  printf '%s\n' "$value"
}

smoke_screenshot_delay_ms() {
  value="${CHAFT_DESKTOP_SMOKE_SCREENSHOT_DELAY_MS:-0}"
  case "$value" in
    ''|*[!0-9]*)
      value=0
      ;;
  esac
  if [ "$value" -lt 0 ]; then
    value=0
  elif [ "$value" -gt 10000 ]; then
    value=10000
  fi
  printf '%s\n' "$value"
}

smoke_watchdog_margin_ms() {
  value="${CHAFT_DESKTOP_SMOKE_WATCHDOG_MARGIN_MS:-20000}"
  case "$value" in
    ''|*[!0-9]*)
      value=20000
      ;;
  esac
  if [ "$value" -lt 5000 ]; then
    value=5000
  elif [ "$value" -gt 60000 ]; then
    value=60000
  fi
  printf '%s\n' "$value"
}

"$script_dir/build.sh" "$profile"

ffi_library="$repo_root/target/$rust_target_dir/$(chaft_desktop_ffi_library_name)"
desktop_binary="$(chaft_desktop_find_binary "$repo_root" "desktop-$profile" || true)"
source_qml_root="$repo_root/apps/desktop-qt/qml"

if [ ! -x "$desktop_binary" ]; then
  printf 'desktop binary not found for %s build\n' "$profile" >&2
  chaft_desktop_binary_candidates "$repo_root" "desktop-$profile" >&2
  exit 1
fi
if [ ! -f "$ffi_library" ]; then
  printf 'FFI library not found: %s\n' "$ffi_library" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-empty-workspace-smoke.XXXXXX")"
trap cleanup EXIT INT TERM
runtime_dir="$smoke_dir/runtime"
mkdir -p "$runtime_dir"
desktop_launch_binary="$(chaft_desktop_prepare_smoke_binary "$desktop_binary" "$smoke_dir")"

if [ "${CHAFT_EMPTY_WORKSPACE_PENDING_REQUEST:-0}" = "1" ]; then
  "$python_bin" - "$runtime_dir/desktop.json" <<'PY'
import json
import os
import sys
from pathlib import Path

request = {
    "kind": "chaft.workspace-join-request.v1",
    "schemaVersion": 1,
    "workspaceId": "wrk_visual_smoke",
    "workspaceName": "Visual Smoke",
    "displayName": "Sam Rivera",
    "deviceId": "dev_visual_smoke_joiner",
    "note": "Design team access",
    "deliveryDisplayName": "Mira Chen",
    "deliveryDeviceId": "dev_visual_smoke_admin",
    "deliveryPeerEndpoint": "direct+tcp://127.0.0.1:44944",
    "sourceType": "workspace_card",
    "sourceDisplayName": "Mira Chen",
    "createdAt": "2026-07-07T12:00:00Z",
}
artifact = {
    "kind": "chaft.join-request-file.v1",
    "schemaVersion": 1,
    "workspaceId": request["workspaceId"],
    "workspaceName": request["workspaceName"],
    "displayName": request["displayName"],
    "deviceId": request["deviceId"],
    "note": request["note"],
    "deliveryDisplayName": request["deliveryDisplayName"],
    "deliveryDeviceId": request["deliveryDeviceId"],
    "deliveryPeerEndpoint": request["deliveryPeerEndpoint"],
    "sourceType": request["sourceType"],
    "sourceDisplayName": request["sourceDisplayName"],
    "createdAt": request["createdAt"],
    "request": request,
}
status = os.environ.get(
    "CHAFT_EMPTY_WORKSPACE_PENDING_REQUEST_STATUS", "ready_to_send"
).strip()
if status not in {"ready_to_send", "sent", "send_failed"}:
    status = "ready_to_send"
sent_at = "2026-07-07T12:03:00Z" if status == "sent" else ""
last_attempt_at = "2026-07-07T12:03:00Z" if status in {"sent", "send_failed"} else ""
config = {
    "pendingJoinRequests": {
        request["workspaceId"]: {
            "workspaceId": request["workspaceId"],
            "workspaceName": request["workspaceName"],
            "displayName": request["displayName"],
            "deliveryDisplayName": request["deliveryDisplayName"],
            "deliveryDeviceId": request["deliveryDeviceId"],
            "deliveryPeerEndpoint": request["deliveryPeerEndpoint"],
            "sourceType": request["sourceType"],
            "sourceDisplayName": request["sourceDisplayName"],
            "status": status,
            "createdAt": request["createdAt"],
            "sentAt": sent_at,
            "lastAttemptAt": last_attempt_at,
            "error": "Teammate was not reachable" if status == "send_failed" else "",
            "artifact": json.dumps(artifact, indent=2),
        }
    }
}
Path(sys.argv[1]).write_text(json.dumps(config, indent=2), encoding="utf-8")
PY
fi

case "$(uname -s)" in
  Linux)
    if [ -z "${DISPLAY:-}" ]; then
      export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"
    fi
    ;;
  Darwin)
    if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
      export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"
    fi
    ;;
  MINGW*|MSYS*|CYGWIN*)
    if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
      export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"
    fi
    ;;
esac

# The in-app timeout cannot fire if Qt blocks inside synchronous screenshot
# capture, so keep the no-workspace helper bounded like the main smoke helper.
watchdog_seconds=$(( ($(smoke_timeout_ms) + $(smoke_screenshot_delay_ms) + $(smoke_watchdog_margin_ms) + 999) / 1000 ))
desktop_status=0

run_desktop_with_watchdog() {
  watchdog_marker="$smoke_dir/watchdog-fired"
  rm -f "$watchdog_marker"
  "$@" &
  desktop_pid=$!
  (
    sleep "$watchdog_seconds"
    if kill -0 "$desktop_pid" 2>/dev/null; then
      : > "$watchdog_marker"
      kill "$desktop_pid" 2>/dev/null || true
    fi
  ) &
  watchdog_pid=$!
  cont_pid=
  if [ "$(uname -s)" = "Darwin" ]; then
    (
      while kill -0 "$desktop_pid" 2>/dev/null; do
        kill -CONT "$desktop_pid" 2>/dev/null || true
        sleep 1
      done
    ) &
    cont_pid=$!
  fi

  command_status=0
  wait "$desktop_pid" || command_status=$?
  kill "$watchdog_pid" 2>/dev/null || true
  wait "$watchdog_pid" 2>/dev/null || true
  if [ -n "$cont_pid" ]; then
    kill "$cont_pid" 2>/dev/null || true
    wait "$cont_pid" 2>/dev/null || true
  fi
  if [ -f "$watchdog_marker" ]; then
    rm -f "$watchdog_marker"
    return 142
  fi
  return "$command_status"
}

run_desktop_with_watchdog \
  env \
  CHAFT_FFI_LIBRARY="$ffi_library" \
  CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
  CHAFT_RUNTIME_DIR="$runtime_dir" \
  CHAFT_DESKTOP_SMOKE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE=1 \
  CHAFT_DESKTOP_SMOKE_SCREENSHOT="$output_path" \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}" \
  "$desktop_launch_binary" || desktop_status=$?

if [ "$desktop_status" -eq 142 ]; then
  printf 'empty workspace smoke harness timed out after %ss\n' "$watchdog_seconds" >&2
  exit 124
fi
exit "$desktop_status"
