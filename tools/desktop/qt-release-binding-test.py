#!/usr/bin/env python3
"""Focused tests for desktop-to-Qt release provenance binding."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).with_name("verify-release-metadata.py")
QT_TOOLS = ROOT / "tools" / "qt"
sys.path.insert(0, str(QT_TOOLS))
import build_qt as qt_sdk  # noqa: E402
import source_bundle as qt_source  # noqa: E402


def load_verifier() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location(
        "qt_release_metadata_verifier", SCRIPT
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


verifier = load_verifier()


def toolchain_contract(platform_name: str) -> dict[str, object]:
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
            "imageVersion": "20260726.1",
        },
        "tools": {
            "cmake": "cmake version 4.1.0",
            "ninja": "1.13.1",
            "compiler": f"synthetic {platform_name} compiler 1.0",
            "python": "3.13.3",
        },
    }


def qt_binding(
    platform_name: str,
    bundle_sha256: str,
    source_root: Path = ROOT,
) -> dict[str, object]:
    release_tools = source_root / "tools" / "qt"
    manifest_path = release_tools / "qt-6.8.4.json"
    manifest = qt_sdk.load_manifest(
        manifest_path, recipe_root=source_root
    )
    contract = toolchain_contract(platform_name)
    fingerprint = qt_sdk.toolchain_fingerprint(contract, platform_name)
    provenance = {
        "schemaVersion": 1,
        "identity": qt_sdk.sdk_identity(
            manifest,
            platform_name,
            fingerprint,
            recipe_root=source_root,
        ),
        "manifestSha256": qt_sdk.manifest_digest(
            manifest, recipe_root=source_root
        ),
        "contractSha256": qt_sdk.contract_digest(
            manifest, recipe_root=source_root
        ),
        "qtVersion": manifest["qtVersion"],
        "sdkRevision": manifest["sdkRevision"],
        "platform": platform_name,
        "platformSpecification": manifest["platforms"][platform_name],
        "buildConfiguration": manifest["build"],
        "generatedAt": "2026-07-26T00:00:00Z",
        "host": {
            "system": contract["runner"]["os"],
            "release": "synthetic",
            "machine": "x86_64",
        },
        "toolchainContract": contract,
        "toolchainFingerprint": fingerprint,
        "sourceMaterials": qt_sdk.expected_source_materials(
            manifest, platform_name
        ),
        "recipeMaterials": qt_sdk.recipe_materials(source_root),
        "commands": [],
        "verification": {
            "completed": True,
            "completedAt": "2026-07-26T00:00:00Z",
        },
    }
    source = qt_source.release_contract(
        manifest_path,
        source_root / "packaging" / "qt",
        release_tools / "source_bundle.py",
        recipe_root=source_root,
    )
    return {
        "schemaVersion": 1,
        "sdk": {
            "identity": provenance["identity"],
            "provenanceSha256": qt_sdk.sha256_bytes(
                qt_sdk.canonical_json(provenance)
            ),
            "provenance": provenance,
        },
        "correspondingSource": {
            **source,
            "bundleSha256": bundle_sha256,
        },
    }


class QtReleaseBindingTests(unittest.TestCase):
    def test_all_platforms_bind_to_the_authenticated_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = root / qt_source.BUNDLE_NAME
            checksum = root / qt_source.CHECKSUM_NAME
            bundle.write_bytes(b"authenticated corresponding source")
            checksum.write_text(
                f"{verifier.file_sha256(bundle)}  {bundle.name}\n",
                encoding="ascii",
            )
            digest = verifier.file_sha256(bundle)

            for platform_name in ("linux", "macos", "windows"):
                with self.subTest(platform=platform_name):
                    provenance = {
                        "qt": qt_binding(platform_name, digest)
                    }
                    with mock.patch.object(
                        verifier.qt_source, "verify_bundle"
                    ) as verify_bundle:
                        verifier.verify_qt_release_binding(
                            provenance,
                            ROOT,
                            platform_name,
                            bundle,
                            checksum,
                        )
                    verify_bundle.assert_called_once()

    def test_bundle_and_sdk_tampering_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bundle = Path(temporary) / qt_source.BUNDLE_NAME
            bundle.write_bytes(b"authenticated corresponding source")
            digest = verifier.file_sha256(bundle)
            provenance = {"qt": qt_binding("linux", digest)}

            provenance["qt"]["correspondingSource"]["bundleSha256"] = "0" * 64
            with (
                mock.patch.object(verifier.qt_source, "verify_bundle"),
                self.assertRaisesRegex(
                    SystemExit, "authenticated release bundle"
                ),
            ):
                verifier.verify_qt_release_binding(
                    provenance, ROOT, "linux", bundle
                )

            provenance = {"qt": qt_binding("linux", digest)}
            provenance["qt"]["sdk"]["provenance"][
                "toolchainFingerprint"
            ] = "0" * 64
            with self.assertRaisesRegex(
                qt_sdk.QtSdkError, "canonical toolchain contract"
            ):
                verifier.verify_qt_release_binding(
                    provenance, ROOT, "linux"
                )

    def test_qt_binding_rejects_coerced_schema_numbers(self) -> None:
        provenance = {"qt": qt_binding("linux", "1" * 64)}
        provenance["qt"]["schemaVersion"] = True
        with self.assertRaisesRegex(SystemExit, "schemaVersion"):
            verifier.verify_qt_release_binding(
                provenance, ROOT, "linux"
            )

        provenance = {"qt": qt_binding("linux", "1" * 64)}
        provenance["qt"]["correspondingSource"]["schemaVersion"] = 1.0
        with self.assertRaisesRegex(
            SystemExit, "contract differs"
        ):
            verifier.verify_qt_release_binding(
                provenance, ROOT, "linux"
            )

    def test_release_source_materials_reject_absolute_symlinks(self) -> None:
        for escaped in verifier.SOURCE_MATERIALS:
            with self.subTest(path=escaped):
                with tempfile.TemporaryDirectory() as temporary:
                    release_root = Path(temporary) / "release"
                    for relative in verifier.SOURCE_MATERIALS:
                        destination = release_root / relative
                        destination.parent.mkdir(
                            parents=True, exist_ok=True
                        )
                        shutil.copyfile(ROOT / relative, destination)
                    destination = release_root / escaped
                    destination.unlink()
                    destination.symlink_to((ROOT / escaped).resolve())
                    with self.assertRaisesRegex(
                        SystemExit, "non-symlink regular file"
                    ):
                        verifier.source_material_rows(release_root)

    def test_detached_release_root_owns_recipe_expectations(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            release_root = Path(temporary) / "release"
            shutil.copytree(QT_TOOLS, release_root / "tools" / "qt")
            shutil.copytree(
                ROOT / "packaging" / "qt",
                release_root / "packaging" / "qt",
            )
            release_driver = release_root / "tools" / "qt" / "build_qt.py"
            release_driver.write_text(
                release_driver.read_text(encoding="utf-8")
                + "\nraise RuntimeError('tag recipe must never execute')\n",
                encoding="utf-8",
            )

            manifest_path = (
                release_root / "tools" / "qt" / "qt-6.8.4.json"
            )
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            digest = qt_sdk.unchecked_contract_digest(
                manifest, recipe_root=release_root
            )[:20]
            manifest["sdkIdentities"] = {
                platform_name: (
                    f"qt-{manifest['qtVersion']}-"
                    f"r{manifest['sdkRevision']}-{platform_name}-"
                    f"{specification['architecture']}-"
                    f"{specification['toolchain']}-{digest}"
                )
                for platform_name, specification in manifest[
                    "platforms"
                ].items()
            }
            manifest_path.write_text(
                json.dumps(manifest, indent=2) + "\n",
                encoding="utf-8",
            )

            binding = qt_binding("linux", "1" * 64, release_root)
            provenance = {"qt": binding}
            verifier.verify_qt_release_binding(
                provenance, release_root, "linux"
            )
            self.assertNotEqual(
                binding["sdk"]["provenance"]["contractSha256"],
                qt_sdk.contract_digest(qt_sdk.load_manifest()),
            )
            with self.assertRaises(
                (qt_sdk.QtSdkError, SystemExit)
            ):
                verifier.verify_qt_release_binding(
                    provenance, ROOT, "linux"
                )


if __name__ == "__main__":
    unittest.main()
