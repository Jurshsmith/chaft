#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
. "$repo_root/tools/desktop/common.sh"

usage() {
  cat >&2 <<'EOF'
usage: tools/macos/build-local.sh [options]

Build, verify, ad-hoc sign, and install a native Chaft.app from this checkout.

Options:
  --yes                    Confirm dependency installation and app replacement.
  --no-install-deps        Fail instead of offering to install missing Brew formulae.
  --install-dir DIR        Install below DIR (default: ~/Applications).
  --expected-commit SHA    Require this exact clean 40-character source commit.
  --skip-launch            Skip the launch smoke and do not open the installed app.
  --skip-open              Run the launch smoke but do not open the installed app.
  -h, --help               Show this help.
EOF
}

assume_yes=0
install_dependencies=1
install_dir="${HOME:?HOME must be set}/Applications"
expected_commit=
skip_launch=0
skip_open=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --yes)
      assume_yes=1
      ;;
    --no-install-deps)
      install_dependencies=0
      ;;
    --install-dir)
      if [ "$#" -lt 2 ]; then
        usage
        exit 2
      fi
      install_dir="$2"
      shift
      ;;
    --expected-commit)
      if [ "$#" -lt 2 ]; then
        usage
        exit 2
      fi
      expected_commit="$2"
      shift
      ;;
    --skip-launch)
      skip_launch=1
      ;;
    --skip-open)
      skip_open=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage
      exit 2
      ;;
  esac
  shift
done

if [ "$(uname -s)" != "Darwin" ]; then
  printf 'the guided local app build is supported only on macOS\n' >&2
  exit 1
fi

translated="$(sysctl -in sysctl.proc_translated 2>/dev/null || printf '0\n')"
if [ "$translated" = "1" ]; then
  printf \
    'this terminal is translated; open a native terminal and rebuild for the Mac architecture directly\n' \
    >&2
  exit 1
fi

architecture="$(chaft_desktop_architecture || true)"
case "$architecture" in
  x86_64|arm64) ;;
  *)
    printf 'unsupported native Mac architecture: %s\n' "$(uname -m)" >&2
    exit 1
    ;;
esac

if ! command -v xcode-select >/dev/null 2>&1 \
    || ! xcode_tools="$(xcode-select -p 2>/dev/null)" \
    || [ ! -d "$xcode_tools" ] \
    || ! command -v xcrun >/dev/null 2>&1 \
    || ! xcrun --find clang >/dev/null 2>&1; then
  printf \
    'Xcode Command Line Tools are required. Install them through Apple, then rerun this script.\n' \
    >&2
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  printf \
    'Git is required before dependency installation so the source revision can be verified.\n' \
    >&2
  exit 1
fi

cd "$repo_root"
if ! source_commit="$(git rev-parse --verify HEAD^{commit} 2>/dev/null)"; then
  printf \
    'this source tree is not a Git checkout; build an immutable Chaft tag or commit checkout\n' \
    >&2
  exit 1
fi
case "$source_commit" in
  *[!0-9a-f]*|'')
    printf 'Git returned a malformed source commit: %s\n' "$source_commit" >&2
    exit 1
    ;;
esac
if [ "${#source_commit}" -ne 40 ]; then
  printf 'Git returned a malformed source commit: %s\n' "$source_commit" >&2
  exit 1
fi
source_status_before="$(git status --porcelain=v1 --untracked-files=normal)"
source_state=clean
if [ -n "$source_status_before" ]; then
  source_state=dirty
fi
if [ -n "$expected_commit" ]; then
  case "$expected_commit" in
    *[!0-9a-fA-F]*|'')
      printf '%s\n' \
        '--expected-commit must be a full 40-character hexadecimal SHA' >&2
      exit 2
      ;;
  esac
  if [ "${#expected_commit}" -ne 40 ]; then
    printf '%s\n' \
      '--expected-commit must be a full 40-character hexadecimal SHA' >&2
    exit 2
  fi
  expected_commit="$(printf '%s' "$expected_commit" | tr '[:upper:]' '[:lower:]')"
  if [ "$source_commit" != "$expected_commit" ]; then
    printf 'source commit mismatch: expected %s, found %s\n' \
      "$expected_commit" "$source_commit" >&2
    exit 1
  fi
  if [ "$source_state" != "clean" ]; then
    printf \
      'the checkout has uncommitted changes; refusing an expected-commit build\n' \
      >&2
    exit 1
  fi
fi

case "$architecture" in
  arm64) native_brew_executable=/opt/homebrew/bin/brew ;;
  x86_64) native_brew_executable=/usr/local/bin/brew ;;
esac
if [ -n "${CHAFT_HOMEBREW_EXECUTABLE:-}" ]; then
  brew_executable="$CHAFT_HOMEBREW_EXECUTABLE"
  if [ ! -x "$brew_executable" ]; then
    printf 'configured Homebrew executable is unavailable: %s\n' \
      "$brew_executable" >&2
    exit 1
  fi
elif [ -x "$native_brew_executable" ]; then
  brew_executable="$native_brew_executable"
elif command -v brew >/dev/null 2>&1; then
  brew_executable="$(command -v brew)"
else
  printf \
    'Homebrew is required. Review and install it from https://brew.sh, then rerun this script.\n' \
    >&2
  exit 1
fi

brew_prefix="$(
  HOMEBREW_NO_AUTO_UPDATE=1 "$brew_executable" --prefix 2>/dev/null || true
)"
if [ -z "$brew_prefix" ] || [ ! -d "$brew_prefix" ]; then
  printf 'unable to resolve the selected Homebrew installation prefix\n' >&2
  exit 1
fi
CHAFT_HOMEBREW_EXECUTABLE="$brew_executable"
export CHAFT_HOMEBREW_EXECUTABLE
brew_config="$(
  HOMEBREW_NO_AUTO_UPDATE=1 "$brew_executable" config 2>/dev/null || true
)"
brew_architecture="$(
  printf '%s\n' "$brew_config" |
    awk -F- '/^macOS: / { print $NF; exit }'
)"
if [ "$brew_architecture" != "$architecture" ]; then
  printf \
    'selected Homebrew is not native to this Mac: expected %s, found %s\n' \
    "$architecture" "${brew_architecture:-unknown}" >&2
  exit 1
fi

CHAFT_DESKTOP_TOOL_DISCOVERY=explicit
export CHAFT_DESKTOP_TOOL_DISCOVERY
PATH="$brew_prefix/bin:$PATH"
export PATH
unset CMAKE_PREFIX_PATH QTDIR QT_ROOT_DIR Qt6_DIR
chaft_desktop_add_tool_paths

version_at_least() {
  actual="$1"
  required="$2"
  awk -v actual="$actual" -v required="$required" '
    BEGIN {
      split(actual, left, ".");
      split(required, right, ".");
      for (part_number = 1; part_number <= 3; part_number++) {
        left[part_number] += 0;
        right[part_number] += 0;
        if (left[part_number] > right[part_number]) exit 0;
        if (left[part_number] < right[part_number]) exit 1;
      }
      exit 0;
    }
  '
}

developer_qt_version() {
  version="$1"
  printf '%s\n' "$version" |
    awk '
      /^[0-9]+\.[0-9]+\.[0-9]+$/ {
        split($0, part, ".");
        if (part[1] == 6 && part[2] == 11 && part[3] >= 1) exit 0;
      }
      { exit 1 }
    '
}

brew_formula_installed() {
  HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_FROM_API=1 \
    "$brew_executable" list --versions "$1" >/dev/null 2>&1
}

activate_homebrew_qt() {
  homebrew_qt_prefixes=
  for qt_formula in qtbase qtdeclarative qtsvg qtshadertools; do
    if brew_formula_installed "$qt_formula"; then
      formula_prefix="$(
        HOMEBREW_NO_AUTO_UPDATE=1 \
          "$brew_executable" --prefix "$qt_formula" 2>/dev/null || true
      )"
      case "$formula_prefix" in
        "$brew_prefix"|"$brew_prefix"/*) ;;
        *)
          printf \
            'Homebrew formula %s resolved outside selected prefix %s: %s\n' \
            "$qt_formula" "$brew_prefix" "${formula_prefix:-missing}" >&2
          return 1
          ;;
      esac
      homebrew_qt_prefixes="$formula_prefix${homebrew_qt_prefixes:+:$homebrew_qt_prefixes}"
    fi
  done
  if brew_formula_installed qtbase; then
    QT_ROOT_DIR="$(
      HOMEBREW_NO_AUTO_UPDATE=1 "$brew_executable" --prefix qtbase
    )"
    QTDIR="$QT_ROOT_DIR"
    CMAKE_PREFIX_PATH="$homebrew_qt_prefixes"
    export QT_ROOT_DIR QTDIR CMAKE_PREFIX_PATH
    chaft_desktop_add_tool_paths
  fi
}

missing_formulae=
add_formula() {
  formula="$1"
  case " $missing_formulae " in
    *" $formula "*) ;;
    *) missing_formulae="${missing_formulae}${missing_formulae:+ }$formula" ;;
  esac
}

if ! command -v cmake >/dev/null 2>&1; then
  add_formula cmake
else
  cmake_version="$(cmake --version | sed -n '1s/^cmake version //p')"
  if [ -z "$cmake_version" ] || ! version_at_least "$cmake_version" 3.28.0; then
    add_formula cmake
  fi
fi
if ! command -v ninja >/dev/null 2>&1; then
  add_formula ninja
fi
if ! command -v python3 >/dev/null 2>&1; then
  add_formula python
fi
if ! command -v cargo >/dev/null 2>&1 \
    || ! command -v rustc >/dev/null 2>&1; then
  add_formula rust
else
  rust_version="$(rustc --version | sed -n 's/^rustc \([0-9][0-9.]*\).*$/\1/p')"
  if [ -z "$rust_version" ] || ! version_at_least "$rust_version" 1.97.1; then
    add_formula rust
  fi
fi

for qt_formula in qtbase qtdeclarative; do
  if ! brew_formula_installed "$qt_formula"; then
    add_formula "$qt_formula"
  fi
done
activate_homebrew_qt
qt_version="$(chaft_desktop_qt_version || true)"
if [ -z "$qt_version" ] || ! developer_qt_version "$qt_version"; then
  add_formula qtbase
  add_formula qtdeclarative
fi
for qt_tool in qmake6 qt-cmake qtpaths6 qmllint qmltestrunner macdeployqt; do
  if ! command -v "$qt_tool" >/dev/null 2>&1; then
    add_formula qtbase
    add_formula qtdeclarative
  fi
done

confirm() {
  prompt="$1"
  if [ "$assume_yes" -eq 1 ]; then
    return 0
  fi
  printf '%s [y/N] ' "$prompt"
  reply=
  IFS= read -r reply || true
  case "$reply" in
    y|Y|yes|YES|Yes) return 0 ;;
    *) return 1 ;;
  esac
}

if [ -n "$missing_formulae" ]; then
  printf 'missing or incompatible Homebrew formulae: %s\n' "$missing_formulae"
  printf 'proposed command: %s install %s\n' \
    "$brew_executable" "$missing_formulae"
  if [ "$install_dependencies" -eq 0 ]; then
    printf 'dependency installation is disabled; install them and rerun\n' >&2
    exit 1
  fi
  if ! confirm "Install these formulae now?"; then
    printf 'dependency installation declined; no packages were installed\n' >&2
    exit 1
  fi
  # Formula names are selected from the fixed list above.
  HOMEBREW_NO_AUTO_UPDATE=1 "$brew_executable" install $missing_formulae
  activate_homebrew_qt
fi

for required_tool in git cmake ninja python3 cargo rustc qmake6 qt-cmake \
  qtpaths6 qmllint qmltestrunner macdeployqt ditto codesign lipo open
do
  if ! command -v "$required_tool" >/dev/null 2>&1; then
    printf 'required tool is still unavailable after prerequisite checks: %s\n' \
      "$required_tool" >&2
    exit 1
  fi
done

cmake_version="$(cmake --version | sed -n '1s/^cmake version //p')"
if [ -z "$cmake_version" ] || ! version_at_least "$cmake_version" 3.28.0; then
  printf 'CMake 3.28 or newer is required, found %s\n' \
    "${cmake_version:-unknown}" >&2
  exit 1
fi
rust_version="$(rustc --version | sed -n 's/^rustc \([0-9][0-9.]*\).*$/\1/p')"
if [ -z "$rust_version" ] || ! version_at_least "$rust_version" 1.97.1; then
  printf 'Rust 1.97.1 or newer is required, found %s\n' \
    "${rust_version:-unknown}" >&2
  exit 1
fi
qt_version="$(chaft_desktop_qt_version || true)"
if [ -z "$qt_version" ] || ! developer_qt_version "$qt_version"; then
  printf \
    'Homebrew Qt >=6.11.1 and <6.12.0 is required, found %s\n' \
    "${qt_version:-unknown}" >&2
  exit 1
fi

activate_homebrew_qt
if [ -z "$homebrew_qt_prefixes" ]; then
  printf 'unable to resolve installed Homebrew Qt prefixes\n' >&2
  exit 1
fi
CHAFT_QT_POLICY=developer
export CHAFT_QT_POLICY
chaft_desktop_add_tool_paths

qt_core_library="$QT_ROOT_DIR/lib/QtCore.framework/Versions/A/QtCore"
if [ ! -f "$qt_core_library" ]; then
  qt_core_library="$QT_ROOT_DIR/lib/QtCore.framework/QtCore"
fi
if [ ! -f "$qt_core_library" ]; then
  qt_core_library="$QT_ROOT_DIR/lib/libQt6Core.dylib"
fi
if [ ! -f "$qt_core_library" ]; then
  printf 'unable to find the selected Homebrew QtCore library below %s\n' \
    "$QT_ROOT_DIR" >&2
  exit 1
fi
qt_architectures="$(lipo -archs "$qt_core_library" 2>/dev/null || true)"
if [ "$qt_architectures" != "$architecture" ]; then
  printf \
    'selected Homebrew Qt is not exactly native %s: %s reports %s\n' \
    "$architecture" "$qt_core_library" "${qt_architectures:-unknown}" >&2
  exit 1
fi

"$repo_root/tools/desktop/preflight.sh"

build_dir="$repo_root/build/macos-local-$architecture"
cargo_target_dir="$repo_root/target/macos-local-$architecture"
case "$install_dir" in
  /*) ;;
  *)
    printf '%s\n' \
      "--install-dir must be an absolute path: $install_dir" >&2
    exit 2
    ;;
esac
build_dir="$(
  python3 "$repo_root/tools/desktop/validate-safe-path.py" \
    --path "$build_dir" \
    --description "local desktop build directory" \
    --within "$repo_root"
)"
cargo_target_dir="$(
  python3 "$repo_root/tools/desktop/validate-safe-path.py" \
    --path "$cargo_target_dir" \
    --description "local Cargo target directory" \
    --within "$repo_root"
)"
stage_dir="$(
  python3 "$repo_root/tools/desktop/validate-safe-path.py" \
    --path "$build_dir/local-install" \
    --description "local app staging directory" \
    --within "$repo_root"
)"
install_dir="$(
  python3 "$repo_root/tools/desktop/validate-safe-path.py" \
    --path "$install_dir" \
    --description "local app installation directory"
)"

CHAFT_DESKTOP_BUILD_DIR="$build_dir" \
CARGO_TARGET_DIR="$cargo_target_dir" \
CHAFT_QT_POLICY=developer \
  "$repo_root/tools/desktop/build.sh" release

"$repo_root/tools/desktop/qml-lint.sh"
QT_QPA_PLATFORM=offscreen \
  ctest --test-dir "$build_dir" --output-on-failure

stage_dir="$(
  python3 "$repo_root/tools/desktop/validate-safe-path.py" \
    --path "$stage_dir" \
    --description "local app staging directory" \
    --within "$repo_root"
)"
rm -rf "$stage_dir"
cmake --install "$build_dir" --prefix "$stage_dir"
staged_app="$stage_dir/Chaft.app"
if [ "$skip_launch" -eq 1 ]; then
  "$script_dir/verify-local-app.sh" \
    --expected-arch "$architecture" \
    "$staged_app"
else
  "$script_dir/verify-local-app.sh" \
    --expected-arch "$architecture" \
    --launch-smoke \
    "$staged_app"
fi

source_status_after="$(git status --porcelain=v1 --untracked-files=normal)"
if ! source_commit_after="$(
  git rev-parse --verify HEAD^{commit} 2>/dev/null
)"; then
  printf \
    'the source revision became unavailable during the build; refusing to install the app\n' \
    >&2
  exit 1
fi
if [ "$source_commit_after" != "$source_commit" ] \
    || [ "$source_status_after" != "$source_status_before" ]; then
  printf \
    'the source checkout changed during the build; refusing to install the app\n' \
    >&2
  git status --short >&2
  exit 1
fi

install_dir="$(
  python3 "$repo_root/tools/desktop/validate-safe-path.py" \
    --path "$install_dir" \
    --description "local app installation directory"
)"
mkdir -p "$install_dir"
install_dir="$(
  python3 "$repo_root/tools/desktop/validate-safe-path.py" \
    --path "$install_dir" \
    --description "local app installation directory"
)"

final_app="$install_dir/Chaft.app"
temporary_app="$install_dir/.Chaft.app.install.$$"
backup_app="$install_dir/.Chaft.app.previous.$$"
replacement_pending=0

cleanup_install() {
  if [ -e "$temporary_app" ] || [ -L "$temporary_app" ]; then
    rm -rf "$temporary_app"
  fi
  if [ "$replacement_pending" -eq 1 ] && [ -e "$backup_app" ]; then
    if [ -e "$final_app" ] || [ -L "$final_app" ]; then
      rm -rf "$final_app"
    fi
    mv "$backup_app" "$final_app"
  fi
}
trap cleanup_install EXIT HUP INT TERM

rm -rf "$temporary_app"
ditto "$staged_app" "$temporary_app"
"$script_dir/verify-local-app.sh" \
  --expected-arch "$architecture" \
  "$temporary_app"

if [ -e "$final_app" ] || [ -L "$final_app" ]; then
  if [ -L "$final_app" ]; then
    printf 'refusing to replace a symbolic-link app path: %s\n' "$final_app" >&2
    exit 1
  fi
  if ! confirm "Replace the existing $final_app?"; then
    printf 'app replacement declined; the existing app was not changed\n' >&2
    exit 1
  fi
  rm -rf "$backup_app"
  mv "$final_app" "$backup_app"
  replacement_pending=1
fi

mv "$temporary_app" "$final_app"
"$script_dir/verify-local-app.sh" \
  --expected-arch "$architecture" \
  "$final_app"
if [ "$replacement_pending" -eq 1 ]; then
  rm -rf "$backup_app"
  replacement_pending=0
fi
trap - EXIT HUP INT TERM

if [ "$skip_launch" -eq 0 ] && [ "$skip_open" -eq 0 ]; then
  open -n "$final_app"
fi

printf '\nChaft local build installed successfully.\n'
printf 'source commit: %s\n' "$source_commit"
printf 'source state: %s\n' "$source_state"
printf 'Qt policy: developer (Homebrew)\n'
printf 'Qt version: %s\n' "$qt_version"
printf 'architecture: %s\n' "$architecture"
printf 'app path: %s\n' "$final_app"
printf 'launch command: open -n "%s"\n' "$final_app"
printf '%s\n' \
  'Signing: local ad-hoc signature only; this is not Apple Developer ID signing or notarization.'
printf '%s\n' \
  'Do not redistribute this locally built app as a trusted or Apple-verified binary.'
