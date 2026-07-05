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

require_tool python3

mkdir -p "$(dirname "$output_path")"
rm -f "$output_path"

CHAFT_DESKTOP_SMOKE_SCREENSHOT="$output_path" \
  "$script_dir/smoke.sh" "$profile"

python3 - "$output_path" <<'PY'
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
  python3 "$script_dir/screenshot-baseline.py" "$output_path" "$baseline_path"
else
  printf 'screenshot baseline not found: %s\n' "$baseline_path" >&2
  exit 1
fi

ui_states="${CHAFT_SMOKE_UI_STATES:-setup,drawer,palette}"
for ui_state in $(printf '%s' "$ui_states" | tr ',' ' '); do
  state_output="$(dirname "$output_path")/visual-smoke-$ui_state.png"
  state_baseline="$script_dir/screenshot-baseline-$ui_state.json"
  if [ ! -f "$state_baseline" ]; then
    printf 'screenshot baseline not found for state %s: %s\n' \
      "$ui_state" "$state_baseline" >&2
    exit 1
  fi
  rm -f "$state_output"
  CHAFT_DESKTOP_SMOKE_SCREENSHOT="$state_output" \
  CHAFT_SMOKE_UI_STATE="$ui_state" \
    "$script_dir/smoke.sh" "$profile"
  python3 "$script_dir/screenshot-baseline.py" "$state_output" "$state_baseline"
  printf 'screenshot state verified: %s at %s\n' "$ui_state" "$state_output"
done
