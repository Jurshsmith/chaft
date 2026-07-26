#!/usr/bin/env sh
set -eu

usage() {
  printf 'usage: %s APPIMAGE_OR_PACKAGE_DIRECTORY\n' "$0" >&2
}

input="${1:-}"
if [ -z "$input" ]; then
  usage
  exit 2
fi

if [ "$(uname -s)" != "Linux" ]; then
  printf 'AppImage smoke testing is supported only on Linux\n' >&2
  exit 1
fi

if [ -d "$input" ]; then
  appimage_count="$(
    find "$input" -maxdepth 1 -type f -name '*.AppImage' | wc -l | tr -d ' '
  )"
  if [ "$appimage_count" -ne 1 ]; then
    printf 'expected exactly one AppImage in %s, found %s\n' \
      "$input" "$appimage_count" >&2
    exit 1
  fi
  appimage="$(find "$input" -maxdepth 1 -type f -name '*.AppImage' -print)"
else
  appimage="$input"
fi

if [ ! -f "$appimage" ]; then
  printf 'AppImage not found: %s\n' "$appimage" >&2
  exit 1
fi

if ! command -v timeout >/dev/null 2>&1; then
  printf 'missing required tool: timeout\n' >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-appimage-smoke.XXXXXX")"
cleanup() {
  rm -rf "$smoke_dir"
}
trap cleanup EXIT INT TERM

launch_dir="$smoke_dir/portable build Ω"
runtime_dir="$smoke_dir/runtime"
home_dir="$smoke_dir/home"
working_dir="$smoke_dir/unrelated cwd"
mkdir -p "$launch_dir" "$runtime_dir" "$home_dir" "$working_dir"

portable_appimage="$launch_dir/Chaft portable.AppImage"
cp "$appimage" "$portable_appimage"
chmod 0755 "$portable_appimage"

extract_dir="$smoke_dir/extracted"
mkdir -p "$extract_dir"
(
  cd "$extract_dir"
  "$portable_appimage" --appimage-extract >/dev/null
)
compliance_dir="$extract_dir/squashfs-root/usr/share/doc/Chaft"
for required_file in \
  LICENSE \
  THIRD_PARTY_NOTICES.txt \
  LICENSE.LGPL3 \
  LICENSE.GPL3 \
  QT-CORRESPONDING-SOURCE.json
do
  if [ ! -f "$compliance_dir/$required_file" ]; then
    printf 'required AppImage package notice is missing: %s\n' \
      "$compliance_dir/$required_file" >&2
    exit 1
  fi
done

unset LD_LIBRARY_PATH
unset QML2_IMPORT_PATH
unset QML_IMPORT_PATH
unset QT_PLUGIN_PATH
unset QT_QPA_PLATFORM_PLUGIN_PATH
unset CHAFT_FFI_LIBRARY
unset QT_ROOT_DIR
unset Qt6_DIR

(
  cd "$working_dir"
  HOME="$home_dir" \
  XDG_CACHE_HOME="$smoke_dir/cache" \
  XDG_CONFIG_HOME="$smoke_dir/config" \
  XDG_DATA_HOME="$smoke_dir/data" \
  APPIMAGE_EXTRACT_AND_RUN=1 \
  QT_QPA_PLATFORM=offscreen \
  CHAFT_RUNTIME_DIR="$runtime_dir" \
  CHAFT_DESKTOP_SMOKE=1 \
  CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE=1 \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS=15000 \
    timeout 45 "$portable_appimage"
)

printf 'portable AppImage smoke passed: %s\n' "$(basename "$appimage")"
