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
qt_policy="$(chaft_desktop_qt_policy)"

source_version="$(
  python3 "$script_dir/release-version.py" --print-source-version
)"
distribution_version="${CHAFT_DISTRIBUTION_VERSION:-$source_version}"
distribution_version="$(
  python3 "$script_dir/release-version.py" \
    --distribution-version "$distribution_version" \
    --print-distribution-version
)"

qt_prefix="$(chaft_desktop_qt_prefix || true)"
ffi_library_name="$(chaft_desktop_ffi_library_name)"
build_dir="${CHAFT_DESKTOP_BUILD_DIR:-$repo_root/build/$preset}"
if ! chaft_desktop_path_is_absolute "$build_dir"; then
  build_dir="$repo_root/$build_dir"
fi
if [ "$build_dir" = "/" ]; then
  printf 'desktop build directory must not be the filesystem root\n' >&2
  exit 2
fi
build_dir="$(
  python3 "$script_dir/validate-safe-path.py" \
    --path "$build_dir" \
    --description "desktop build directory"
)"
cargo_target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if ! chaft_desktop_path_is_absolute "$cargo_target_dir"; then
  cargo_target_dir="$repo_root/$cargo_target_dir"
fi
if [ "$cargo_target_dir" = "/" ]; then
  printf 'Cargo target directory must not be the filesystem root\n' >&2
  exit 2
fi
cargo_target_dir="$(
  python3 "$script_dir/validate-safe-path.py" \
    --path "$cargo_target_dir" \
    --description "Cargo target directory"
)"
export CARGO_TARGET_DIR="$cargo_target_dir"
ffi_library="$cargo_target_dir/$rust_target_dir/$ffi_library_name"

cd "$repo_root"

cargo build --locked -p chaft-ffi $cargo_profile

set -- \
  "-DCHAFT_FFI_LIBRARY_PATH=$ffi_library" \
  "-DCHAFT_DISTRIBUTION_VERSION=$distribution_version" \
  "-DCHAFT_QT_POLICY=$qt_policy"
if [ -n "$qt_prefix" ]; then
  set -- "-DCMAKE_PREFIX_PATH=$qt_prefix" "$@"
fi
if [ "$(uname -s)" = "Darwin" ]; then
  architecture="$(chaft_desktop_architecture || true)"
  if [ -z "$architecture" ]; then
    printf 'unsupported native macOS architecture: %s\n' "$(uname -m)" >&2
    exit 1
  fi
  set -- "-DCMAKE_OSX_ARCHITECTURES=$architecture" "$@"
fi
qt_compatibility_arguments="$(
  chaft_desktop_qt_compatibility_cmake_arguments "$profile"
)"
while IFS= read -r argument; do
  if [ -n "$argument" ]; then
    set -- "$@" "$argument"
  fi
done <<EOF
$qt_compatibility_arguments
EOF

if [ -n "${CHAFT_DESKTOP_BUILD_DIR:-}" ]; then
  cmake \
    -S "$repo_root" \
    -B "$build_dir" \
    -G Ninja \
    "-DCMAKE_BUILD_TYPE=$([ "$profile" = "debug" ] && printf Debug || printf Release)" \
    "$@"
else
  cmake --preset "$preset" "$@"
fi

cmake --build "$build_dir"

desktop_binary="$(chaft_desktop_find_binary_in_build_dir "$build_dir" || true)"
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
  printf 'desktop build output: %s\n' "$build_dir"
fi
