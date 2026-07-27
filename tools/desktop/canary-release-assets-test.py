#!/usr/bin/env python3
"""Focused tests for Chaft's exact 14/19 unsigned-canary asset layout."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
SCRIPT = TOOLS / "canary-release-assets.py"
SPEC = importlib.util.spec_from_file_location("canary_release_assets", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
assets_tool = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = assets_tool
SPEC.loader.exec_module(assets_tool)
import unsigned_canary_policy as policy  # noqa: E402


VERSION = "0.1.0-canary.1"
TAG = f"v{VERSION}"
COMMIT = "a" * 40
REPOSITORY = "Jurshsmith/chaft"
RELEASE_ID = 91
PACKAGE_NAMES = {
    "windows": f"Chaft-{VERSION}-Windows-x86_64.zip",
    "macos": f"Chaft-{VERSION}-macOS-x86_64.dmg",
    "linux": f"Chaft-{VERSION}-Linux-x86_64.AppImage",
}
PACKAGE_FORMATS = {
    "windows": "windows-zip",
    "macos": "macos-dmg",
    "linux": "linux-appimage",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def write_platform(assets: Path, platform: str, qt_sha256: str) -> None:
    package = assets / PACKAGE_NAMES[platform]
    package.write_bytes(f"{platform} package".encode())
    row = {
        "name": package.name,
        "packageFormat": PACKAGE_FORMATS[platform],
        "sizeBytes": package.stat().st_size,
        "sha256": sha256(package),
    }
    names = assets_tool.stager.METADATA_FILENAMES[platform]
    (assets / names["checksums"]).write_text(
        f"{row['sha256']}  {row['name']}\n", encoding="utf-8"
    )
    write_json(
        assets / names["sbom"],
        {
            "bomFormat": "CycloneDX",
            "metadata": {
                "properties": [
                    {"name": "chaft:packagePlatform", "value": platform}
                ]
            },
        },
    )
    write_json(
        assets / names["provenance"],
        {
            "schemaVersion": "chaft.desktop.provenance.v1",
            "profile": "release",
            "packagePlatform": platform,
            "version": VERSION,
            "source": {
                "commit": COMMIT,
                "repository": f"git@github.com:{REPOSITORY}.git",
                "dirty": False,
            },
            "qt": {
                "correspondingSource": {
                    "bundle": assets_tool.QT_SOURCE_BUNDLE,
                    "checksum": assets_tool.QT_SOURCE_CHECKSUM,
                    "bundleSha256": qt_sha256,
                }
            },
            "artifacts": [row],
        },
    )


def add_receipts(assets: Path) -> None:
    generator_script = TOOLS / "generate-unsigned-canary-receipt.py"
    generator_spec = importlib.util.spec_from_file_location(
        "canary_receipt_generator_for_assets_test", generator_script
    )
    assert generator_spec is not None and generator_spec.loader is not None
    generator = importlib.util.module_from_spec(generator_spec)
    generator_spec.loader.exec_module(generator)
    runner_os = {"windows": "Windows", "macos": "macOS", "linux": "Linux"}
    for index, platform in enumerate(policy.PLATFORMS, 1):
        generator.generate_receipt(
            platform=platform,
            package=assets / PACKAGE_NAMES[platform],
            output=assets / policy.RECEIPT_FILENAMES[platform],
            version=VERSION,
            tag=TAG,
            commit=COMMIT,
            repository=REPOSITORY,
            release_id=RELEASE_ID,
            asset_id=100 + index,
            workflow_run_id=200,
            workflow_run_attempt=1,
            runner_os=runner_os[platform],
            runner_arch="X64",
            smoke_command=f"smoke-{platform}",
            verified_at="2026-07-26T12:00:00Z",
        )


class CanaryReleaseAssetsTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="chaft-canary-release-assets-test-"
        )
        self.root = Path(self.temporary.name)
        self.assets = self.root / "assets"
        self.assets.mkdir()
        bundle = self.assets / assets_tool.QT_SOURCE_BUNDLE
        bundle.write_bytes(b"synthetic authenticated Qt source")
        checksum = self.assets / assets_tool.QT_SOURCE_CHECKSUM
        checksum.write_text(
            f"{sha256(bundle)}  {bundle.name}\n", encoding="ascii"
        )
        for platform in policy.PLATFORMS:
            write_platform(self.assets, platform, sha256(bundle))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def qt_verifier(bundle: Path, checksum: Path) -> None:
        assets_tool.verify_qt_sidecar(bundle, checksum)

    def test_verifies_exact_14_asset_base(self) -> None:
        assets_tool.verify_base_assets(
            self.assets,
            version=VERSION,
            tag=TAG,
            commit=COMMIT,
            qt_verifier=self.qt_verifier,
        )
        self.assertEqual(len(list(self.assets.iterdir())), 14)

    def test_finalizes_and_verifies_exact_19_asset_namespace(self) -> None:
        add_receipts(self.assets)
        assets_tool.finalize_assets(
            self.assets,
            version=VERSION,
            tag=TAG,
            commit=COMMIT,
            repository=REPOSITORY,
            release_id=RELEASE_ID,
            qt_verifier=self.qt_verifier,
        )
        self.assertEqual(len(list(self.assets.iterdir())), 19)
        inventory = json.loads(
            (self.assets / assets_tool.INVENTORY_FILENAME).read_text()
        )
        self.assertEqual(inventory["assetCount"], 17)
        self.assertEqual(inventory["channel"], "canary")
        self.assertEqual(inventory["signingStatus"], "unsigned-canary")
        checksums = (
            self.assets / assets_tool.AGGREGATE_CHECKSUM_FILENAME
        ).read_text()
        self.assertEqual(len(checksums.splitlines()), 18)

    def test_rejects_missing_extra_and_tampered_assets(self) -> None:
        (self.assets / PACKAGE_NAMES["windows"]).unlink()
        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError, "exactly 14|missing"
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_complete_verification_rejects_stale_inventory_and_checksums(self) -> None:
        add_receipts(self.assets)
        assets_tool.finalize_assets(
            self.assets,
            version=VERSION,
            tag=TAG,
            commit=COMMIT,
            repository=REPOSITORY,
            release_id=RELEASE_ID,
            qt_verifier=self.qt_verifier,
        )
        package = self.assets / PACKAGE_NAMES["linux"]
        package.write_bytes(b"substituted package")
        with self.assertRaises(
            assets_tool.CanaryReleaseAssetError
        ):
            assets_tool.verify_complete_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                repository=REPOSITORY,
                release_id=RELEASE_ID,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_receipt_release_identity_or_duplicate_asset_id(self) -> None:
        add_receipts(self.assets)
        linux = self.assets / policy.RECEIPT_FILENAMES["linux"]
        receipt = json.loads(linux.read_text())
        receipt["release"]["id"] = RELEASE_ID + 1
        write_json(linux, receipt)
        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError, "release ID"
        ):
            assets_tool.finalize_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                repository=REPOSITORY,
                release_id=RELEASE_ID,
                qt_verifier=self.qt_verifier,
            )


if __name__ == "__main__":
    unittest.main()
