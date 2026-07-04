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

print(value)
PY
}

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

require_tool cargo
require_tool python3

"$script_dir/build.sh" "$profile" >/dev/null

ffi_library_name="$(chaft_desktop_ffi_library_name)"
ffi_library="$repo_root/target/$rust_target_dir/$ffi_library_name"
desktop_binary="$(chaft_desktop_find_binary "$repo_root" "desktop-$profile" || true)"
build_qml_root="$repo_root/build/desktop-$profile/apps/desktop-qt/Chaft/qml"
build_qml_file="$build_qml_root/Chaft/App.qml"

if [ ! -x "$desktop_binary" ]; then
  printf 'desktop binary not found for %s build\n' "$profile" >&2
  exit 1
fi

cli_bin="$repo_root/target/$rust_target_dir/$(chaft_desktop_cli_binary_name)"
if [ ! -x "$cli_bin" ]; then
  cargo build -p chaft-cli $cargo_profile
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
    ;;
esac

CHAFT_FFI_LIBRARY="$ffi_library" \
CHAFT_DESKTOP_QML_FILE="$build_qml_file" \
CHAFT_DESKTOP_QML_IMPORT_ROOT="$build_qml_root" \
CHAFT_RUNTIME_DIR="$runtime_dir" \
CHAFT_WORKSPACE_ID="$workspace_id" \
CHAFT_DESKTOP_SMOKE=1 \
CHAFT_DESKTOP_SMOKE_EXPECT_TEXT="$expected_text" \
CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}" \
  "$desktop_binary"
