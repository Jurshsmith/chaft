#!/usr/bin/env python3
"""Stdlib tests for stage-website-release-assets.py."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).with_name("stage-website-release-assets.py")
SPEC = importlib.util.spec_from_file_location("chaft_release_asset_stager", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load {SCRIPT}")
stager = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = stager
SPEC.loader.exec_module(stager)


VERSION = "1.2.3"
EXPECTED_TARGETS = (
    "windows-x86_64",
    "macos-x86_64",
    "macos-arm64",
    "linux-x86_64",
)
PACKAGE_NAMES = {
    target.name: target.package_name(VERSION)
    for target in stager.release_targets.TARGETS
}
PACKAGE_FORMATS = {
    "windows": "windows-zip",
    "macos": "macos-dmg",
    "linux": "linux-appimage",
}
WINDOWS_SIGNER = "A" * 40
APPLE_TEAM_ID = "AB12CD34EF"
LINUX_SIGNER = "B" * 40


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def target_contract(
    target_name: str,
) -> stager.release_targets.ReleaseTarget:
    return stager.release_targets.TARGET_BY_NAME[target_name]


def metadata_paths(assets: Path, target_name: str) -> dict[str, Path]:
    return {
        kind: assets / name
        for kind, name in stager.METADATA_FILENAMES[target_name].items()
    }


def write_target_assets(
    assets: Path,
    target_name: str,
    *,
    with_signature: bool = False,
    version: str = VERSION,
) -> None:
    target = target_contract(target_name)
    package_name = target.package_name(version)
    package_path = assets / package_name
    package_path.write_bytes(f"synthetic {target_name} package\n".encode())
    rows = [
        {
            "name": package_name,
            "packageFormat": PACKAGE_FORMATS[target.platform],
            "sizeBytes": package_path.stat().st_size,
            "sha256": sha256(package_path),
        }
    ]
    if with_signature:
        signature_path = assets / f"{package_name}.sig"
        signature_path.write_bytes(f"signature for {package_name}\n".encode())
        rows.append(
            {
                "name": signature_path.name,
                "packageFormat": "detached-signature",
                "signatureFormat": "sig",
                "signedArtifact": package_name,
                "sizeBytes": signature_path.stat().st_size,
                "sha256": sha256(signature_path),
            }
        )

    paths = metadata_paths(assets, target_name)
    paths["checksums"].write_text(
        "".join(f"{row['sha256']}  {row['name']}\n" for row in rows),
        encoding="utf-8",
    )
    write_json(
        paths["sbom"],
        {
            "bomFormat": "CycloneDX",
            "metadata": {
                "properties": [
                    {"name": "chaft:packageTarget", "value": target.name},
                    {"name": "chaft:packagePlatform", "value": target.platform},
                    {
                        "name": "chaft:packageArchitecture",
                        "value": target.architecture,
                    },
                ]
            },
        },
    )
    write_json(
        paths["provenance"],
        {
            "schemaVersion": "chaft.desktop.provenance.v2",
            "profile": "release",
            "packageTarget": target.name,
            "packagePlatform": target.platform,
            "packageArchitecture": target.architecture,
            "distributionVersion": version,
            "version": version,
            "artifacts": rows,
        },
    )


def write_receipt(assets: Path, target_name: str) -> None:
    target = target_contract(target_name)
    package = assets / PACKAGE_NAMES[target_name]
    policies = {
        "windows": {
            "publisherIdentity": {
                "type": "authenticode-signer-certificate-thumbprint",
                "value": WINDOWS_SIGNER,
                "algorithm": "sha1",
            }
        },
        "macos": {
            "publisherIdentity": {
                "type": "apple-developer-team-id",
                "value": APPLE_TEAM_ID,
            }
        },
        "linux": {
            "publisherIdentity": {
                "type": "openpgp-primary-key-fingerprint",
                "value": LINUX_SIGNER,
            },
            "trustedKeyring": {
                "filename": "chaft-release-signing-keys.gpg",
                "sha256": "c" * 64,
            },
        },
    }
    signatures = []
    if target.platform == "linux":
        signature = assets / f"{package.name}.sig"
        signatures.append(
            {
                "filename": signature.name,
                "signedArtifact": package.name,
                "sha256": sha256(signature),
                "signerFingerprint": LINUX_SIGNER,
                "trustedFingerprint": LINUX_SIGNER,
            }
        )
    write_json(
        assets / stager.VERIFICATION_RECEIPT_FILENAMES[target_name],
        {
            "schemaVersion": "chaft.desktop.platform-verification.v2",
            "target": target.name,
            "platform": target.platform,
            "architecture": target.architecture,
            "status": "verified",
            "verificationPolicy": policies[target.platform],
            "artifacts": [
                {"filename": package.name, "sha256": sha256(package)}
            ],
            "signatures": signatures,
        },
    )


def write_unsigned_canary_receipt(
    assets: Path,
    target_name: str,
    *,
    version: str,
    asset_id: int,
) -> None:
    target = target_contract(target_name)
    package = next(
        path
        for path in assets.iterdir()
        if path.name == target.package_name(version)
    )
    write_json(
        assets / stager.UNSIGNED_CANARY_RECEIPT_FILENAMES[target_name],
        {
            "schemaVersion": stager.unsigned_canary.SCHEMA_VERSION,
            "target": target.name,
            "platform": target.platform,
            "verificationType": stager.unsigned_canary.VERIFICATION_TYPE,
            "status": stager.unsigned_canary.STATUS,
            "signingStatus": stager.unsigned_canary.SIGNING_STATUS,
            "signatureVerification": (
                stager.unsigned_canary.SIGNATURE_VERIFICATION[target.platform]
            ),
            "signatureAndNotarization": dict(
                stager.unsigned_canary.SIGNATURE_AND_NOTARIZATION[
                    target.platform
                ]
            ),
            "productionEligible": False,
            "warning": stager.unsigned_canary.WARNING,
            "version": version,
            "tag": f"v{version}",
            "commit": "a" * 40,
            "repository": "Jurshsmith/chaft",
            "architecture": target.architecture,
            "verifiedAt": "2026-07-18T12:34:56Z",
            "release": {"id": 4321},
            "asset": {
                "id": asset_id,
                "filename": package.name,
                "sizeBytes": package.stat().st_size,
                "sha256": sha256(package),
            },
            "runner": {
                "os": stager.unsigned_canary.RUNNER_OS[target.platform],
                "architecture": target.architecture,
                "workflowRunId": 1234,
                "workflowRunAttempt": 1,
            },
            "smoke": {
                "status": stager.unsigned_canary.STATUS,
                "command": f"synthetic {target_name} smoke",
            },
            "receiptGenerator": {
                "name": "Chaft unsigned-canary receipt generator",
                "version": "1",
            },
        },
    )


class ReleaseAssetStagingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="chaft-release-asset-stager-test-"
        )
        self.root = Path(self.temporary.name)
        self.assets = self.root / "downloaded-assets"
        self.assets.mkdir()
        for target_name in stager.TARGETS:
            write_target_assets(
                self.assets,
                target_name,
                with_signature=target_name == "linux-x86_64",
            )
        for target_name in stager.NATIVE_RECEIPT_TARGETS:
            write_receipt(self.assets, target_name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def rewrite_provenance(self, target_name: str, mutate: object) -> None:
        path = metadata_paths(self.assets, target_name)["provenance"]
        value = json.loads(path.read_text(encoding="utf-8"))
        mutate(value)  # type: ignore[operator]
        write_json(path, value)

    def test_stages_exact_platform_layout_and_separate_receipts(self) -> None:
        output = self.root / "staged"
        plan = stager.stage_assets(self.assets, output)

        self.assertEqual(stager.TARGETS, EXPECTED_TARGETS)
        self.assertEqual(tuple(plan.targets), EXPECTED_TARGETS)
        for target_name in stager.TARGETS:
            package_dir = output / stager.PACKAGE_DIRECTORY_NAMES[target_name]
            expected = set(stager.METADATA_FILENAMES[target_name].values())
            expected.add(PACKAGE_NAMES[target_name])
            if target_name == "linux-x86_64":
                expected.add(f"{PACKAGE_NAMES[target_name]}.sig")
            self.assertEqual(
                {path.name for path in package_dir.iterdir()}, expected
            )
            self.assertFalse(
                any("verification" in path.name for path in package_dir.iterdir())
            )

        receipt_dir = output / "verification-receipts"
        self.assertEqual(
            {path.name for path in receipt_dir.iterdir()},
            {
                stager.VERIFICATION_RECEIPT_FILENAMES[target_name]
                for target_name in stager.NATIVE_RECEIPT_TARGETS
            },
        )
        self.assertIsNone(plan.targets["linux-x86_64"].receipt)

    def test_stages_an_optional_linux_receipt_separately(self) -> None:
        write_receipt(self.assets, "linux-x86_64")
        output = self.root / "staged"
        stager.stage_assets(self.assets, output)
        self.assertTrue(
            (
                output
                / "verification-receipts"
                / stager.VERIFICATION_RECEIPT_FILENAMES["linux-x86_64"]
            ).is_file()
        )

    def test_linux_receipt_binds_exact_detached_signature_bytes(self) -> None:
        write_receipt(self.assets, "linux-x86_64")
        signature = self.assets / f"{PACKAGE_NAMES['linux-x86_64']}.sig"
        signature.write_bytes(b"substituted signature\n")
        provenance_path = metadata_paths(
            self.assets, "linux-x86_64"
        )["provenance"]
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        row = next(
            item for item in provenance["artifacts"] if item["name"] == signature.name
        )
        row["sha256"] = sha256(signature)
        row["sizeBytes"] = signature.stat().st_size
        write_json(provenance_path, provenance)
        metadata_paths(self.assets, "linux-x86_64")["checksums"].write_text(
            "".join(
                f"{item['sha256']}  {item['name']}\n"
                for item in provenance["artifacts"]
            ),
            encoding="utf-8",
        )

        with self.assertRaisesRegex(stager.AssetStagingError, "exact detached-signature"):
            stager.stage_assets(self.assets, self.root / "staged")

    def test_rejects_unexpected_assets_unless_exactly_allowlisted(self) -> None:
        extra = self.assets / "release-notes.txt"
        extra.write_text("release notes\n", encoding="utf-8")
        with self.assertRaisesRegex(stager.AssetStagingError, "unexpected.*release-notes"):
            stager.build_stage_plan(self.assets)

        output = self.root / "staged"
        stager.stage_assets(
            self.assets,
            output,
            allowed_extra_assets=[extra.name],
        )
        self.assertFalse((output / extra.name).exists())

    def test_never_allows_unreferenced_installers_to_be_ignored(self) -> None:
        extra = self.assets / "Old-Chaft-Windows.exe"
        extra.write_bytes(b"old installer\n")
        with self.assertRaisesRegex(stager.AssetStagingError, "managed.*allowlisted"):
            stager.build_stage_plan(
                self.assets,
                allowed_extra_assets=[extra.name],
            )

    def test_rejects_missing_and_tampered_artifacts(self) -> None:
        (self.assets / PACKAGE_NAMES["linux-x86_64"]).unlink()
        with self.assertRaisesRegex(stager.AssetStagingError, "missing.*provenance artifact"):
            stager.build_stage_plan(self.assets)

        write_target_assets(
            self.assets, "linux-x86_64", with_signature=True
        )
        package = self.assets / PACKAGE_NAMES["linux-x86_64"]
        package.write_bytes(package.read_bytes() + b"tampered\n")
        with self.assertRaisesRegex(stager.AssetStagingError, "SHA-256 is stale"):
            stager.build_stage_plan(self.assets)

    def test_rejects_traversal_and_duplicate_provenance_rows(self) -> None:
        self.rewrite_provenance(
            "windows-x86_64",
            lambda value: value["artifacts"][0].__setitem__(
                "name", "../escape.zip"
            ),
        )
        with self.assertRaisesRegex(stager.AssetStagingError, "without traversal"):
            stager.build_stage_plan(self.assets)

        write_target_assets(self.assets, "windows-x86_64")
        self.rewrite_provenance(
            "windows-x86_64",
            lambda value: value["artifacts"].append(
                dict(value["artifacts"][0])
            ),
        )
        with self.assertRaisesRegex(stager.AssetStagingError, "duplicate.*artifact row"):
            stager.build_stage_plan(self.assets)

    def test_rejects_platform_mismatches_in_provenance_sbom_and_receipt(self) -> None:
        self.rewrite_provenance(
            "windows-x86_64",
            lambda value: value.__setitem__("packagePlatform", "linux"),
        )
        with self.assertRaisesRegex(stager.AssetStagingError, "packagePlatform"):
            stager.build_stage_plan(self.assets)

        write_target_assets(self.assets, "windows-x86_64")
        sbom_path = metadata_paths(self.assets, "windows-x86_64")["sbom"]
        sbom = json.loads(sbom_path.read_text(encoding="utf-8"))
        platform_property = next(
            item
            for item in sbom["metadata"]["properties"]
            if item["name"] == "chaft:packagePlatform"
        )
        platform_property["value"] = "linux"
        write_json(sbom_path, sbom)
        with self.assertRaisesRegex(stager.AssetStagingError, "SBOM package platform"):
            stager.build_stage_plan(self.assets)

        write_target_assets(self.assets, "windows-x86_64")
        receipt_path = (
            self.assets
            / stager.VERIFICATION_RECEIPT_FILENAMES["windows-x86_64"]
        )
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        receipt["platform"] = "linux"
        write_json(receipt_path, receipt)
        with self.assertRaisesRegex(stager.AssetStagingError, "receipt platform"):
            stager.build_stage_plan(self.assets)

    def test_rejects_v2_target_and_architecture_mismatches(self) -> None:
        self.rewrite_provenance(
            "macos-arm64",
            lambda value: value.__setitem__("packageTarget", "macos-x86_64"),
        )
        with self.assertRaisesRegex(stager.AssetStagingError, "packageTarget"):
            stager.build_stage_plan(self.assets)

        write_target_assets(self.assets, "macos-arm64")
        self.rewrite_provenance(
            "macos-arm64",
            lambda value: value.__setitem__("packageArchitecture", "x86_64"),
        )
        with self.assertRaisesRegex(
            stager.AssetStagingError, "packageArchitecture"
        ):
            stager.build_stage_plan(self.assets)

        write_target_assets(self.assets, "macos-arm64")
        sbom_path = metadata_paths(self.assets, "macos-arm64")["sbom"]
        sbom = json.loads(sbom_path.read_text(encoding="utf-8"))
        architecture_property = next(
            item
            for item in sbom["metadata"]["properties"]
            if item["name"] == "chaft:packageArchitecture"
        )
        architecture_property["value"] = "x86_64"
        write_json(sbom_path, sbom)
        with self.assertRaisesRegex(
            stager.AssetStagingError, "SBOM package architecture"
        ):
            stager.build_stage_plan(self.assets)

        write_target_assets(self.assets, "macos-arm64")
        receipt_path = (
            self.assets
            / stager.VERIFICATION_RECEIPT_FILENAMES["macos-arm64"]
        )
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        receipt["target"] = "macos-x86_64"
        write_json(receipt_path, receipt)
        with self.assertRaisesRegex(stager.AssetStagingError, "receipt target"):
            stager.build_stage_plan(self.assets)

        write_receipt(self.assets, "macos-arm64")
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        receipt["architecture"] = "x86_64"
        write_json(receipt_path, receipt)
        with self.assertRaisesRegex(
            stager.AssetStagingError, "receipt architecture"
        ):
            stager.build_stage_plan(self.assets)

    def test_rejects_omitted_apple_silicon_assets_and_receipt(self) -> None:
        (self.assets / PACKAGE_NAMES["macos-arm64"]).unlink()
        with self.assertRaisesRegex(
            stager.AssetStagingError,
            "missing macos-arm64 provenance artifact",
        ):
            stager.build_stage_plan(self.assets)

        write_target_assets(self.assets, "macos-arm64")
        (
            self.assets
            / stager.VERIFICATION_RECEIPT_FILENAMES["macos-arm64"]
        ).unlink()
        with self.assertRaisesRegex(
            stager.AssetStagingError,
            "missing macos-arm64 native verification receipt",
        ):
            stager.build_stage_plan(self.assets)

    def test_rejects_one_package_referenced_by_both_macos_targets(self) -> None:
        intel_package = self.assets / PACKAGE_NAMES["macos-x86_64"]
        paths = metadata_paths(self.assets, "macos-arm64")
        provenance = json.loads(paths["provenance"].read_text(encoding="utf-8"))
        provenance["artifacts"][0] = {
            "name": intel_package.name,
            "packageFormat": "macos-dmg",
            "sizeBytes": intel_package.stat().st_size,
            "sha256": sha256(intel_package),
        }
        write_json(paths["provenance"], provenance)
        paths["checksums"].write_text(
            f"{sha256(intel_package)}  {intel_package.name}\n",
            encoding="utf-8",
        )

        with self.assertRaisesRegex(
            stager.AssetStagingError,
            "referenced by both macos-x86_64 and macos-arm64 provenance",
        ):
            stager.build_stage_plan(self.assets)

    def test_rejects_coherently_renamed_noncanonical_target_package(self) -> None:
        target_name = "macos-arm64"
        canonical = self.assets / PACKAGE_NAMES[target_name]
        renamed = self.assets / f"Chaft-{VERSION}-macOS-mislabeled.dmg"
        canonical.rename(renamed)
        paths = metadata_paths(self.assets, target_name)
        provenance = json.loads(paths["provenance"].read_text(encoding="utf-8"))
        provenance["artifacts"][0]["name"] = renamed.name
        write_json(paths["provenance"], provenance)
        paths["checksums"].write_text(
            f"{sha256(renamed)}  {renamed.name}\n",
            encoding="utf-8",
        )
        receipt_path = (
            self.assets
            / stager.VERIFICATION_RECEIPT_FILENAMES[target_name]
        )
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        receipt["artifacts"][0]["filename"] = renamed.name
        write_json(receipt_path, receipt)

        with self.assertRaisesRegex(
            stager.AssetStagingError,
            f"package set must contain exactly {PACKAGE_NAMES[target_name]}",
        ):
            stager.build_stage_plan(self.assets)

    def test_rejects_a_package_suffix_for_the_wrong_platform(self) -> None:
        old_package = self.assets / PACKAGE_NAMES["windows-x86_64"]
        wrong_name = f"Chaft-{VERSION}-Windows-x86_64.dmg"
        old_package.rename(self.assets / wrong_name)
        paths = metadata_paths(self.assets, "windows-x86_64")
        provenance = json.loads(paths["provenance"].read_text(encoding="utf-8"))
        provenance["artifacts"][0]["name"] = wrong_name
        provenance["artifacts"][0]["packageFormat"] = "macos-dmg"
        write_json(paths["provenance"], provenance)
        paths["checksums"].write_text(
            f"{provenance['artifacts'][0]['sha256']}  {wrong_name}\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(stager.AssetStagingError, "macos package"):
            stager.build_stage_plan(self.assets)

    def test_rejects_missing_native_and_mismatched_receipts(self) -> None:
        receipt = (
            self.assets
            / stager.VERIFICATION_RECEIPT_FILENAMES["windows-x86_64"]
        )
        receipt.unlink()
        with self.assertRaisesRegex(
            stager.AssetStagingError, "missing windows-x86_64.*receipt"
        ):
            stager.build_stage_plan(self.assets)

        write_receipt(self.assets, "windows-x86_64")
        value = json.loads(receipt.read_text(encoding="utf-8"))
        value["artifacts"][0]["sha256"] = "0" * 64
        write_json(receipt, value)
        with self.assertRaisesRegex(stager.AssetStagingError, "exact package set"):
            stager.build_stage_plan(self.assets)

    def test_rejects_case_insensitive_filename_collisions(self) -> None:
        probe = self.assets / ".case-sensitivity-probe"
        probe.write_text("probe\n", encoding="utf-8")
        if (self.assets / ".CASE-SENSITIVITY-PROBE").exists():
            probe.unlink()
            self.skipTest("filesystem does not permit case-variant filenames")
        probe.unlink()
        collision = (
            self.assets / "CHAFT-DESKTOP-LINUX-X86_64-SBOM.CDX.JSON"
        )
        collision.write_text("{}\n", encoding="utf-8")
        with self.assertRaisesRegex(stager.AssetStagingError, "case-insensitive duplicate"):
            stager.build_stage_plan(self.assets)

    def test_rejects_symlinks_and_nested_directories(self) -> None:
        nested = self.assets / "nested"
        nested.mkdir()
        with self.assertRaisesRegex(stager.AssetStagingError, "must be flat"):
            stager.build_stage_plan(self.assets)
        nested.rmdir()

        target = self.root / "external.txt"
        target.write_text("external\n", encoding="utf-8")
        link = self.assets / "external-link.txt"
        try:
            link.symlink_to(target)
        except (NotImplementedError, OSError):
            self.skipTest("symbolic links are unavailable")
        with self.assertRaisesRegex(stager.AssetStagingError, "symbolic link"):
            stager.build_stage_plan(self.assets)

    def test_existing_output_is_untouched_and_partial_copy_is_not_published(self) -> None:
        output = self.root / "staged"
        output.mkdir()
        marker = output / "owned-by-user"
        marker.write_text("keep\n", encoding="utf-8")
        with self.assertRaisesRegex(stager.AssetStagingError, "already exists"):
            stager.stage_assets(self.assets, output)
        self.assertEqual(marker.read_text(encoding="utf-8"), "keep\n")

        marker.unlink()
        output.rmdir()
        original_copy = stager.copy_fingerprinted_file
        calls = 0

        def failing_copy(source: object, destination: object) -> None:
            nonlocal calls
            calls += 1
            if calls == 2:
                raise stager.AssetStagingError("synthetic copy failure")
            original_copy(source, destination)

        with mock.patch.object(stager, "copy_fingerprinted_file", failing_copy):
            with self.assertRaisesRegex(stager.AssetStagingError, "synthetic"):
                stager.stage_assets(self.assets, output)
        self.assertFalse(output.exists())
        self.assertEqual(
            list(self.root.glob(f".{output.name}.staging-*")), []
        )

    def test_rejects_output_nested_within_the_assets_directory(self) -> None:
        with self.assertRaisesRegex(stager.AssetStagingError, "must not contain"):
            stager.stage_assets(self.assets, self.assets / "staged")

    def test_rejects_a_broken_output_symlink(self) -> None:
        output = self.root / "staged"
        try:
            output.symlink_to(self.root / "missing-target", target_is_directory=True)
        except (NotImplementedError, OSError):
            self.skipTest("symbolic links are unavailable")
        with self.assertRaisesRegex(stager.AssetStagingError, "already exists"):
            stager.stage_assets(self.assets, output)
        self.assertTrue(output.is_symlink())


if __name__ == "__main__":
    unittest.main()
