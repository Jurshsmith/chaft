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

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept empty workspace smoke directory: %s\n' "$smoke_dir"
  fi
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
CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
CHAFT_RUNTIME_DIR="$runtime_dir" \
CHAFT_DESKTOP_SMOKE=1 \
CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE=1 \
CHAFT_DESKTOP_SMOKE_SCREENSHOT="$output_path" \
CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}" \
  "$desktop_binary"
