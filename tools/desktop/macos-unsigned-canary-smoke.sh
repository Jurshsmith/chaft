#!/usr/bin/env sh
set -eu

usage() {
  printf 'usage: %s DMG_OR_PACKAGE_DIRECTORY\n' "$0" >&2
}

input="${1:-}"
if [ -z "$input" ]; then
  usage
  exit 2
fi

if [ "$(uname -s)" != "Darwin" ]; then
  printf 'macOS unsigned-canary inspection is supported only on macOS\n' >&2
  exit 1
fi

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

require_tool codesign
require_tool hdiutil
require_tool python3
require_tool xcrun

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

inspection_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-macos-unsigned-canary.XXXXXX")"
mount_dir="$inspection_dir/mounted"
mounted=0
mkdir -p "$mount_dir"

cleanup() {
  if [ "$mounted" -eq 1 ]; then
    hdiutil detach -quiet "$mount_dir" >/dev/null 2>&1 || true
  fi
  rm -rf "$inspection_dir"
}
trap cleanup EXIT HUP INT TERM

hdiutil attach -readonly -nobrowse -quiet \
  -mountpoint "$mount_dir" "$dmg_path"
mounted=1

app_count="$(
  find "$mount_dir" -maxdepth 1 -type d -name '*.app' | wc -l | tr -d ' '
)"
if [ "$app_count" -ne 1 ]; then
  printf 'expected exactly one app bundle in %s, found %s\n' \
    "$dmg_path" "$app_count" >&2
  exit 1
fi
app_path="$mount_dir/Chaft.app"
if [ ! -d "$app_path" ]; then
  printf 'expected macOS application bundle is missing: %s\n' "$app_path" >&2
  exit 1
fi
python3 - "$app_path/Contents/Info.plist" <<'PY'
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
PY
bundle_icon="$app_path/Contents/Resources/Chaft.icns"
if [ ! -s "$bundle_icon" ]; then
  printf 'packaged macOS application icon is missing: %s\n' "$bundle_icon" >&2
  exit 1
fi

# Ad-hoc signing keeps a locally built bundle internally consistent but
# carries no Apple trust identity. It must never be represented as Developer
# ID signing or notarization.
signature_details="$(
  codesign --display --verbose=4 "$app_path" 2>&1
)"
if ! printf '%s\n' "$signature_details" | grep -qx 'Signature=adhoc'; then
  printf 'macOS canary app is not explicitly ad-hoc signed\n%s\n' \
    "$signature_details" >&2
  exit 1
fi
if ! printf '%s\n' "$signature_details" | grep -qx 'TeamIdentifier=not set'; then
  printf 'macOS canary app unexpectedly has an Apple team identifier\n%s\n' \
    "$signature_details" >&2
  exit 1
fi
if printf '%s\n' "$signature_details" | grep -q '^Authority='; then
  printf 'macOS canary app unexpectedly has a signing authority\n%s\n' \
    "$signature_details" >&2
  exit 1
fi
codesign --verify --deep --strict --verbose=4 "$app_path"

if codesign --display --verbose=4 "$dmg_path" >/dev/null 2>&1; then
  printf 'macOS canary DMG unexpectedly has a code signature\n' >&2
  exit 1
fi
if xcrun stapler validate "$app_path" >/dev/null 2>&1; then
  printf 'macOS canary app unexpectedly has a notarization ticket\n' >&2
  exit 1
fi
if xcrun stapler validate "$dmg_path" >/dev/null 2>&1; then
  printf 'macOS canary DMG unexpectedly has a notarization ticket\n' >&2
  exit 1
fi

hdiutil detach -quiet "$mount_dir"
mounted=0

printf '%s\n' \
  'macOS unsigned-canary state passed: ad-hoc app, no team, unsigned DMG, no notarization'
