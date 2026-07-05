#!/usr/bin/env python3
"""Verify WCAG 2.x contrast ratios for every theme in Themes.qml.

Format contract with apps/desktop-qt/qml/Chaft/tokens/Themes.qml (the file is
strictly formatted, so this is a deliberate line-based parser, not a QML
parser):

  * The catalog opens on a line whose content is exactly
    `readonly property var catalog: [` and closes on the first following
    line whose content is exactly `]`.
  * Each theme object opens on a line whose content is exactly `{` and
    closes on a line whose content is exactly `}` or `},`.
  * Inside a theme object every line declares exactly one property:
      - string:   key: "value",          (id, name, tagline)
      - boolean:  dark: true|false,
      - color:    key: "#rrggbb",        (exactly six hex digits)
    Only the final property of an object may omit the trailing comma.
  * Every theme must define id, name, tagline, dark, and all 23 color roles
    in COLOR_ROLES.

The script fails loudly (exit 1 with a message) when the file drifts from
this contract, when fewer than 20 themes parse, or when any theme is missing
a required role.

Fixed theme-role contrast checks are the (foreground, background, minimum)
rows of CONTRAST_PAIRS, using WCAG 2.x relative luminance with sRGB
linearization. Generated author avatar colors are also sampled across all 360
hues with the same dynamic black/white text selection used by Themes.qml.
Each failing pair prints one line:

  theme-id: roleA/roleB ratio X.XX < Y.Y

and the script exits 1. On success it prints a summary and exits 0.

Usage:
  python3 tools/desktop/theme-contrast-check.py
  python3 tools/desktop/theme-contrast-check.py --json

With --json, stdout is only the full ratio matrix (theme -> "fg/bg" -> ratio)
for debugging; the exit status still reflects pass/fail.
"""

import colorsys
import json
import re
import sys
from pathlib import Path

THEMES_QML = Path("apps") / "desktop-qt" / "qml" / "Chaft" / "tokens" / "Themes.qml"
CATALOG_OPEN = "readonly property var catalog: ["
CATALOG_CLOSE = "]"
MIN_THEMES = 20

STRING_KEYS = ("id", "name", "tagline")
BOOL_KEYS = ("dark",)
COLOR_ROLES = (
    "rail",
    "railElevated",
    "railText",
    "sidebar",
    "sidebarInput",
    "sidebarActive",
    "sidebarText",
    "sidebarTextStrong",
    "sidebarTextSoft",
    "sidebarTextMuted",
    "surfaceBase",
    "surfaceRaised",
    "borderSubtle",
    "textStrong",
    "textMuted",
    "accent",
    "onAccent",
    "success",
    "secure",
    "secureSurface",
    "warning",
    "warningSurface",
    "warningText",
)
REQUIRED_KEYS = STRING_KEYS + BOOL_KEYS + COLOR_ROLES

# (foreground role, background role, minimum WCAG contrast ratio)
CONTRAST_PAIRS = (
    ("textStrong", "surfaceBase", 4.5),
    ("textStrong", "surfaceRaised", 4.5),
    ("textMuted", "surfaceBase", 4.5),
    ("sidebarText", "sidebar", 4.5),
    ("sidebarTextStrong", "sidebarActive", 4.5),
    ("sidebarTextMuted", "sidebar", 3.0),
    ("onAccent", "accent", 4.5),
    ("warningText", "warningSurface", 4.5),
    ("secure", "secureSurface", 4.5),
    ("railText", "railElevated", 4.5),
    ("borderSubtle", "surfaceBase", 1.4),
)
AUTHOR_COLOR_HUES = range(360)
AUTHOR_COLOR_MIN_CONTRAST = 4.5

PROPERTY_LINE = re.compile(
    r'^(?P<key>[A-Za-z][A-Za-z0-9]*): (?P<value>"[^"]*"|true|false)(?P<comma>,?)$'
)
COLOR_VALUE = re.compile(r'^"#[0-9a-f]{6}"$')


def fail(message: str) -> None:
    raise SystemExit(f"theme-contrast-check: {message}")


def parse_catalog(path: Path) -> list[dict[str, str]]:
    if not path.is_file():
        fail(f"themes file not found: {path}")

    themes: list[dict[str, str]] = []
    theme: dict[str, str] | None = None
    in_catalog = False
    catalog_closed = False

    for line_no, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.strip()
        if not in_catalog:
            if line == CATALOG_OPEN:
                in_catalog = True
            continue

        if theme is None:
            if line == "{":
                theme = {}
            elif line == CATALOG_CLOSE:
                catalog_closed = True
                break
            else:
                fail(f"line {line_no}: expected '{{' or ']' in catalog, got: {raw_line}")
            continue

        if line in ("}", "},"):
            themes.append(theme)
            theme = None
            continue

        match = PROPERTY_LINE.match(line)
        if not match:
            fail(f"line {line_no}: unrecognized theme property line: {raw_line}")
        key, value = match.group("key"), match.group("value")
        if key in theme:
            fail(f"line {line_no}: duplicate key '{key}' in theme entry")
        if key in COLOR_ROLES and not COLOR_VALUE.match(value):
            fail(f'line {line_no}: color role \'{key}\' must be a "#rrggbb" string, got: {value}')
        theme[key] = value.strip('"')

    if not in_catalog:
        fail(f"catalog marker not found in {path}: {CATALOG_OPEN}")
    if theme is not None or not catalog_closed:
        fail(f"catalog in {path} is not terminated by '{CATALOG_CLOSE}'")
    if len(themes) < MIN_THEMES:
        fail(f"only {len(themes)} theme(s) parsed from {path}, expected at least {MIN_THEMES}")

    for index, entry in enumerate(themes):
        theme_id = entry.get("id", f"catalog[{index}]")
        missing = sorted(key for key in REQUIRED_KEYS if key not in entry)
        if missing:
            fail(f"theme '{theme_id}' is missing required role(s): {', '.join(missing)}")

    return themes


Rgb = tuple[float, float, float]


def linear_channel(value: float) -> float:
    scaled = value / 255.0 if value > 1.0 else value
    if scaled <= 0.03928:
        return scaled / 12.92
    return ((scaled + 0.055) / 1.055) ** 2.4


def rgb_from_hex(color: str) -> Rgb:
    return (
        int(color[1:3], 16) / 255.0,
        int(color[3:5], 16) / 255.0,
        int(color[5:7], 16) / 255.0,
    )


def relative_luminance_rgb(color: Rgb) -> float:
    red = linear_channel(color[0])
    green = linear_channel(color[1])
    blue = linear_channel(color[2])
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast_ratio_rgb(color_a: Rgb, color_b: Rgb) -> float:
    lum_a = relative_luminance_rgb(color_a)
    lum_b = relative_luminance_rgb(color_b)
    lighter, darker = max(lum_a, lum_b), min(lum_a, lum_b)
    return (lighter + 0.05) / (darker + 0.05)


def contrast_ratio(color_a: str, color_b: str) -> float:
    return contrast_ratio_rgb(rgb_from_hex(color_a), rgb_from_hex(color_b))


def author_color_rgb(hue: int, dark: bool) -> Rgb:
    lightness = 0.62 if dark else 0.42
    saturation = 0.55 if dark else 0.62
    return colorsys.hls_to_rgb(hue / 360.0, lightness, saturation)


def readable_text_rgb(background: Rgb) -> Rgb:
    black = (0.0, 0.0, 0.0)
    white = (1.0, 1.0, 1.0)
    return black if contrast_ratio_rgb(black, background) >= contrast_ratio_rgb(white, background) else white


def author_color_worst_ratio(dark: bool) -> tuple[float, int]:
    worst_ratio = float("inf")
    worst_hue = 0
    for hue in AUTHOR_COLOR_HUES:
        background = author_color_rgb(hue, dark)
        foreground = readable_text_rgb(background)
        ratio = contrast_ratio_rgb(foreground, background)
        if ratio < worst_ratio:
            worst_ratio = ratio
            worst_hue = hue
    return worst_ratio, worst_hue


def main() -> int:
    json_mode = False
    for argument in sys.argv[1:]:
        if argument == "--json":
            json_mode = True
        else:
            fail(f"unknown argument: {argument} (only --json is supported)")

    root = Path(__file__).resolve().parents[2]
    themes = parse_catalog(root / THEMES_QML)

    failures: list[str] = []
    matrix: dict[str, dict[str, float]] = {}
    for theme in themes:
        ratios: dict[str, float] = {}
        for foreground, background, minimum in CONTRAST_PAIRS:
            ratio = contrast_ratio(theme[foreground], theme[background])
            ratios[f"{foreground}/{background}"] = round(ratio, 3)
            if ratio < minimum:
                failures.append(
                    f"{theme['id']}: {foreground}/{background} ratio {ratio:.2f} < {minimum:.1f}"
                )
        author_ratio, author_hue = author_color_worst_ratio(theme["dark"] == "true")
        ratios["authorText/authorColor"] = round(author_ratio, 3)
        if author_ratio < AUTHOR_COLOR_MIN_CONTRAST:
            failures.append(
                f"{theme['id']}: authorText/authorColor hue {author_hue} "
                f"ratio {author_ratio:.2f} < {AUTHOR_COLOR_MIN_CONTRAST:.1f}"
            )
        matrix[theme["id"]] = ratios

    if json_mode:
        print(json.dumps(matrix, indent=2, sort_keys=True))
        return 1 if failures else 0

    if failures:
        for failure in failures:
            print(failure)
        return 1

    print(
        f"theme contrast verified: {len(themes)} theme(s), "
        f"{len(themes) * len(CONTRAST_PAIRS)} fixed pair(s), "
        f"{len(themes) * len(AUTHOR_COLOR_HUES)} author color sample(s)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
