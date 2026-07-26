#!/usr/bin/env python3
import hashlib
import json
import re
import struct
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LINUX_PACKAGING = ROOT / "packaging" / "linux"
QT_PACKAGING = ROOT / "packaging" / "qt"
DESKTOP_ID = "io.github.jurshsmith.chaft"
PACKAGE_NOTICE_FILES = (
    "LICENSE",
    "THIRD_PARTY_NOTICES.txt",
    "LICENSE.LGPL3",
    "LICENSE.GPL3",
    "QT-CORRESPONDING-SOURCE.json",
)


def fail(message):
    raise SystemExit(message)


def load_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"{path}: invalid JSON: {error}")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


package_manifest_path = QT_PACKAGING / "QT-CORRESPONDING-SOURCE.json"
build_manifest_path = ROOT / "tools" / "qt" / "qt-6.8.4.json"
package_manifest = load_json(package_manifest_path)
build_manifest = load_json(build_manifest_path)

if package_manifest.get("version") != build_manifest.get("qtVersion"):
    fail("packaged Qt version does not match the source-build manifest")

expected_release_assets = {
    "bundle": "Chaft-Qt-6.8.4-corresponding-source.zip",
    "checksum": "Chaft-Qt-6.8.4-corresponding-source.zip.sha256",
}
if package_manifest.get("releaseAssets") != expected_release_assets:
    fail("packaged Qt manifest must name the exact corresponding-source assets")

platform_names = {
    "Linux": "linux",
    "macOS": "macos",
    "Windows": "windows",
}


def package_module_contract(module):
    try:
        platforms = tuple(platform_names[name] for name in module["platforms"])
        return module["name"], platforms, module["url"], module["sha256"]
    except (KeyError, TypeError) as error:
        fail(f"invalid packaged Qt module record: {error}")


def build_module_contract(module):
    try:
        return (
            module["name"],
            tuple(module["platforms"]),
            module["url"],
            module["sha256"],
        )
    except (KeyError, TypeError) as error:
        fail(f"invalid source-build Qt module record: {error}")


packaged_modules = tuple(
    package_module_contract(module)
    for module in package_manifest.get("sourceModules", ())
)
build_modules = tuple(
    build_module_contract(module)
    for module in sorted(
        build_manifest.get("modules", ()),
        key=lambda module: module.get("order", -1),
    )
)
if packaged_modules != build_modules:
    fail(
        "packaged Qt module order/platforms/URLs/digests do not match "
        "tools/qt/qt-6.8.4.json"
    )

all_platforms = ("linux", "macos", "windows")


def package_patch_contract(patch):
    try:
        platforms = tuple(platform_names[name] for name in patch["platforms"])
        return patch["module"], platforms, patch["url"], patch["sha256"]
    except (KeyError, TypeError) as error:
        fail(f"invalid packaged Qt patch record: {error}")


def build_patch_contract(patch):
    try:
        return patch["module"], all_platforms, patch["url"], patch["sha256"]
    except (KeyError, TypeError) as error:
        fail(f"invalid source-build Qt patch record: {error}")


packaged_patches = tuple(
    package_patch_contract(patch)
    for patch in package_manifest.get("securityPatches", ())
)
build_patches = tuple(
    build_patch_contract(patch)
    for patch in sorted(
        build_manifest.get("patches", ()),
        key=lambda patch: patch.get("order", -1),
    )
)
if packaged_patches != build_patches:
    fail(
        "packaged Qt patch order/platforms/URLs/digests do not match "
        "tools/qt/qt-6.8.4.json"
    )

expected_license_hashes = {
    "LICENSE.LGPL3": "da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768",
    "LICENSE.GPL3": "8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903",
}
for filename, expected_digest in expected_license_hashes.items():
    path = QT_PACKAGING / filename
    if sha256(path) != expected_digest:
        fail(f"{path} is not the verbatim license from the pinned Qt source")

notice = (QT_PACKAGING / "THIRD_PARTY_NOTICES.txt").read_text(encoding="utf-8")
for required_notice in (
    "Qt 6.8.4",
    "GNU Lesser General Public License, version 3",
    "QT-CORRESPONDING-SOURCE.json",
    "dynamically linked",
):
    if required_notice not in notice:
        fail(f"Qt third-party notice is missing: {required_notice}")


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

cmake = (ROOT / "apps" / "desktop-qt" / "CMakeLists.txt").read_text(
    encoding="utf-8"
)
for required_file in PACKAGE_NOTICE_FILES:
    if required_file not in cmake:
        fail(f"CMake install rules omit package notice: {required_file}")
for required_destination in (
    "ChaftDesktop.app/Contents/Resources/doc/Chaft",
    "${CMAKE_INSTALL_DATADIR}/doc/Chaft",
):
    if required_destination not in cmake:
        fail(f"CMake install rules omit package notice destination: {required_destination}")

for smoke_name in (
    "appimage-smoke.sh",
    "macos-dmg-smoke.sh",
    "windows-zip-smoke.ps1",
):
    smoke_path = ROOT / "tools" / "desktop" / smoke_name
    smoke = smoke_path.read_text(encoding="utf-8")
    for required_file in PACKAGE_NOTICE_FILES:
        if required_file not in smoke:
            fail(f"{smoke_path} does not verify package notice: {required_file}")

package_smoke = (
    ROOT / "tools" / "desktop" / "package-smoke.sh"
).read_text(encoding="utf-8")
macos_dmg_smoke_path = (
    ROOT / "tools" / "desktop" / "macos-dmg-smoke.sh"
)
macos_dmg_smoke = macos_dmg_smoke_path.read_text(encoding="utf-8")
if '"$script_dir/macos-dmg-smoke.sh"' not in package_smoke:
    fail("package smoke must delegate macOS validation to the DMG smoke")
if "hdiutil" in package_smoke:
    fail("package smoke must not inspect a DMG and then launch staging bytes")
for required_contract in (
    'ditto "$mounted_app" "$portable_app"',
    'desktop_binary="$portable_app/Contents/MacOS/ChaftDesktop"',
    'hdiutil detach -quiet "$dmg_mount_dir"',
    "QT_QPA_PLATFORM=cocoa",
    'exec "$desktop_binary"',
    'macOS DMG smoke timed out after %ss',
):
    if required_contract not in macos_dmg_smoke:
        fail(
            "macOS DMG smoke is missing required contract: "
            f"{required_contract}"
        )
if macos_dmg_smoke.rindex('hdiutil detach -quiet "$dmg_mount_dir"') > (
    macos_dmg_smoke.index('exec "$desktop_binary"')
):
    fail("macOS DMG smoke must launch only after copying and detaching the DMG")
if "offscreen" in macos_dmg_smoke:
    fail("macOS DMG smoke must not require a non-shipping offscreen plugin")

linux_dependencies_path = (
    ROOT / "tools" / "qt" / "install-linux-dependencies.sh"
)
linux_dependencies = linux_dependencies_path.read_text(encoding="utf-8")
try:
    appimage_runtime_packages = linux_dependencies.split(
        "runtime_packages=(\n", 1
    )[1].split("\n)", 1)[0]
except IndexError:
    fail("Linux dependencies must define an AppImage runtime package set")
for package in ("libegl1", "libopengl0"):
    if not re.search(
        rf"^\s*{re.escape(package)}\s*$",
        appimage_runtime_packages,
        re.M,
    ):
        fail(f"AppImage runtime profile must install {package}")

appimage_runtime_install = (
    "tools/qt/install-linux-dependencies.sh install appimage-runtime"
)
for workflow_name in ("ci.yml", "build-desktop-release-inputs.yml"):
    workflow_path = ROOT / ".github" / "workflows" / workflow_name
    workflow = workflow_path.read_text(encoding="utf-8")
    if appimage_runtime_install not in workflow:
        fail(
            f"{workflow_path} must install the centralized host GL dispatch "
            "runtime before clean AppImage smoke"
        )

icon_path = LINUX_PACKAGING / f"{DESKTOP_ID}.png"
with icon_path.open("rb") as icon:
    header = icon.read(24)
if len(header) != 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
    fail("Linux icon is not a PNG")
width, height = struct.unpack(">II", header[16:24])
if (width, height) != (512, 512):
    fail(f"Linux icon must be 512x512, got {width}x{height}")

print("Linux AppImage packaging contract passed")
