#!/usr/bin/env python3
"""Contract tests for the deterministic Qt 6.8.4 source SDK."""

from __future__ import annotations

import copy
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import types
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("build_qt.py")
MANIFEST = Path(__file__).with_name("qt-6.8.4.json")
PROBE = Path(__file__).with_name("probe")
ROOT = SCRIPT.parents[2]


def load_script() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location("build_qt", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


qt = load_script()


def synthetic_toolchain(platform_name: str, image_version: str = "20260726.1"):
    return {
        "schemaVersion": 1,
        "platform": platform_name,
        "runner": {
            "os": {
                "linux": "Linux",
                "macos": "macOS",
                "windows": "Windows",
            }[platform_name],
            "architecture": "X64",
            "imageOS": f"synthetic-{platform_name}",
            "imageVersion": image_version,
        },
        "tools": {
            "cmake": "cmake version 4.1.0",
            "ninja": "1.13.1",
            "compiler": f"synthetic {platform_name} compiler 1.0",
            "python": "3.13.3",
        },
    }


class ManifestContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest = qt.load_manifest(MANIFEST)

    def test_exact_offline_identities_are_checked_in(self) -> None:
        self.assertEqual(
            self.manifest["platforms"]["windows"]["runner"],
            "windows-2022",
        )
        self.assertEqual(
            self.manifest["sdkIdentities"],
            {
                "linux": (
                    "qt-6.8.4-r1-linux-x86_64-gcc-11-"
                    "ac9f86b615071195d1a8"
                ),
                "macos": (
                    "qt-6.8.4-r1-macos-x86_64-apple-clang-"
                    "ac9f86b615071195d1a8"
                ),
                "windows": (
                    "qt-6.8.4-r1-windows-x86_64-msvc-2022-"
                    "ac9f86b615071195d1a8"
                ),
            },
        )
        for platform_name, expected in self.manifest["sdkIdentities"].items():
            with self.subTest(platform=platform_name):
                self.assertEqual(qt.sdk_identity(self.manifest, platform_name), expected)

    def test_manifest_edit_invalidates_checked_in_identity(self) -> None:
        changed = copy.deepcopy(self.manifest)
        changed["modules"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(qt.QtSdkError, "identities are stale"):
            qt.validate_manifest(changed)

    def test_manifest_rejects_coerced_integer_and_boolean_types(self) -> None:
        mutations = (
            (
                lambda value: value.__setitem__("schemaVersion", True),
                "schemaVersion",
            ),
            (
                lambda value: value.__setitem__("sdkRevision", 1.0),
                "sdkRevision",
            ),
            (
                lambda value: value["build"].__setitem__("parallel", 4.0),
                "build configuration",
            ),
            (
                lambda value: value["build"].__setitem__("shared", 1),
                "build configuration",
            ),
            (
                lambda value: value["modules"][0].__setitem__("order", True),
                "positive integer order",
            ),
        )
        for mutate, message in mutations:
            with self.subTest(message=message):
                changed = copy.deepcopy(self.manifest)
                mutate(changed)
                with self.assertRaisesRegex(qt.QtSdkError, message):
                    qt.validate_manifest(changed)

    def test_identity_covers_build_driver_and_verification_probes(self) -> None:
        materials = qt.recipe_materials()
        self.assertEqual(
            [row["path"] for row in materials],
            [
                "tools/qt/build_qt.py",
                "tools/qt/probe/CMakeLists.txt",
                "tools/qt/probe/main.cpp",
                "tools/qt/probe/tst_QtSdk.qml",
            ],
        )
        for row in materials:
            self.assertRegex(row["sha256"], r"^[0-9a-f]{64}$")
        changed = copy.deepcopy(self.manifest)
        with mock.patch.object(
            qt,
            "recipe_materials",
            return_value=materials
            + [{"path": "tools/qt/probe/new-contract", "sha256": "0" * 64}],
        ):
            with self.assertRaisesRegex(qt.QtSdkError, "identities are stale"):
                qt.validate_manifest(changed)

    def test_module_order_and_platform_selection_are_minimal(self) -> None:
        expected_common = [
            "qtbase",
            "qtshadertools",
            "qtsvg",
            "qtdeclarative",
        ]
        self.assertEqual(
            [row["name"] for row in qt.selected_modules(self.manifest, "macos")],
            expected_common,
        )
        self.assertEqual(
            [row["name"] for row in qt.selected_modules(self.manifest, "windows")],
            expected_common,
        )
        self.assertEqual(
            [row["name"] for row in qt.selected_modules(self.manifest, "linux")],
            expected_common + ["qtwayland"],
        )

    def test_source_archives_are_exact_official_materials(self) -> None:
        self.assertEqual(
            {row["name"]: row["sha256"] for row in self.manifest["modules"]},
            {
                "qtbase": (
                    "532dfbf3fa3cbc68fa37441ea9e81c5009da044eaecda78ffaeafd8bd125532f"
                ),
                "qtshadertools": (
                    "379a70692b52903b82897869112c4759f7f6f4e76abc4987700e9cdd87c87ebd"
                ),
                "qtsvg": (
                    "e2a83b315b97eeaffa1d3f17e8192436541fe53e62171e82c7311c56ee9aac07"
                ),
                "qtdeclarative": (
                    "e4e68aad4c07fbb9da670ecd621fabe51b56a2bfcce8da775653bdf8dcd768fc"
                ),
                "qtwayland": (
                    "b2475cfae9f5b8f40ea456b762cc634e7d083873aa554eb63954ee20e8a7bb8b"
                ),
            },
        )
        for row in self.manifest["modules"]:
            with self.subTest(module=row["name"]):
                self.assertTrue(
                    row["url"].startswith(
                        "https://download.qt.io/official_releases/qt/"
                        "6.8/6.8.4/submodules/"
                    )
                )

    def test_all_six_official_security_patches_have_fixed_order(self) -> None:
        self.assertEqual(
            [(row["order"], row["name"], row["sha256"]) for row in self.manifest["patches"]],
            [
                (
                    10,
                    "CVE-2025-10728-qtsvg-6.8.diff",
                    "cfb2399cdf094b378c40cfa3faebd2e2b69de1b910b63286986e081cda474a99",
                ),
                (
                    20,
                    "CVE-2025-10729-qtsvg-6.8.diff",
                    "7fe9c1a9e20e8919bca677f4cdd791cc0179a31669e8b5f5793dabddb094c727",
                ),
                (
                    30,
                    "CVE-2025-12385-qtdeclarative-6.8-0001.diff",
                    "c0fcc2971682c303b554c37617af337ed2ca36be9d6f5aa3702d4a2cf561f173",
                ),
                (
                    40,
                    "CVE-2025-12385-qtdeclarative-6.8-0002.diff",
                    "9bfe5e7f02d5d1bcc11355df01d6bef9016e434c5df3a371f88113e94047c7fd",
                ),
                (
                    50,
                    "CVE-2025-14576-qtdeclarative-6.8.diff",
                    "554c6381035326ea6dc315555d8d1591c707929ff3eabcbe6af747d10ea95458",
                ),
                (
                    60,
                    "CVE-2026-6210-qtsvg-6.8.diff",
                    "a1edf7b97643432042446538bf8438ae3d5c1d7cf54d28528c9a8503c3cb9d08",
                ),
            ],
        )

    def test_configure_contract_is_shared_release_without_extra_products(self) -> None:
        command = qt.cmake_configure_command(
            self.manifest,
            "linux",
            Path("/source/qtbase"),
            Path("/build/qtbase"),
            Path("/install/qt"),
        )
        for expected in (
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DBUILD_SHARED_LIBS=ON",
            "-DQT_BUILD_EXAMPLES=OFF",
            "-DQT_BUILD_TESTS=OFF",
            "-DQT_BUILD_BENCHMARKS=OFF",
            "-DQT_BUILD_DOCS=OFF",
            "-DCMAKE_C_COMPILER=gcc",
            "-DCMAKE_CXX_COMPILER=g++",
            "-DFEATURE_xcb=ON",
            "-DFEATURE_opengl=ON",
            "-DFEATURE_egl=ON",
        ):
            self.assertIn(expected, command)
        self.assertEqual(self.manifest["build"]["parallel"], 4)
        module_command = qt.qt_configure_module_command(
            self.manifest,
            "linux",
            Path("/source/qtwayland"),
            Path("/install/qt"),
        )
        self.assertEqual(
            module_command[0], "/install/qt/bin/qt-configure-module"
        )
        self.assertEqual(module_command[1:3], ["/source/qtwayland", "--"])
        self.assertIn("-DFEATURE_wayland_client=ON", module_command)
        self.assertIn("-DFEATURE_wayland_egl=ON", module_command)
        self.assertIn(
            "-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0",
            self.manifest["platforms"]["macos"]["cmakeArguments"],
        )
        self.assertEqual(
            self.manifest["platforms"]["linux"]["requiredPlatformPlugins"],
            ["libqoffscreen.so", "libqwayland-egl.so", "libqxcb.so"],
        )

    def test_cli_identity_stdout_is_stable_and_quiet(self) -> None:
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "identity", "--platform", "windows"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(
            result.stdout,
            "qt-6.8.4-r1-windows-x86_64-msvc-2022-"
            "ac9f86b615071195d1a8\n",
        )
        self.assertEqual(result.stderr, "")

    def test_toolchain_fingerprint_invalidates_dynamic_cache_identity(self) -> None:
        linux = synthetic_toolchain("linux")
        updated_image = synthetic_toolchain("linux", "20260727.1")
        first = qt.toolchain_fingerprint(linux, "linux")
        second = qt.toolchain_fingerprint(updated_image, "linux")
        self.assertNotEqual(first, second)
        self.assertNotEqual(
            qt.sdk_identity(self.manifest, "linux", first),
            qt.sdk_identity(self.manifest, "linux", second),
        )
        self.assertTrue(
            qt.sdk_identity(self.manifest, "linux", first).endswith(
                f"-tc-{first[:20]}"
            )
        )

    def test_toolchain_contract_rejects_platform_and_multiline_versions(self) -> None:
        contract = synthetic_toolchain("macos")
        contract["schemaVersion"] = True
        with self.assertRaisesRegex(qt.QtSdkError, "schemaVersion"):
            qt.toolchain_fingerprint(contract, "macos")
        contract = synthetic_toolchain("macos")
        contract["platform"] = "windows"
        with self.assertRaisesRegex(qt.QtSdkError, "platform mismatch"):
            qt.toolchain_fingerprint(contract, "macos")
        contract = synthetic_toolchain("macos")
        contract["tools"]["compiler"] = "line one\nline two"
        with self.assertRaisesRegex(qt.QtSdkError, "one non-empty line"):
            qt.toolchain_fingerprint(contract, "macos")


class MaterialSafetyTests(unittest.TestCase):
    def test_digest_is_checked_before_archive_extraction(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            unverified = work / "unverified.tar.xz"
            unverified.write_bytes(b"not the official archive")
            with (
                mock.patch.object(qt, "download_verified", return_value=unverified),
                mock.patch.object(qt, "extract_archive") as extract,
            ):
                with self.assertRaisesRegex(qt.QtSdkError, "SHA-256 mismatch"):
                    qt.prepare_sources(manifest, "macos", work / "work")
                extract.assert_not_called()

    def test_archive_traversal_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "unsafe.tar.xz"
            with tarfile.open(archive, "w:xz") as handle:
                entry = tarfile.TarInfo("../outside")
                payload = b"unsafe"
                entry.size = len(payload)
                handle.addfile(entry, io.BytesIO(payload))
            with self.assertRaisesRegex(qt.QtSdkError, "unsafe path"):
                qt.extract_archive(archive, root / "extract")
            self.assertFalse((root / "outside").exists())

    def test_patch_commands_follow_manifest_order(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def fake_download(row, _download_dir):
                path = root / row.get("name", Path(row["url"]).name)
                path.touch(exist_ok=True)
                return path

            def fake_extract(_archive, destination):
                destination.mkdir(parents=True)
                source_root = destination / "source"
                source_root.mkdir()
                return source_root

            with (
                mock.patch.object(qt, "download_verified", side_effect=fake_download),
                mock.patch.object(qt, "verify_digest"),
                mock.patch.object(qt, "extract_archive", side_effect=fake_extract),
                mock.patch.object(qt, "run") as run,
            ):
                qt.prepare_sources(manifest, "macos", root / "work")

            patch_commands = [
                call.args[0]
                for call in run.call_args_list
                if call.args[0][:2] == ["git", "apply"]
            ]
            expected_names = [
                row["name"]
                for row in manifest["patches"]
                for _ in range(2)
            ]
            self.assertEqual(
                [Path(command[-1]).name for command in patch_commands],
                expected_names,
            )
            for check, apply in zip(
                patch_commands[0::2], patch_commands[1::2], strict=True
            ):
                self.assertIn("--check", check)
                self.assertIn("--whitespace=nowarn", apply)


class VerificationContractTests(unittest.TestCase):
    def test_desktop_build_and_preflight_require_exact_qt_6_8_4(self) -> None:
        cmake = (ROOT / "apps" / "desktop-qt" / "CMakeLists.txt").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "find_package(Qt6 6.8.4 EXACT REQUIRED COMPONENTS "
            "Network Qml Quick Widgets)",
            cmake,
        )
        self.assertIn("qt_standard_project_setup(REQUIRES 6.8.4)", cmake)

        preflight = (
            ROOT / "tools" / "desktop" / "preflight.sh"
        ).read_text(encoding="utf-8")
        self.assertIn('[ "$qt_version" != "6.8.4" ]', preflight)
        self.assertIn("requires exactly Qt 6.8.4", preflight)
        self.assertNotIn("Qt 6.8+", preflight)

    def test_probe_requires_exact_quick_qml_and_desktop_components(self) -> None:
        cmake = (PROBE / "CMakeLists.txt").read_text(encoding="utf-8")
        self.assertIn(
            "find_package(Qt6 6.8.4 EXACT REQUIRED COMPONENTS "
            "Network Qml Quick Widgets)",
            cmake,
        )
        qml = (PROBE / "tst_QtSdk.qml").read_text(encoding="utf-8")
        self.assertIn("import QtQuick", qml)
        self.assertIn("import QtTest", qml)
        script = SCRIPT.read_text(encoding="utf-8")
        self.assertIn('"--qt-version"', script)
        self.assertIn("qmltestrunner", script)
        self.assertIn('"--parallel"', script)

    def test_provenance_identity_and_manifest_are_enforced(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        toolchain = synthetic_toolchain("linux")
        fingerprint = qt.toolchain_fingerprint(toolchain, "linux")
        expected = {
            "schemaVersion": 1,
            "identity": qt.sdk_identity(manifest, "linux", fingerprint),
            "manifestSha256": qt.manifest_digest(manifest),
            "contractSha256": qt.contract_digest(manifest),
            "qtVersion": "6.8.4",
            "sdkRevision": 1,
            "platform": "linux",
            "platformSpecification": manifest["platforms"]["linux"],
            "buildConfiguration": manifest["build"],
            "generatedAt": "2026-07-26T00:00:00Z",
            "host": {
                "system": "Linux",
                "release": "synthetic",
                "machine": "x86_64",
            },
            "toolchainContract": toolchain,
            "toolchainFingerprint": fingerprint,
            "sourceMaterials": qt.expected_source_materials(manifest, "linux"),
            "recipeMaterials": qt.recipe_materials(),
            "commands": [],
            "verification": {
                "completed": True,
                "completedAt": "2026-07-26T00:00:00Z",
            },
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "provenance.json"
            path.write_text(json.dumps(expected), encoding="utf-8")
            self.assertEqual(
                qt.load_and_validate_provenance(path, manifest, "linux"),
                expected,
            )
            changed = copy.deepcopy(expected)
            changed["schemaVersion"] = True
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "schemaVersion"):
                qt.load_and_validate_provenance(
                    path, manifest, "linux"
                )
            changed = copy.deepcopy(expected)
            changed["sdkRevision"] = True
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "sdkRevision mismatch"):
                qt.load_and_validate_provenance(
                    path, manifest, "linux"
                )
            changed = copy.deepcopy(expected)
            changed["buildConfiguration"]["shared"] = 1
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(
                qt.QtSdkError, "buildConfiguration mismatch"
            ):
                qt.load_and_validate_provenance(
                    path, manifest, "linux"
                )
            changed = copy.deepcopy(expected)
            changed["verification"]["completed"] = 1
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(
                qt.QtSdkError, "completed verification"
            ):
                qt.load_and_validate_provenance(
                    path, manifest, "linux"
                )
            expected["manifestSha256"] = "0" * 64
            path.write_text(json.dumps(expected), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "manifestSha256 mismatch"):
                qt.load_and_validate_provenance(path, manifest, "linux")

    def test_restore_rejects_incomplete_or_wrong_source_provenance(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        toolchain = synthetic_toolchain("macos")
        fingerprint = qt.toolchain_fingerprint(toolchain, "macos")
        provenance = {
            "schemaVersion": 1,
            "identity": qt.sdk_identity(manifest, "macos", fingerprint),
            "manifestSha256": qt.manifest_digest(manifest),
            "contractSha256": qt.contract_digest(manifest),
            "qtVersion": "6.8.4",
            "sdkRevision": 1,
            "platform": "macos",
            "platformSpecification": manifest["platforms"]["macos"],
            "buildConfiguration": manifest["build"],
            "generatedAt": "2026-07-26T00:00:00Z",
            "host": {
                "system": "macOS",
                "release": "synthetic",
                "machine": "x86_64",
            },
            "toolchainContract": toolchain,
            "toolchainFingerprint": fingerprint,
            "sourceMaterials": qt.expected_source_materials(manifest, "macos"),
            "recipeMaterials": qt.recipe_materials(),
            "commands": [],
            "verification": {"completed": False, "completedAt": None},
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "provenance.json"
            path.write_text(json.dumps(provenance), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "completed verification"):
                qt.load_and_validate_provenance(path, manifest, "macos")
            qt.load_and_validate_provenance(
                path, manifest, "macos", allow_incomplete=True
            )
            provenance["verification"] = {
                "completed": True,
                "completedAt": "2026-07-26T00:00:00Z",
            }
            provenance["sourceMaterials"][0]["sha256"] = "0" * 64
            path.write_text(json.dumps(provenance), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "sourceMaterials mismatch"):
                qt.load_and_validate_provenance(path, manifest, "macos")

    def test_restore_rejects_a_different_runner_toolchain_fingerprint(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        toolchain = synthetic_toolchain("windows")
        fingerprint = qt.toolchain_fingerprint(toolchain, "windows")
        provenance = {
            "schemaVersion": 1,
            "identity": qt.sdk_identity(manifest, "windows", fingerprint),
            "manifestSha256": qt.manifest_digest(manifest),
            "contractSha256": qt.contract_digest(manifest),
            "qtVersion": "6.8.4",
            "sdkRevision": 1,
            "platform": "windows",
            "platformSpecification": manifest["platforms"]["windows"],
            "buildConfiguration": manifest["build"],
            "generatedAt": "2026-07-26T00:00:00Z",
            "host": {
                "system": "Windows",
                "release": "synthetic",
                "machine": "x86_64",
            },
            "toolchainContract": toolchain,
            "toolchainFingerprint": fingerprint,
            "sourceMaterials": qt.expected_source_materials(
                manifest, "windows"
            ),
            "recipeMaterials": qt.recipe_materials(),
            "commands": [],
            "verification": {
                "completed": True,
                "completedAt": "2026-07-26T00:00:00Z",
            },
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "provenance.json"
            path.write_text(json.dumps(provenance), encoding="utf-8")
            stale = qt.toolchain_fingerprint(
                synthetic_toolchain("windows", "20260727.1"),
                "windows",
            )
            with self.assertRaisesRegex(
                qt.QtSdkError, "toolchainFingerprint mismatch"
            ):
                qt.load_and_validate_provenance(
                    path,
                    manifest,
                    "windows",
                    expected_toolchain_fingerprint=stale,
                )

    def test_activation_writes_github_environment_and_path(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prefix = root / "qt"
            prefix.mkdir()
            github_env = root / "github-env"
            github_path = root / "github-path"
            resolved_prefix = prefix.resolve()
            with mock.patch.dict("os.environ", {}, clear=True):
                qt.activate_sdk(prefix, github_env, github_path)
            self.assertEqual(
                github_env.read_text(encoding="utf-8").splitlines(),
                [
                    f"QTDIR={resolved_prefix}",
                    f"QT_ROOT_DIR={resolved_prefix}",
                    f"CMAKE_PREFIX_PATH={resolved_prefix}",
                ],
            )
            self.assertEqual(
                github_path.read_text(encoding="utf-8"),
                f"{resolved_prefix / 'bin'}\n",
            )


if __name__ == "__main__":
    unittest.main()
