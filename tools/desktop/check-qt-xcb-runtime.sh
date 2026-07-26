#!/usr/bin/env sh
set -eu

usage() {
  printf 'usage: %s QT_PREFIX\n' "$0" >&2
}

if [ "$#" -ne 1 ]; then
  usage
  exit 2
fi

qt_prefix="$1"
if [ ! -d "$qt_prefix" ]; then
  printf 'Qt prefix not found: %s\n' "$qt_prefix" >&2
  exit 1
fi
qt_prefix="$(CDPATH= cd "$qt_prefix" && pwd)"
qt_library_dir="$qt_prefix/lib"
qt_xcb_plugin="$qt_prefix/plugins/platforms/libqxcb.so"
qt_xcb_integration_dir="$qt_prefix/plugins/xcbglintegrations"

if [ ! -d "$qt_library_dir" ]; then
  printf 'Qt library directory not found: %s\n' "$qt_library_dir" >&2
  exit 1
fi
if [ ! -f "$qt_xcb_plugin" ]; then
  printf 'Qt XCB platform plugin not found: %s\n' "$qt_xcb_plugin" >&2
  exit 1
fi
if [ ! -d "$qt_xcb_integration_dir" ]; then
  printf 'Qt XCB GL integration directory not found: %s\n' \
    "$qt_xcb_integration_dir" >&2
  exit 1
fi

unresolved_report=""
ldd_failure_report=""

check_shared_object() {
  shared_object="$1"
  if ldd_output="$(
    LD_LIBRARY_PATH="$qt_library_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      ldd "$shared_object" 2>&1
  )"; then
    :
  else
    ldd_failure_report="${ldd_failure_report}
$shared_object:
$ldd_output"
  fi

  unresolved="$(
    printf '%s\n' "$ldd_output" |
      awk '
        $2 == "=>" && $3 == "not" && $4 == "found" && !seen[$1]++ {
          print $1
        }
      '
  )"
  if [ -n "$unresolved" ]; then
    unresolved_report="${unresolved_report}
$shared_object:
$(printf '%s\n' "$unresolved" | sed 's/^/  /')"
  fi
}

check_shared_object "$qt_xcb_plugin"

integration_count=0
for integration in "$qt_xcb_integration_dir"/*.so; do
  if [ ! -f "$integration" ]; then
    continue
  fi
  integration_count=$((integration_count + 1))
  check_shared_object "$integration"
done
if [ "$integration_count" -eq 0 ]; then
  printf 'Qt XCB GL integration plugins not found: %s/*.so\n' \
    "$qt_xcb_integration_dir" >&2
  exit 1
fi

if [ -n "$ldd_failure_report" ] || [ -n "$unresolved_report" ]; then
  printf 'Qt XCB runtime dependency preflight failed.\n' >&2
  if [ -n "$ldd_failure_report" ]; then
    printf 'ldd could not inspect:%s\n' "$ldd_failure_report" >&2
  fi
  if [ -n "$unresolved_report" ]; then
    printf 'Unresolved dependencies:%s\n' "$unresolved_report" >&2
  fi
  exit 1
fi

printf 'Qt XCB runtime dependency preflight passed: %s\n' "$qt_prefix"
