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

# Qt 6.8's X11 requirements enumerate the libraries used by the XCB platform
# plugin. Keep their Ubuntu 22.04 runtime packages explicit here so linuxdeploy
# can resolve the restored SDK's plugins and copy every non-baseline library
# into the AppDir. These are packaging-host inputs, not deterministic Qt SDK
# inputs; clean AppImage smoke runners deliberately do not install them.
qt_xcb_runtime_packages=(
  libfontconfig1
  libfreetype6
  libglib2.0-0
  libice6
  libsm6
  libx11-6
  libx11-xcb1
  libxcb1
  libxcb-cursor0
  libxcb-glx0
  libxcb-icccm4
  libxcb-image0
  libxcb-keysyms1
  libxcb-randr0
  libxcb-render0
  libxcb-render-util0
  libxcb-shape0
  libxcb-shm0
  libxcb-sync1
  libxcb-util1
  libxcb-xfixes0
  libxcb-xkb1
  libxext6
  libxkbcommon0
  libxkbcommon-x11-0
  libxrender1
)

if [[ "$action" == "list" ]]; then
  "$base_installer" list "$profile"
  printf '%s\n' "${qt_xcb_runtime_packages[@]}"
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
packages+=("${qt_xcb_runtime_packages[@]}")

sudo apt-get update
sudo apt-get install --no-install-recommends -y "${packages[@]}"
