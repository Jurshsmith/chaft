#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
. "$script_dir/common.sh"

chaft_desktop_add_tool_paths

missing=0

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    missing=1
  fi
}

require_tool cargo
require_tool cmake
require_tool ninja

qt_prefix="$(chaft_desktop_qt_prefix || true)"

if [ -z "$qt_prefix" ]; then
  printf 'missing Qt 6 command-line tools: expected qt-cmake or qmake6\n' >&2
  missing=1
fi

qt_version=""
if command -v qtpaths6 >/dev/null 2>&1; then
  qt_version="$(qtpaths6 --qt-version 2>/dev/null | sed -n '1p')"
elif command -v qtpaths >/dev/null 2>&1; then
  qt_version="$(qtpaths --qt-version 2>/dev/null | sed -n '1p')"
elif command -v qmake6 >/dev/null 2>&1; then
  qt_version="$(qmake6 --version | sed -n 's/^Using Qt version \([0-9][0-9.]*\).*$/\1/p' | sed -n '1p')"
fi

if [ -z "$qt_version" ]; then
  printf 'unable to determine the installed Qt version\n' >&2
  missing=1
elif [ "$qt_version" != "6.8.4" ]; then
  printf 'Qt %s is unsupported: Chaft desktop requires exactly Qt 6.8.4\n' "$qt_version" >&2
  missing=1
fi

if [ "$missing" -ne 0 ]; then
  printf 'install Rust, CMake 3.28+, Ninja, and Qt 6.8.4 before building apps/desktop-qt\n' >&2
  exit 1
fi

cargo --version
cmake --version | sed -n '1p'
ninja --version | sed -n '1s/^/ninja /p'
printf 'qt prefix: %s\n' "$qt_prefix"
printf 'Qt version: %s\n' "$qt_version"

if command -v qt-cmake >/dev/null 2>&1; then
  printf 'qt-cmake: %s\n' "$(command -v qt-cmake)"
fi

if command -v qmake6 >/dev/null 2>&1; then
  qmake6 --version
fi
