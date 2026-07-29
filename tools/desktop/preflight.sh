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
require_tool python3

qt_prefix="$(chaft_desktop_qt_prefix || true)"
if [ -z "$qt_prefix" ]; then
  printf 'missing Qt 6 command-line tools: expected qt-cmake or qmake6\n' >&2
  missing=1
fi

qt_version="$(chaft_desktop_qt_version || true)"
if [ -z "$qt_version" ]; then
  printf 'unable to determine the installed Qt version\n' >&2
  missing=1
fi

if [ "$missing" -ne 0 ]; then
  printf \
    'install Rust 1.97.1+, CMake 3.28+, Ninja, Python 3, and a policy-compatible Qt before building apps/desktop-qt\n' \
    >&2
  exit 1
fi

policy="$(chaft_desktop_qt_policy)"

python3 - "$qt_version" "$policy" <<'PY'
import re
import sys

version, policy = sys.argv[1:]
match = re.fullmatch(r"([0-9]+)\.([0-9]+)\.([0-9]+)", version)
if match is None:
    raise SystemExit(f"unsupported Qt version format: {version!r}")
parsed = tuple(int(part) for part in match.groups())
if policy == "release":
    if parsed != (6, 8, 4):
        raise SystemExit(
            f"Qt {version} is unsupported by the release policy; "
            "official builds require exactly Qt 6.8.4"
        )
elif not ((6, 11, 1) <= parsed < (6, 12, 0)):
    raise SystemExit(
        f"Qt {version} is unsupported by the developer policy; "
        "macOS Homebrew builds require Qt >=6.11.1 and <6.12.0"
    )
PY

case "$policy" in
  developer)
    platform="$(chaft_desktop_platform || true)"
    if [ "$platform" != "macos" ]; then
      printf \
        'CHAFT_QT_POLICY=developer is currently supported only for native macOS Homebrew builds\n' \
        >&2
      exit 1
    fi
    developer_brew="${CHAFT_HOMEBREW_EXECUTABLE:-}"
    if [ -z "$developer_brew" ]; then
      developer_brew="$(command -v brew 2>/dev/null || true)"
    fi
    if [ -z "$developer_brew" ] || [ ! -x "$developer_brew" ]; then
      printf 'developer Qt policy requires Homebrew\n' >&2
      exit 1
    fi
    brew_prefix="$("$developer_brew" --prefix 2>/dev/null || true)"
    if [ -z "$brew_prefix" ] || [ ! -d "$brew_prefix" ] \
        || [ -z "$qt_prefix" ] || [ ! -d "$qt_prefix" ]; then
      printf \
        'developer Qt policy could not resolve the selected Homebrew and Qt prefixes\n' \
        >&2
      exit 1
    fi
    qt_prefix_resolved="$(CDPATH= cd "$qt_prefix" 2>/dev/null && pwd -P || true)"
    brew_prefix_resolved="$(
      CDPATH= cd "$brew_prefix" 2>/dev/null && pwd -P || true
    )"
    if [ -z "$qt_prefix_resolved" ] || [ -z "$brew_prefix_resolved" ]; then
      printf \
        'developer Qt policy could not resolve the selected Homebrew and Qt prefixes\n' \
        >&2
      exit 1
    fi
    case "$qt_prefix_resolved" in
      "$brew_prefix_resolved") ;;
      "$brew_prefix_resolved"/*) ;;
      *)
        printf \
          'developer Qt policy requires a Homebrew Qt prefix, got %s\n' \
          "$qt_prefix" >&2
        exit 1
        ;;
    esac
    ;;
  release)
    expected_platform="$(chaft_desktop_platform || true)"
    expected_architecture="$(chaft_desktop_architecture || true)"
    expected_target="$(chaft_desktop_qt_sdk_target || true)"
    if [ -z "$expected_target" ]; then
      printf 'release Qt policy does not support this host target\n' >&2
      exit 1
    fi

    for name in \
      QTDIR \
      QT_ROOT_DIR \
      CHAFT_QT_SDK_BUILD_TYPE \
      CHAFT_QT_SDK_TARGET \
      CHAFT_QT_SDK_PLATFORM \
      CHAFT_QT_SDK_ARCHITECTURE \
      CHAFT_QT_SDK_VERSION \
      CHAFT_QT_SDK_IDENTITY \
      CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT \
      CHAFT_QT_SDK_PROVENANCE
    do
      eval "value=\${$name:-}"
      if [ -z "$value" ]; then
        printf \
          'release Qt policy requires verified SDK activation variable %s\n' \
          "$name" >&2
        exit 1
      fi
    done

    if [ "$CHAFT_QT_SDK_BUILD_TYPE" != "Release" ] \
        || [ "$CHAFT_QT_SDK_VERSION" != "6.8.4" ] \
        || [ "$CHAFT_QT_SDK_TARGET" != "$expected_target" ] \
        || [ "$CHAFT_QT_SDK_PLATFORM" != "$expected_platform" ] \
        || [ "$CHAFT_QT_SDK_ARCHITECTURE" != "$expected_architecture" ]; then
      printf \
        'release Qt SDK activation does not match exact target %s and Qt 6.8.4\n' \
        "$expected_target" >&2
      exit 1
    fi

    case "$CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT" in
      *[!0-9a-f]*|'')
        printf 'release Qt SDK toolchain fingerprint is malformed\n' >&2
        exit 1
        ;;
    esac
    if [ "${#CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT}" -ne 64 ]; then
      printf 'release Qt SDK toolchain fingerprint is malformed\n' >&2
      exit 1
    fi
    if [ ! -f "$CHAFT_QT_SDK_PROVENANCE" ]; then
      printf \
        'verified Qt SDK provenance is missing: %s\n' \
        "$CHAFT_QT_SDK_PROVENANCE" >&2
      exit 1
    fi

    python3 - \
      "$CHAFT_QT_SDK_PROVENANCE" \
      "$qt_prefix" \
      "$QTDIR" \
      "$QT_ROOT_DIR" \
      "$expected_target" \
      "$expected_platform" \
      "$expected_architecture" \
      "$CHAFT_QT_SDK_IDENTITY" \
      "$CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT" <<'PY'
import json
from pathlib import Path
import sys

(
    provenance_name,
    detected_prefix_name,
    qtdir_name,
    qt_root_name,
    target,
    platform,
    architecture,
    identity,
    fingerprint,
) = sys.argv[1:]

provenance_path = Path(provenance_name)
try:
    provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
except (OSError, json.JSONDecodeError) as error:
    raise SystemExit(f"unable to read verified Qt SDK provenance: {error}")

expected = {
    "schemaVersion": 2,
    "qtVersion": "6.8.4",
    "target": target,
    "platform": platform,
    "architecture": architecture,
    "identity": identity,
    "toolchainFingerprint": fingerprint,
}
for key, value in expected.items():
    if provenance.get(key) != value:
        raise SystemExit(
            f"verified Qt SDK provenance {key} mismatch: "
            f"expected {value!r}, got {provenance.get(key)!r}"
        )

verification = provenance.get("verification")
if not isinstance(verification, dict) or verification.get("completed") is not True:
    raise SystemExit("verified Qt SDK provenance is not marked complete")

specification = provenance.get("targetSpecification")
if not isinstance(specification, dict):
    raise SystemExit("verified Qt SDK provenance target specification is missing")
for key, value in {
    "platform": platform,
    "architecture": architecture,
}.items():
    if specification.get(key) != value:
        raise SystemExit(
            f"verified Qt SDK target specification {key} mismatch"
        )

prefixes = [
    Path(detected_prefix_name).resolve(),
    Path(qtdir_name).resolve(),
    Path(qt_root_name).resolve(),
    provenance_path.resolve().parent,
]
if any(prefix != prefixes[0] for prefix in prefixes[1:]):
    raise SystemExit(
        "release Qt SDK prefix, activation paths, and provenance directory differ"
    )
PY

    python3 "$script_dir/../qt/build_qt.py" verify \
      --target "$expected_target" \
      --prefix "$qt_prefix" \
      --provenance "$CHAFT_QT_SDK_PROVENANCE" \
      --toolchain-fingerprint "$CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT"
    ;;
esac

cargo --version
cmake --version | sed -n '1p'
ninja --version | sed -n '1s/^/ninja /p'
printf 'Qt policy: %s\n' "$policy"
printf 'qt prefix: %s\n' "$qt_prefix"
printf 'Qt version: %s\n' "$qt_version"

if command -v qt-cmake >/dev/null 2>&1; then
  printf 'qt-cmake: %s\n' "$(command -v qt-cmake)"
fi
if command -v qmake6 >/dev/null 2>&1; then
  qmake6 --version
fi
