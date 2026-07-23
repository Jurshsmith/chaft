#!/usr/bin/env python3
import re
import struct
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LINUX_PACKAGING = ROOT / "packaging" / "linux"
DESKTOP_ID = "io.github.jurshsmith.chaft"


def fail(message):
    raise SystemExit(message)


def parse_lock_file(path):
    values = {}
    assignment = re.compile(r"^([A-Z0-9_]+)='([^']+)'$")
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        match = assignment.fullmatch(line)
        if not match:
            fail(f"{path}:{line_number}: invalid lock-file assignment")
        key, value = match.groups()
        if key in values:
            fail(f"{path}:{line_number}: duplicate {key}")
        values[key] = value
    return values


lock_values = parse_lock_file(LINUX_PACKAGING / "appimage-tools.lock")
tool_prefixes = (
    "LINUXDEPLOY",
    "LINUXDEPLOY_PLUGIN_QT",
    "LINUXDEPLOY_PLUGIN_APPIMAGE",
)
for prefix in tool_prefixes:
    version = lock_values.get(f"{prefix}_VERSION", "")
    url = lock_values.get(f"{prefix}_URL", "")
    digest = lock_values.get(f"{prefix}_SHA256", "")
    if not version:
        fail(f"{prefix}_VERSION is missing")
    if not url.startswith("https://github.com/") or f"/download/{version}/" not in url:
        fail(f"{prefix}_URL must pin the declared GitHub release")
    if "latest" in url.lower() or "continuous" in url.lower():
        fail(f"{prefix}_URL must not use a mutable release")
    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        fail(f"{prefix}_SHA256 must be a lowercase SHA-256 digest")

desktop_values = {}
desktop_path = LINUX_PACKAGING / f"{DESKTOP_ID}.desktop"
for raw_line in desktop_path.read_text(encoding="utf-8").splitlines():
    if "=" in raw_line and not raw_line.startswith("#"):
        key, value = raw_line.split("=", 1)
        desktop_values[key] = value
if desktop_values.get("Exec") != "ChaftDesktop":
    fail("desktop entry must launch ChaftDesktop")
if desktop_values.get("Icon") != DESKTOP_ID:
    fail("desktop entry icon must match the AppStream component id")

metainfo_path = LINUX_PACKAGING / f"{DESKTOP_ID}.metainfo.xml"
component = ET.parse(metainfo_path).getroot()
if component.findtext("id") != DESKTOP_ID:
    fail("AppStream component id does not match the desktop id")
launchable = component.find("launchable")
if launchable is None or launchable.text != f"{DESKTOP_ID}.desktop":
    fail("AppStream launchable does not match the desktop filename")
provided_binary = component.find("./provides/binary")
if provided_binary is None or provided_binary.text != "ChaftDesktop":
    fail("AppStream metadata must provide the installed binary")

packaging_script = (
    ROOT / "tools" / "desktop" / "package-linux-appimage.sh"
).read_text(encoding="utf-8")
for required_contract in (
    "--library \"$ffi_library\"",
    "EXTRA_PLATFORM_PLUGINS=libqoffscreen.so",
    "QML_SOURCES_PATHS=",
):
    if required_contract not in packaging_script:
        fail(f"AppImage packager is missing required contract: {required_contract}")

icon_path = LINUX_PACKAGING / f"{DESKTOP_ID}.png"
with icon_path.open("rb") as icon:
    header = icon.read(24)
if len(header) != 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
    fail("Linux icon is not a PNG")
width, height = struct.unpack(">II", header[16:24])
if (width, height) != (512, 512):
    fail(f"Linux icon must be 512x512, got {width}x{height}")

print("Linux AppImage packaging contract passed")
