#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
base_installer="$repo_root/tools/qt/install-linux-dependencies.sh"

usage() {
  printf 'usage: %s install|list desktop-package|release-package\n' "$0" >&2
}

if [[ "$#" -ne 2 ]]; then
  usage
  exit 2
fi

action="$1"
profile="$2"
case "$action" in
  install|list) ;;
  *)
    usage
    exit 2
    ;;
esac
case "$profile" in
  desktop-package|release-package) ;;
  *)
    usage
    exit 2
    ;;
esac

# These libraries are inputs to linuxdeploy, not the deterministic Qt SDK.
# They must be present on the packaging host so linuxdeploy can copy them into
# the AppDir; clean AppImage smoke runners deliberately do not install them.
package_host_libraries=(
  libxcb-cursor0
)

if [[ "$action" == "list" ]]; then
  "$base_installer" list "$profile"
  printf '%s\n' "${package_host_libraries[@]}"
  exit 0
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  printf 'Linux package dependency installation requires a Linux host\n' >&2
  exit 1
fi

base_package_list="$("$base_installer" list "$profile")"
packages=()
while IFS= read -r package; do
  if [[ -n "$package" ]]; then
    packages+=("$package")
  fi
done < <(printf '%s\n' "$base_package_list")
packages+=("${package_host_libraries[@]}")

sudo apt-get update
sudo apt-get install --no-install-recommends -y "${packages[@]}"
