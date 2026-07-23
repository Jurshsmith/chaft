#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

usage() {
  printf 'usage: %s [debug|release]\n' "$0" >&2
}

profile="${1:-release}"
case "$profile" in
  debug|release) preset="desktop-$profile" ;;
  *)
    usage
    exit 2
    ;;
esac

if [ "$(uname -s)" != "Linux" ]; then
  printf 'AppImage packaging is supported only on Linux\n' >&2
  exit 1
fi

case "$(uname -m)" in
  x86_64|amd64) architecture=x86_64 ;;
  *)
    printf 'unsupported AppImage architecture: %s\n' "$(uname -m)" >&2
    exit 1
    ;;
esac

require_executable_path() {
  variable_name="$1"
  candidate="$2"
  if [ -z "$candidate" ] || [ ! -x "$candidate" ]; then
    printf '%s must name an executable file\n' "$variable_name" >&2
    exit 1
  fi
  readlink -f "$candidate"
}

linuxdeploy="$(
  require_executable_path CHAFT_LINUXDEPLOY "${CHAFT_LINUXDEPLOY:-}"
)"
linuxdeploy_plugin_qt="$(
  require_executable_path \
    CHAFT_LINUXDEPLOY_PLUGIN_QT "${CHAFT_LINUXDEPLOY_PLUGIN_QT:-}"
)"
linuxdeploy_plugin_appimage="$(
  require_executable_path \
    CHAFT_LINUXDEPLOY_PLUGIN_APPIMAGE "${CHAFT_LINUXDEPLOY_PLUGIN_APPIMAGE:-}"
)"

version="$(python3 "$script_dir/release-version.py" --print-version)"
build_dir="$repo_root/build/$preset"
package_dir="$build_dir/package"
work_dir="$build_dir/appimage"
app_dir="$work_dir/Chaft.AppDir"
tool_dir="$work_dir/tools"
output_path="$package_dir/Chaft-$version-$architecture.AppImage"

rm -rf "$work_dir" "$package_dir"
mkdir -p "$app_dir" "$package_dir" "$tool_dir"

DESTDIR="$app_dir" cmake --install "$build_dir" --prefix /usr

desktop_binary="$app_dir/usr/bin/ChaftDesktop"
ffi_library="$app_dir/usr/lib/libchaft_ffi.so"
desktop_file="$app_dir/usr/share/applications/io.github.jurshsmith.chaft.desktop"
icon_file="$app_dir/usr/share/icons/hicolor/512x512/apps/io.github.jurshsmith.chaft.png"

for required_file in \
  "$desktop_binary" \
  "$ffi_library" \
  "$desktop_file" \
  "$icon_file"
do
  if [ ! -f "$required_file" ]; then
    printf 'AppImage input is missing: %s\n' "$required_file" >&2
    exit 1
  fi
done

ln -s "$linuxdeploy" "$tool_dir/linuxdeploy"
ln -s "$linuxdeploy_plugin_qt" "$tool_dir/linuxdeploy-plugin-qt"
ln -s "$linuxdeploy_plugin_appimage" \
  "$tool_dir/linuxdeploy-plugin-appimage"

PATH="$tool_dir:$PATH" \
ARCH="$architecture" \
VERSION="$version" \
OUTPUT="$output_path" \
APPIMAGE_EXTRACT_AND_RUN=1 \
EXTRA_PLATFORM_PLUGINS=libqoffscreen.so \
QML_SOURCES_PATHS="$repo_root/apps/desktop-qt/qml" \
  "$tool_dir/linuxdeploy" \
    --appdir "$app_dir" \
    --executable "$desktop_binary" \
    --library "$ffi_library" \
    --desktop-file "$desktop_file" \
    --icon-file "$icon_file" \
    --plugin qt \
    --output appimage

if [ ! -f "$output_path" ]; then
  printf 'linuxdeploy did not create the expected AppImage: %s\n' \
    "$output_path" >&2
  exit 1
fi
chmod 0755 "$output_path"

appimage_count="$(
  find "$package_dir" -maxdepth 1 -type f -name '*.AppImage' | wc -l | tr -d ' '
)"
if [ "$appimage_count" -ne 1 ]; then
  printf 'expected exactly one AppImage in %s, found %s\n' \
    "$package_dir" "$appimage_count" >&2
  exit 1
fi

printf 'AppImage: %s\n' "$output_path"
