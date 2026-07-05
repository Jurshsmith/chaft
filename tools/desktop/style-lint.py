#!/usr/bin/env python3
"""Lint desktop QML for hard-coded style values that belong in theme tokens.

Scans every apps/desktop-qt/qml/**/*.qml file, excluding the token catalog
directory (qml/Chaft/tokens/) and any qml/ChaftStyle/ directory (excluded by
path pattern even though it may not exist yet).

Violations, checked line by line:
  * a quoted hex color, i.e. "#" plus 3-8 hex digits inside double quotes
    (e.g. color: "#12151f")
  * the quoted named color "white"; "transparent" is allowed
  * font.pixelSize: followed by an integer literal (e.g. font.pixelSize: 13)
  * radius: followed by an integer literal greater than 3; only the bare
    `radius` property counts (dotted properties such as shadow.radius do not)

border.width integer literals are allowed (no rule matches plain integers).
A line containing the marker comment `// style-lint-allow` is never flagged.

Output is one `path:line: message` line per violation (paths relative to the
repo root, sorted) and exit status 1 when any violation exists. On success it
prints `style lint passed: N file(s)` and exits 0.

Wired into the desktop CI gate; run manually with:
  python3 tools/desktop/style-lint.py
"""

import re
import sys
from pathlib import Path

QML_ROOT = Path("apps") / "desktop-qt" / "qml"
EXCLUDED_SUBTREES = (("Chaft", "tokens"), ("ChaftStyle",))
ALLOW_MARKER = "// style-lint-allow"

HEX_COLOR = re.compile(r'"#[0-9a-fA-F]{3,8}"')
NAMED_WHITE = re.compile(r'"white"')
FONT_PIXEL_SIZE = re.compile(r"font\.pixelSize:\s*(\d+)(?![\w.])")
BARE_RADIUS = re.compile(r"(?<![\w.])radius:\s*(\d+)(?![\w.])")
MAX_RADIUS_LITERAL = 3


def fail(message: str) -> None:
    raise SystemExit(f"style-lint: {message}")


def excluded(relative: Path) -> bool:
    parts = relative.parts
    return any(
        parts[index : index + len(subtree)] == subtree
        for subtree in EXCLUDED_SUBTREES
        for index in range(len(parts))
    )


def qml_files(root: Path) -> list[Path]:
    qml_root = root / QML_ROOT
    if not qml_root.is_dir():
        fail(f"QML directory not found: {qml_root}")
    return sorted(
        path
        for path in qml_root.rglob("*.qml")
        if path.is_file() and not excluded(path.relative_to(qml_root))
    )


def line_violations(line: str) -> list[str]:
    messages: list[str] = []
    for match in HEX_COLOR.finditer(line):
        messages.append(f"hard-coded hex color {match.group(0)} (use theme tokens)")
    for _ in NAMED_WHITE.finditer(line):
        messages.append('named color "white" (use theme tokens; only "transparent" is allowed)')
    for match in FONT_PIXEL_SIZE.finditer(line):
        messages.append(f"hard-coded font.pixelSize {match.group(1)} (use token type scale)")
    for match in BARE_RADIUS.finditer(line):
        if int(match.group(1)) > MAX_RADIUS_LITERAL:
            messages.append(f"hard-coded radius {match.group(1)} (integer > {MAX_RADIUS_LITERAL}, use token radii)")
    return messages


def main() -> int:
    if len(sys.argv) > 1:
        fail(f"unknown argument: {sys.argv[1]} (no arguments are supported)")

    root = Path(__file__).resolve().parents[2]
    files = qml_files(root)

    violations: list[str] = []
    for path in files:
        display = path.relative_to(root).as_posix()
        for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if ALLOW_MARKER in line:
                continue
            violations.extend(f"{display}:{line_no}: {message}" for message in line_violations(line))

    if violations:
        for violation in violations:
            print(violation)
        return 1

    print(f"style lint passed: {len(files)} file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
