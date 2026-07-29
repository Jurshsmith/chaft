#!/usr/bin/env python3
"""Focused tests for Chaft's exact 18/22/24 unsigned-canary asset layout."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


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
SOURCE_VERSION = assets_tool.release_metadata.release_version.validated_source_version(
    assets_tool.REPOSITORY_ROOT
)[0]
EXPECTED_TARGETS = (
    "windows-x86_64",
    "macos-x86_64",
    "macos-arm64",
    "linux-x86_64",
)
PACKAGE_NAMES = {
    target.name: target.package_name(VERSION)
    for target in assets_tool.release_targets.TARGETS
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


def qt_source_contract(qt_sha256: str) -> dict[str, object]:
    contract = assets_tool.expected_qt_source_contract()
    contract["bundleSha256"] = qt_sha256
    return contract


def qt_release_binding(
    target_name: str,
    qt_sha256: str,
) -> dict[str, object]:
    root = assets_tool.REPOSITORY_ROOT
    manifest = assets_tool.qt_sdk.load_manifest(
        root / "tools" / "qt" / "qt-6.8.4.json",
        recipe_root=root,
    )
    specification = manifest["targets"][target_name]
    platform_name = specification["platform"]
    runner_os = {
        "linux": "Linux",
        "macos": "macOS",
        "windows": "Windows",
    }[platform_name]
    contract = {
        "schemaVersion": 2,
        "target": target_name,
        "platform": platform_name,
        "runner": {
            "os": runner_os,
            "architecture": specification["architecture"],
            "imageOS": f"synthetic-{platform_name}",
            "imageVersion": "20260729.1",
        },
        "tools": {
            "cmake": "cmake version 4.1.0",
            "ninja": "1.13.1",
            "compiler": f"synthetic {platform_name} compiler 1.0",
            "python": "3.13.3",
        },
    }
    fingerprint = assets_tool.qt_sdk.toolchain_fingerprint(
        contract,
        manifest,
        target_name,
    )
    sdk_provenance = {
        "schemaVersion": 2,
        "identity": assets_tool.qt_sdk.sdk_identity(
            manifest,
            target_name,
            fingerprint,
            recipe_root=root,
        ),
        "manifestSha256": assets_tool.qt_sdk.manifest_digest(
            manifest,
            recipe_root=root,
        ),
        "contractSha256": assets_tool.qt_sdk.contract_digest(
            manifest,
            recipe_root=root,
        ),
        "qtVersion": manifest["qtVersion"],
        "sdkRevision": manifest["sdkRevision"],
        "target": target_name,
        "platform": platform_name,
        "architecture": specification["architecture"],
        "targetSpecification": specification,
        "buildConfiguration": manifest["build"],
        "generatedAt": "2026-07-29T00:00:00Z",
        "host": {
            "system": runner_os,
            "release": "synthetic",
            "machine": specification["architecture"],
        },
        "toolchainContract": contract,
        "toolchainFingerprint": fingerprint,
        "sourceMaterials": assets_tool.qt_sdk.expected_source_materials(
            manifest,
            target_name,
        ),
        "recipeMaterials": assets_tool.qt_sdk.recipe_materials(root),
        "commands": [],
        "verification": {
            "completed": True,
            "completedAt": "2026-07-29T00:00:00Z",
        },
    }
    return {
        "schemaVersion": 1,
        "sdk": {
            "identity": sdk_provenance["identity"],
            "provenanceSha256": assets_tool.qt_sdk.sha256_bytes(
                assets_tool.qt_sdk.canonical_json(sdk_provenance)
            ),
            "provenance": sdk_provenance,
        },
        "correspondingSource": qt_source_contract(qt_sha256),
    }


def write_target(assets: Path, target_name: str, qt_sha256: str) -> None:
    target = assets_tool.release_targets.TARGET_BY_NAME[target_name]
    package = assets / PACKAGE_NAMES[target_name]
    package.write_bytes(f"{target_name} package".encode())
    row = {
        "name": package.name,
        "packageFormat": PACKAGE_FORMATS[target.platform],
        "sizeBytes": package.stat().st_size,
        "sha256": sha256(package),
    }
    names = assets_tool.stager.METADATA_FILENAMES[target_name]
    (assets / names["checksums"]).write_text(
        f"{row['sha256']}  {row['name']}\n", encoding="utf-8"
    )
    write_json(
        assets / names["sbom"],
        {
            "bomFormat": "CycloneDX",
            "specVersion": "1.5",
            "metadata": {
                "timestamp": "2026-07-29T00:00:00Z",
                "component": {
                    "type": "application",
                    "name": "Chaft Desktop",
                    "version": VERSION,
                    "bom-ref": f"pkg:generic/chaft-desktop@{VERSION}",
                },
                "properties": [
                    {"name": "chaft:sourceCommit", "value": COMMIT},
                    {"name": "chaft:sourceVersion", "value": SOURCE_VERSION},
                    {
                        "name": "chaft:distributionVersion",
                        "value": VERSION,
                    },
                    {"name": "chaft:packageTarget", "value": target.name},
                    {"name": "chaft:packagePlatform", "value": target.platform},
                    {
                        "name": "chaft:packageArchitecture",
                        "value": target.architecture,
                    },
                ]
            },
            "components": [
                {
                    "type": "library",
                    "name": "synthetic-fixture",
                    "version": "1.0.0",
                }
            ],
            "properties": [
                {
                    "name": f"chaft:artifact:{row['name']}:sha256",
                    "value": row["sha256"],
                },
                {
                    "name": f"chaft:artifact:{row['name']}:packageFormat",
                    "value": row["packageFormat"],
                },
            ],
        },
    )
    materials = [
        {"name": name, **fingerprint}
        for name, fingerprint in (
            assets_tool.release_metadata.source_material_rows(
                assets_tool.REPOSITORY_ROOT
            )
        ).items()
    ]
    write_json(
        assets / names["provenance"],
        {
            "schemaVersion": "chaft.desktop.provenance.v2",
            "profile": "release",
            "packageTarget": target.name,
            "packagePlatform": target.platform,
            "packageArchitecture": target.architecture,
            "sourceVersion": SOURCE_VERSION,
            "distributionVersion": VERSION,
            "version": VERSION,
            "createdAt": "2026-07-29T00:00:00Z",
            "platform": {
                "system": target.platform,
                "release": "synthetic",
                "machine": target.architecture,
            },
            "source": {
                "commit": COMMIT,
                "repository": f"git@github.com:{REPOSITORY}.git",
                "dirty": False,
            },
            "github": {
                "GITHUB_REPOSITORY": REPOSITORY,
                "GITHUB_RUN_ID": "200",
                "GITHUB_SHA": COMMIT,
                "CHAFT_RELEASE_COMMIT": COMMIT,
            },
            "qt": qt_release_binding(target_name, qt_sha256),
            "materials": materials,
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
    for index, target_name in enumerate(policy.TARGETS, 1):
        target = assets_tool.release_targets.TARGET_BY_NAME[target_name]
        generator.generate_receipt(
            target=target_name,
            package=assets / PACKAGE_NAMES[target_name],
            output=assets / policy.RECEIPT_FILENAMES[target_name],
            version=VERSION,
            tag=TAG,
            commit=COMMIT,
            repository=REPOSITORY,
            release_id=RELEASE_ID,
            asset_id=100 + index,
            workflow_run_id=200,
            workflow_run_attempt=1,
            runner_os=policy.RUNNER_OS[target.platform],
            runner_arch=target.architecture,
            smoke_command=f"smoke-{target_name}",
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
        for target_name in policy.TARGETS:
            write_target(self.assets, target_name, sha256(bundle))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def qt_verifier(bundle: Path, checksum: Path) -> None:
        assets_tool.verify_qt_sidecar(bundle, checksum)

    def test_verifies_exact_18_asset_base(self) -> None:
        self.assertEqual(policy.TARGETS, EXPECTED_TARGETS)
        with mock.patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )
        self.assertEqual(len(list(self.assets.iterdir())), 18)

    def test_ci_rejects_missing_github_provenance_context(self) -> None:
        names = assets_tool.stager.METADATA_FILENAMES["windows-x86_64"]
        provenance_path = self.assets / names["provenance"]
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        del provenance["github"]
        write_json(provenance_path, provenance)

        with mock.patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}):
            with self.assertRaisesRegex(
                assets_tool.CanaryReleaseAssetError,
                "CI provenance is missing github context",
            ):
                assets_tool.verify_base_assets(
                    self.assets,
                    version=VERSION,
                    tag=TAG,
                    commit=COMMIT,
                    qt_verifier=self.qt_verifier,
                )

    def test_ci_rejects_mismatched_release_commit_context(self) -> None:
        names = assets_tool.stager.METADATA_FILENAMES["windows-x86_64"]
        provenance_path = self.assets / names["provenance"]
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["github"]["CHAFT_RELEASE_COMMIT"] = "b" * 40
        write_json(provenance_path, provenance)

        with mock.patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}):
            with self.assertRaisesRegex(
                assets_tool.CanaryReleaseAssetError,
                "CHAFT_RELEASE_COMMIT does not match the expected release commit",
            ):
                assets_tool.verify_base_assets(
                    self.assets,
                    version=VERSION,
                    tag=TAG,
                    commit=COMMIT,
                    qt_verifier=self.qt_verifier,
                )

    def test_base_rejects_noncanonical_target_package_filename(self) -> None:
        target_name = "macos-arm64"
        canonical = self.assets / PACKAGE_NAMES[target_name]
        renamed = self.assets / f"Chaft-{VERSION}-macOS-aarch64.dmg"
        canonical.rename(renamed)
        names = assets_tool.stager.METADATA_FILENAMES[target_name]
        provenance_path = self.assets / names["provenance"]
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["artifacts"][0]["name"] = renamed.name
        write_json(provenance_path, provenance)
        (self.assets / names["checksums"]).write_text(
            f"{sha256(renamed)}  {renamed.name}\n",
            encoding="utf-8",
        )

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "provenance package set must contain exactly "
            f"{PACKAGE_NAMES[target_name]}",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_legacy_qt_source_asset_aliases(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "windows-x86_64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        corresponding_source = provenance["qt"]["correspondingSource"]
        corresponding_source["bundle"] = corresponding_source.pop("bundleName")
        corresponding_source["checksum"] = corresponding_source.pop("checksumName")
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "keys differ from the release contract",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_changed_qt_source_contract(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "macos-arm64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["qt"]["correspondingSource"]["contractSha256"] = "0" * 64
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "contract differs from the release checkout",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_extra_qt_source_contract_key(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "linux-x86_64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["qt"]["correspondingSource"]["unexpected"] = True
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "keys differ from the release contract",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_stale_qt_source_bundle_digest(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "windows-x86_64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["qt"]["correspondingSource"]["bundleSha256"] = "0" * 64
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "Qt source bundle digest is stale",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_missing_qt_sdk_provenance_binding(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "macos-arm64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["qt"]["sdk"] = None
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "Qt SDK binding is missing or malformed",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_mismatched_qt_sdk_provenance_target(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "macos-arm64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["qt"]["sdk"]["provenance"]["target"] = "macos-x86_64"
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "Qt SDK provenance target mismatch",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_wrong_native_host_architecture(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "macos-arm64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["platform"]["machine"] = "x86_64"
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "host architecture x86_64 does not match native target arm64",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_missing_source_checkout_materials(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "macos-arm64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        del provenance["materials"]
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "provenance materials array is missing",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_missing_qt_sdk_source_materials(self) -> None:
        provenance_path = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "macos-arm64"
            ]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["qt"]["sdk"]["provenance"]["sourceMaterials"] = []
        write_json(provenance_path, provenance)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "Qt SDK provenance sourceMaterials mismatch",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_finalizes_and_verifies_exact_24_asset_namespace(self) -> None:
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
        self.assertEqual(len(list(self.assets.iterdir())), 24)
        inventory = json.loads(
            (self.assets / assets_tool.INVENTORY_FILENAME).read_text()
        )
        self.assertEqual(inventory["assetCount"], 22)
        self.assertEqual(inventory["channel"], "canary")
        self.assertEqual(inventory["signingStatus"], "unsigned-canary")
        checksums = (
            self.assets / assets_tool.AGGREGATE_CHECKSUM_FILENAME
        ).read_text()
        self.assertEqual(len(checksums.splitlines()), 23)

    def test_rejects_missing_extra_and_tampered_assets(self) -> None:
        (self.assets / PACKAGE_NAMES["windows-x86_64"]).unlink()
        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError, "exactly 18|missing"
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_omitted_apple_silicon_target_metadata(self) -> None:
        provenance = (
            self.assets
            / assets_tool.stager.METADATA_FILENAMES[
                "macos-arm64"
            ]["provenance"]
        )
        provenance.unlink()
        (self.assets / "unexpected-placeholder.txt").write_text(
            "preserve the exact count\n", encoding="utf-8"
        )

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "missing macos-arm64 provenance metadata",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_one_package_referenced_by_both_macos_targets(self) -> None:
        intel_package = self.assets / PACKAGE_NAMES["macos-x86_64"]
        names = assets_tool.stager.METADATA_FILENAMES["macos-arm64"]
        provenance_path = self.assets / names["provenance"]
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["artifacts"][0] = {
            "name": intel_package.name,
            "packageFormat": "macos-dmg",
            "sizeBytes": intel_package.stat().st_size,
            "sha256": sha256(intel_package),
        }
        write_json(provenance_path, provenance)
        (self.assets / names["checksums"]).write_text(
            f"{sha256(intel_package)}  {intel_package.name}\n",
            encoding="utf-8",
        )

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "referenced by both macos-x86_64 and macos-arm64 provenance",
        ):
            assets_tool.verify_base_assets(
                self.assets,
                version=VERSION,
                tag=TAG,
                commit=COMMIT,
                qt_verifier=self.qt_verifier,
            )

    def test_rejects_unsigned_receipt_for_the_wrong_macos_target(self) -> None:
        add_receipts(self.assets)
        receipt_path = (
            self.assets / policy.RECEIPT_FILENAMES["macos-arm64"]
        )
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        receipt["target"] = "macos-x86_64"
        write_json(receipt_path, receipt)

        with self.assertRaisesRegex(
            assets_tool.CanaryReleaseAssetError,
            "macos-arm64.*receipt.*target",
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
        package = self.assets / PACKAGE_NAMES["linux-x86_64"]
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
        linux = self.assets / policy.RECEIPT_FILENAMES["linux-x86_64"]
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
