#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"

usage() {
  printf 'usage: %s DMG_OR_PACKAGE_DIRECTORY\n' "$0" >&2
}

input="${1:-}"
if [ -z "$input" ]; then
  usage
  exit 2
fi

if [ "$(uname -s)" != "Darwin" ]; then
  printf 'DMG smoke testing is supported only on macOS\n' >&2
  exit 1
fi

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

require_tool ditto
require_tool hdiutil

if [ -d "$input" ]; then
  dmg_count="$(
    find "$input" -maxdepth 1 -type f -name '*.dmg' | wc -l | tr -d ' '
  )"
  if [ "$dmg_count" -ne 1 ]; then
    printf 'expected exactly one DMG in %s, found %s\n' \
      "$input" "$dmg_count" >&2
    exit 1
  fi
  dmg_path="$(find "$input" -maxdepth 1 -type f -name '*.dmg' -print)"
else
  dmg_path="$input"
fi

if [ ! -f "$dmg_path" ]; then
  printf 'DMG not found: %s\n' "$dmg_path" >&2
  exit 1
fi

source_version="$(
  python3 "$script_dir/release-version.py" --print-source-version
)"
if [ -n "${CHAFT_SOURCE_VERSION:-}" ] \
    && [ "$CHAFT_SOURCE_VERSION" != "$source_version" ]; then
  printf 'expected source version %s, repository declares %s\n' \
    "$CHAFT_SOURCE_VERSION" "$source_version" >&2
  exit 1
fi
distribution_version="${CHAFT_DISTRIBUTION_VERSION:-$source_version}"
distribution_version="$(
  python3 "$script_dir/release-version.py" \
    --distribution-version "$distribution_version" \
    --print-distribution-version
)"
expected_name="Chaft-$distribution_version-macOS-x86_64.dmg"
if [ "$(basename "$dmg_path")" != "$expected_name" ]; then
  printf 'expected DMG filename %s, got %s\n' \
    "$expected_name" "$(basename "$dmg_path")" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-macos-dmg-smoke.XXXXXX")"
dmg_mount_dir=
desktop_pid=
watchdog_pid=
continue_pid=

cleanup() {
  for pid in \
    "${continue_pid:-}" \
    "${watchdog_pid:-}" \
    "${desktop_pid:-}"
  do
    if [ -n "$pid" ]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
    fi
  done
  if [ -n "${dmg_mount_dir:-}" ]; then
    hdiutil detach -quiet "$dmg_mount_dir" >/dev/null 2>&1 || true
  fi
  if [ "${CHAFT_KEEP_SMOKE:-0}" = "1" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  else
    rm -rf "$smoke_dir"
  fi
}
trap cleanup EXIT HUP INT TERM

runtime_dir="${CHAFT_RUNTIME_DIR:-$smoke_dir/runtime}"
home_dir="$smoke_dir/home"
working_dir="$smoke_dir/unrelated cwd"
portable_root="$smoke_dir/portable package"
dmg_mount_dir="$smoke_dir/mounted dmg"
mkdir -p \
  "$runtime_dir" \
  "$home_dir" \
  "$working_dir" \
  "$portable_root" \
  "$dmg_mount_dir"

hdiutil attach -readonly -nobrowse -quiet \
  -mountpoint "$dmg_mount_dir" "$dmg_path"

app_count="$(
  find "$dmg_mount_dir" -maxdepth 1 -type d -name '*.app' |
    wc -l |
    tr -d ' '
)"
if [ "$app_count" -ne 1 ]; then
  printf 'expected exactly one app bundle in %s, found %s\n' \
    "$dmg_path" "$app_count" >&2
  exit 1
fi
mounted_app="$dmg_mount_dir/Chaft.app"
if [ ! -d "$mounted_app" ]; then
  printf 'expected macOS application bundle is missing: %s\n' "$mounted_app" >&2
  exit 1
fi
mounted_binary="$mounted_app/Contents/MacOS/Chaft"
if [ ! -x "$mounted_binary" ]; then
  printf 'packaged macOS executable is missing: %s\n' "$mounted_binary" >&2
  exit 1
fi

compliance_dir="$mounted_app/Contents/Resources/doc/Chaft"
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

for required_path in \
  Contents/Frameworks \
  Contents/Info.plist \
  Contents/PlugIns \
  Contents/PlugIns/platforms/libqcocoa.dylib \
  Contents/Resources
do
  if [ ! -e "$mounted_app/$required_path" ]; then
    printf 'required macOS package path is missing: %s\n' \
      "$mounted_app/$required_path" >&2
    exit 1
  fi
done

plist_versions="$(
  python3 - "$mounted_app/Contents/Info.plist" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as handle:
    plist = plistlib.load(handle)
expected = {
    "CFBundleName": "Chaft",
    "CFBundleExecutable": "Chaft",
    "CFBundleIconFile": "Chaft.icns",
}
for key, expected_value in expected.items():
    value = plist.get(key)
    if value != expected_value:
        raise SystemExit(
            f"macOS package Info.plist {key} must be "
            f"{expected_value!r}, got {value!r}"
        )
for key in ("CFBundleShortVersionString", "CFBundleVersion"):
    value = plist.get(key)
    if not isinstance(value, str) or not value:
        raise SystemExit(f"macOS package Info.plist is missing {key}")
    print(value)
PY
)"
bundle_icon="$mounted_app/Contents/Resources/Chaft.icns"
if [ ! -s "$bundle_icon" ]; then
  printf 'packaged macOS application icon is missing: %s\n' "$bundle_icon" >&2
  exit 1
fi
short_version="$(printf '%s\n' "$plist_versions" | sed -n '1p')"
bundle_version="$(printf '%s\n' "$plist_versions" | sed -n '2p')"
if [ "$short_version" != "$source_version" ]; then
  printf 'expected embedded macOS short version %s, got %s\n' \
    "$source_version" "$short_version" >&2
  exit 1
fi
if [ "$bundle_version" != "$source_version" ]; then
  printf 'expected embedded macOS bundle version %s, got %s\n' \
    "$source_version" "$bundle_version" >&2
  exit 1
fi

# Keep the full DMG-derived bundle, but avoid a .app suffix because direct
# executable launches from .app bundles can be left launched-suspended by
# some hosted macOS shells.
portable_app="$portable_root/Chaft-dmg-smoke"
ditto "$mounted_app" "$portable_app"
desktop_binary="$portable_app/Contents/MacOS/Chaft"
if [ ! -x "$desktop_binary" ]; then
  printf 'copied macOS package executable is missing: %s\n' \
    "$desktop_binary" >&2
  exit 1
fi
if ! cmp -s "$mounted_binary" "$desktop_binary"; then
  printf 'copied macOS package executable differs from the DMG payload\n' >&2
  exit 1
fi

# The copied bundle must be independently launchable after the image is gone.
hdiutil detach -quiet "$dmg_mount_dir"
dmg_mount_dir=

unset CHAFT_FFI_LIBRARY
unset CMAKE_PREFIX_PATH
unset DYLD_FALLBACK_FRAMEWORK_PATH
unset DYLD_FALLBACK_LIBRARY_PATH
unset DYLD_FRAMEWORK_PATH
unset DYLD_INSERT_LIBRARIES
unset DYLD_LIBRARY_PATH
unset DYLD_ROOT_PATH
unset QML2_IMPORT_PATH
unset QML_IMPORT_PATH
unset QTDIR
unset QT_PLUGIN_PATH
unset QT_QPA_PLATFORM_PLUGIN_PATH
unset QT_ROOT_DIR
unset Qt6_DIR

smoke_timeout_ms="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}"
case "$smoke_timeout_ms" in
  ''|*[!0-9]*)
    printf 'invalid desktop smoke timeout: %s\n' "$smoke_timeout_ms" >&2
    exit 2
    ;;
esac
if [ "$smoke_timeout_ms" -lt 1000 ] || [ "$smoke_timeout_ms" -gt 60000 ]; then
  printf 'desktop smoke timeout must be between 1000 and 60000 ms\n' >&2
  exit 2
fi

default_watchdog_seconds=$(( (smoke_timeout_ms + 30000 + 999) / 1000 ))
watchdog_seconds="${CHAFT_DMG_SMOKE_WATCHDOG_SECONDS:-$default_watchdog_seconds}"
case "$watchdog_seconds" in
  ''|*[!0-9]*|0)
    printf 'invalid DMG smoke watchdog: %s\n' "$watchdog_seconds" >&2
    exit 2
    ;;
esac
if [ "$watchdog_seconds" -gt 300 ]; then
  printf 'DMG smoke watchdog must not exceed 300 seconds\n' >&2
  exit 2
fi

if [ -z "${CHAFT_WORKSPACE_ID:-}" ] \
    && [ -z "${CHAFT_DESKTOP_SMOKE_EXPECT_TEXT:-}" ] \
    && [ -z "${CHAFT_DESKTOP_SMOKE_READY_TEXT:-}" ]; then
  CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE=1
  export CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE
fi

HOME="$home_dir"
PATH=/usr/bin:/bin:/usr/sbin:/sbin
XDG_CACHE_HOME="$smoke_dir/cache"
XDG_CONFIG_HOME="$smoke_dir/config"
XDG_DATA_HOME="$smoke_dir/data"
QT_QPA_PLATFORM=cocoa
CHAFT_RUNTIME_DIR="$runtime_dir"
CHAFT_DESKTOP_SMOKE=1
CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="$smoke_timeout_ms"
export \
  HOME \
  PATH \
  XDG_CACHE_HOME \
  XDG_CONFIG_HOME \
  XDG_DATA_HOME \
  QT_QPA_PLATFORM \
  CHAFT_RUNTIME_DIR \
  CHAFT_DESKTOP_SMOKE \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS

watchdog_marker="$smoke_dir/watchdog-fired"
(
  cd "$working_dir"
  exec "$desktop_binary"
) &
desktop_pid=$!

(
  elapsed_seconds=0
  while [ "$elapsed_seconds" -lt "$watchdog_seconds" ]; do
    sleep 1
    elapsed_seconds=$((elapsed_seconds + 1))
  done
  if kill -0 "$desktop_pid" 2>/dev/null; then
    : > "$watchdog_marker"
    kill -TERM "$desktop_pid" 2>/dev/null || true
    sleep 2
    kill -KILL "$desktop_pid" 2>/dev/null || true
  fi
) &
watchdog_pid=$!

# Hosted macOS shells can transiently launch a copied GUI executable in a
# suspended state. Continuing it does not alter the packaged launch path.
(
  while kill -0 "$desktop_pid" 2>/dev/null; do
    kill -CONT "$desktop_pid" 2>/dev/null || true
    sleep 1
  done
) &
continue_pid=$!

if wait "$desktop_pid"; then
  desktop_status=0
else
  desktop_status=$?
fi
desktop_pid=

kill "$continue_pid" >/dev/null 2>&1 || true
wait "$continue_pid" >/dev/null 2>&1 || true
continue_pid=
kill "$watchdog_pid" >/dev/null 2>&1 || true
wait "$watchdog_pid" >/dev/null 2>&1 || true
watchdog_pid=

if [ -f "$watchdog_marker" ]; then
  printf 'macOS DMG smoke timed out after %ss\n' "$watchdog_seconds" >&2
  exit 124
fi
if [ "$desktop_status" -ne 0 ]; then
  printf 'packaged macOS app exited with code %s\n' "$desktop_status" >&2
  exit "$desktop_status"
fi

printf 'portable macOS DMG smoke passed: %s\n' "$(basename "$dmg_path")"
