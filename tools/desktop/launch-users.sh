#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
launch="$script_dir/launch.sh"

profile="${1:-debug}"
count="${2:-2}"
prefix="${3:-user}"
fresh="${CHAFT_DEV_USERS_FRESH:-0}"
dry_run="${CHAFT_DEV_USERS_DRY_RUN:-0}"
max_users=20

case "$profile" in
  debug|release) ;;
  *)
    printf 'profile must be debug or release: %s\n' "$profile" >&2
    exit 2
    ;;
esac

case "$count" in
  ''|*[!0-9]*)
    printf 'N must be a positive integer: %s\n' "$count" >&2
    exit 2
    ;;
esac
if [ "$count" -lt 1 ] || [ "$count" -gt "$max_users" ]; then
  printf 'N must be between 1 and %s: %s\n' "$max_users" "$count" >&2
  exit 2
fi

case "$prefix" in
  ''|*[!A-Za-z0-9._-]*)
    printf 'PREFIX may contain only letters, numbers, dot, underscore, and dash: %s\n' "$prefix" >&2
    exit 2
    ;;
esac
case "$fresh" in
  0|1) ;;
  *)
    printf 'FRESH must be 0 or 1: %s\n' "$fresh" >&2
    exit 2
    ;;
esac
case "$dry_run" in
  0|1) ;;
  *)
    printf 'DRY_RUN must be 0 or 1: %s\n' "$dry_run" >&2
    exit 2
    ;;
esac

index=1
while [ "$index" -le "$count" ]; do
  instance="${prefix}${index}"
  if [ "${#instance}" -gt 64 ]; then
    printf 'generated instance name is longer than 64 characters: %s\n' "$instance" >&2
    exit 2
  fi

  set -- "$profile" --instance "$instance" --detached
  if [ "$fresh" = "1" ]; then
    set -- "$@" --fresh
  fi
  if [ "$index" -gt 1 ]; then
    set -- "$@" --no-build
  fi
  if [ "$dry_run" = "1" ]; then
    set -- "$@" --print-instance
    printf 'resolving %s (%s)\n' "$instance" \
      "$(if [ "$index" -eq 1 ]; then printf 'build once'; else printf 'reuse build'; fi)"
  else
    printf 'launching %s (%s)\n' "$instance" \
      "$(if [ "$index" -eq 1 ]; then printf 'build once'; else printf 'reuse build'; fi)"
  fi
  "$launch" "$@"
  index=$((index + 1))
done
