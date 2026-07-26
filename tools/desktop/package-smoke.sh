#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
. "$script_dir/common.sh"

chaft_desktop_add_tool_paths

usage() {
  printf 'usage: %s [debug|release]\n' "$0" >&2
}

profile="${1:-release}"
case "$profile" in
  debug)
    preset=desktop-debug
    cargo_profile=
    rust_target_dir=debug
    ;;
  release)
    preset=desktop-release
    cargo_profile=--release
    rust_target_dir=release
    ;;
  *)
    usage
    exit 2
    ;;
esac

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
  if [ -n "${dmg_mount_dir:-}" ]; then
    hdiutil detach -quiet "$dmg_mount_dir" >/dev/null 2>&1 || true
  fi
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

require_tool cargo
require_tool python3

"$script_dir/package.sh" "$profile"

installed_binary="$(chaft_desktop_find_installed_binary "$repo_root" "$preset" || true)"
if [ ! -x "$installed_binary" ]; then
  printf 'installed desktop binary not found for %s package\n' "$profile" >&2
  chaft_desktop_installed_binary_candidates "$repo_root" "$preset" >&2
  exit 1
fi

cli_bin="$repo_root/target/$rust_target_dir/$(chaft_desktop_cli_binary_name)"
if [ ! -x "$cli_bin" ]; then
  cargo build -p chaft-cli $cargo_profile
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-desktop-package-smoke.XXXXXX")"
trap cleanup EXIT INT TERM

runtime_dir="$smoke_dir/runtime"
mkdir -p "$runtime_dir"
desktop_launch_binary="$(chaft_desktop_prepare_smoke_binary "$installed_binary" "$smoke_dir")"

if [ "$(uname -s)" = "Darwin" ]; then
  require_tool hdiutil
  package_dir="$repo_root/build/$preset/package"
  dmg_count="$(
    find "$package_dir" -maxdepth 1 -type f -name '*.dmg' | wc -l | tr -d ' '
  )"
  if [ "$dmg_count" -ne 1 ]; then
    printf 'expected exactly one DMG in %s, found %s\n' \
      "$package_dir" "$dmg_count" >&2
    exit 1
  fi
  dmg_path="$(find "$package_dir" -maxdepth 1 -type f -name '*.dmg' -print)"
  dmg_mount_dir="$smoke_dir/dmg"
  mkdir -p "$dmg_mount_dir"
  hdiutil attach -readonly -nobrowse -quiet \
    -mountpoint "$dmg_mount_dir" "$dmg_path"
  compliance_dir="$dmg_mount_dir/ChaftDesktop.app/Contents/Resources/doc/Chaft"
  for required_file in \
    LICENSE \
    THIRD_PARTY_NOTICES.txt \
    LICENSE.LGPL3 \
    LICENSE.GPL3 \
    QT-CORRESPONDING-SOURCE.json
  do
    if [ ! -f "$compliance_dir/$required_file" ]; then
      printf 'required macOS package notice is missing: %s\n' \
        "$compliance_dir/$required_file" >&2
      exit 1
    fi
  done
  hdiutil detach -quiet "$dmg_mount_dir"
  dmg_mount_dir=
fi

created_json="$smoke_dir/created.json"
"$cli_bin" --data-dir "$runtime_dir" init-workspace \
  --name "Chaft Package Smoke" \
  --channel general > "$created_json"

workspace_id="$(json_field "$created_json" workspaceId)"
channel_id="$(json_field "$created_json" channelId)"
parent_text="package smoke parent"
expected_text="package smoke reply"

"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$channel_id" \
  --text "$parent_text" > "$smoke_dir/parent-message.json"
parent_message_id="$(json_field "$smoke_dir/parent-message.json" messageId)"

"$cli_bin" --data-dir "$runtime_dir" send-message \
  --workspace-id "$workspace_id" \
  --channel-id "$channel_id" \
  --reply-to "$parent_message_id" \
  --text "$expected_text" > "$smoke_dir/reply-message.json"

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

CHAFT_RUNTIME_DIR="$runtime_dir" \
CHAFT_WORKSPACE_ID="$workspace_id" \
CHAFT_DESKTOP_SMOKE=1 \
CHAFT_DESKTOP_SMOKE_EXPECT_TEXT="$expected_text" \
CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}" \
  "$desktop_launch_binary"

if [ "$(uname -s)" = "Linux" ]; then
  "$script_dir/appimage-smoke.sh" "$repo_root/build/$preset/package"
fi
