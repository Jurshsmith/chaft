#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
launch="$script_dir/launch.sh"
launch_users="$script_dir/launch-users.sh"
smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-instance-smoke.XXXXXX")"
trap 'rm -rf "$smoke_dir"' EXIT INT TERM

first_spawn="$smoke_dir/first/client"
second_spawn="$smoke_dir/second/client"
mkdir -p "$first_spawn" "$second_spawn"

runtime_for() {
  spawn="$1"
  shift
  (cd "$spawn" && "$launch" debug --print-instance "$@") |
    sed -n 's/^runtime dir: //p'
}

first_runtime="$(runtime_for "$first_spawn")"
first_runtime_again="$(runtime_for "$first_spawn")"
second_runtime="$(runtime_for "$second_spawn")"
alice_runtime="$(runtime_for "$first_spawn" --instance alice)"
bob_runtime="$(runtime_for "$first_spawn" --instance bob)"
explicit_runtime="$(runtime_for "$first_spawn" --data-dir "$smoke_dir/explicit")"

if [ "$first_runtime" != "$first_runtime_again" ]; then
  printf 'directory-derived runtime was not stable\n' >&2
  exit 1
fi
if [ "$first_runtime" = "$second_runtime" ]; then
  printf 'different spawn directories resolved to the same runtime\n' >&2
  exit 1
fi
if [ "$alice_runtime" = "$bob_runtime" ]; then
  printf 'named instances from one directory resolved to the same runtime\n' >&2
  exit 1
fi
if [ "$explicit_runtime" != "$smoke_dir/explicit/runtime" ]; then
  printf 'explicit data directory did not take precedence: %s\n' "$explicit_runtime" >&2
  exit 1
fi
if (cd "$first_spawn" && "$launch" debug --print-instance --instance 'not valid') \
    >/dev/null 2>&1; then
  printf 'invalid instance name was accepted\n' >&2
  exit 1
fi

case "$first_runtime" in
  "$repo_root"/scratch/desktop-instances/*/runtime) ;;
  *)
    printf 'derived runtime is outside the expected instance root: %s\n' "$first_runtime" >&2
    exit 1
    ;;
esac

multi_user_output="$(
  cd "$first_spawn"
  CHAFT_DEV_USERS_DRY_RUN=1 "$launch_users" debug 3 smoke
)"
for expected_label in smoke1 smoke2 smoke3; do
  case "$multi_user_output" in
    *"instance label: $expected_label"*) ;;
    *)
      printf 'numbered launcher omitted instance %s\n' "$expected_label" >&2
      exit 1
      ;;
  esac
done
if CHAFT_DEV_USERS_DRY_RUN=1 "$launch_users" debug 0 smoke >/dev/null 2>&1; then
  printf 'numbered launcher accepted a zero instance count\n' >&2
  exit 1
fi
if CHAFT_DEV_USERS_DRY_RUN=1 "$launch_users" debug 21 smoke >/dev/null 2>&1; then
  printf 'numbered launcher accepted more than 20 instances\n' >&2
  exit 1
fi

printf 'desktop instance smoke passed\n'
