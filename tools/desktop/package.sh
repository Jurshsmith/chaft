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
  debug|release)
    preset="desktop-$profile"
    ;;
  *)
    usage
    exit 2
    ;;
esac

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

chaft_desktop_add_tool_paths
require_tool cmake
require_tool cpack

build_dir="$repo_root/build/$preset"
install_dir="$build_dir/install"
package_dir="$build_dir/package"

"$script_dir/build.sh" "$profile"

rm -rf "$install_dir" "$package_dir"
cmake --install "$build_dir" --prefix "$install_dir"
cpack --config "$build_dir/CPackConfig.cmake"
rm -rf "$package_dir/_CPack_Packages"

printf 'install tree: %s\n' "$install_dir"
printf 'package artifacts:\n'
find "$package_dir" -maxdepth 1 -type f -print | sort
