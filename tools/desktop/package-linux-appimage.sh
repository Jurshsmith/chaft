#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
. "$script_dir/common.sh"

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

chaft_desktop_add_tool_paths
qt_prefix="${QTDIR:-${QT_ROOT_DIR:-}}"
if [ -z "$qt_prefix" ]; then
  qt_prefix="$(chaft_desktop_qt_prefix || true)"
fi
if [ -z "$qt_prefix" ]; then
  printf 'unable to resolve the Qt prefix for AppImage packaging\n' >&2
  exit 1
fi
qt_prefix="$(CDPATH= cd "$qt_prefix" && pwd)"
qt_library_dir="$qt_prefix/lib"
qt_qmake="$qt_prefix/bin/qmake6"
qt_quick_library="$qt_library_dir/libQt6Quick.so.6"
qt_xcb_runtime_check="$script_dir/check-qt-xcb-runtime.sh"
if [ ! -d "$qt_library_dir" ]; then
  printf 'Qt library directory not found: %s\n' "$qt_library_dir" >&2
  exit 1
fi
if [ ! -x "$qt_qmake" ]; then
  printf 'Qt qmake executable not found: %s\n' "$qt_qmake" >&2
  exit 1
fi
if [ ! -f "$qt_quick_library" ]; then
  printf 'Qt Quick library not found: %s\n' "$qt_quick_library" >&2
  exit 1
fi
if [ ! -x "$qt_xcb_runtime_check" ]; then
  printf 'Qt XCB runtime check is not executable: %s\n' \
    "$qt_xcb_runtime_check" >&2
  exit 1
fi

"$qt_xcb_runtime_check" "$qt_prefix"

source_version="$(
  python3 "$script_dir/release-version.py" --print-source-version
)"
distribution_version="${CHAFT_DISTRIBUTION_VERSION:-$source_version}"
distribution_version="$(
  python3 "$script_dir/release-version.py" \
    --distribution-version "$distribution_version" \
    --print-distribution-version
)"
build_dir="$repo_root/build/$preset"
package_dir="$build_dir/package"
work_dir="$build_dir/appimage"
app_dir="$work_dir/Chaft.AppDir"
tool_dir="$work_dir/tools"
output_path="$package_dir/Chaft-$distribution_version-Linux-$architecture.AppImage"

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
VERSION="$distribution_version" \
OUTPUT="$output_path" \
APPIMAGE_EXTRACT_AND_RUN=1 \
LD_LIBRARY_PATH="$qt_library_dir" \
QMAKE="$qt_qmake" \
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

if [ ! -f "$app_dir/usr/lib/libQt6Quick.so.6" ]; then
  rm -f "$output_path"
  printf 'linuxdeploy did not bundle Qt Quick from %s\n' \
    "$qt_library_dir" >&2
  exit 1
fi

missing_xcb_library=""
for xcb_library in \
  libxcb-cursor.so.0 \
  libxcb-glx.so.0 \
  libxcb-icccm.so.4 \
  libxcb-image.so.0 \
  libxcb-keysyms.so.1 \
  libxcb-randr.so.0 \
  libxcb-render.so.0 \
  libxcb-render-util.so.0 \
  libxcb-shape.so.0 \
  libxcb-shm.so.0 \
  libxcb-sync.so.1 \
  libxcb-util.so.1 \
  libxcb-xfixes.so.0 \
  libxcb-xkb.so.1 \
  libxkbcommon.so.0 \
  libxkbcommon-x11.so.0
do
  if [ ! -f "$app_dir/usr/lib/$xcb_library" ]; then
    missing_xcb_library="${missing_xcb_library}
  $xcb_library"
  fi
done
if [ -n "$missing_xcb_library" ]; then
  rm -f "$output_path"
  printf 'linuxdeploy did not bundle required XCB/XKB libraries:%s\n' \
    "$missing_xcb_library" >&2
  exit 1
fi

for host_gl_pattern in \
  'libEGL.so*' \
  'libGL.so*' \
  'libGLdispatch.so*' \
  'libGLX.so*' \
  'libOpenGL.so*'
do
  host_gl_library="$(
    find "$app_dir/usr/lib" -maxdepth 1 \
      \( -type f -o -type l \) \
      -name "$host_gl_pattern" -print -quit
  )"
  if [ -n "$host_gl_library" ]; then
    rm -f "$output_path"
    printf 'AppImage must use the host GL dispatch library: %s\n' \
      "$host_gl_library" >&2
    exit 1
  fi
done

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
