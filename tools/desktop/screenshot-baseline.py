#!/usr/bin/env python3
import json
import math
import struct
import sys
import zlib
from pathlib import Path


def fail(message):
    raise SystemExit(message)


def parse_png(path):
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        fail("screenshot is not a PNG file")

    offset = 8
    width = height = bit_depth = color_type = interlace = None
    idat = bytearray()

    while offset + 12 <= len(data):
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        chunk_type = data[offset + 4 : offset + 8]
        chunk_start = offset + 8
        chunk_end = chunk_start + length
        crc_end = chunk_end + 4
        if crc_end > len(data):
            fail("PNG chunk extends past end of file")

        chunk_data = data[chunk_start:chunk_end]
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
    if bit_depth != 8:
        fail(f"unsupported screenshot bit depth: {bit_depth}")
    if interlace != 0:
        fail("interlaced screenshots are not supported")

    channels_by_color_type = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
    channels = channels_by_color_type.get(color_type)
    if channels is None:
        fail(f"unsupported screenshot color type: {color_type}")

    try:
        raw = zlib.decompress(bytes(idat))
    except zlib.error as error:
        fail(f"failed to decompress screenshot pixels: {error}")

    stride = width * channels
    bpp = max(1, channels)
    expected_len = (stride + 1) * height
    if len(raw) < expected_len:
        fail("decompressed screenshot is shorter than expected")

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

    return data, width, height, channels, rows


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


def luminance(pixel):
    if len(pixel) == 1:
        return float(pixel[0])
    return 0.2126 * pixel[0] + 0.7152 * pixel[1] + 0.0722 * pixel[2]


def screenshot_metrics(path):
    data, width, height, channels, rows = parse_png(path)
    pixels = []
    y_step = max(1, height // 200)
    x_step = max(1, width // 200)
    for y in range(0, height, y_step):
        row = rows[y]
        for x in range(0, width, x_step):
            index = x * channels
            pixels.append(tuple(row[index : index + min(channels, 3)]))

    if not pixels:
        fail("screenshot produced no sampled pixels")

    lum = [luminance(pixel) for pixel in pixels]
    mean = sum(lum) / len(lum)
    variance = sum((value - mean) ** 2 for value in lum) / len(lum)
    quadrant_lum = [[], [], [], []]

    for y in range(0, height, y_step):
        row = rows[y]
        for x in range(0, width, x_step):
            index = x * channels
            qx = 1 if x >= width // 2 else 0
            qy = 1 if y >= height // 2 else 0
            quadrant_lum[qy * 2 + qx].append(
                luminance(tuple(row[index : index + min(channels, 3)]))
            )

    quadrant_means = [
        sum(values) / len(values) if values else 0.0 for values in quadrant_lum
    ]
    return {
        "bytes": len(data),
        "distinctSampledPixels": len(set(pixels)),
        "height": height,
        "luminanceStddev": math.sqrt(variance),
        "quadrantLuminanceSpread": max(quadrant_means) - min(quadrant_means),
        "width": width,
    }


def assert_minimum(metrics, baseline, metric_name, baseline_name):
    expected = baseline.get(baseline_name)
    if expected is None:
        return
    actual = metrics[metric_name]
    if actual < expected:
        fail(f"{metric_name} {actual:.3f} below baseline minimum {expected}")


def main():
    if len(sys.argv) != 3:
        fail("usage: screenshot-baseline.py screenshot.png baseline.json")

    screenshot_path = sys.argv[1]
    baseline = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
    metrics = screenshot_metrics(screenshot_path)

    assert_minimum(metrics, baseline, "width", "minWidth")
    assert_minimum(metrics, baseline, "height", "minHeight")
    assert_minimum(metrics, baseline, "bytes", "minBytes")
    assert_minimum(
        metrics, baseline, "distinctSampledPixels", "minDistinctSampledPixels"
    )
    assert_minimum(metrics, baseline, "luminanceStddev", "minLuminanceStddev")
    assert_minimum(
        metrics, baseline, "quadrantLuminanceSpread", "minQuadrantLuminanceSpread"
    )

    print(
        "screenshot baseline verified: "
        f"{metrics['width']}x{metrics['height']} "
        f"distinct={metrics['distinctSampledPixels']} "
        f"luma_stddev={metrics['luminanceStddev']:.2f} "
        f"quadrant_spread={metrics['quadrantLuminanceSpread']:.2f}"
    )


if __name__ == "__main__":
    main()
