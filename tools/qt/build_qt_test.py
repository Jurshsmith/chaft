#!/usr/bin/env python3
"""Contract tests for the deterministic Qt 6.8.4 source SDK."""

from __future__ import annotations

import copy
import importlib.util
import io
import json
import os
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


def synthetic_toolchain(target_name: str, image_version: str = "20260726.1"):
    specification = qt.load_manifest(MANIFEST)["targets"][target_name]
    platform_name = specification["platform"]
    return {
        "schemaVersion": 2,
        "target": target_name,
        "platform": platform_name,
        "runner": {
            "os": {
                "linux": "Linux",
                "macos": "macOS",
                "windows": "Windows",
            }[platform_name],
            "architecture": specification["architecture"],
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
            self.manifest["targets"]["windows-x86_64"]["runner"],
            "windows-2022",
        )
        self.assertEqual(
            self.manifest["targets"]["macos-arm64"],
            {
                "platform": "macos",
                "runner": "macos-15",
                "architecture": "arm64",
                "toolchain": "apple-clang",
                "cmakeArguments": [
                    "-DCMAKE_C_COMPILER=clang",
                    "-DCMAKE_CXX_COMPILER=clang++",
                    "-DCMAKE_OSX_ARCHITECTURES=arm64",
                    "-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0",
                ],
                "moduleCmakeArguments": {},
                "requiredPlatformPlugins": [
                    "libqcocoa.dylib",
                    "libqoffscreen.dylib",
                ],
            },
        )
        self.assertEqual(
            self.manifest["sdkIdentities"],
            {
                "linux-x86_64": (
                    "qt-6.8.4-r2-linux-x86_64-gcc-11-"
                    "3654d12c199fa2c2bd6a"
                ),
                "macos-arm64": (
                    "qt-6.8.4-r2-macos-arm64-apple-clang-"
                    "3654d12c199fa2c2bd6a"
                ),
                "macos-x86_64": (
                    "qt-6.8.4-r2-macos-x86_64-apple-clang-"
                    "3654d12c199fa2c2bd6a"
                ),
                "windows-x86_64": (
                    "qt-6.8.4-r2-windows-x86_64-msvc-2022-"
                    "3654d12c199fa2c2bd6a"
                ),
            },
        )
        for target_name, expected in self.manifest["sdkIdentities"].items():
            with self.subTest(target=target_name):
                self.assertEqual(
                    qt.sdk_identity(self.manifest, target_name), expected
                )
        self.assertEqual(
            qt.resolve_target(self.manifest, platform_name="linux"),
            "linux-x86_64",
        )
        with self.assertRaisesRegex(qt.QtSdkError, "ambiguous"):
            qt.resolve_target(self.manifest, platform_name="macos")

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
                "tools/qt/install-linux-dependencies.sh",
                "tools/qt/probe/CMakeLists.txt",
                "tools/qt/probe/main.cpp",
                "tools/qt/probe/tst_QtSdk.qml",
            ],
        )
        for row in materials:
            self.assertRegex(row["sha256"], r"^[0-9a-f]{64}$")
        changed_materials = copy.deepcopy(materials)
        dependency_profile = next(
            row
            for row in changed_materials
            if row["path"] == "tools/qt/install-linux-dependencies.sh"
        )
        dependency_profile["sha256"] = "0" * 64
        changed = copy.deepcopy(self.manifest)
        with mock.patch.object(
            qt,
            "recipe_materials",
            return_value=changed_materials,
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
            [
                row["name"]
                for row in qt.selected_modules(
                    self.manifest, "macos-arm64"
                )
            ],
            expected_common,
        )
        self.assertEqual(
            [
                row["name"]
                for row in qt.selected_modules(
                    self.manifest, "windows-x86_64"
                )
            ],
            expected_common,
        )
        self.assertEqual(
            [
                row["name"]
                for row in qt.selected_modules(
                    self.manifest, "linux-x86_64"
                )
            ],
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
            "linux-x86_64",
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
            "linux-x86_64",
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
            self.manifest["targets"]["macos-x86_64"]["cmakeArguments"],
        )
        self.assertEqual(
            self.manifest["targets"]["linux-x86_64"][
                "requiredPlatformPlugins"
            ],
            ["libqoffscreen.so", "libqwayland-egl.so", "libqxcb.so"],
        )

    def test_cli_identity_stdout_is_stable_and_quiet(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "identity",
                "--target",
                "windows-x86_64",
            ],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(
            result.stdout,
            "qt-6.8.4-r2-windows-x86_64-msvc-2022-"
            "3654d12c199fa2c2bd6a\n",
        )
        self.assertEqual(result.stderr, "")

    def test_toolchain_fingerprint_invalidates_dynamic_cache_identity(self) -> None:
        target = "linux-x86_64"
        linux = synthetic_toolchain(target)
        updated_image = synthetic_toolchain(target, "20260727.1")
        first = qt.toolchain_fingerprint(linux, self.manifest, target)
        second = qt.toolchain_fingerprint(
            updated_image, self.manifest, target
        )
        self.assertNotEqual(first, second)
        self.assertNotEqual(
            qt.sdk_identity(self.manifest, target, first),
            qt.sdk_identity(self.manifest, target, second),
        )
        self.assertTrue(
            qt.sdk_identity(self.manifest, target, first).endswith(
                f"-tc-{first[:20]}"
            )
        )

    def test_toolchain_contract_rejects_platform_and_multiline_versions(self) -> None:
        target = "macos-arm64"
        contract = synthetic_toolchain(target)
        contract["schemaVersion"] = True
        with self.assertRaisesRegex(qt.QtSdkError, "schemaVersion"):
            qt.toolchain_fingerprint(contract, self.manifest, target)
        contract = synthetic_toolchain(target)
        contract["platform"] = "windows"
        with self.assertRaisesRegex(qt.QtSdkError, "platform mismatch"):
            qt.toolchain_fingerprint(contract, self.manifest, target)
        contract = synthetic_toolchain(target)
        contract["runner"]["architecture"] = "X64"
        with self.assertRaisesRegex(qt.QtSdkError, "architecture mismatch"):
            qt.toolchain_fingerprint(contract, self.manifest, target)
        contract = synthetic_toolchain(target)
        contract["tools"]["compiler"] = "line one\nline two"
        with self.assertRaisesRegex(qt.QtSdkError, "one non-empty line"):
            qt.toolchain_fingerprint(contract, self.manifest, target)

    def test_architecture_aliases_are_normalized_and_rosetta_is_rejected(
        self,
    ) -> None:
        self.assertEqual(qt.normalize_architecture("aarch64"), "arm64")
        self.assertEqual(qt.normalize_architecture("ARM64"), "arm64")
        self.assertEqual(qt.normalize_architecture("AMD64"), "x86_64")

        with (
            mock.patch.object(
                qt, "normalized_host_platform", return_value="macos"
            ),
            mock.patch.object(qt, "normalized_machine", return_value="arm64"),
            mock.patch.object(
                qt, "macos_process_is_translated", return_value=True
            ),
        ):
            with self.assertRaisesRegex(qt.QtSdkError, "Rosetta"):
                qt.validate_build_host(
                    self.manifest, "macos-arm64", check_tools=False
                )
            with self.assertRaisesRegex(qt.QtSdkError, "native Intel host"):
                qt.validate_build_host(
                    self.manifest, "macos-x86_64", check_tools=False
                )

        with (
            mock.patch.object(
                qt, "normalized_host_platform", return_value="macos"
            ),
            mock.patch.object(qt, "normalized_machine", return_value="arm64"),
            mock.patch.object(
                qt, "macos_process_is_translated", return_value=False
            ),
        ):
            qt.validate_build_host(
                self.manifest, "macos-arm64", check_tools=False
            )
            with self.assertRaisesRegex(qt.QtSdkError, "requires x86_64"):
                qt.validate_build_host(
                    self.manifest, "macos-x86_64", check_tools=False
                )


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
                    qt.prepare_sources(
                        manifest, "macos-x86_64", work / "work"
                    )
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
                qt.prepare_sources(
                    manifest, "macos-x86_64", root / "work"
                )

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
    def test_desktop_release_policy_requires_exact_qt_6_8_4(self) -> None:
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
        self.assertIn('if policy == "release":', preflight)
        self.assertIn("if parsed != (6, 8, 4):", preflight)
        self.assertIn("official builds require exactly Qt 6.8.4", preflight)
        self.assertIn('elif not ((6, 11, 1) <= parsed < (6, 12, 0)):', preflight)
        self.assertIn('case "$policy" in', preflight)

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
        target = "linux-x86_64"
        specification = manifest["targets"][target]
        toolchain = synthetic_toolchain(target)
        fingerprint = qt.toolchain_fingerprint(
            toolchain, manifest, target
        )
        expected = {
            "schemaVersion": 2,
            "identity": qt.sdk_identity(manifest, target, fingerprint),
            "manifestSha256": qt.manifest_digest(manifest),
            "contractSha256": qt.contract_digest(manifest),
            "qtVersion": "6.8.4",
            "sdkRevision": 2,
            "target": target,
            "platform": "linux",
            "architecture": "x86_64",
            "targetSpecification": specification,
            "buildConfiguration": manifest["build"],
            "generatedAt": "2026-07-26T00:00:00Z",
            "host": {
                "system": "Linux",
                "release": "synthetic",
                "machine": "x86_64",
            },
            "toolchainContract": toolchain,
            "toolchainFingerprint": fingerprint,
            "sourceMaterials": qt.expected_source_materials(
                manifest, target
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
            path.write_text(json.dumps(expected), encoding="utf-8")
            self.assertEqual(
                qt.load_and_validate_provenance(path, manifest, target),
                expected,
            )
            changed = copy.deepcopy(expected)
            changed["target"] = "macos-arm64"
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "target mismatch"):
                qt.load_and_validate_provenance(path, manifest, target)
            changed = copy.deepcopy(expected)
            changed["schemaVersion"] = True
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "schemaVersion"):
                qt.load_and_validate_provenance(
                    path, manifest, target
                )
            changed = copy.deepcopy(expected)
            changed["sdkRevision"] = True
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "sdkRevision mismatch"):
                qt.load_and_validate_provenance(
                    path, manifest, target
                )
            changed = copy.deepcopy(expected)
            changed["buildConfiguration"]["shared"] = 1
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(
                qt.QtSdkError, "buildConfiguration mismatch"
            ):
                qt.load_and_validate_provenance(
                    path, manifest, target
                )
            changed = copy.deepcopy(expected)
            changed["verification"]["completed"] = 1
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(
                qt.QtSdkError, "completed verification"
            ):
                qt.load_and_validate_provenance(
                    path, manifest, target
                )
            expected["manifestSha256"] = "0" * 64
            path.write_text(json.dumps(expected), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "manifestSha256 mismatch"):
                qt.load_and_validate_provenance(path, manifest, target)

    def test_restore_rejects_incomplete_or_wrong_source_provenance(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        target = "macos-arm64"
        specification = manifest["targets"][target]
        toolchain = synthetic_toolchain(target)
        fingerprint = qt.toolchain_fingerprint(
            toolchain, manifest, target
        )
        provenance = {
            "schemaVersion": 2,
            "identity": qt.sdk_identity(manifest, target, fingerprint),
            "manifestSha256": qt.manifest_digest(manifest),
            "contractSha256": qt.contract_digest(manifest),
            "qtVersion": "6.8.4",
            "sdkRevision": 2,
            "target": target,
            "platform": "macos",
            "architecture": "arm64",
            "targetSpecification": specification,
            "buildConfiguration": manifest["build"],
            "generatedAt": "2026-07-26T00:00:00Z",
            "host": {
                "system": "macOS",
                "release": "synthetic",
                "machine": "arm64",
            },
            "toolchainContract": toolchain,
            "toolchainFingerprint": fingerprint,
            "sourceMaterials": qt.expected_source_materials(
                manifest, target
            ),
            "recipeMaterials": qt.recipe_materials(),
            "commands": [],
            "verification": {"completed": False, "completedAt": None},
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "provenance.json"
            path.write_text(json.dumps(provenance), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "completed verification"):
                qt.load_and_validate_provenance(path, manifest, target)
            qt.load_and_validate_provenance(
                path, manifest, target, allow_incomplete=True
            )
            provenance["verification"] = {
                "completed": True,
                "completedAt": "2026-07-26T00:00:00Z",
            }
            provenance["sourceMaterials"][0]["sha256"] = "0" * 64
            path.write_text(json.dumps(provenance), encoding="utf-8")
            with self.assertRaisesRegex(qt.QtSdkError, "sourceMaterials mismatch"):
                qt.load_and_validate_provenance(path, manifest, target)

    def test_restore_rejects_a_different_runner_toolchain_fingerprint(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        target = "windows-x86_64"
        specification = manifest["targets"][target]
        toolchain = synthetic_toolchain(target)
        fingerprint = qt.toolchain_fingerprint(
            toolchain, manifest, target
        )
        provenance = {
            "schemaVersion": 2,
            "identity": qt.sdk_identity(manifest, target, fingerprint),
            "manifestSha256": qt.manifest_digest(manifest),
            "contractSha256": qt.contract_digest(manifest),
            "qtVersion": "6.8.4",
            "sdkRevision": 2,
            "target": target,
            "platform": "windows",
            "architecture": "x86_64",
            "targetSpecification": specification,
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
                manifest, target
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
                synthetic_toolchain(target, "20260727.1"),
                manifest,
                target,
            )
            with self.assertRaisesRegex(
                qt.QtSdkError, "toolchainFingerprint mismatch"
            ):
                qt.load_and_validate_provenance(
                    path,
                    manifest,
                    target,
                    expected_toolchain_fingerprint=stale,
                )

    def test_activation_writes_github_environment_and_path(self) -> None:
        manifest = qt.load_manifest(MANIFEST)
        target = "macos-arm64"
        specification = manifest["targets"][target]
        toolchain = synthetic_toolchain(target)
        fingerprint = qt.toolchain_fingerprint(
            toolchain, manifest, target
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prefix = root / "qt"
            prefix.mkdir()
            provenance_path = prefix / qt.PROVENANCE_NAME
            provenance_path.write_text(
                json.dumps(
                    {
                        "schemaVersion": 2,
                        "identity": qt.sdk_identity(
                            manifest, target, fingerprint
                        ),
                        "manifestSha256": qt.manifest_digest(manifest),
                        "contractSha256": qt.contract_digest(manifest),
                        "qtVersion": manifest["qtVersion"],
                        "sdkRevision": manifest["sdkRevision"],
                        "target": target,
                        "platform": specification["platform"],
                        "architecture": specification["architecture"],
                        "targetSpecification": specification,
                        "buildConfiguration": manifest["build"],
                        "generatedAt": "2026-07-26T00:00:00Z",
                        "host": {
                            "system": "Darwin",
                            "release": "synthetic",
                            "machine": "arm64",
                        },
                        "toolchainFingerprint": fingerprint,
                        "toolchainContract": toolchain,
                        "sourceMaterials": qt.expected_source_materials(
                            manifest, target
                        ),
                        "recipeMaterials": qt.recipe_materials(),
                        "commands": [],
                        "verification": {
                            "completed": True,
                            "completedAt": "2026-07-26T00:00:00Z",
                        },
                    }
                ),
                encoding="utf-8",
            )
            github_env = root / "github-env"
            github_path = root / "github-path"
            resolved_prefix = prefix.resolve()
            with mock.patch.dict("os.environ", {}, clear=True):
                qt.activate_sdk(
                    manifest, prefix, github_env, github_path
                )
            self.assertEqual(
                github_env.read_text(encoding="utf-8").splitlines(),
                [
                    f"QTDIR={resolved_prefix}",
                    f"QT_ROOT_DIR={resolved_prefix}",
                    f"CMAKE_PREFIX_PATH={resolved_prefix}",
                    "CHAFT_QT_SDK_BUILD_TYPE=Release",
                    f"CHAFT_QT_SDK_TARGET={target}",
                    "CHAFT_QT_SDK_PLATFORM=macos",
                    "CHAFT_QT_SDK_ARCHITECTURE=arm64",
                    "CHAFT_QT_SDK_VERSION=6.8.4",
                    (
                        "CHAFT_QT_SDK_IDENTITY="
                        f"{qt.sdk_identity(manifest, target, fingerprint)}"
                    ),
                    (
                        "CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT="
                        f"{fingerprint}"
                    ),
                    (
                        "CHAFT_QT_SDK_PROVENANCE="
                        f"{resolved_prefix / qt.PROVENANCE_NAME}"
                    ),
                ],
            )
            self.assertEqual(
                github_path.read_text(encoding="utf-8"),
                f"{resolved_prefix / 'bin'}\n",
            )

    def test_release_only_windows_sdk_aligns_debug_consumer_runtime(self) -> None:
        common = ROOT / "tools" / "desktop" / "common.sh"

        def arguments(
            platform_name: str, profile: str, build_type: str | None
        ) -> list[str]:
            environment = os.environ.copy()
            if build_type is None:
                environment.pop("CHAFT_QT_SDK_BUILD_TYPE", None)
            else:
                environment["CHAFT_QT_SDK_BUILD_TYPE"] = build_type
            command = (
                f'. "{common}"; '
                f"uname() {{ printf '%s\\n' '{platform_name}'; }}; "
                "chaft_desktop_qt_compatibility_cmake_arguments "
                f"'{profile}'"
            )
            result = subprocess.run(
                ["sh", "-c", command],
                check=True,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=environment,
            )
            self.assertEqual(result.stderr, "")
            return result.stdout.splitlines()

        self.assertEqual(
            arguments("MINGW64_NT-10.0", "debug", "Release"),
            [
                "-DCHAFT_DEBUG_USES_RELEASE_QT=ON",
                "-DCMAKE_MAP_IMPORTED_CONFIG_DEBUG=Release",
                "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL",
            ],
        )
        self.assertEqual(arguments("Linux", "debug", "Release"), [])
        self.assertEqual(
            arguments("MINGW64_NT-10.0", "release", "Release"), []
        )
        self.assertEqual(arguments("MINGW64_NT-10.0", "debug", None), [])

    def test_explicit_qt_root_stays_ahead_of_ambient_homebrew_qt(self) -> None:
        common = ROOT / "tools" / "desktop" / "common.sh"
        with tempfile.TemporaryDirectory(prefix="chaft-qt-path-test-") as name:
            root = Path(name)
            explicit = root / "verified-qt"
            homebrew = root / "homebrew"
            (explicit / "bin").mkdir(parents=True)
            for formula in (
                "qtbase",
                "qtdeclarative",
                "qtshadertools",
                "qtsvg",
                "qt",
                "qt@6",
            ):
                (homebrew / "opt" / formula / "bin").mkdir(parents=True)
            command = (
                f'. "{common}"; '
                "brew() { "
                f'if [ "$1" = "--prefix" ] && [ "$#" -eq 1 ]; then '
                f'printf "%s\\n" "{homebrew}"; '
                f'else printf "%s\\n" "{homebrew}/opt/$2"; fi; '
                "}; "
                f'QT_ROOT_DIR="{explicit}"; '
                f'PATH="/usr/bin:/bin:{explicit}/bin"; '
                "export QT_ROOT_DIR PATH; "
                "chaft_desktop_add_tool_paths; "
                'printf "%s\\n" "${PATH%%:*}"'
            )
            result = subprocess.run(
                ["sh", "-c", command],
                check=True,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(result.stderr, "")
            self.assertEqual(
                result.stdout.strip(),
                str(explicit / "bin"),
            )


if __name__ == "__main__":
    unittest.main()
