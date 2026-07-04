#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"
. "$script_dir/common.sh"

chaft_desktop_add_tool_paths

if ! command -v qmllint >/dev/null 2>&1; then
  printf 'missing required tool: qmllint\n' >&2
  printf 'install Qt 6.8+ QML tooling before linting apps/desktop-qt\n' >&2
  exit 1
fi

qml_import_root="$repo_root/apps/desktop-qt/qml"
qml_root="$qml_import_root/Chaft"

if [ ! -d "$qml_root" ]; then
  printf 'QML source directory not found: %s\n' "$qml_root" >&2
  exit 1
fi

python3 "$script_dir/qml-module-check.py"

tmp_files="$(mktemp "${TMPDIR:-/tmp}/chaft-qml-files.XXXXXX")"
tmp_logs="$(mktemp -d "${TMPDIR:-/tmp}/chaft-qml-lint.XXXXXX")"
trap 'rm -f "$tmp_files"; rm -rf "$tmp_logs"' EXIT HUP INT TERM

find "$qml_root" -type f -name '*.qml' | sort > "$tmp_files"

failed=0
count=0

while IFS= read -r qml_file; do
  count=$((count + 1))
  relative_path="${qml_file#$repo_root/}"
  log_file="$tmp_logs/$count.log"
  printf 'linting %s\n' "$relative_path"
  if ! qmllint -I "$qml_import_root" "$qml_file" > "$log_file" 2>&1; then
    printf 'qmllint failed for %s\n' "$relative_path" >&2
    cat "$log_file" >&2
    failed=1
  fi
done < "$tmp_files"

if [ "$failed" -ne 0 ]; then
  exit 1
fi

printf 'QML lint passed for %s file(s)\n' "$count"
