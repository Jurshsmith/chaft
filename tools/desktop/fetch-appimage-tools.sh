#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
lock_file="$script_dir/../../packaging/linux/appimage-tools.lock"

usage() {
  printf 'usage: %s OUTPUT_DIRECTORY\n' "$0" >&2
}

output_dir="${1:-}"
if [ -z "$output_dir" ]; then
  usage
  exit 2
fi

for tool in curl; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$tool" >&2
    exit 1
  fi
done

if command -v sha256sum >/dev/null 2>&1; then
  sha256_command=sha256sum
elif command -v shasum >/dev/null 2>&1; then
  sha256_command=shasum
else
  printf 'missing required SHA-256 tool: sha256sum or shasum\n' >&2
  exit 1
fi

# shellcheck disable=SC1090
. "$lock_file"

temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-appimage-tools.XXXXXX")"
cleanup() {
  rm -rf "$temporary_dir"
}
trap cleanup EXIT INT TERM

file_sha256() {
  path="$1"
  if [ "$sha256_command" = "sha256sum" ]; then
    sha256sum "$path" | awk '{ print $1 }'
  else
    shasum -a 256 "$path" | awk '{ print $1 }'
  fi
}

download() {
  name="$1"
  url="$2"
  expected_sha256="$3"
  destination="$temporary_dir/$name"

  printf 'fetching %s\n' "$name"
  curl --fail --location --proto '=https' --silent --show-error \
    --output "$destination" "$url"

  actual_sha256="$(file_sha256 "$destination")"
  if [ "$actual_sha256" != "$expected_sha256" ]; then
    printf '%s SHA-256 mismatch\nexpected: %s\nactual:   %s\n' \
      "$name" "$expected_sha256" "$actual_sha256" >&2
    exit 1
  fi
  chmod 0755 "$destination"
}

download linuxdeploy "$LINUXDEPLOY_URL" "$LINUXDEPLOY_SHA256"
download linuxdeploy-plugin-qt \
  "$LINUXDEPLOY_PLUGIN_QT_URL" "$LINUXDEPLOY_PLUGIN_QT_SHA256"
download linuxdeploy-plugin-appimage \
  "$LINUXDEPLOY_PLUGIN_APPIMAGE_URL" "$LINUXDEPLOY_PLUGIN_APPIMAGE_SHA256"

mkdir -p "$output_dir"
for name in linuxdeploy linuxdeploy-plugin-qt linuxdeploy-plugin-appimage; do
  mv "$temporary_dir/$name" "$output_dir/$name"
done

printf 'verified AppImage tools: %s\n' "$output_dir"
