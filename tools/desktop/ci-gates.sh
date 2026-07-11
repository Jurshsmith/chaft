#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

usage() {
  printf 'usage: %s [Linux|macOS|Windows]\n' "$0" >&2
}

platform="${1:-}"
if [ -z "$platform" ]; then
  case "$(uname -s)" in
    Darwin) platform=macOS ;;
    MINGW*|MSYS*|CYGWIN*) platform=Windows ;;
    Linux) platform=Linux ;;
    *)
      printf 'unable to infer desktop package platform from uname\n' >&2
      usage
      exit 2
      ;;
  esac
fi

case "$platform" in
  Linux|macOS|Windows) ;;
  *)
    printf 'unsupported desktop package platform: %s\n' "$platform" >&2
    usage
    exit 2
    ;;
esac

run_step() {
  label="$1"
  shift
  printf '\n==> %s\n' "$label"
  "$@"
}

cd "$repo_root"

run_step "desktop instance isolation" "$script_dir/instance-smoke.sh"
run_step "desktop preflight" "$script_dir/preflight.sh"
run_step "QML lint" "$script_dir/qml-lint.sh"
run_step "QML style lint" python3 "$script_dir/style-lint.py"
run_step "theme contrast check" python3 "$script_dir/theme-contrast-check.py"
run_step "desktop debug smoke" "$script_dir/smoke.sh" debug

if [ "$platform" = "Linux" ] && [ "${CHAFT_DESKTOP_SKIP_SCREENSHOT:-0}" != "1" ]; then
  run_step "desktop screenshot baseline" "$script_dir/screenshot-smoke.sh" debug
fi

if [ "${CHAFT_DESKTOP_SKIP_PACKAGE:-0}" != "1" ]; then
  run_step "desktop release package smoke" "$script_dir/package-smoke.sh" release
  run_step "generate release metadata" python3 "$script_dir/release-metadata.py" release
  run_step \
    "verify release metadata" \
    python3 "$script_dir/verify-release-metadata.py" release --platform "$platform"
fi
