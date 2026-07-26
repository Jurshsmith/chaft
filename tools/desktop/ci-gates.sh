#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

usage() {
  printf \
    'usage: %s [--stage all|contracts|debug|package] [Linux|macOS|Windows]\n' \
    "$0" >&2
}

stage=all
if [ "${1:-}" = "--stage" ]; then
  if [ "$#" -lt 2 ]; then
    usage
    exit 2
  fi
  stage="$2"
  shift 2
fi
case "$stage" in
  all|contracts|debug|package) ;;
  *)
    printf 'unsupported desktop CI stage: %s\n' "$stage" >&2
    usage
    exit 2
    ;;
esac
if [ "$#" -gt 1 ]; then
  usage
  exit 2
fi

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

run_contracts() {
  run_step "desktop instance isolation" "$script_dir/instance-smoke.sh"
  if [ "$stage" = "all" ]; then
    run_step "desktop preflight" "$script_dir/preflight.sh"
  fi
  run_step "QML lint" "$script_dir/qml-lint.sh"
  run_step "QML style lint" python3 "$script_dir/style-lint.py"
  run_step \
    "invite and form contracts" \
    python3 "$script_dir/invite-form-contract-check.py"
  run_step "theme contrast check" python3 "$script_dir/theme-contrast-check.py"
  if [ "$platform" = "Linux" ]; then
    run_step \
      "desktop reactivity contract" \
      python3 "$repo_root/apps/desktop-qt/tests/check_reactivity_contract.py"
    run_step \
      "QML contract tests" \
      env QT_QPA_PLATFORM=offscreen \
      qmltestrunner \
      -input "$repo_root/apps/desktop-qt/tests" \
      -import "$repo_root/apps/desktop-qt/qml"
  fi
}

run_debug() {
  run_step "desktop debug build" "$script_dir/build.sh" debug
  run_step \
    "desktop debug smoke" \
    env CHAFT_DESKTOP_SKIP_BUILD=1 \
    "$script_dir/smoke.sh" debug
  run_step \
    "desktop delayed live-sync smoke" \
    env CHAFT_DESKTOP_SKIP_BUILD=1 \
    "$script_dir/live-sync-smoke.sh" debug

  if [ "$platform" = "Linux" ] \
      && [ "${CHAFT_DESKTOP_SKIP_SCREENSHOT:-0}" != "1" ]; then
    run_step \
      "desktop screenshot baseline" \
      env CHAFT_DESKTOP_SKIP_BUILD=1 \
      "$script_dir/screenshot-smoke.sh" debug
  fi
}

run_package() {
  if [ "${CHAFT_DESKTOP_SKIP_PACKAGE:-0}" = "1" ]; then
    if [ "$stage" = "all" ]; then
      return
    fi
    printf 'package stage cannot be skipped when selected explicitly\n' >&2
    exit 2
  fi
  run_step "desktop release package smoke" "$script_dir/package-smoke.sh" release
  run_step "generate release metadata" python3 "$script_dir/release-metadata.py" release
  run_step \
    "verify release metadata" \
    python3 "$script_dir/verify-release-metadata.py" release --platform "$platform"
}

case "$stage" in
  all)
    run_contracts
    run_debug
    run_package
    ;;
  contracts) run_contracts ;;
  debug) run_debug ;;
  package) run_package ;;
esac
