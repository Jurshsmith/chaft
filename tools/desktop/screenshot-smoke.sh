#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

profile="${1:-debug}"
case "$profile" in
  debug|release) ;;
  *)
    printf 'usage: %s [debug|release] [output.png]\n' "$0" >&2
    exit 2
    ;;
esac

output_path="${2:-$repo_root/build/desktop-$profile/smoke/visual-smoke.png}"
baseline_path="${CHAFT_DESKTOP_SCREENSHOT_BASELINE:-$script_dir/screenshot-baseline.json}"
case "$output_path" in
  /*) ;;
  *)
    output_dir="$(dirname "$output_path")"
    output_base="$(basename "$output_path")"
    mkdir -p "$output_dir"
    output_path="$(CDPATH= cd "$output_dir" && pwd)/$output_base"
    ;;
esac

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

python_bin="${CHAFT_PYTHON_BIN:-}"
if [ -z "$python_bin" ]; then
  if [ -x /usr/bin/python3 ]; then
    python_bin=/usr/bin/python3
  else
    python_bin=python3
  fi
fi

require_tool "$python_bin"

mkdir -p "$(dirname "$output_path")"
rm -f "$output_path"

default_screenshot_timeout_ms="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-30000}"
default_ui_states="setup,setup-identity,setup-add-device,setup-access-updates,setup-security,setup-backup,setup-room-access,setup-request,setup-request-approved,setup-request-lost,setup-request-reinvite,setup-invite,setup-approval-invite,setup-invite-lost,first-sync-waiting,first-sync-recovery,drawer,member-roles,direct-message,palette,entry,entry-join,entry-restore,entry-restore-failed,entry-join-invite,entry-approval-invite,entry-workspace-card,entry-workspace-card-invite-only,entry-request-sent,post-create,add-workspace,channel-details,private-channel-details,private-channel-repair-failed,private-channel-repair-saved,private-channel-inspector,channel-archived,reaction-picker,external-link"
explicit_ui_states=0
ui_states="${CHAFT_SMOKE_UI_STATES:-$default_ui_states}"
if [ -n "${CHAFT_SMOKE_UI_STATES:-}" ]; then
  explicit_ui_states=1
fi

state_requested() {
  state_name="$1"
  if [ "$explicit_ui_states" -eq 0 ]; then
    return 0
  fi
  for requested_state in $(printf '%s' "$ui_states" | tr ',' ' '); do
    if [ "$requested_state" = "$state_name" ]; then
      return 0
    fi
  done
  return 1
}

if state_requested default; then
  CHAFT_DESKTOP_SMOKE_SCREENSHOT="$output_path" \
  CHAFT_DESKTOP_SMOKE_SCREENSHOT_DELAY_MS="${CHAFT_DESKTOP_SMOKE_SCREENSHOT_DELAY_MS:-1500}" \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="$default_screenshot_timeout_ms" \
    "$script_dir/smoke.sh" "$profile"

  "$python_bin" - "$output_path" <<'PY'
import os
import struct
import sys
import zlib

path = sys.argv[1]
data = open(path, "rb").read()

def fail(message):
    raise SystemExit(message)

if len(data) < 32 * 1024:
    fail(f"screenshot is unexpectedly small: {len(data)} bytes")

if data[:8] != b"\x89PNG\r\n\x1a\n":
    fail("screenshot is not a PNG file")

offset = 8
width = height = bit_depth = color_type = interlace = None
idat = bytearray()

while offset + 12 <= len(data):
    length = struct.unpack(">I", data[offset : offset + 4])[0]
    chunk_type = data[offset + 4 : offset + 8]
    chunk_data_start = offset + 8
    chunk_data_end = chunk_data_start + length
    crc_end = chunk_data_end + 4
    if crc_end > len(data):
        fail("PNG chunk extends past end of file")

    chunk_data = data[chunk_data_start:chunk_data_end]
    if chunk_type == b"IHDR":
        width, height, bit_depth, color_type, _, _, interlace = struct.unpack(
            ">IIBBBBB", chunk_data
        )
    elif chunk_type == b"IDAT":
        idat.extend(chunk_data)
    elif chunk_type == b"IEND":
        break

    offset = crc_end

if width is None or height is None:
    fail("PNG is missing IHDR")
if width < 800 or height < 500:
    fail(f"screenshot dimensions are too small: {width}x{height}")
if bit_depth != 8:
    fail(f"unsupported smoke screenshot bit depth: {bit_depth}")
if interlace != 0:
    fail("interlaced smoke screenshots are not supported by this verifier")

channels_by_color_type = {
    0: 1,
    2: 3,
    3: 1,
    4: 2,
    6: 4,
}
channels = channels_by_color_type.get(color_type)
if channels is None:
    fail(f"unsupported smoke screenshot color type: {color_type}")

try:
    raw = zlib.decompress(bytes(idat))
except zlib.error as error:
    fail(f"failed to decompress screenshot pixels: {error}")

stride = width * channels
bpp = max(1, channels)
expected_len = (stride + 1) * height
if len(raw) < expected_len:
    fail("decompressed screenshot is shorter than expected")

def paeth(a, b, c):
    p = a + b - c
    pa = abs(p - a)
    pb = abs(p - b)
    pc = abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c

rows = []
pos = 0
previous = bytearray(stride)
for _ in range(height):
    filter_type = raw[pos]
    pos += 1
    row = bytearray(raw[pos : pos + stride])
    pos += stride

    for index in range(stride):
        left = row[index - bpp] if index >= bpp else 0
        up = previous[index]
        up_left = previous[index - bpp] if index >= bpp else 0
        if filter_type == 0:
            value = row[index]
        elif filter_type == 1:
            value = (row[index] + left) & 0xFF
        elif filter_type == 2:
            value = (row[index] + up) & 0xFF
        elif filter_type == 3:
            value = (row[index] + ((left + up) // 2)) & 0xFF
        elif filter_type == 4:
            value = (row[index] + paeth(left, up, up_left)) & 0xFF
        else:
            fail(f"unsupported PNG row filter: {filter_type}")
        row[index] = value

    rows.append(row)
    previous = row

first_pixel = None
distinct_pixel_count = 0
step = max(1, height // 200)
for row in rows[::step]:
    for index in range(0, stride, channels * max(1, width // 200)):
        pixel = bytes(row[index : index + channels])
        if first_pixel is None:
            first_pixel = pixel
        elif pixel != first_pixel:
            distinct_pixel_count += 1
            if distinct_pixel_count >= 8:
                print(
                    f"screenshot smoke verified: {width}x{height} {len(data)} bytes at {path}"
                )
                raise SystemExit(0)

fail("screenshot appears blank or nearly uniform")
PY

  if [ -f "$baseline_path" ]; then
    "$python_bin" "$script_dir/screenshot-baseline.py" "$output_path" "$baseline_path"
  else
    printf 'screenshot baseline not found: %s\n' "$baseline_path" >&2
    exit 1
  fi
fi

empty_output="$(dirname "$output_path")/visual-smoke-empty.png"
empty_baseline="$script_dir/screenshot-baseline-empty.json"
if state_requested empty; then
  if [ ! -f "$empty_baseline" ]; then
    printf 'screenshot baseline not found for empty workspace: %s\n' \
      "$empty_baseline" >&2
    exit 1
  fi
  rm -f "$empty_output"
  "$script_dir/empty-workspace-smoke.sh" "$profile" "$empty_output"
  "$python_bin" "$script_dir/screenshot-baseline.py" "$empty_output" "$empty_baseline"
  printf 'screenshot state verified: empty at %s\n' "$empty_output"
fi

empty_request_output="$(dirname "$output_path")/visual-smoke-empty-request-ready.png"
empty_request_baseline="$script_dir/screenshot-baseline-empty-request-ready.json"
if state_requested empty-request-ready; then
  if [ ! -f "$empty_request_baseline" ]; then
    printf 'screenshot baseline not found for empty pending request: %s\n' \
      "$empty_request_baseline" >&2
    exit 1
  fi
  rm -f "$empty_request_output"
  CHAFT_EMPTY_WORKSPACE_PENDING_REQUEST=1 \
    "$script_dir/empty-workspace-smoke.sh" "$profile" "$empty_request_output"
  "$python_bin" "$script_dir/screenshot-baseline.py" "$empty_request_output" \
    "$empty_request_baseline"
  printf 'screenshot state verified: empty-request-ready at %s\n' \
    "$empty_request_output"
fi

empty_request_sent_output="$(dirname "$output_path")/visual-smoke-empty-request-sent.png"
empty_request_sent_baseline="$script_dir/screenshot-baseline-empty-request-sent.json"
if state_requested empty-request-sent; then
  if [ ! -f "$empty_request_sent_baseline" ]; then
    printf 'screenshot baseline not found for sent empty pending request: %s\n' \
      "$empty_request_sent_baseline" >&2
    exit 1
  fi
  rm -f "$empty_request_sent_output"
  CHAFT_EMPTY_WORKSPACE_PENDING_REQUEST=1 \
  CHAFT_EMPTY_WORKSPACE_PENDING_REQUEST_STATUS=sent \
    "$script_dir/empty-workspace-smoke.sh" "$profile" "$empty_request_sent_output"
  "$python_bin" "$script_dir/screenshot-baseline.py" "$empty_request_sent_output" \
    "$empty_request_sent_baseline"
  printf 'screenshot state verified: empty-request-sent at %s\n' \
    "$empty_request_sent_output"
fi

empty_request_failed_output="$(dirname "$output_path")/visual-smoke-empty-request-failed.png"
empty_request_failed_baseline="$script_dir/screenshot-baseline-empty-request-failed.json"
if state_requested empty-request-failed; then
  if [ ! -f "$empty_request_failed_baseline" ]; then
    printf 'screenshot baseline not found for failed empty pending request: %s\n' \
      "$empty_request_failed_baseline" >&2
    exit 1
  fi
  rm -f "$empty_request_failed_output"
  CHAFT_EMPTY_WORKSPACE_PENDING_REQUEST=1 \
  CHAFT_EMPTY_WORKSPACE_PENDING_REQUEST_STATUS=send_failed \
    "$script_dir/empty-workspace-smoke.sh" "$profile" "$empty_request_failed_output"
  "$python_bin" "$script_dir/screenshot-baseline.py" "$empty_request_failed_output" \
    "$empty_request_failed_baseline"
  printf 'screenshot state verified: empty-request-failed at %s\n' \
    "$empty_request_failed_output"
fi

for ui_state in $(printf '%s' "$ui_states" | tr ',' ' '); do
  case "$ui_state" in
    default)
      # The default workspace screenshot is captured before this loop using
      # screenshot-baseline.json. Allow CHAFT_SMOKE_UI_STATES=default as an
      # explicit focused smoke run without looking for a duplicate baseline.
      continue
      ;;
    empty|empty-request-ready|empty-request-sent|empty-request-failed)
      # These no-workspace states are captured above with empty-workspace-smoke.
      # Running them through the seeded visual workspace path would overwrite
      # the intended screenshot with an unrelated workspace view.
      continue
      ;;
  esac
  ui_state_bytes="$(printf '%s' "$ui_state" | wc -c | tr -d ' ')"
  if [ "$ui_state_bytes" -gt 32 ]; then
    printf 'screenshot state %s is %s bytes; desktop smoke states must be <= 32 bytes\n' \
      "$ui_state" "$ui_state_bytes" >&2
    exit 1
  fi
  state_output="$(dirname "$output_path")/visual-smoke-$ui_state.png"
  state_baseline="$script_dir/screenshot-baseline-$ui_state.json"
  if [ ! -f "$state_baseline" ]; then
    printf 'screenshot baseline not found for state %s: %s\n' \
      "$ui_state" "$state_baseline" >&2
    exit 1
  fi
  screenshot_delay_ms=250
  screenshot_timeout_ms="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-15000}"
  if [ "$ui_state" = "setup-invite" ]; then
    screenshot_delay_ms=2500
  elif [ "$ui_state" = "setup-identity" ]; then
    screenshot_delay_ms=750
  elif [ "$ui_state" = "setup-add-device" ]; then
    screenshot_delay_ms=1000
  elif [ "$ui_state" = "setup-access-updates" ]; then
    screenshot_delay_ms=1500
    screenshot_timeout_ms="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-30000}"
  elif [ "$ui_state" = "setup-security" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "setup-backup" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "setup-room-access" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "setup-approval-invite" ]; then
    screenshot_delay_ms=2500
  elif [ "$ui_state" = "setup-invite-lost" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "first-sync-waiting" ] || \
       [ "$ui_state" = "first-sync-recovery" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "setup-request-approved" ]; then
    screenshot_delay_ms=2500
  elif [ "$ui_state" = "setup-request-lost" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "setup-request-reinvite" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "setup-request" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "reaction-picker" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "member-roles" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "direct-message" ]; then
    screenshot_delay_ms=2500
  elif [ "$ui_state" = "entry-restore" ] || \
       [ "$ui_state" = "entry-restore-failed" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "entry-approval-invite" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "entry-request-sent" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "post-create" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "add-workspace" ]; then
    screenshot_delay_ms=1500
    screenshot_timeout_ms="${CHAFT_DESKTOP_SMOKE_TIMEOUT_MS:-60000}"
  elif [ "$ui_state" = "channel-details" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "private-channel-details" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "private-channel-repair-failed" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "private-channel-repair-saved" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "private-channel-inspector" ]; then
    screenshot_delay_ms=1500
  elif [ "$ui_state" = "channel-archived" ]; then
    screenshot_delay_ms=2500
  elif [ "$ui_state" = "external-link" ]; then
    screenshot_delay_ms=1500
  fi
  archive_design=0
  if [ "$ui_state" = "channel-archived" ]; then
    archive_design=1
  fi
  access_policy="${CHAFT_VISUAL_SMOKE_ACCESS_POLICY:-invite-only}"
  if [ "$ui_state" = "setup-request" ] || \
     [ "$ui_state" = "setup-request-approved" ] || \
     [ "$ui_state" = "setup-request-lost" ] || \
     [ "$ui_state" = "setup-request-reinvite" ] || \
     [ "$ui_state" = "setup-approval-invite" ]; then
    access_policy="request-access"
  fi
  reinvite_request=0
  if [ "$ui_state" = "setup-request-reinvite" ]; then
    reinvite_request=1
  fi
  request_lost_invite=0
  if [ "$ui_state" = "setup-request-lost" ]; then
    request_lost_invite=1
  fi
  lost_invite=0
  if [ "$ui_state" = "setup-invite-lost" ]; then
    lost_invite=1
  fi
  rm -f "$state_output"
  CHAFT_DESKTOP_SMOKE_SCREENSHOT="$state_output" \
  CHAFT_DESKTOP_SMOKE_SCREENSHOT_DELAY_MS="$screenshot_delay_ms" \
  CHAFT_DESKTOP_SMOKE_TIMEOUT_MS="$screenshot_timeout_ms" \
  CHAFT_VISUAL_SMOKE_ARCHIVE_DESIGN="$archive_design" \
  CHAFT_VISUAL_SMOKE_ACCESS_POLICY="$access_policy" \
  CHAFT_VISUAL_SMOKE_REINVITE_REQUEST="$reinvite_request" \
  CHAFT_VISUAL_SMOKE_REQUEST_LOST_INVITE="$request_lost_invite" \
  CHAFT_VISUAL_SMOKE_LOST_INVITE="$lost_invite" \
  CHAFT_SMOKE_UI_STATE="$ui_state" \
    "$script_dir/smoke.sh" "$profile"
  "$python_bin" "$script_dir/screenshot-baseline.py" "$state_output" "$state_baseline"
  printf 'screenshot state verified: %s at %s\n' "$ui_state" "$state_output"
done
