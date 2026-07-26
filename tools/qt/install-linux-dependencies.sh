#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s install|list PROFILE\n' "$0" >&2
  printf \
    'profiles: sdk-build, sdk-consumer, desktop-package, release-package, appimage-runtime\n' \
    >&2
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

# A restored shared Qt SDK still needs the native compiler/linker tools used by
# its verification probe. libglvnd-dev supplies the OpenGL, GLX, EGL, and GL
# development targets that Qt6Gui resolves again on every consumer runner.
consumer_packages=(
  build-essential
  cmake
  libglvnd-dev
  ninja-build
)

qt_build_packages=(
  libatspi2.0-dev
  libcups2-dev
  libdbus-1-dev
  libdrm-dev
  libegl1-mesa-dev
  libfontconfig1-dev
  libfreetype6-dev
  libgl1-mesa-dev
  libinput-dev
  libudev-dev
  libwayland-dev
  libwayland-egl-backend-dev
  libx11-dev
  libx11-xcb-dev
  libxcb-cursor-dev
  libxcb-glx0-dev
  libxcb-icccm4-dev
  libxcb-image0-dev
  libxcb-keysyms1-dev
  libxcb-randr0-dev
  libxcb-render0-dev
  libxcb-render-util0-dev
  libxcb-shape0-dev
  libxcb-shm0-dev
  libxcb-sync-dev
  libxcb-util-dev
  libxcb-xfixes0-dev
  libxcb-xkb-dev
  libxcb-xinerama0-dev
  libxcb1-dev
  libxext-dev
  libxfixes-dev
  libxi-dev
  libxkbcommon-dev
  libxkbcommon-x11-dev
  libxrender-dev
  perl
  wayland-protocols
)

packaging_packages=(
  appstream
  desktop-file-utils
  patchelf
)

runtime_packages=(
  libegl1
  libglx0
  libopengl0
)

case "$profile" in
  sdk-build)
    packages=("${consumer_packages[@]}" "${qt_build_packages[@]}")
    ;;
  sdk-consumer)
    packages=("${consumer_packages[@]}")
    ;;
  desktop-package)
    packages=("${consumer_packages[@]}" "${packaging_packages[@]}")
    ;;
  release-package)
    packages=(
      "${consumer_packages[@]}"
      "${qt_build_packages[@]}"
      "${packaging_packages[@]}"
    )
    ;;
  appimage-runtime)
    packages=("${runtime_packages[@]}")
    ;;
  *)
    usage
    exit 2
    ;;
esac

if [[ "$action" == "list" ]]; then
  printf '%s\n' "${packages[@]}"
  exit 0
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  printf 'Linux dependency installation requires a Linux host\n' >&2
  exit 1
fi

sudo apt-get update
sudo apt-get install --no-install-recommends -y "${packages[@]}"
