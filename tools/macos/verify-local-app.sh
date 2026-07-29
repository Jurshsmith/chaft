#!/usr/bin/env sh
set -eu

usage() {
  cat >&2 <<'EOF'
usage: tools/macos/verify-local-app.sh [--expected-arch x86_64|arm64] [--launch-smoke] APP
EOF
}

expected_arch=
launch_smoke=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --expected-arch)
      if [ "$#" -lt 2 ]; then
        usage
        exit 2
      fi
      expected_arch="$2"
      shift
      ;;
    --launch-smoke)
      launch_smoke=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      printf 'unknown option: %s\n' "$1" >&2
      usage
      exit 2
      ;;
    *)
      if [ -n "${app_path:-}" ]; then
        usage
        exit 2
      fi
      app_path="$1"
      ;;
  esac
  shift
done

app_path="${app_path:-}"
if [ -z "$app_path" ]; then
  usage
  exit 2
fi
if [ "$(uname -s)" != "Darwin" ]; then
  printf 'local macOS app verification requires Darwin\n' >&2
  exit 1
fi

case "$expected_arch" in
  "")
    expected_arch="$(uname -m | tr '[:upper:]' '[:lower:]')"
    ;;
  x86_64|arm64) ;;
  *)
    printf 'unsupported expected architecture: %s\n' "$expected_arch" >&2
    exit 2
    ;;
esac
case "$expected_arch" in
  amd64|x64) expected_arch=x86_64 ;;
  aarch64) expected_arch=arm64 ;;
esac

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required verification tool: %s\n' "$name" >&2
    exit 1
  fi
}

require_tool codesign
require_tool lipo
require_tool python3

if [ ! -d "$app_path" ]; then
  printf 'Chaft app bundle not found: %s\n' "$app_path" >&2
  exit 1
fi
app_path="$(CDPATH= cd "$app_path" && pwd -P)"
binary="$app_path/Contents/MacOS/Chaft"
ffi_library="$app_path/Contents/MacOS/libchaft_ffi.dylib"
plist="$app_path/Contents/Info.plist"
icon="$app_path/Contents/Resources/Chaft.icns"
documentation="$app_path/Contents/Resources/doc/Chaft"
local_qt_notice="$documentation/QT-LOCAL-BUILD-NOTICE.txt"
for required_path in \
  "$binary" \
  "$ffi_library" \
  "$plist" \
  "$icon" \
  "$documentation/LICENSE" \
  "$documentation/LICENSE.LGPL3" \
  "$documentation/LICENSE.GPL3" \
  "$local_qt_notice"
do
  if [ ! -s "$required_path" ]; then
    printf 'required Chaft app path is missing or empty: %s\n' \
      "$required_path" >&2
    exit 1
  fi
done
for release_only_notice in \
  "$documentation/THIRD_PARTY_NOTICES.txt" \
  "$documentation/QT-CORRESPONDING-SOURCE.json"
do
  if [ -e "$release_only_notice" ] || [ -L "$release_only_notice" ]; then
    printf \
      'local app must not contain the Qt 6.8.4 release-only notice: %s\n' \
      "$release_only_notice" >&2
    exit 1
  fi
done
if [ ! -x "$binary" ]; then
  printf 'Chaft app executable is not executable: %s\n' "$binary" >&2
  exit 1
fi

python3 - "$plist" "$icon" "$local_qt_notice" <<'PY'
import plistlib
from pathlib import Path
import re
import struct
import sys

plist_path, icon_path, notice_path = (Path(value) for value in sys.argv[1:])
with plist_path.open("rb") as handle:
    plist = plistlib.load(handle)
expected = {
    "CFBundleName": "Chaft",
    "CFBundleExecutable": "Chaft",
    "CFBundleIconFile": "Chaft.icns",
    "CFBundleIdentifier": "app.chaft.desktop",
}
for key, value in expected.items():
    if plist.get(key) != value:
        raise SystemExit(
            f"Chaft app Info.plist {key} must be {value!r}, "
            f"got {plist.get(key)!r}"
        )
for key in ("CFBundleShortVersionString", "CFBundleVersion"):
    if not isinstance(plist.get(key), str) or not plist[key]:
        raise SystemExit(f"Chaft app Info.plist is missing {key}")

icon = icon_path.read_bytes()
if len(icon) < 8 or icon[:4] != b"icns":
    raise SystemExit("Chaft app icon is not a valid ICNS container")
if struct.unpack(">I", icon[4:8])[0] != len(icon):
    raise SystemExit("Chaft app icon container length is invalid")

notice = notice_path.read_text(encoding="utf-8")
normalized_notice = " ".join(notice.split())
if "Homebrew's open-source Qt" not in normalized_notice:
    raise SystemExit("Chaft local app Qt notice does not identify Homebrew Qt")
if re.search(r"\bQt 6\.11\.[1-9][0-9]*\b", normalized_notice) is None:
    raise SystemExit("Chaft local app Qt notice has no supported Qt 6.11 version")
if "verified Chaft Qt 6.8.4 release SDK provenance" not in normalized_notice:
    raise SystemExit("Chaft local app Qt notice does not distinguish release provenance")
PY

python3 - "$app_path" "$expected_arch" <<'PY'
from pathlib import Path
import subprocess
import sys

app = Path(sys.argv[1])
expected = sys.argv[2]
mach_o_magics = {
    b"\xca\xfe\xba\xbe",
    b"\xbe\xba\xfe\xca",
    b"\xca\xfe\xba\xbf",
    b"\xbf\xba\xfe\xca",
    b"\xfe\xed\xfa\xce",
    b"\xce\xfa\xed\xfe",
    b"\xfe\xed\xfa\xcf",
    b"\xcf\xfa\xed\xfe",
}
required = {
    app / "Contents" / "MacOS" / "Chaft",
    app / "Contents" / "MacOS" / "libchaft_ffi.dylib",
}
candidate_paths = set(required)
for path in sorted(app.rglob("*")):
    if path.is_symlink() or not path.is_file():
        continue
    try:
        with path.open("rb") as handle:
            magic = handle.read(4)
    except OSError as error:
        raise SystemExit(f"cannot inspect bundled file {path}: {error}")
    if magic in mach_o_magics:
        candidate_paths.add(path)

inspected = []
for path in sorted(candidate_paths):
    result = subprocess.run(
        ["lipo", "-archs", str(path)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"lipo failed for bundled Mach-O {path}: {result.stderr.strip()}"
        )
    aliases = {
        "aarch64": "arm64",
        "x86-64": "x86_64",
    }
    architectures = {
        aliases.get(value.lower(), value.lower())
        for value in result.stdout.split()
    }
    if architectures != {expected}:
        raise SystemExit(
            f"Mach-O architecture mismatch for {path.relative_to(app)}: "
            f"expected only {expected}, got {sorted(architectures)}"
        )
    inspected.append(path)

if not required.issubset(inspected):
    raise SystemExit("Chaft main executable and FFI library were not inspected")
print(f"verified {len(inspected)} native {expected} Mach-O payloads")
PY

signature_details="$(codesign --display --verbose=4 "$app_path" 2>&1)"
if ! printf '%s\n' "$signature_details" | grep -qx 'Signature=adhoc'; then
  printf 'Chaft local app is not explicitly ad-hoc signed\n%s\n' \
    "$signature_details" >&2
  exit 1
fi
if ! printf '%s\n' "$signature_details" | grep -qx 'TeamIdentifier=not set'; then
  printf 'Chaft local app unexpectedly has an Apple team identifier\n%s\n' \
    "$signature_details" >&2
  exit 1
fi
if printf '%s\n' "$signature_details" | grep -q '^Authority='; then
  printf 'Chaft local app unexpectedly has a signing authority\n%s\n' \
    "$signature_details" >&2
  exit 1
fi
codesign --verify --deep --strict --verbose=4 "$app_path"

if [ "$launch_smoke" -eq 1 ]; then
  offscreen_plugin="$app_path/Contents/PlugIns/platforms/libqoffscreen.dylib"
  if [ ! -s "$offscreen_plugin" ]; then
    printf 'Chaft local app has no bundled offscreen smoke plugin: %s\n' \
      "$offscreen_plugin" >&2
    exit 1
  fi
  smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-local-app-smoke.XXXXXX")"
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
    rm -rf "$smoke_dir"
  }
  trap cleanup EXIT HUP INT TERM

  mkdir -p "$smoke_dir/home" "$smoke_dir/runtime" "$smoke_dir/working"
  (
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
    cd "$smoke_dir/working"
    exec env \
      HOME="$smoke_dir/home" \
      PATH=/usr/bin:/bin:/usr/sbin:/sbin \
      QT_QPA_PLATFORM=offscreen \
      CHAFT_RUNTIME_DIR="$smoke_dir/runtime" \
      CHAFT_DESKTOP_SMOKE=1 \
      CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE=1 \
      CHAFT_DESKTOP_SMOKE_TIMEOUT_MS=15000 \
      "$binary"
  ) &
  desktop_pid=$!

  watchdog_marker="$smoke_dir/watchdog-fired"
  (
    elapsed=0
    while [ "$elapsed" -lt 45 ]; do
      sleep 1
      elapsed=$((elapsed + 1))
    done
    if kill -0 "$desktop_pid" 2>/dev/null; then
      : > "$watchdog_marker"
      kill -TERM "$desktop_pid" 2>/dev/null || true
      sleep 2
      kill -KILL "$desktop_pid" 2>/dev/null || true
    fi
  ) &
  watchdog_pid=$!

  (
    while kill -0 "$desktop_pid" 2>/dev/null; do
      kill -CONT "$desktop_pid" 2>/dev/null || true
      sleep 1
    done
  ) &
  continue_pid=$!

  desktop_status=0
  wait "$desktop_pid" || desktop_status=$?
  desktop_pid=
  kill "$continue_pid" >/dev/null 2>&1 || true
  wait "$continue_pid" >/dev/null 2>&1 || true
  continue_pid=
  kill "$watchdog_pid" >/dev/null 2>&1 || true
  wait "$watchdog_pid" >/dev/null 2>&1 || true
  watchdog_pid=

  if [ -f "$watchdog_marker" ]; then
    printf 'Chaft local app launch smoke timed out\n' >&2
    exit 124
  fi
  if [ "$desktop_status" -ne 0 ]; then
    printf 'Chaft local app launch smoke exited with code %s\n' \
      "$desktop_status" >&2
    exit "$desktop_status"
  fi
fi

printf 'verified native %s Chaft local app: %s\n' \
  "$expected_arch" "$app_path"
