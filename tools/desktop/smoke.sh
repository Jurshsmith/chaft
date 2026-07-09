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

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

require_tool cargo
require_tool "$python_bin"

"$script_dir/build.sh" "$profile"

ffi_library_name="$(chaft_desktop_ffi_library_name)"
ffi_library="$repo_root/target/$rust_target_dir/$ffi_library_name"
desktop_binary="$(chaft_desktop_find_binary "$repo_root" "desktop-$profile" || true)"
source_qml_root="$repo_root/apps/desktop-qt/qml"

if [ ! -x "$desktop_binary" ]; then
  printf 'desktop binary not found for %s build\n' "$profile" >&2
  exit 1
fi

cli_bin="$repo_root/target/$rust_target_dir/$(chaft_desktop_cli_binary_name)"
cargo build -p chaft-cli $cargo_profile
if [ ! -x "$cli_bin" ]; then
  printf 'missing chaft-cli binary: %s\n' "$cli_bin" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-desktop-smoke.XXXXXX")"
trap cleanup EXIT INT TERM

manifest_json="$smoke_dir/visual-workspace.json"
CHAFT_CLI_BIN="$cli_bin" \
CHAFT_VISUAL_SMOKE_DIR="$smoke_dir/visual-workspace" \
  "$repo_root/tools/smoke/visual-workspace.sh" > "$manifest_json"

runtime_dir="$(json_field "$manifest_json" runtimeDir)"
workspace_id="$(json_field "$manifest_json" workspaceId)"
expected_text="$(json_field "$manifest_json" desktopExpectedText)"

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
    if [ -n "${CHAFT_DESKTOP_SMOKE_SCREENSHOT:-}" ] \
        && [ -z "${QT_QUICK_BACKEND:-}" ]; then
      case "${CHAFT_SMOKE_UI_STATE:-}" in
        drawer|setup-backup|private-channel-repair-failed|private-channel-repair-saved)
          # QQuickWindow::grabWindow can hang with the hardware renderer on a
          # small set of expanded/failure visual states on macOS. Software plus
          # the async item capture path avoids leaving these states to the outer
          # harness watchdog without changing the production launch path.
          export QT_QUICK_BACKEND=software
          if [ "${CHAFT_SMOKE_UI_STATE:-}" = "drawer" ] \
              || [ "${CHAFT_SMOKE_UI_STATE:-}" = "setup-backup" ] \
              || [ "${CHAFT_SMOKE_UI_STATE:-}" = "private-channel-repair-failed" ] \
              || [ "${CHAFT_SMOKE_UI_STATE:-}" = "private-channel-repair-saved" ]; then
            quick_item_capture="${CHAFT_DESKTOP_SMOKE_QUICK_ITEM_CAPTURE:-1}"
            export CHAFT_DESKTOP_SMOKE_QUICK_ITEM_CAPTURE="$quick_item_capture"
          fi
          ;;
      esac
    fi
    ;;
esac

# The in-app timeout cannot fire if Qt blocks inside synchronous screenshot
# capture, so keep an outer harness watchdog around focused visual states.
watchdog_seconds=$(( ($(smoke_timeout_ms) + $(smoke_screenshot_delay_ms) + $(smoke_watchdog_margin_ms) + 999) / 1000 ))
desktop_status=0

if command -v perl >/dev/null 2>&1; then
  CHAFT_FFI_LIBRARY="$ffi_library" \
  CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
  CHAFT_RUNTIME_DIR="$runtime_dir" \
  CHAFT_WORKSPACE_ID="$workspace_id" \
  CHAFT_DESKTOP_SMOKE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_TEXT="$expected_text" \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}" \
    perl -e 'alarm shift @ARGV; exec @ARGV' "$watchdog_seconds" \
      "$desktop_binary" || desktop_status=$?
else
  CHAFT_FFI_LIBRARY="$ffi_library" \
  CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
  CHAFT_RUNTIME_DIR="$runtime_dir" \
  CHAFT_WORKSPACE_ID="$workspace_id" \
  CHAFT_DESKTOP_SMOKE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_TEXT="$expected_text" \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}" \
    "$desktop_binary" || desktop_status=$?
fi

if [ "$desktop_status" -eq 142 ]; then
  printf 'desktop smoke harness timed out after %ss\n' "$watchdog_seconds" >&2
  exit 124
fi
exit "$desktop_status"
