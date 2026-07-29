#!/usr/bin/env python3
"""Focused stdlib tests for export-website-release-manifest.py."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).with_name("export-website-release-manifest.py")
SPEC = importlib.util.spec_from_file_location("chaft_release_exporter", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load {SCRIPT}")
exporter = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = exporter
SPEC.loader.exec_module(exporter)


VERSION = "1.2.3"
TAG = f"v{VERSION}"
COMMIT = "0123456789abcdef0123456789abcdef01234567"
REPOSITORY = "Jurshsmith/chaft"
WINDOWS_SIGNER = "A" * 40
APPLE_TEAM_ID = "AB12CD34EF"
LINUX_SIGNER = "B" * 40
WINDOWS_TARGET = "windows-x86_64"
MACOS_X86_64_TARGET = "macos-x86_64"
MACOS_ARM64_TARGET = "macos-arm64"
LINUX_TARGET = "linux-x86_64"
PACKAGE_NAMES = {
    target_name: exporter.release_targets.TARGET_BY_NAME[target_name].package_name(VERSION)
    for target_name in exporter.TARGETS
}
PACKAGE_FORMATS = {
    WINDOWS_TARGET: "windows-zip",
    MACOS_X86_64_TARGET: "macos-dmg",
    MACOS_ARM64_TARGET: "macos-dmg",
    LINUX_TARGET: "linux-appimage",
}


def elf_payload(machine: int = 62, *, encoding: int = 1) -> bytes:
    header = bytearray(64)
    header[:4] = b"\x7fELF"
    header[4] = 2
    header[5] = encoding
    header[6] = 1
    byte_order = "<" if encoding == 1 else ">"
    struct.pack_into(f"{byte_order}H", header, 18, machine)
    return bytes(header) + b"synthetic executable payload\n"


def write_tar(path: Path, entries: list[tuple[str, bytes]]) -> None:
    with tarfile.open(path, mode="w:gz") as archive:
        for name, payload in entries:
            member = tarfile.TarInfo(name)
            member.size = len(payload)
            archive.addfile(member, io.BytesIO(payload))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def write_platform_package(
    root: Path,
    target_name: str,
    *,
    version: str = VERSION,
    commit: str = COMMIT,
    repository: str = f"git@github.com:{REPOSITORY}.git",
    with_signature: bool = False,
    provenance_package_format: str | None = None,
) -> Path:
    target = exporter.release_targets.TARGET_BY_NAME[target_name]
    platform = target.platform
    package_dir = root / f"{target_name}-package"
    package_dir.mkdir(parents=True)
    package_name = target.package_name(version)
    package_path = package_dir / package_name
    package_path.write_bytes(
        elf_payload()
        if platform == "linux"
        else f"synthetic {platform} package {version}\n".encode()
    )
    artifacts = [
        {
            "name": package_name,
            "packageFormat": provenance_package_format or PACKAGE_FORMATS[target_name],
            "sizeBytes": package_path.stat().st_size,
            "sha256": sha256(package_path),
        }
    ]
    if with_signature:
        signature_path = package_dir / f"{package_name}.sig"
        signature_path.write_bytes(f"synthetic signature for {package_name}\n".encode())
        artifacts.append(
            {
                "name": signature_path.name,
                "packageFormat": "detached-signature",
                "signatureFormat": "sig",
                "signedArtifact": package_name,
                "sizeBytes": signature_path.stat().st_size,
                "sha256": sha256(signature_path),
            }
        )

    names = exporter.METADATA_FILENAMES[target_name]
    checksum_path = package_dir / names["checksums"]
    checksum_path.write_text(
        "".join(f"{row['sha256']}  {row['name']}\n" for row in artifacts),
        encoding="utf-8",
    )
    write_json(package_dir / names["sbom"], {"bomFormat": "CycloneDX"})
    write_json(
        package_dir / names["provenance"],
        {
            "schemaVersion": "chaft.desktop.provenance.v2",
            "profile": "release",
            "packageTarget": target_name,
            "packagePlatform": platform,
            "packageArchitecture": target.architecture,
            "version": version,
            "source": {
                "commit": commit,
                "repository": repository,
                "dirty": False,
            },
            "platform": {"machine": target.architecture},
            "artifacts": artifacts,
        },
    )
    return package_dir


def write_verification_receipt(
    root: Path,
    target_name: str,
    package_dir: Path,
    *,
    version: str = VERSION,
    tag: str = TAG,
    commit: str = COMMIT,
    architecture: str | None = None,
    stale_hash: bool = False,
) -> Path:
    target = exporter.release_targets.TARGET_BY_NAME[target_name]
    platform = target.platform
    architecture = architecture or target.architecture
    receipt_dir = root / "verification-receipts"
    receipt_dir.mkdir(parents=True, exist_ok=True)
    package_paths = sorted(
        path
        for path in package_dir.iterdir()
        if path.is_file() and exporter.package_description(path.name) is not None
    )
    artifacts = [
        {
            "filename": path.name,
            "sha256": "0" * 64 if stale_hash else sha256(path),
        }
        for path in package_paths
    ]
    signatures = []
    if platform == "linux":
        for signature_path in sorted(package_dir.iterdir()):
            description = exporter.signature_description(signature_path.name)
            if description is None:
                continue
            signed_artifact, _signature_format = description
            signatures.append(
                {
                    "filename": signature_path.name,
                    "signedArtifact": signed_artifact,
                    "sha256": sha256(signature_path),
                    "signerFingerprint": LINUX_SIGNER,
                    "trustedFingerprint": LINUX_SIGNER,
                }
            )
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
    if platform == "windows":
        verification_details = [
            {
                "filename": path.name,
                "architecture": architecture,
                "verifiedPayloads": [
                    {
                        "payload": path.name,
                        "signatureType": "Authenticode",
                        "signerThumbprint": WINDOWS_SIGNER,
                        "signerThumbprintAlgorithm": "sha1",
                        "signerCertificateSha1": WINDOWS_SIGNER,
                        "signerCertificateSha256": "D" * 64,
                        "architecture": architecture,
                        "msiTemplate": "x64;1033",
                    }
                ],
            }
            for path in package_paths
        ]
    elif platform == "macos":
        verification_details = [
            {
                "filename": path.name,
                "architecture": architecture,
                "teamIdentifier": APPLE_TEAM_ID,
                "verifiedApplications": [
                    {
                        "application": "Chaft.app",
                        "executable": "Chaft",
                        "architecture": architecture,
                        "teamIdentifier": APPLE_TEAM_ID,
                    }
                ],
            }
            for path in package_paths
        ]
    else:
        signatures_by_artifact = {
            row["signedArtifact"]: row for row in signatures
        }
        verification_details = [
            {
                "filename": path.name,
                "architecture": architecture,
                "signature": {
                    "filename": signatures_by_artifact[path.name]["filename"],
                    "sha256": signatures_by_artifact[path.name]["sha256"],
                    "signerFingerprint": signatures_by_artifact[path.name][
                        "signerFingerprint"
                    ],
                    "trustedFingerprint": signatures_by_artifact[path.name][
                        "trustedFingerprint"
                    ],
                    "trustedKeyring": policies[platform]["trustedKeyring"],
                },
            }
            for path in package_paths
        ]
    path = receipt_dir / exporter.VERIFICATION_RECEIPT_FILENAMES[target_name]
    write_json(
        path,
        {
            "schemaVersion": "chaft.desktop.platform-verification.v2",
            "target": target_name,
            "platform": platform,
            "verificationType": exporter.VERIFICATION_TYPES[platform],
            "status": "verified",
            "version": version,
            "tag": tag,
            "commit": commit,
            "architecture": architecture,
            "verifiedAt": "2026-07-18T12:34:56Z",
            "verifier": {"name": "synthetic-native-verifier", "version": "1.0"},
            "artifacts": artifacts,
            "verificationPolicy": policies[platform],
            "signatures": signatures,
            "verificationDetails": verification_details,
            "receiptGenerator": {
                "name": "synthetic-receipt-generator",
                "version": "1.0",
            },
        },
    )
    return path


def write_unsigned_canary_receipt(
    root: Path,
    target_name: str,
    package_dir: Path,
    *,
    version: str,
    tag: str,
    release_id: int,
    asset_id: int,
) -> Path:
    target = exporter.release_targets.TARGET_BY_NAME[target_name]
    platform = target.platform
    receipt_dir = root / "unsigned-canary-receipts"
    receipt_dir.mkdir(parents=True, exist_ok=True)
    package = next(
        path
        for path in package_dir.iterdir()
        if exporter.package_description(path.name) is not None
    )
    receipt = {
        "schemaVersion": exporter.unsigned_canary.SCHEMA_VERSION,
        "target": target_name,
        "platform": platform,
        "verificationType": exporter.unsigned_canary.VERIFICATION_TYPE,
        "status": exporter.unsigned_canary.STATUS,
        "signingStatus": exporter.unsigned_canary.SIGNING_STATUS,
        "signatureVerification": (
            exporter.unsigned_canary.SIGNATURE_VERIFICATION[platform]
        ),
        "signatureAndNotarization": dict(
            exporter.unsigned_canary.SIGNATURE_AND_NOTARIZATION[platform]
        ),
        "productionEligible": False,
        "warning": exporter.unsigned_canary.WARNING,
        "version": version,
        "tag": tag,
        "commit": COMMIT,
        "repository": REPOSITORY,
        "architecture": target.architecture,
        "verifiedAt": "2026-07-18T12:34:56Z",
        "release": {"id": release_id},
        "asset": {
            "id": asset_id,
            "filename": package.name,
            "sizeBytes": package.stat().st_size,
            "sha256": sha256(package),
        },
        "runner": {
            "os": exporter.unsigned_canary.RUNNER_OS[platform],
            "architecture": target.architecture,
            "workflowRunId": 1234,
            "workflowRunAttempt": 1,
        },
        "smoke": {
            "status": exporter.unsigned_canary.STATUS,
            "command": f"synthetic {platform} packaged-app smoke",
        },
        "receiptGenerator": {
            "name": "Chaft unsigned-canary receipt generator",
            "version": "1",
        },
    }
    path = receipt_dir / exporter.VERIFICATION_RECEIPT_FILENAMES[target_name]
    write_json(path, receipt)
    return path


def write_canary_release_evidence(
    root: Path,
    *,
    package_dirs: dict[str, Path],
    receipts: dict[str, Path],
    version: str,
    tag: str,
    release_id: int,
) -> Path:
    directory = root / "release-evidence"
    directory.mkdir(parents=True)
    qt_source = directory / exporter.RELEASE_EVIDENCE_FILENAMES["qtSource"]
    qt_source.write_bytes(b"synthetic corresponding Qt source\n")
    qt_checksums = (
        directory / exporter.RELEASE_EVIDENCE_FILENAMES["qtSourceChecksums"]
    )
    qt_checksums.write_text(
        f"{sha256(qt_source)}  {qt_source.name}\n",
        encoding="utf-8",
    )

    release_assets = [
        path
        for package_dir in package_dirs.values()
        for path in package_dir.iterdir()
        if path.is_file()
    ]
    release_assets.extend(receipts.values())
    release_assets.extend((qt_source, qt_checksums))
    claims: dict[str, dict[str, str]] = {
        qt_source.name: {"kind": "qt-corresponding-source"},
        qt_checksums.name: {
            "kind": "qt-corresponding-source-checksum"
        },
    }
    metadata_kinds = {
        "checksums": "platform-checksums",
        "sbom": "sbom",
        "provenance": "provenance",
    }
    for target_name, package_dir in package_dirs.items():
        target = exporter.release_targets.TARGET_BY_NAME[target_name]
        target_claim = {
            "target": target_name,
            "platform": target.platform,
        }
        claims[target.package_name(version)] = {
            "kind": "package",
            **target_claim,
        }
        for key, filename in target.metadata_names.items():
            claims[filename] = {
                "kind": metadata_kinds[key],
                **target_claim,
            }
        claims[receipts[target_name].name] = {
            "kind": "unsigned-canary-verification",
            **target_claim,
        }
    inventory = {
        "schemaVersion": exporter.CANARY_INVENTORY_SCHEMA,
        "channel": "canary",
        "signingStatus": exporter.CANARY_SIGNING_STATE,
        "warning": exporter.unsigned_canary.WARNING,
        "repository": REPOSITORY,
        "version": version,
        "tag": tag,
        "commit": COMMIT,
        "releaseId": release_id,
        "assetCount": len(release_assets),
        "assets": [
            {
                "filename": path.name,
                "sizeBytes": path.stat().st_size,
                "sha256": sha256(path),
                **claims[path.name],
            }
            for path in sorted(release_assets, key=lambda item: item.name)
        ],
    }
    inventory_path = directory / exporter.RELEASE_EVIDENCE_FILENAMES["inventory"]
    write_json(inventory_path, inventory)
    aggregate_path = (
        directory / exporter.RELEASE_EVIDENCE_FILENAMES["aggregateChecksums"]
    )
    aggregate_assets = [*release_assets, inventory_path]
    aggregate_path.write_text(
        "".join(
            f"{sha256(path)}  {path.name}\n"
            for path in sorted(aggregate_assets, key=lambda item: item.name)
        ),
        encoding="utf-8",
    )
    return directory


def no_op_verifier(
    _package_dir: Path,
    _platform: str,
    _source_root: Path,
    _expected_commit: str,
) -> None:
    return None


class WebsiteReleaseManifestExportTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="chaft-release-export-test-")
        self.root = Path(self.temporary.name)
        self.package_dirs = {
            target_name: write_platform_package(self.root, target_name)
            for target_name in exporter.TARGETS
        }
        self.receipts = {
            WINDOWS_TARGET: write_verification_receipt(
                self.root, WINDOWS_TARGET, self.package_dirs[WINDOWS_TARGET]
            ),
            MACOS_X86_64_TARGET: write_verification_receipt(
                self.root,
                MACOS_X86_64_TARGET,
                self.package_dirs[MACOS_X86_64_TARGET],
            ),
            MACOS_ARM64_TARGET: write_verification_receipt(
                self.root,
                MACOS_ARM64_TARGET,
                self.package_dirs[MACOS_ARM64_TARGET],
            ),
            LINUX_TARGET: None,
        }
        trusted_root = self.root / "trusted-native-rerun"
        self.trusted_receipts = {
            WINDOWS_TARGET: write_verification_receipt(
                trusted_root, WINDOWS_TARGET, self.package_dirs[WINDOWS_TARGET]
            ),
            MACOS_X86_64_TARGET: write_verification_receipt(
                trusted_root,
                MACOS_X86_64_TARGET,
                self.package_dirs[MACOS_X86_64_TARGET],
            ),
            MACOS_ARM64_TARGET: write_verification_receipt(
                trusted_root,
                MACOS_ARM64_TARGET,
                self.package_dirs[MACOS_ARM64_TARGET],
            ),
            LINUX_TARGET: None,
        }
        self.architectures = {
            WINDOWS_TARGET: "amd64",
            MACOS_X86_64_TARGET: "x86_64",
            MACOS_ARM64_TARGET: "arm64",
            LINUX_TARGET: "x64",
        }
        self.signing_states = {
            WINDOWS_TARGET: "signed",
            MACOS_X86_64_TARGET: "notarized",
            MACOS_ARM64_TARGET: "notarized",
            LINUX_TARGET: "checksummed",
        }
        self.publisher_identities = {
            "windows": WINDOWS_SIGNER,
            "macos": APPLE_TEAM_ID,
            "linux": None,
        }

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def build(self, **overrides: object) -> dict[str, object]:
        arguments = {
            "repository": REPOSITORY,
            "tag": TAG,
            "source_root": self.root,
            "published_at": "2026-07-18T13:34:56+01:00",
            "channel": "stable",
            "package_directories": self.package_dirs,
            "architectures": self.architectures,
            "signing_states": self.signing_states,
            "verification_receipts": self.receipts,
            "trusted_verification_receipts": self.trusted_receipts,
            "publisher_identities": self.publisher_identities,
            "verifier": no_op_verifier,
            "tag_resolver": lambda _root, _tag: COMMIT,
        }
        arguments.update(overrides)
        return exporter.build_manifest(**arguments)

    def test_builds_schema_v2_manifest_from_local_verified_bytes(self) -> None:
        manifest = self.build()

        self.assertEqual(manifest["schemaVersion"], 2)
        self.assertEqual(manifest["status"], "published")
        self.assertEqual(manifest["version"], VERSION)
        self.assertEqual(manifest["commit"], COMMIT)
        self.assertEqual(manifest["publishedAt"], "2026-07-18T12:34:56Z")
        self.assertEqual(
            manifest["releaseUrl"],
            f"https://github.com/{REPOSITORY}/releases/tag/{TAG}",
        )
        assets = manifest["assets"]
        self.assertEqual(len(assets), 4)
        by_id = {asset["id"]: asset for asset in assets}
        windows = by_id[f"{WINDOWS_TARGET}-zip"]
        self.assertEqual(windows["format"], "zip")
        self.assertEqual(windows["arch"], "x86_64")
        self.assertEqual(
            windows["sha256"],
            sha256(
                self.package_dirs[WINDOWS_TARGET] / PACKAGE_NAMES[WINDOWS_TARGET]
            ),
        )
        self.assertTrue(
            windows["url"].endswith(f"/{PACKAGE_NAMES[WINDOWS_TARGET]}")
        )
        self.assertIsNotNone(windows["evidence"]["verification"])
        self.assertTrue(
            windows["evidence"]["checksums"]["url"].endswith(
                "/chaft-desktop-windows-x86_64-SHA256SUMS"
            )
        )
        self.assertEqual(by_id[f"{MACOS_X86_64_TARGET}-dmg"]["arch"], "x86_64")
        self.assertEqual(by_id[f"{MACOS_ARM64_TARGET}-dmg"]["arch"], "arm64")
        linux = by_id[f"{LINUX_TARGET}-appimage"]
        self.assertEqual(linux["signingStatus"], "checksummed")
        self.assertIsNone(linux["evidence"]["signature"])
        self.assertIsNone(linux["evidence"]["verification"])

    def test_builds_exact_unsigned_canary_manifest_with_release_evidence(self) -> None:
        version = f"{VERSION}-canary.1"
        tag = f"v{version}"
        canary_root = self.root / "canary"
        canary_root.mkdir()
        package_dirs = {
            target_name: write_platform_package(
                canary_root, target_name, version=version
            )
            for target_name in exporter.TARGETS
        }
        release_id = 4321
        receipts = {
            target_name: write_unsigned_canary_receipt(
                canary_root,
                target_name,
                package_dirs[target_name],
                version=version,
                tag=tag,
                release_id=release_id,
                asset_id=100 + index,
            )
            for index, target_name in enumerate(exporter.TARGETS)
        }
        release_evidence = write_canary_release_evidence(
            canary_root,
            package_dirs=package_dirs,
            receipts=receipts,
            version=version,
            tag=tag,
            release_id=release_id,
        )

        manifest = self.build(
            tag=tag,
            channel="canary",
            package_directories=package_dirs,
            signing_states={
                target_name: "unsigned-canary"
                for target_name in exporter.TARGETS
            },
            verification_receipts=receipts,
            trusted_verification_receipts={
                target_name: None for target_name in exporter.TARGETS
            },
            publisher_identities={
                platform: None for platform in exporter.PLATFORMS
            },
            release_evidence_directory=release_evidence,
        )

        self.assertEqual(manifest["channel"], "canary")
        self.assertEqual(manifest["tag"], tag)
        self.assertEqual(
            set(manifest["releaseEvidence"]),
            set(exporter.RELEASE_EVIDENCE_FILENAMES),
        )
        for asset in manifest["assets"]:
            self.assertEqual(asset["signingStatus"], "unsigned-canary")
            self.assertIsNone(asset["evidence"]["signature"])
            self.assertIsNotNone(asset["evidence"]["verification"])

    def test_canary_inventory_requires_exact_target_classification(self) -> None:
        version = f"{VERSION}-canary.1"
        tag = f"v{version}"
        for field, replacement in (
            ("kind", "sbom"),
            ("target", MACOS_X86_64_TARGET),
            ("platform", "windows"),
        ):
            with self.subTest(field=field):
                canary_root = self.root / f"inventory-{field}"
                canary_root.mkdir()
                package_dirs = {
                    target_name: write_platform_package(
                        canary_root,
                        target_name,
                        version=version,
                    )
                    for target_name in exporter.TARGETS
                }
                receipts = {
                    target_name: write_unsigned_canary_receipt(
                        canary_root,
                        target_name,
                        package_dirs[target_name],
                        version=version,
                        tag=tag,
                        release_id=4321,
                        asset_id=100 + index,
                    )
                    for index, target_name in enumerate(exporter.TARGETS)
                }
                release_evidence = write_canary_release_evidence(
                    canary_root,
                    package_dirs=package_dirs,
                    receipts=receipts,
                    version=version,
                    tag=tag,
                    release_id=4321,
                )
                inventory_path = (
                    release_evidence
                    / exporter.RELEASE_EVIDENCE_FILENAMES["inventory"]
                )
                inventory = json.loads(
                    inventory_path.read_text(encoding="utf-8")
                )
                row = next(
                    item
                    for item in inventory["assets"]
                    if item.get("target") == MACOS_ARM64_TARGET
                    and item.get("kind") == "package"
                )
                row[field] = replacement
                write_json(inventory_path, inventory)

                with self.assertRaisesRegex(
                    exporter.ManifestExportError,
                    f"inventory {field} is incoherent",
                ):
                    self.build(
                        tag=tag,
                        channel="canary",
                        package_directories=package_dirs,
                        signing_states={
                            target_name: "unsigned-canary"
                            for target_name in exporter.TARGETS
                        },
                        verification_receipts=receipts,
                        trusted_verification_receipts={
                            target_name: None
                            for target_name in exporter.TARGETS
                        },
                        publisher_identities={
                            platform: None
                            for platform in exporter.PLATFORMS
                        },
                        release_evidence_directory=release_evidence,
                    )

    def test_canary_channel_rejects_stable_tags_before_publication(self) -> None:
        with self.assertRaisesRegex(
            exporter.ManifestExportError, "exact vX.Y.Z-canary.N"
        ):
            self.build(channel="canary")

    def test_invokes_platform_metadata_verifier_for_every_directory(self) -> None:
        calls: list[tuple[Path, str]] = []

        def recording_verifier(
            package_dir: Path,
            platform: str,
            source_root: Path,
            expected_commit: str,
        ) -> None:
            self.assertEqual(source_root, self.root.resolve())
            self.assertEqual(expected_commit, COMMIT)
            calls.append((package_dir, platform))

        self.build(verifier=recording_verifier)
        self.assertEqual(
            calls,
            [
                (self.package_dirs[target_name].resolve(), target_name)
                for target_name in exporter.TARGETS
            ],
        )

    def test_rejects_package_bytes_changed_after_metadata_verification(self) -> None:
        package = self.package_dirs[LINUX_TARGET] / PACKAGE_NAMES[LINUX_TARGET]
        package.write_bytes(package.read_bytes() + b"tampered\n")
        with self.assertRaisesRegex(exporter.ManifestExportError, "metadata is stale"):
            self.build()

    def test_rejects_platform_version_and_tag_incoherence(self) -> None:
        self.package_dirs[LINUX_TARGET] = write_platform_package(
            self.root / "other-version", LINUX_TARGET, version="1.2.4"
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "versions do not agree"):
            self.build()

        self.package_dirs[LINUX_TARGET] = write_platform_package(
            self.root / "tag-version", LINUX_TARGET
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "requested tag"):
            self.build(tag="v1.2.4")

    def test_rejects_platform_commit_or_repository_incoherence(self) -> None:
        mismatched_commit = "f" * 40
        replacement_root = self.root / "other-commit"
        replacement_root.mkdir()
        self.package_dirs[LINUX_TARGET] = write_platform_package(
            replacement_root, LINUX_TARGET, commit=mismatched_commit
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "commits do not agree"):
            self.build()

        repository_root = self.root / "other-repository"
        repository_root.mkdir()
        self.package_dirs[LINUX_TARGET] = write_platform_package(
            repository_root,
            LINUX_TARGET,
            repository="https://github.com/example/not-chaft.git",
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "does not match requested"):
            self.build()

    def test_rejects_tag_target_commit_incoherence(self) -> None:
        with self.assertRaisesRegex(exporter.ManifestExportError, "Git tag target"):
            self.build(tag_resolver=lambda _root, _tag: "f" * 40)

        with self.assertRaisesRegex(exporter.ManifestExportError, "resolved Git tag commit"):
            self.build(tag_resolver=lambda _root, _tag: "not-a-commit")

    def test_resolves_annotated_release_tag_to_its_commit(self) -> None:
        source_root = self.root / "tagged-source"
        source_root.mkdir()
        subprocess.run(["git", "init", "-q", str(source_root)], check=True)
        subprocess.run(
            ["git", "-C", str(source_root), "config", "user.name", "Chaft Test"],
            check=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(source_root),
                "config",
                "user.email",
                "test@chaft.invalid",
            ],
            check=True,
        )
        (source_root / "release.txt").write_text("release input\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(source_root), "add", "release.txt"], check=True)
        subprocess.run(
            ["git", "-C", str(source_root), "commit", "-q", "-m", "Release input"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(source_root), "tag", "-a", TAG, "-m", "Release tag"],
            check=True,
        )
        expected = subprocess.run(
            ["git", "-C", str(source_root), "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

        self.assertEqual(exporter.resolve_git_tag_commit(source_root, TAG), expected)

    def test_native_signing_states_require_coherent_verification_receipts(self) -> None:
        receipts = dict(self.receipts)
        receipts[WINDOWS_TARGET] = None
        with self.assertRaisesRegex(exporter.ManifestExportError, "requires.*receipt"):
            self.build(verification_receipts=receipts)

        trusted_receipts = dict(self.trusted_receipts)
        trusted_receipts[WINDOWS_TARGET] = None
        with self.assertRaisesRegex(
            exporter.ManifestExportError, "requires a trusted native.*receipt"
        ):
            self.build(trusted_verification_receipts=trusted_receipts)

        receipts[WINDOWS_TARGET] = write_verification_receipt(
            self.root,
            WINDOWS_TARGET,
            self.package_dirs[WINDOWS_TARGET],
            stale_hash=True,
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "exact verified package set"):
            self.build(verification_receipts=receipts)

        receipts[WINDOWS_TARGET] = write_verification_receipt(
            self.root,
            WINDOWS_TARGET,
            self.package_dirs[WINDOWS_TARGET],
            architecture="arm64",
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "architecture does not match"):
            self.build(verification_receipts=receipts)

    def test_rejects_public_receipt_with_forged_native_verification_details(self) -> None:
        public_path = self.receipts[WINDOWS_TARGET]
        assert public_path is not None
        public = json.loads(public_path.read_text(encoding="utf-8"))
        # Preserve the package, tag, architecture, and pinned publisher identity while
        # forging a native verifier result that the trusted rerun did not observe.
        public["verificationDetails"][0]["verifiedPayloads"][0][
            "signatureType"
        ] = "ForgedCatalog"
        write_json(public_path, public)

        with self.assertRaisesRegex(
            exporter.ManifestExportError,
            "security-relevant claim 'verificationDetails'.*trusted native",
        ):
            self.build()

    def test_manifest_links_public_receipt_and_allows_operational_rerun_differences(
        self,
    ) -> None:
        trusted_path = self.trusted_receipts[WINDOWS_TARGET]
        public_path = self.receipts[WINDOWS_TARGET]
        assert trusted_path is not None and public_path is not None
        trusted = json.loads(trusted_path.read_text(encoding="utf-8"))
        trusted["verifiedAt"] = "2026-07-18T12:45:00Z"
        trusted["verifier"] = {
            "name": "trusted-native-rerun-verifier",
            "version": "2.0",
        }
        trusted["receiptGenerator"] = {
            "name": "trusted-native-rerun-generator",
            "version": "2.0",
        }
        write_json(trusted_path, trusted)

        manifest = self.build()

        windows = next(asset for asset in manifest["assets"] if asset["os"] == "windows")
        evidence = windows["evidence"]["verification"]
        self.assertEqual(evidence["sha256"], sha256(public_path))
        self.assertNotEqual(evidence["sha256"], sha256(trusted_path))
        self.assertTrue(evidence["url"].endswith(f"/{public_path.name}"))

    def test_detached_sidecar_alone_does_not_claim_linux_signed(self) -> None:
        signed_root = self.root / "signed-linux"
        signed_root.mkdir()
        self.package_dirs[LINUX_TARGET] = write_platform_package(
            signed_root, LINUX_TARGET, with_signature=True
        )
        with self.assertRaisesRegex(
            exporter.ManifestExportError,
            "checksummed state must not include detached signatures",
        ):
            self.build()

        signing_states = dict(self.signing_states)
        signing_states[LINUX_TARGET] = "signed"
        with self.assertRaisesRegex(exporter.ManifestExportError, "requires.*receipt"):
            self.build(signing_states=signing_states)

        receipts = dict(self.receipts)
        receipts[LINUX_TARGET] = write_verification_receipt(
            self.root, LINUX_TARGET, self.package_dirs[LINUX_TARGET]
        )
        trusted_receipts = dict(self.trusted_receipts)
        trusted_receipts[LINUX_TARGET] = write_verification_receipt(
            self.root / "trusted-signed-linux",
            LINUX_TARGET,
            self.package_dirs[LINUX_TARGET],
        )
        manifest = self.build(
            signing_states=signing_states,
            verification_receipts=receipts,
            trusted_verification_receipts=trusted_receipts,
            publisher_identities={
                **self.publisher_identities,
                "linux": LINUX_SIGNER,
            },
        )
        linux = next(asset for asset in manifest["assets"] if asset["os"] == "linux")
        self.assertEqual(linux["signingStatus"], "signed")
        self.assertEqual(linux["evidence"]["signature"]["format"], "sig")
        self.assertIsNotNone(linux["evidence"]["verification"])

        signature_path = (
            self.package_dirs[LINUX_TARGET] / f"{PACKAGE_NAMES[LINUX_TARGET]}.sig"
        )
        signature_path.write_bytes(b"substituted signature bytes\n")
        names = exporter.METADATA_FILENAMES[LINUX_TARGET]
        provenance_path = self.package_dirs[LINUX_TARGET] / names["provenance"]
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        signature_row = next(
            row
            for row in provenance["artifacts"]
            if row["name"] == signature_path.name
        )
        signature_row["sizeBytes"] = signature_path.stat().st_size
        signature_row["sha256"] = sha256(signature_path)
        write_json(provenance_path, provenance)
        checksum_path = self.package_dirs[LINUX_TARGET] / names["checksums"]
        checksum_path.write_text(
            "".join(
                f"{row['sha256']}  {row['name']}\n"
                for row in provenance["artifacts"]
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "exact detached-signature"):
            self.build(
                signing_states=signing_states,
                verification_receipts=receipts,
                trusted_verification_receipts=trusted_receipts,
                publisher_identities={
                    **self.publisher_identities,
                    "linux": LINUX_SIGNER,
                },
            )

    def test_receipt_publisher_identity_must_match_protected_policy(self) -> None:
        identities = dict(self.publisher_identities)
        identities["windows"] = "D" * 40
        with self.assertRaisesRegex(exporter.ManifestExportError, "protected release policy"):
            self.build(publisher_identities=identities)

    def test_rejects_format_metadata_that_disagrees_with_filename(self) -> None:
        replacement_root = self.root / "wrong-format"
        replacement_root.mkdir()
        self.package_dirs[WINDOWS_TARGET] = write_platform_package(
            replacement_root,
            WINDOWS_TARGET,
            provenance_package_format="windows-msi",
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "packageFormat is incoherent"):
            self.build()

    def test_rejects_non_rfc3339_publication_timestamp(self) -> None:
        with self.assertRaisesRegex(exporter.ManifestExportError, "RFC 3339"):
            self.build(published_at="2026-07-18 12:34:56")

    def test_linux_checksummed_architecture_is_bound_to_appimage_payload(self) -> None:
        architectures = dict(self.architectures)
        architectures[LINUX_TARGET] = "arm64"
        with self.assertRaisesRegex(
            exporter.ManifestExportError,
            "architecture must be x86_64",
        ):
            self.build(architectures=architectures)

    def test_linux_checksummed_architecture_must_also_match_provenance(self) -> None:
        provenance_path = (
            self.package_dirs[LINUX_TARGET]
            / exporter.METADATA_FILENAMES[LINUX_TARGET]["provenance"]
        )
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["platform"]["machine"] = "arm64"
        write_json(provenance_path, provenance)
        with self.assertRaisesRegex(
            exporter.ManifestExportError,
            "provenance host architecture",
        ):
            self.build()

    def test_linux_appimage_architecture_is_read_from_elf_header(self) -> None:
        appimage = self.root / "architecture.AppImage"
        appimage.write_bytes(elf_payload(machine=183))
        self.assertEqual(exporter.linux_package_architectures(appimage), ["arm64"])

        appimage.write_bytes(elf_payload(machine=62, encoding=2))
        self.assertEqual(exporter.linux_package_architectures(appimage), ["x86_64"])

    def test_linux_tar_inspects_every_elf_and_rejects_mixed_architectures(self) -> None:
        archive = self.root / "architecture.tar.gz"
        write_tar(
            archive,
            [
                ("chaft/bin/chaft", elf_payload(machine=62)),
                ("chaft/lib/helper", elf_payload(machine=62)),
                ("chaft/README.txt", b"documentation\n"),
            ],
        )
        self.assertEqual(
            exporter.linux_package_architectures(archive),
            ["x86_64", "x86_64"],
        )

        write_tar(
            archive,
            [
                ("chaft/bin/chaft", elf_payload(machine=62)),
                ("chaft/bin/helper", elf_payload(machine=183)),
            ],
        )
        artifact = exporter.Artifact(
            name=archive.name,
            package_format="linux-tgz",
            website_format="tar.gz",
            size_bytes=archive.stat().st_size,
            sha256=sha256(archive),
        )
        with self.assertRaisesRegex(exporter.ManifestExportError, "mixed payload"):
            exporter.verify_linux_package_architectures(
                self.root, [artifact], "x86_64"
            )

    def test_linux_payload_inspection_rejects_malformed_or_unsupported_elf(self) -> None:
        appimage = self.root / "malformed.AppImage"
        appimage.write_bytes(b"not an ELF AppImage")
        with self.assertRaisesRegex(exporter.ManifestExportError, "invalid header"):
            exporter.linux_package_architectures(appimage)

        appimage.write_bytes(elf_payload(machine=3))
        with self.assertRaisesRegex(exporter.ManifestExportError, "unsupported machine"):
            exporter.linux_package_architectures(appimage)

        appimage.write_bytes(elf_payload(machine=62, encoding=0))
        with self.assertRaisesRegex(exporter.ManifestExportError, "invalid byte order"):
            exporter.linux_package_architectures(appimage)

    def test_linux_tar_rejects_invalid_unsafe_and_elf_free_archives(self) -> None:
        archive = self.root / "unsafe.tar.gz"
        archive.write_bytes(b"not a gzip-compressed tar archive")
        with self.assertRaisesRegex(exporter.ManifestExportError, "not a valid"):
            exporter.linux_package_architectures(archive)

        write_tar(archive, [("../escape", elf_payload())])
        with self.assertRaisesRegex(exporter.ManifestExportError, "unsafe entry"):
            exporter.linux_package_architectures(archive)

        write_tar(archive, [("chaft/README.txt", b"documentation\n")])
        with self.assertRaisesRegex(exporter.ManifestExportError, "no ELF payload"):
            exporter.linux_package_architectures(archive)

    def test_linux_tar_enforces_entry_and_expansion_limits(self) -> None:
        archive = self.root / "limits.tgz"
        write_tar(archive, [("chaft/bin/chaft", elf_payload())])
        with mock.patch.object(exporter, "MAX_TAR_ENTRY_BYTES", 8):
            with self.assertRaisesRegex(exporter.ManifestExportError, "entry is too large"):
                exporter.linux_package_architectures(archive)

        write_tar(
            archive,
            [
                ("chaft/bin/chaft", elf_payload()),
                ("chaft/README.txt", b"documentation\n"),
            ],
        )
        with mock.patch.object(exporter, "MAX_TAR_TOTAL_BYTES", 64):
            with self.assertRaisesRegex(exporter.ManifestExportError, "expands beyond"):
                exporter.linux_package_architectures(archive)

    def test_tar_gz_suffix_has_an_explicit_website_format(self) -> None:
        self.assertEqual(
            exporter.package_description(f"Chaft-{VERSION}-Linux.tar.gz"),
            ("linux", "linux-tgz", "tar.gz"),
        )

    def test_publishing_archives_previous_published_current_manifest(self) -> None:
        output = self.root / "data" / "release-manifest.json"
        output.parent.mkdir()
        previous = {
            "schemaVersion": 2,
            "channel": "stable",
            "status": "published",
            "version": "1.2.2",
            "tag": "v1.2.2",
        }
        write_json(output, previous)
        manifest = self.build()

        exporter.publish_manifest(output, manifest)

        self.assertEqual(json.loads(output.read_text(encoding="utf-8")), manifest)
        archive = output.parent / "release-history" / "1.2.2.json"
        self.assertEqual(json.loads(archive.read_text(encoding="utf-8")), previous)
        self.assertFalse(list(output.parent.glob(".release-manifest.json.*.tmp")))

    def test_refuses_to_mutate_an_existing_published_version(self) -> None:
        output = self.root / "release-manifest.json"
        manifest = self.build()
        changed = dict(manifest)
        changed["channel"] = "canary"
        write_json(output, changed)
        with self.assertRaisesRegex(exporter.ManifestExportError, "refusing to mutate"):
            exporter.publish_manifest(output, manifest)

    def test_stable_current_is_not_displaced_by_canary(self) -> None:
        output = self.root / "data" / "release-manifest.json"
        output.parent.mkdir()
        stable = dict(self.build())
        stable["version"] = "1.2.2"
        stable["tag"] = "v1.2.2"
        stable["channel"] = "stable"
        write_json(output, stable)
        canary = dict(self.build())
        canary["channel"] = "canary"
        canary["version"] = f"{VERSION}-canary.1"
        canary["tag"] = f"{TAG}-canary.1"

        exporter.publish_manifest(output, canary)

        self.assertEqual(json.loads(output.read_text(encoding="utf-8")), stable)
        archived = output.parent / "release-history" / f"{VERSION}-canary.1.json"
        self.assertEqual(json.loads(archived.read_text(encoding="utf-8")), canary)

    def test_refuses_same_channel_downgrade_and_republished_history_mutation(self) -> None:
        output = self.root / "data" / "release-manifest.json"
        history = output.parent / "release-history"
        history.mkdir(parents=True)
        current = dict(self.build())
        current["version"] = "2.0.0"
        current["tag"] = "v2.0.0"
        write_json(output, current)

        with self.assertRaisesRegex(exporter.ManifestExportError, "non-newer"):
            exporter.publish_manifest(output, self.build())

        archived = dict(self.build())
        archived["channel"] = "canary"
        archived["version"] = f"{VERSION}-canary.1"
        archived["tag"] = f"{TAG}-canary.1"
        write_json(history / f"{VERSION}-canary.1.json", archived)
        changed = dict(archived)
        changed["publishedAt"] = "2026-07-19T12:34:56Z"
        with self.assertRaisesRegex(exporter.ManifestExportError, "refusing to mutate"):
            exporter.publish_manifest(output, changed)

    def test_semantic_version_precedence_matches_semver_rules(self) -> None:
        ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ]
        for older, newer in zip(ordered, ordered[1:]):
            self.assertLess(exporter.compare_semantic_versions(older, newer), 0)
        self.assertEqual(
            exporter.compare_semantic_versions("1.0.0+build.1", "1.0.0+build.2"),
            0,
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
