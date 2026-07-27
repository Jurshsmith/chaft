#!/usr/bin/env python3
import hashlib
import json
import os
import re
import struct
import subprocess
import tempfile
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
QT_XCB_RUNTIME_PACKAGES = {
    "libfontconfig1",
    "libfreetype6",
    "libglib2.0-0",
    "libice6",
    "libsm6",
    "libx11-6",
    "libx11-xcb1",
    "libxcb1",
    "libxcb-cursor0",
    "libxcb-glx0",
    "libxcb-icccm4",
    "libxcb-image0",
    "libxcb-keysyms1",
    "libxcb-randr0",
    "libxcb-render0",
    "libxcb-render-util0",
    "libxcb-shape0",
    "libxcb-shm0",
    "libxcb-sync1",
    "libxcb-util1",
    "libxcb-xfixes0",
    "libxcb-xkb1",
    "libxext6",
    "libxkbcommon0",
    "libxkbcommon-x11-0",
    "libxrender1",
}
BUNDLED_XCB_XKB_SONAMES = {
    "libxcb-cursor.so.0",
    "libxcb-glx.so.0",
    "libxcb-icccm.so.4",
    "libxcb-image.so.0",
    "libxcb-keysyms.so.1",
    "libxcb-randr.so.0",
    "libxcb-render.so.0",
    "libxcb-render-util.so.0",
    "libxcb-shape.so.0",
    "libxcb-shm.so.0",
    "libxcb-sync.so.1",
    "libxcb-util.so.1",
    "libxcb-xfixes.so.0",
    "libxcb-xkb.so.1",
    "libxkbcommon.so.0",
    "libxkbcommon-x11.so.0",
}


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
package_smoke_script = (
    ROOT / "tools" / "desktop" / "package-smoke.sh"
).read_text(encoding="utf-8")
appimage_smoke_script = (
    ROOT / "tools" / "desktop" / "appimage-smoke.sh"
).read_text(encoding="utf-8")
for required_contract in (
    "--library \"$ffi_library\"",
    "--print-distribution-version",
    'output_path="$package_dir/Chaft-$distribution_version-Linux-$architecture.AppImage"',
    'qt_prefix="${QTDIR:-${QT_ROOT_DIR:-}}"',
    'qt_quick_library="$qt_library_dir/libQt6Quick.so.6"',
    'LD_LIBRARY_PATH="$qt_library_dir"',
    'QMAKE="$qt_qmake"',
    'qt_xcb_runtime_check="$script_dir/check-qt-xcb-runtime.sh"',
    '"$qt_xcb_runtime_check" "$qt_prefix"',
    "EXTRA_PLATFORM_PLUGINS=libqoffscreen.so",
    "QML_SOURCES_PATHS=",
):
    if required_contract not in packaging_script:
        fail(f"AppImage packager is missing required contract: {required_contract}")
for host_gl_pattern in (
    "libEGL.so*",
    "libGL.so*",
    "libGLdispatch.so*",
    "libGLX.so*",
    "libOpenGL.so*",
):
    if host_gl_pattern not in packaging_script:
        fail(
            "AppImage packager must reject bundled host GL dispatch library: "
            f"{host_gl_pattern}"
        )
for soname in BUNDLED_XCB_XKB_SONAMES:
    if soname not in packaging_script:
        fail(f"AppImage packager does not require bundled library: {soname}")
if packaging_script.index('"$qt_xcb_runtime_check" "$qt_prefix"') > (
    packaging_script.index('"$tool_dir/linuxdeploy"')
):
    fail("Qt XCB dependency preflight must run before linuxdeploy")
for required_contract in (
    "CHAFT_APPIMAGE_SMOKE_RUNTIME_DIR",
    "CHAFT_APPIMAGE_SMOKE_EXPECT_NO_WORKSPACE=0",
    "CHAFT_APPIMAGE_SMOKE_EXPECT_TEXT",
    "CHAFT_APPIMAGE_SMOKE_WORKSPACE_ID",
    '"$script_dir/appimage-smoke.sh"',
):
    if required_contract not in package_smoke_script:
        fail(
            "Linux package smoke must exercise the packaged AppImage with "
            f"the functional workspace contract: {required_contract}"
        )
if 'Darwin|Linux) ;;' not in package_smoke_script:
    fail("Linux package smoke must not launch the raw install tree")
for required_contract in (
    "CHAFT_APPIMAGE_SMOKE_RUNTIME_DIR",
    "CHAFT_APPIMAGE_SMOKE_EXPECT_NO_WORKSPACE",
    "CHAFT_APPIMAGE_SMOKE_EXPECT_TEXT",
    "CHAFT_APPIMAGE_SMOKE_WORKSPACE_ID",
    'expected_name="Chaft-$distribution_version-Linux-x86_64.AppImage"',
):
    if required_contract not in appimage_smoke_script:
        fail(
            "AppImage smoke does not expose the package-smoke override: "
            f"{required_contract}"
        )

xcb_runtime_check_path = (
    ROOT / "tools" / "desktop" / "check-qt-xcb-runtime.sh"
)
with tempfile.TemporaryDirectory() as temporary_directory:
    temporary = Path(temporary_directory)
    qt_prefix = temporary / "qt"
    platform_directory = qt_prefix / "plugins" / "platforms"
    integration_directory = qt_prefix / "plugins" / "xcbglintegrations"
    fake_bin = temporary / "bin"
    for directory in (
        qt_prefix / "lib",
        platform_directory,
        integration_directory,
        fake_bin,
    ):
        directory.mkdir(parents=True, exist_ok=True)
    shared_objects = (
        platform_directory / "libqxcb.so",
        integration_directory / "libqxcb-egl-integration.so",
        integration_directory / "libqxcb-glx-integration.so",
    )
    for shared_object in shared_objects:
        shared_object.touch()

    call_log = temporary / "ldd-calls"
    fake_ldd = fake_bin / "ldd"
    fake_ldd.write_text(
        """#!/usr/bin/env sh
set -eu
printf '%s|%s\\n' "$1" "${LD_LIBRARY_PATH:-}" >> "$LDD_CALL_LOG"
case "$(basename "$1")" in
  libqxcb.so)
    printf '%s\\n' \
      'libxcb-cursor.so.0 => not found' \
      'libxcb-icccm.so.4 => not found'
    ;;
  libqxcb-glx-integration.so)
    printf '%s\\n' 'libxcb-glx.so.0 => not found'
    ;;
  *)
    printf '%s\\n' 'libxcb.so.1 => /usr/lib/libxcb.so.1 (0x1)'
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_ldd.chmod(0o755)
    environment = os.environ.copy()
    environment["PATH"] = f"{fake_bin}{os.pathsep}{environment['PATH']}"
    environment["LDD_CALL_LOG"] = str(call_log)
    environment["LD_LIBRARY_PATH"] = "/host/runtime"
    completed = subprocess.run(
        [str(xcb_runtime_check_path), str(qt_prefix)],
        cwd=ROOT,
        env=environment,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 1:
        fail("Qt XCB preflight must reject unresolved dependencies")
    for soname in (
        "libxcb-cursor.so.0",
        "libxcb-icccm.so.4",
        "libxcb-glx.so.0",
    ):
        if soname not in completed.stderr:
            fail(f"Qt XCB preflight did not report unresolved {soname}")
    expected_library_path = f"{qt_prefix / 'lib'}:/host/runtime"
    if call_log.read_text(encoding="utf-8").splitlines() != [
        f"{shared_object}|{expected_library_path}"
        for shared_object in shared_objects
    ]:
        fail(
            "Qt XCB preflight must inspect every plugin against the exact "
            "restored Qt library directory"
        )

    call_log.unlink()
    fake_ldd.write_text(
        """#!/usr/bin/env sh
set -eu
printf '%s|%s\\n' "$1" "${LD_LIBRARY_PATH:-}" >> "$LDD_CALL_LOG"
printf '%s\\n' 'libxcb.so.1 => /usr/lib/libxcb.so.1 (0x1)'
""",
        encoding="utf-8",
    )
    completed = subprocess.run(
        [str(xcb_runtime_check_path), str(qt_prefix)],
        cwd=ROOT,
        env=environment,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        fail(f"Qt XCB preflight rejected a complete runtime: {completed.stderr}")
    if call_log.read_text(encoding="utf-8").splitlines() != [
        f"{shared_object}|{expected_library_path}"
        for shared_object in shared_objects
    ]:
        fail("successful Qt XCB preflight did not inspect every plugin")

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
for required_package_name in (
    "Chaft-${CHAFT_DISTRIBUTION_VERSION}-macOS-x86_64",
    "Chaft-${CHAFT_DISTRIBUTION_VERSION}-Windows-x86_64",
):
    if required_package_name not in cmake:
        fail(f"CMake omits distribution package name: {required_package_name}")
if 'CHAFT_DESKTOP_VERSION="${PROJECT_VERSION}"' not in cmake:
    fail("native embedded desktop version must remain the stable source version")

macos_codesign_option = "DEPLOY_TOOL_OPTIONS -codesign=-"
macos_deploy_rule = "install(SCRIPT ${chaft_desktop_deploy_script})"
macos_verify_rule = (
    '"${CMAKE_SOURCE_DIR}/tools/desktop/macos-adhoc-verify.cmake"'
)
if macos_codesign_option not in cmake:
    fail("macdeployqt must explicitly ad-hoc sign the deployed macOS app")
if macos_deploy_rule not in cmake or macos_verify_rule not in cmake:
    fail("CMake must verify the final deployed macOS app signature")
if cmake.index(macos_verify_rule) < cmake.index(macos_deploy_rule):
    fail("macOS signature verification must run after the Qt deployment install script")

macos_sign_script = (
    ROOT / "tools" / "desktop" / "macos-adhoc-verify.cmake"
).read_text(encoding="utf-8")
for required_contract in (
    '"$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/ChaftDesktop.app"',
    '"/usr/bin/codesign"',
    'COMMAND "/bin/test" -x "${CHAFT_CODESIGN_EXECUTABLE}"',
    "--deep",
    "--verify",
    "--strict",
):
    if required_contract not in macos_sign_script:
        fail(
            "final macOS ad-hoc signing is missing required contract: "
            f"{required_contract}"
        )
if "CHAFT_DESKTOP_SKIP_CODESIGN" in macos_sign_script:
    fail("final macOS package signature verification must not be bypassable")
for forbidden_contract in ("--force", "--sign"):
    if forbidden_contract in macos_sign_script:
        fail(
            "final macOS verification hook must not mutate signatures: "
            f"{forbidden_contract}"
        )

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

windows_smoke = (
    ROOT / "tools" / "desktop" / "windows-zip-smoke.ps1"
).read_text(encoding="utf-8")
for required_contract in (
    "ExpectedDistributionVersion",
    '"Chaft-$ExpectedDistributionVersion-Windows-x86_64.zip"',
    "ProductVersion.StartsWith($ExpectedSourceVersion)",
    '[Alias("ExpectedVersion")]',
):
    if required_contract not in windows_smoke:
        fail(f"Windows package smoke is missing version contract: {required_contract}")

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
packaging_dependencies_path = (
    ROOT / "tools" / "desktop" / "install-linux-package-dependencies.sh"
)
for profile in ("desktop-package", "release-package"):
    packaging_dependencies = subprocess.run(
        [str(packaging_dependencies_path), "list", profile],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.splitlines()
    base_dependencies = subprocess.run(
        [str(linux_dependencies_path), "list", profile],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.splitlines()
    if len(packaging_dependencies) != len(set(packaging_dependencies)):
        fail(f"{profile} packaging dependency list contains duplicates")
    expected_dependencies = set(base_dependencies) | QT_XCB_RUNTIME_PACKAGES
    if set(packaging_dependencies) != expected_dependencies:
        fail(f"{profile} does not install the exact Qt XCB runtime closure")
runtime_package_set = set(appimage_runtime_packages.split())
if not QT_XCB_RUNTIME_PACKAGES.isdisjoint(runtime_package_set):
    fail("clean AppImage smoke must consume the bundled Qt XCB runtime")
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
