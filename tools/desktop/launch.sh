#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
. "$script_dir/common.sh"

usage() {
  cat >&2 <<'EOF'
usage: tools/desktop/launch.sh [debug|release] [options]

Builds Chaft Desktop, prepares a persistent local runtime, and launches the app.

Options:
  --fresh            Recreate launch data before launching.
  --smoke-workspace  Seed and select the deterministic visual smoke workspace.
  --data-dir DIR     Store launch data under DIR. Default: scratch/desktop-test.
  --detached         Start the app in the background and return immediately.
  --no-build         Reuse an existing desktop build and FFI library.
  -h, --help         Show this help.
EOF
}

profile=debug
data_root="${CHAFT_DESKTOP_LAUNCH_DIR:-$repo_root/scratch/desktop-test}"
fresh=0
detached=0
build=1
smoke_workspace="${CHAFT_DESKTOP_LAUNCH_SMOKE:-0}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    debug|release)
      profile="$1"
      ;;
    --fresh)
      fresh=1
      ;;
    --smoke-workspace)
      smoke_workspace=1
      ;;
    --data-dir)
      if [ "$#" -lt 2 ]; then
        printf 'missing value for --data-dir\n' >&2
        exit 2
      fi
      data_root="$2"
      shift
      ;;
    --detached)
      detached=1
      ;;
    --no-build)
      build=0
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage
      exit 2
      ;;
  esac
  shift
done

case "$data_root" in
  /*) ;;
  *) data_root="$repo_root/$data_root" ;;
esac

case "$profile" in
  debug)
    cargo_profile=
    rust_target_dir=debug
    ;;
  release)
    cargo_profile=--release
    rust_target_dir=release
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

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

write_desktop_config() {
  runtime_dir="$1"
  workspace_id="$2"
  python3 - "$runtime_dir" "$workspace_id" <<'PY'
import json
import os
import sys

runtime_dir, workspace_id = sys.argv[1:]
os.makedirs(runtime_dir, exist_ok=True)
config_path = os.path.join(runtime_dir, "desktop.json")
config = {}
if os.path.exists(config_path):
    try:
        with open(config_path, encoding="utf-8") as handle:
            value = json.load(handle)
        if isinstance(value, dict):
            config = value
    except (OSError, json.JSONDecodeError):
        config = {}

if workspace_id:
    config["workspaceId"] = workspace_id
else:
    config.pop("workspaceId", None)
tmp_path = config_path + ".tmp"
with open(tmp_path, "w", encoding="utf-8") as handle:
    json.dump(config, handle, indent=2, sort_keys=True)
    handle.write("\n")
os.replace(tmp_path, config_path)
PY
}

chaft_desktop_add_tool_paths
require_tool python3

if [ "$build" -eq 1 ]; then
  "$script_dir/build.sh" "$profile"
fi

ffi_library="$repo_root/target/$rust_target_dir/$(chaft_desktop_ffi_library_name)"
desktop_binary="$(chaft_desktop_find_binary "$repo_root" "desktop-$profile" || true)"
if [ ! -x "$desktop_binary" ]; then
  printf 'desktop binary not found for %s build\n' "$profile" >&2
  chaft_desktop_binary_candidates "$repo_root" "desktop-$profile" >&2
  exit 1
fi
if [ ! -f "$ffi_library" ]; then
  printf 'FFI library not found: %s\n' "$ffi_library" >&2
  exit 1
fi

manifest_json=
workspace_id=
runtime_dir="$data_root/runtime"

if [ "$smoke_workspace" = "1" ]; then
  cli_bin="$repo_root/target/$rust_target_dir/$(chaft_desktop_cli_binary_name)"
  if [ ! -x "$cli_bin" ]; then
    require_tool cargo
    (cd "$repo_root" && cargo build -p chaft-cli $cargo_profile)
  fi

  manifest_json="$data_root/manifest.json"
  workspace_seed_dir="$data_root/visual-workspace"

  if [ "$fresh" -eq 1 ] || [ ! -f "$manifest_json" ]; then
    mkdir -p "$data_root"
    CHAFT_CLI_BIN="$cli_bin" \
    CHAFT_VISUAL_SMOKE_DIR="$workspace_seed_dir" \
      "$repo_root/tools/smoke/visual-workspace.sh" > "$manifest_json"
  fi

  runtime_dir="$(json_field "$manifest_json" runtimeDir)"
  workspace_id="$(json_field "$manifest_json" workspaceId)"
else
  if [ "$fresh" -eq 1 ]; then
    rm -rf "$runtime_dir"
  fi
  mkdir -p "$runtime_dir"
fi

write_desktop_config "$runtime_dir" "$workspace_id"

source_qml_root="$repo_root/apps/desktop-qt/qml"
log_file="$data_root/desktop-$profile.log"
platform="$(uname -s)"

printf 'desktop binary: %s\n' "$desktop_binary"
printf 'runtime dir: %s\n' "$runtime_dir"
printf 'workspace id: %s\n' "${workspace_id:-"(none)"}"
if [ -n "$manifest_json" ]; then
  printf 'manifest: %s\n' "$manifest_json"
fi
if [ "$detached" -eq 1 ] && [ "$platform" != "Darwin" ]; then
  printf 'log file: %s\n' "$log_file"
fi

if [ "$detached" -eq 1 ]; then
  case "$platform" in
    Darwin)
      app_bundle="$(CDPATH= cd "$(dirname "$desktop_binary")/../.." && pwd)"
      open -n "$app_bundle" --args \
        --ffi-library "$ffi_library" \
        --qml-import-root "$source_qml_root" \
        --runtime-dir "$runtime_dir" \
        --workspace-id "$workspace_id"
      printf 'desktop app: %s\n' "$app_bundle"
      ;;
    *)
      cd "$repo_root"
      nohup env \
        CHAFT_FFI_LIBRARY="$ffi_library" \
        CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
        CHAFT_RUNTIME_DIR="$runtime_dir" \
        CHAFT_WORKSPACE_ID="$workspace_id" \
        "$desktop_binary" > "$log_file" 2>&1 &
      printf 'desktop pid: %s\n' "$!"
      ;;
  esac
else
  cd "$repo_root"
  CHAFT_FFI_LIBRARY="$ffi_library" \
  CHAFT_DESKTOP_QML_IMPORT_ROOT="$source_qml_root" \
  CHAFT_RUNTIME_DIR="$runtime_dir" \
  CHAFT_WORKSPACE_ID="$workspace_id" \
    "$desktop_binary"
fi
