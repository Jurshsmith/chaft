#!/usr/bin/env python3
"""Stage downloaded GitHub Release assets for the website manifest exporter.

The input is a flat, local directory containing assets downloaded individually from
one GitHub Release. No network access is performed. Platform-qualified provenance is
the authority for installer and detached-signature filenames; checksums and file
sizes are verified before any completed output becomes visible.

The resulting layout is::

    OUTPUT/
      windows-package/
      macos-package/
      linux-package/
      verification-receipts/

Pass the three package directories, plus the applicable receipt files, to
``export-website-release-manifest.py``.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence


PLATFORMS = ("windows", "macos", "linux")
NATIVE_RECEIPT_PLATFORMS = frozenset(("windows", "macos"))
PACKAGE_DIRECTORY_NAMES = {
    platform: f"{platform}-package" for platform in PLATFORMS
}
METADATA_FILENAMES = {
    platform: {
        "checksums": f"chaft-desktop-{platform}-SHA256SUMS",
        "sbom": f"chaft-desktop-{platform}-sbom.cdx.json",
        "provenance": f"chaft-desktop-{platform}-provenance.json",
    }
    for platform in PLATFORMS
}
VERIFICATION_RECEIPT_FILENAMES = {
    platform: f"chaft-desktop-{platform}-verification.json"
    for platform in PLATFORMS
}
PACKAGE_SUFFIXES = (
    (".tar.gz", "linux", "linux-tgz"),
    (".appimage", "linux", "linux-appimage"),
    (".tgz", "linux", "linux-tgz"),
    (".dmg", "macos", "macos-dmg"),
    (".zip", "windows", "windows-zip"),
    (".msi", "windows", "windows-msi"),
    (".exe", "windows", "windows-exe"),
)
SIGNATURE_SUFFIXES = (".sig", ".asc")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
AUTHENTICODE_THUMBPRINT_PATTERN = re.compile(r"^(?:[0-9A-F]{40}|[0-9A-F]{64})$")
APPLE_TEAM_ID_PATTERN = re.compile(r"^[A-Z0-9]{10}$")
OPENPGP_FINGERPRINT_PATTERN = re.compile(r"^(?:[0-9A-F]{40}|[0-9A-F]{64})$")


class AssetStagingError(ValueError):
    """Downloaded release assets do not form one safe, coherent release set."""


@dataclass(frozen=True)
class Fingerprint:
    name: str
    path: Path
    size_bytes: int
    sha256: str


@dataclass(frozen=True)
class PlatformStagePlan:
    platform: str
    package_files: tuple[Fingerprint, ...]
    receipt: Fingerprint | None


@dataclass(frozen=True)
class StagePlan:
    source_directory: Path
    platforms: Mapping[str, PlatformStagePlan]


def fail(message: str) -> None:
    raise AssetStagingError(message)


def safe_filename(value: object, context: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{context} must be a non-empty filename")
    if (
        value in {".", ".."}
        or Path(value).name != value
        or "/" in value
        or "\\" in value
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        fail(f"{context} must be a plain filename without traversal: {value!r}")
    return value


def package_description(name: str) -> tuple[str, str] | None:
    lowered = name.lower()
    for suffix, platform, package_format in PACKAGE_SUFFIXES:
        if lowered.endswith(suffix):
            return platform, package_format
    return None


def signature_description(name: str) -> tuple[str, str] | None:
    lowered = name.lower()
    for suffix in SIGNATURE_SUFFIXES:
        if lowered.endswith(suffix):
            return name[: -len(suffix)], suffix[1:]
    return None


def is_managed_release_name(name: str) -> bool:
    metadata_names = {
        filename
        for names in METADATA_FILENAMES.values()
        for filename in names.values()
    }
    return (
        name in metadata_names
        or name in VERIFICATION_RECEIPT_FILENAMES.values()
        or name.startswith("chaft-desktop-")
        or package_description(name) is not None
        or signature_description(name) is not None
    )


def _open_readonly_no_follow(path: Path) -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_BINARY"):
        flags |= os.O_BINARY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open release asset {path.name}: {error}")
    details = os.fstat(descriptor)
    if not stat.S_ISREG(details.st_mode):
        os.close(descriptor)
        fail(f"release asset must be a regular file: {path.name}")
    if details.st_size <= 0:
        os.close(descriptor)
        fail(f"release asset is empty: {path.name}")
    return descriptor


def fingerprint_file(path: Path) -> Fingerprint:
    descriptor = _open_readonly_no_follow(path)
    digest = hashlib.sha256()
    size_bytes = 0
    try:
        with os.fdopen(descriptor, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
                size_bytes += len(chunk)
    except OSError as error:
        fail(f"cannot read release asset {path.name}: {error}")
    return Fingerprint(
        name=path.name,
        path=path,
        size_bytes=size_bytes,
        sha256=digest.hexdigest(),
    )


def load_json_object(path: Path, context: str) -> tuple[dict[str, object], Fingerprint]:
    fingerprint = fingerprint_file(path)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        fail(f"cannot read {context} {path.name}: {error}")
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{context} {path.name} is not valid UTF-8 JSON: {error}")
    if not isinstance(value, dict):
        fail(f"{context} {path.name} must contain a JSON object")
    # Detect a source change between the fingerprinted read and JSON parsing.
    if fingerprint_file(path) != fingerprint:
        fail(f"release asset changed while it was being read: {path.name}")
    return value, fingerprint


def parse_checksums(path: Path) -> tuple[dict[str, str], Fingerprint]:
    fingerprint = fingerprint_file(path)
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"cannot read checksum metadata {path.name}: {error}")
    except UnicodeDecodeError as error:
        fail(f"checksum metadata {path.name} is not UTF-8: {error}")
    if fingerprint_file(path) != fingerprint:
        fail(f"release asset changed while it was being read: {path.name}")

    rows: dict[str, str] = {}
    for line_number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-f]{64})  ([^/\\\r\n]+)", line)
        if match is None:
            fail(f"invalid {path.name} line {line_number}: {line!r}")
        digest, raw_name = match.groups()
        name = safe_filename(raw_name, f"{path.name} line {line_number} filename")
        if name in rows:
            fail(f"duplicate {path.name} row for {name}")
        rows[name] = digest
    if not rows:
        fail(f"{path.name} must contain at least one artifact checksum")
    return rows, fingerprint


def scan_flat_directory(source_directory: Path) -> dict[str, Path]:
    if source_directory.is_symlink():
        fail(f"assets directory must not be a symbolic link: {source_directory}")
    if not source_directory.is_dir():
        fail(f"assets directory not found: {source_directory}")

    files: dict[str, Path] = {}
    casefolded: dict[str, str] = {}
    for path in sorted(source_directory.iterdir(), key=lambda item: item.name):
        name = safe_filename(path.name, "release asset name")
        if path.is_symlink():
            fail(f"release asset must not be a symbolic link: {name}")
        if not path.is_file():
            fail(f"assets directory must be flat; unexpected entry: {name}")
        folded = name.casefold()
        previous = casefolded.get(folded)
        if previous is not None:
            fail(f"case-insensitive duplicate release assets: {previous}, {name}")
        casefolded[folded] = name
        files[name] = path
    if not files:
        fail(f"assets directory is empty: {source_directory}")
    return files


def require_source_file(
    source_files: Mapping[str, Path], name: str, context: str
) -> Path:
    path = source_files.get(name)
    if path is None:
        fail(f"missing {context}: {name}")
    return path


def sbom_platform(sbom: Mapping[str, object], context: str) -> str:
    metadata = sbom.get("metadata")
    if not isinstance(metadata, dict):
        fail(f"{context}.metadata must be an object")
    properties = metadata.get("properties")
    if not isinstance(properties, list):
        fail(f"{context}.metadata.properties must be an array")
    values: list[object] = []
    for item in properties:
        if isinstance(item, dict) and item.get("name") == "chaft:packagePlatform":
            values.append(item.get("value"))
    if len(values) != 1 or not isinstance(values[0], str):
        fail(f"{context} must declare exactly one chaft:packagePlatform property")
    return values[0]


def provenance_artifacts(
    provenance: Mapping[str, object], platform: str
) -> tuple[dict[str, dict[str, object]], set[str]]:
    context = f"{platform} provenance"
    if provenance.get("schemaVersion") != "chaft.desktop.provenance.v1":
        fail(f"{context} schemaVersion is unsupported")
    if provenance.get("profile") != "release":
        fail(f"{context} profile must be 'release'")
    if provenance.get("packagePlatform") != platform:
        fail(f"{context} packagePlatform does not match {platform}")
    raw_rows = provenance.get("artifacts")
    if not isinstance(raw_rows, list) or not raw_rows:
        fail(f"{context}.artifacts must be a non-empty array")

    rows: dict[str, dict[str, object]] = {}
    packages: set[str] = set()
    signatures: dict[str, str] = {}
    for index, value in enumerate(raw_rows):
        row_context = f"{context}.artifacts[{index}]"
        if not isinstance(value, dict):
            fail(f"{row_context} must be an object")
        name = safe_filename(value.get("name"), f"{row_context}.name")
        if name in rows:
            fail(f"duplicate {context} artifact row for {name}")
        digest = value.get("sha256")
        if not isinstance(digest, str) or SHA256_PATTERN.fullmatch(digest) is None:
            fail(f"{row_context}.sha256 must be a lowercase SHA-256 digest")
        size_bytes = value.get("sizeBytes")
        if (
            not isinstance(size_bytes, int)
            or isinstance(size_bytes, bool)
            or size_bytes <= 0
        ):
            fail(f"{row_context}.sizeBytes must be a positive integer")

        package = package_description(name)
        signature = signature_description(name)
        if package is not None:
            artifact_platform, expected_format = package
            if artifact_platform != platform:
                fail(
                    f"{context} references a {artifact_platform} package: {name}"
                )
            if value.get("packageFormat") != expected_format:
                fail(f"{row_context}.packageFormat is incoherent for {name}")
            packages.add(name)
        elif signature is not None:
            signed_artifact, signature_format = signature
            safe_filename(signed_artifact, f"{row_context}.signedArtifact filename")
            if value.get("packageFormat") != "detached-signature":
                fail(f"{row_context}.packageFormat must be 'detached-signature'")
            if value.get("signedArtifact") != signed_artifact:
                fail(f"{row_context}.signedArtifact is incoherent for {name}")
            if value.get("signatureFormat") != signature_format:
                fail(f"{row_context}.signatureFormat is incoherent for {name}")
            if signed_artifact in signatures:
                fail(f"multiple detached signatures reference {signed_artifact}")
            signatures[signed_artifact] = name
        else:
            fail(f"{context} contains an unsupported artifact filename: {name}")
        rows[name] = value

    if not packages:
        fail(f"{context} contains no installable package")
    orphaned = sorted(set(signatures) - packages)
    if orphaned:
        fail(
            f"{context} detached signatures reference missing packages: "
            + ", ".join(orphaned)
        )
    return rows, packages


def validate_receipt(
    receipt: Mapping[str, object],
    platform: str,
    package_fingerprints: Mapping[str, Fingerprint],
    signature_fingerprints: Mapping[str, Fingerprint],
) -> None:
    context = f"{platform} verification receipt"
    if receipt.get("schemaVersion") != "chaft.desktop.platform-verification.v1":
        fail(f"{context} schemaVersion is unsupported")
    if receipt.get("platform") != platform:
        fail(f"{context} platform does not match its platform-qualified filename")
    if receipt.get("status") != "verified":
        fail(f"{context} status must be 'verified'")

    policy = receipt.get("verificationPolicy")
    if not isinstance(policy, dict):
        fail(f"{context}.verificationPolicy must be an object")
    identity = policy.get("publisherIdentity")
    if not isinstance(identity, dict):
        fail(f"{context}.verificationPolicy.publisherIdentity must be an object")
    identity_type = {
        "windows": "authenticode-signer-certificate-thumbprint",
        "macos": "apple-developer-team-id",
        "linux": "openpgp-primary-key-fingerprint",
    }[platform]
    if identity.get("type") != identity_type:
        fail(f"{context} publisher identity type must be {identity_type!r}")
    identity_value = identity.get("value")
    if not isinstance(identity_value, str):
        fail(f"{context} publisher identity value must be a string")
    if platform == "windows":
        if AUTHENTICODE_THUMBPRINT_PATTERN.fullmatch(identity_value) is None:
            fail(f"{context} Authenticode thumbprint is invalid")
        expected_algorithm = "sha1" if len(identity_value) == 40 else "sha256"
        if identity.get("algorithm") != expected_algorithm:
            fail(f"{context} Authenticode algorithm must be {expected_algorithm}")
    elif platform == "macos":
        if APPLE_TEAM_ID_PATTERN.fullmatch(identity_value) is None:
            fail(f"{context} Apple Developer Team ID is invalid")
    else:
        if OPENPGP_FINGERPRINT_PATTERN.fullmatch(identity_value) is None:
            fail(f"{context} OpenPGP publisher fingerprint is invalid")
        trusted_keyring = policy.get("trustedKeyring")
        if not isinstance(trusted_keyring, dict):
            fail(f"{context}.verificationPolicy.trustedKeyring must be an object")
        keyring_name = safe_filename(
            trusted_keyring.get("filename"),
            f"{context}.verificationPolicy.trustedKeyring.filename",
        )
        keyring_digest = trusted_keyring.get("sha256")
        if not isinstance(keyring_digest, str) or SHA256_PATTERN.fullmatch(keyring_digest) is None:
            fail(f"{context} trusted keyring SHA-256 is invalid")
        if not keyring_name:
            fail(f"{context} trusted keyring filename is invalid")
    raw_rows = receipt.get("artifacts")
    if not isinstance(raw_rows, list) or not raw_rows:
        fail(f"{context}.artifacts must be a non-empty array")
    rows: dict[str, str] = {}
    for index, value in enumerate(raw_rows):
        row_context = f"{context}.artifacts[{index}]"
        if not isinstance(value, dict):
            fail(f"{row_context} must be an object")
        name = safe_filename(value.get("filename"), f"{row_context}.filename")
        digest = value.get("sha256")
        if not isinstance(digest, str) or SHA256_PATTERN.fullmatch(digest) is None:
            fail(f"{row_context}.sha256 must be a lowercase SHA-256 digest")
        if name in rows:
            fail(f"{context} has duplicate artifact receipt for {name}")
        rows[name] = digest
    expected = {
        name: fingerprint.sha256
        for name, fingerprint in package_fingerprints.items()
    }
    if rows != expected:
        fail(f"{context} does not cover the exact package set")

    raw_signatures = receipt.get("signatures")
    if not isinstance(raw_signatures, list):
        fail(f"{context}.signatures must be an array")
    if platform != "linux":
        if raw_signatures:
            fail(f"{context}.signatures must be empty for embedded-signature platforms")
        return

    actual_signatures: dict[str, tuple[str, str]] = {}
    for index, value in enumerate(raw_signatures):
        row_context = f"{context}.signatures[{index}]"
        if not isinstance(value, dict):
            fail(f"{row_context} must be an object")
        filename = safe_filename(value.get("filename"), f"{row_context}.filename")
        signed_artifact = safe_filename(
            value.get("signedArtifact"), f"{row_context}.signedArtifact"
        )
        digest = value.get("sha256")
        signer = value.get("signerFingerprint")
        trusted = value.get("trustedFingerprint")
        if not isinstance(digest, str) or SHA256_PATTERN.fullmatch(digest) is None:
            fail(f"{row_context}.sha256 is invalid")
        if not isinstance(signer, str) or OPENPGP_FINGERPRINT_PATTERN.fullmatch(signer) is None:
            fail(f"{row_context}.signerFingerprint is invalid")
        if trusted != identity_value:
            fail(f"{row_context}.trustedFingerprint does not match publisher policy")
        if signed_artifact in actual_signatures:
            fail(f"{context} has duplicate signature evidence for {signed_artifact}")
        actual_signatures[signed_artifact] = (filename, digest)

    expected_signatures: dict[str, tuple[str, str]] = {}
    for filename, fingerprint in signature_fingerprints.items():
        description = signature_description(filename)
        if description is None:
            fail(f"{context} received a non-signature artifact: {filename}")
        signed_artifact, _signature_format = description
        expected_signatures[signed_artifact] = (filename, fingerprint.sha256)
    if actual_signatures != expected_signatures:
        fail(f"{context} does not bind the exact detached-signature set")


def build_stage_plan(
    assets_directory: Path,
    *,
    allowed_extra_assets: Sequence[str] = (),
) -> StagePlan:
    original_source = Path(assets_directory)
    source_directory = original_source.resolve()
    source_files = scan_flat_directory(original_source)
    plans: dict[str, PlatformStagePlan] = {}
    expected_names: set[str] = set()
    artifact_owners: dict[str, str] = {}

    for platform in PLATFORMS:
        names = METADATA_FILENAMES[platform]
        provenance_path = require_source_file(
            source_files, names["provenance"], f"{platform} provenance metadata"
        )
        checksums_path = require_source_file(
            source_files, names["checksums"], f"{platform} checksum metadata"
        )
        sbom_path = require_source_file(
            source_files, names["sbom"], f"{platform} SBOM metadata"
        )

        provenance, provenance_fingerprint = load_json_object(
            provenance_path, f"{platform} provenance"
        )
        rows, package_names = provenance_artifacts(provenance, platform)
        checksums, checksums_fingerprint = parse_checksums(checksums_path)
        if set(checksums) != set(rows):
            missing = sorted(set(rows) - set(checksums))
            extra = sorted(set(checksums) - set(rows))
            details: list[str] = []
            if missing:
                details.append("missing: " + ", ".join(missing))
            if extra:
                details.append("extra: " + ", ".join(extra))
            fail(
                f"{platform} provenance/checksum artifact sets differ "
                f"({'; '.join(details)})"
            )

        sbom, sbom_fingerprint = load_json_object(sbom_path, f"{platform} SBOM")
        if sbom.get("bomFormat") != "CycloneDX":
            fail(f"{platform} SBOM bomFormat must be CycloneDX")
        if sbom_platform(sbom, f"{platform} SBOM") != platform:
            fail(f"{platform} SBOM package platform does not match its filename")

        artifact_fingerprints: dict[str, Fingerprint] = {}
        for name, row in rows.items():
            owner = artifact_owners.get(name)
            if owner is not None:
                fail(
                    f"release artifact filename {name!r} is referenced by both "
                    f"{owner} and {platform} provenance"
                )
            artifact_owners[name] = platform
            path = require_source_file(
                source_files, name, f"{platform} provenance artifact"
            )
            fingerprint = fingerprint_file(path)
            if fingerprint.sha256 != row["sha256"]:
                fail(f"{platform} provenance SHA-256 is stale for {name}")
            if fingerprint.sha256 != checksums[name]:
                fail(f"{platform} checksum is stale for {name}")
            if fingerprint.size_bytes != row["sizeBytes"]:
                fail(f"{platform} provenance sizeBytes is stale for {name}")
            artifact_fingerprints[name] = fingerprint

        receipt_name = VERIFICATION_RECEIPT_FILENAMES[platform]
        receipt_path = source_files.get(receipt_name)
        if receipt_path is None and platform in NATIVE_RECEIPT_PLATFORMS:
            fail(f"missing {platform} native verification receipt: {receipt_name}")
        receipt_fingerprint = None
        if receipt_path is not None:
            receipt, receipt_fingerprint = load_json_object(
                receipt_path, f"{platform} verification receipt"
            )
            validate_receipt(
                receipt,
                platform,
                {
                    name: artifact_fingerprints[name]
                    for name in package_names
                },
                {
                    name: fingerprint
                    for name, fingerprint in artifact_fingerprints.items()
                    if signature_description(name) is not None
                },
            )

        package_files = (
            checksums_fingerprint,
            sbom_fingerprint,
            provenance_fingerprint,
            *(
                artifact_fingerprints[name]
                for name in sorted(artifact_fingerprints)
            ),
        )
        expected_names.update(item.name for item in package_files)
        if receipt_fingerprint is not None:
            expected_names.add(receipt_fingerprint.name)
        plans[platform] = PlatformStagePlan(
            platform=platform,
            package_files=tuple(
                sorted(package_files, key=lambda item: item.name)
            ),
            receipt=receipt_fingerprint,
        )

    allowed = set()
    for raw_name in allowed_extra_assets:
        name = safe_filename(raw_name, "allowed extra asset")
        if name in allowed:
            fail(f"duplicate --allow-extra-asset value: {name}")
        if is_managed_release_name(name):
            fail(f"managed release asset cannot be allowlisted as extra: {name}")
        if name not in source_files:
            fail(f"allowlisted extra asset was not downloaded: {name}")
        allowed.add(name)

    unexpected = sorted(set(source_files) - expected_names - allowed)
    if unexpected:
        fail("unexpected release asset(s): " + ", ".join(unexpected))
    return StagePlan(source_directory=source_directory, platforms=plans)


def copy_fingerprinted_file(source: Fingerprint, destination: Path) -> None:
    descriptor = _open_readonly_no_follow(source.path)
    digest = hashlib.sha256()
    size_bytes = 0
    try:
        with os.fdopen(descriptor, "rb") as input_handle:
            try:
                with destination.open("xb") as output_handle:
                    for chunk in iter(
                        lambda: input_handle.read(1024 * 1024), b""
                    ):
                        output_handle.write(chunk)
                        digest.update(chunk)
                        size_bytes += len(chunk)
                    output_handle.flush()
                    os.fsync(output_handle.fileno())
            except OSError as error:
                fail(f"cannot stage release asset {source.name}: {error}")
    except OSError as error:
        fail(f"cannot read release asset {source.name}: {error}")
    if size_bytes != source.size_bytes or digest.hexdigest() != source.sha256:
        fail(f"release asset changed before it could be staged: {source.name}")
    os.chmod(destination, 0o644)


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    try:
        descriptor = os.open(path, flags)
    except OSError:
        return
    try:
        os.fsync(descriptor)
    except OSError:
        pass
    finally:
        os.close(descriptor)


def _paths_overlap(first: Path, second: Path) -> bool:
    return first == second or first in second.parents or second in first.parents


def stage_assets(
    assets_directory: Path,
    output_directory: Path,
    *,
    allowed_extra_assets: Sequence[str] = (),
) -> StagePlan:
    source_directory = Path(assets_directory).resolve()
    requested_output = Path(output_directory)
    if os.path.lexists(requested_output):
        fail(f"output directory already exists: {requested_output}")
    output_directory = requested_output.resolve()
    if _paths_overlap(source_directory, output_directory):
        fail("assets and output directories must not contain one another")

    plan = build_stage_plan(
        assets_directory,
        allowed_extra_assets=allowed_extra_assets,
    )
    output_directory.parent.mkdir(parents=True, exist_ok=True)
    if os.path.lexists(output_directory):
        fail(f"output directory already exists: {output_directory}")

    temporary_directory = Path(
        tempfile.mkdtemp(
            dir=output_directory.parent,
            prefix=f".{output_directory.name}.staging-",
        )
    )
    published = False
    try:
        receipts_directory = temporary_directory / "verification-receipts"
        receipts_directory.mkdir(mode=0o755)
        for platform in PLATFORMS:
            platform_plan = plan.platforms[platform]
            package_directory = (
                temporary_directory / PACKAGE_DIRECTORY_NAMES[platform]
            )
            package_directory.mkdir(mode=0o755)
            for source in platform_plan.package_files:
                copy_fingerprinted_file(source, package_directory / source.name)
            _fsync_directory(package_directory)
            if platform_plan.receipt is not None:
                receipt = platform_plan.receipt
                copy_fingerprinted_file(receipt, receipts_directory / receipt.name)

        _fsync_directory(receipts_directory)
        os.chmod(temporary_directory, 0o755)
        _fsync_directory(temporary_directory)
        if os.path.lexists(output_directory):
            fail(f"output directory appeared while staging: {output_directory}")
        try:
            os.rename(temporary_directory, output_directory)
        except OSError as error:
            fail(f"cannot publish staged output directory atomically: {error}")
        published = True
        _fsync_directory(output_directory.parent)
    finally:
        if not published and temporary_directory.exists():
            shutil.rmtree(temporary_directory)
    return plan


def argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Stage a flat directory of locally downloaded GitHub Release assets "
            "for export-website-release-manifest.py. No network access is used."
        )
    )
    parser.add_argument(
        "--assets-dir",
        required=True,
        type=Path,
        help="Flat directory containing individually downloaded release assets",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        type=Path,
        help="New directory to create atomically with staged platform packages",
    )
    parser.add_argument(
        "--allow-extra-asset",
        action="append",
        default=[],
        metavar="FILENAME",
        help=(
            "Ignore one exact unrelated downloaded asset (repeatable). Installer, "
            "signature, and chaft-desktop-* names can never be ignored."
        ),
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    try:
        plan = stage_assets(
            args.assets_dir,
            args.output_dir,
            allowed_extra_assets=args.allow_extra_asset,
        )
    except AssetStagingError as error:
        parser.exit(2, f"release asset staging failed: {error}\n")

    output = args.output_dir.resolve()
    print(f"release assets staged: {output}")
    for platform in PLATFORMS:
        print(f"{platform} package directory: {output / PACKAGE_DIRECTORY_NAMES[platform]}")
        receipt = plan.platforms[platform].receipt
        if receipt is not None:
            print(
                f"{platform} verification receipt: "
                f"{output / 'verification-receipts' / receipt.name}"
            )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise SystemExit(1)
