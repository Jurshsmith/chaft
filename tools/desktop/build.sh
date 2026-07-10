#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
. "$script_dir/common.sh"

usage() {
  printf 'usage: %s [debug|release]\n' "$0" >&2
}

profile="${1:-debug}"
case "$profile" in
  debug)
    preset=desktop-debug
    cargo_profile=
    rust_target_dir=debug
    ;;
  release)
    preset=desktop-release
    cargo_profile=--release
    rust_target_dir=release
    ;;
  *)
    usage
    exit 2
    ;;
esac

chaft_desktop_add_tool_paths
"$script_dir/preflight.sh"

qt_prefix="$(chaft_desktop_qt_prefix || true)"
ffi_library_name="$(chaft_desktop_ffi_library_name)"
ffi_library="$repo_root/target/$rust_target_dir/$ffi_library_name"

cd "$repo_root"

cargo build -p chaft-ffi $cargo_profile

if [ -n "$qt_prefix" ]; then
  cmake --preset "$preset" \
    "-DCMAKE_PREFIX_PATH=$qt_prefix" \
    "-DCHAFT_FFI_LIBRARY_PATH=$ffi_library"
else
  cmake --preset "$preset" \
    "-DCHAFT_FFI_LIBRARY_PATH=$ffi_library"
fi

cmake --build --preset "$preset"

desktop_binary="$(chaft_desktop_find_binary "$repo_root" "$preset" || true)"
if [ -n "$desktop_binary" ] \
  && [ "$(uname -s)" = "Darwin" ] \
  && [ "${CHAFT_DESKTOP_SKIP_CODESIGN:-0}" != "1" ] \
  && command -v codesign >/dev/null 2>&1; then
  case "$desktop_binary" in
    *.app/Contents/MacOS/*)
      app_bundle="${desktop_binary%%.app/Contents/MacOS/*}.app"
      codesign --force --deep --sign - "$app_bundle"
      ;;
  esac
fi

printf 'ffi library: %s\n' "$ffi_library"
if [ -n "$desktop_binary" ]; then
  printf 'desktop binary: %s\n' "$desktop_binary"
  printf 'run command: CHAFT_FFI_LIBRARY=%s %s\n' "$ffi_library" "$desktop_binary"
else
  printf 'desktop build output: %s\n' "$repo_root/build/$preset\n"
fi
