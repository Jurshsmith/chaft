#!/usr/bin/env python3
"""Shared, fail-closed policy for Chaft unsigned-canary smoke receipts."""

from __future__ import annotations

import hashlib
import os
import re
import stat
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Mapping


SCHEMA_VERSION = "chaft.desktop.unsigned-canary-verification.v1"
VERIFICATION_TYPE = "packaged-app-smoke"
SIGNING_STATUS = "unsigned-canary"
STATUS = "passed"
WARNING = (
    "Unsigned canary. Do not use Chaft canary builds for sensitive or "
    "production communication."
)
PLATFORMS = ("windows", "macos", "linux")
RECEIPT_FILENAMES = {
    platform: f"chaft-desktop-{platform}-verification.json"
    for platform in PLATFORMS
}
DEFAULT_SIGNATURE_AND_NOTARIZATION = {
    "authenticode": "not-performed",
    "appleCodeSigning": "not-performed",
    "appleNotarization": "not-performed",
    "openPgpDetachedSignature": "not-performed",
}
SIGNATURE_VERIFICATION = {
    "windows": "not-performed",
    "macos": "native-inspected",
    "linux": "not-performed",
}
SIGNATURE_AND_NOTARIZATION = {
    "windows": dict(DEFAULT_SIGNATURE_AND_NOTARIZATION),
    "macos": {
        **DEFAULT_SIGNATURE_AND_NOTARIZATION,
        # Ad-hoc signing has no Apple trust identity. It only keeps the
        # locally built bundle internally consistent after Qt deployment.
        "appleCodeSigning": "ad-hoc",
    },
    "linux": dict(DEFAULT_SIGNATURE_AND_NOTARIZATION),
}
CANARY_VERSION = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-canary\.([1-9]\d*)$"
)
COMMIT_PATTERN = re.compile(r"^[0-9a-f]{40}$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
REPOSITORY_PATTERN = re.compile(
    r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})/"
    r"[A-Za-z0-9](?:[A-Za-z0-9._-]{0,99})$"
)
RFC3339_TIMESTAMP = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}"
    r"(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$"
)
RUNNER_OS = {
    "windows": "Windows",
    "macos": "macOS",
    "linux": "Linux",
}
ARCHITECTURE_ALIASES = {
    "amd64": "x86_64",
    "x64": "x86_64",
    "x86-64": "x86_64",
    "x86_64": "x86_64",
    "aarch64": "arm64",
    "arm64": "arm64",
}
PACKAGE_SUFFIXES = {
    "windows": (".zip", ".msi", ".exe"),
    "macos": (".dmg",),
    "linux": (".appimage", ".tar.gz", ".tgz"),
}


class UnsignedCanaryPolicyError(ValueError):
    """Unsigned-canary evidence is missing, malformed, or dishonest."""


@dataclass(frozen=True)
class FileFingerprint:
    filename: str
    size_bytes: int
    sha256: str


def fail(message: str) -> None:
    raise UnsignedCanaryPolicyError(message)


def plain_filename(value: object, context: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{context} must be a non-empty filename")
    if (
        value in {".", ".."}
        or Path(value).name != value
        or "/" in value
        or "\\" in value
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        fail(f"{context} must be a plain filename")
    return value


def require_string(record: Mapping[str, object], key: str, context: str) -> str:
    value = record.get(key)
    if not isinstance(value, str) or not value.strip():
        fail(f"{context}.{key} must be a non-empty string")
    if any(ord(character) < 32 or ord(character) == 127 for character in value):
        fail(f"{context}.{key} must not contain control characters")
    return value


def require_positive_integer(
    record: Mapping[str, object], key: str, context: str
) -> int:
    value = record.get(key)
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        fail(f"{context}.{key} must be a positive integer")
    return value


def normalize_architecture(value: str, context: str = "architecture") -> str:
    normalized = ARCHITECTURE_ALIASES.get(value.strip().lower())
    if normalized is None:
        fail(f"{context} is unsupported")
    return normalized


def normalize_timestamp(value: str, context: str = "verifiedAt") -> str:
    if RFC3339_TIMESTAMP.fullmatch(value) is None:
        fail(f"{context} must be an RFC 3339 date-time with an explicit timezone")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        fail(f"{context} is not a valid date-time")
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        fail(f"{context} must include an explicit timezone")
    return parsed.astimezone(timezone.utc).isoformat(timespec="seconds").replace(
        "+00:00", "Z"
    )


def validate_release_identity(
    *, version: str, tag: str, commit: str, repository: str
) -> None:
    if CANARY_VERSION.fullmatch(version) is None:
        fail("version must be an exact X.Y.Z-canary.N version with N greater than zero")
    if tag != f"v{version}":
        fail("tag must equal v followed by version")
    if COMMIT_PATTERN.fullmatch(commit) is None:
        fail("commit must be a full lowercase 40-character Git revision")
    if REPOSITORY_PATTERN.fullmatch(repository) is None or repository.endswith(".git"):
        fail("repository must be a GitHub OWNER/REPOSITORY slug")


def validate_package_filename(
    filename: str, platform: str, version: str
) -> None:
    plain_filename(filename, "package filename")
    if platform not in PLATFORMS:
        fail(f"unsupported platform: {platform}")
    if version not in filename:
        fail("package filename must contain the exact canary version")
    if not filename.lower().endswith(PACKAGE_SUFFIXES[platform]):
        fail(f"package filename is not a supported {platform} package")


def fingerprint_file(path: Path) -> FileFingerprint:
    path = Path(path)
    if path.is_symlink():
        fail(f"package must not be a symbolic link: {path}")
    flags = os.O_RDONLY
    if hasattr(os, "O_BINARY"):
        flags |= os.O_BINARY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open package {path}: {error}")
    before = os.fstat(descriptor)
    if not stat.S_ISREG(before.st_mode) or before.st_size <= 0:
        os.close(descriptor)
        fail(f"package must be a non-empty regular file: {path}")
    digest = hashlib.sha256()
    size_bytes = 0
    try:
        with os.fdopen(descriptor, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
                size_bytes += len(chunk)
            after = os.fstat(handle.fileno())
    except OSError as error:
        fail(f"cannot read package {path}: {error}")
    stable_fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(getattr(before, field) != getattr(after, field) for field in stable_fields):
        fail(f"package changed while it was being fingerprinted: {path.name}")
    return FileFingerprint(
        filename=plain_filename(path.name, "package filename"),
        size_bytes=size_bytes,
        sha256=digest.hexdigest(),
    )


def validate_receipt_document(
    receipt: Mapping[str, object],
    *,
    expected_platform: str | None = None,
    expected_package: FileFingerprint | None = None,
    expected_version: str | None = None,
    expected_tag: str | None = None,
    expected_commit: str | None = None,
    expected_repository: str | None = None,
) -> None:
    context = "unsigned-canary receipt"
    expected_keys = {
        "schemaVersion",
        "platform",
        "verificationType",
        "status",
        "signingStatus",
        "signatureVerification",
        "signatureAndNotarization",
        "productionEligible",
        "warning",
        "version",
        "tag",
        "commit",
        "repository",
        "architecture",
        "verifiedAt",
        "release",
        "asset",
        "runner",
        "smoke",
        "receiptGenerator",
    }
    if set(receipt) != expected_keys:
        fail(f"{context} keys differ from the reviewed schema")
    if receipt.get("schemaVersion") != SCHEMA_VERSION:
        fail(f"{context} schemaVersion is unsupported")
    platform = receipt.get("platform")
    if platform not in PLATFORMS:
        fail(f"{context}.platform is unsupported")
    assert isinstance(platform, str)
    if expected_platform is not None and platform != expected_platform:
        fail(f"{context}.platform does not match the expected platform")
    if receipt.get("verificationType") != VERIFICATION_TYPE:
        fail(f"{context}.verificationType must be {VERIFICATION_TYPE!r}")
    if receipt.get("status") != STATUS:
        fail(f"{context}.status must be {STATUS!r}")
    if receipt.get("signingStatus") != SIGNING_STATUS:
        fail(f"{context}.signingStatus must be {SIGNING_STATUS!r}")
    expected_signature_verification = SIGNATURE_VERIFICATION[platform]
    if receipt.get("signatureVerification") != expected_signature_verification:
        fail(
            f"{context}.signatureVerification must be "
            f"{expected_signature_verification!r} for {platform}"
        )
    expected_signature_state = SIGNATURE_AND_NOTARIZATION[platform]
    if receipt.get("signatureAndNotarization") != expected_signature_state:
        fail(
            f"{context}.signatureAndNotarization does not match the reviewed "
            f"unsigned-canary policy for {platform}"
        )
    if receipt.get("productionEligible") is not False:
        fail(f"{context}.productionEligible must be false")
    if receipt.get("warning") != WARNING:
        fail(f"{context}.warning must contain the reviewed unsigned warning")

    version = require_string(receipt, "version", context)
    tag = require_string(receipt, "tag", context)
    commit = require_string(receipt, "commit", context)
    repository = require_string(receipt, "repository", context)
    validate_release_identity(
        version=version, tag=tag, commit=commit, repository=repository
    )
    for actual, expected, label in (
        (version, expected_version, "version"),
        (tag, expected_tag, "tag"),
        (commit, expected_commit, "commit"),
        (repository, expected_repository, "repository"),
    ):
        if expected is not None and actual != expected:
            fail(f"{context}.{label} does not match the expected release")

    architecture = require_string(receipt, "architecture", context)
    normalized_architecture = normalize_architecture(
        architecture, f"{context}.architecture"
    )
    if architecture != normalized_architecture:
        fail(f"{context}.architecture must already be normalized")
    normalize_timestamp(require_string(receipt, "verifiedAt", context))

    release = receipt.get("release")
    if not isinstance(release, dict) or set(release) != {"id"}:
        fail(f"{context}.release must contain exactly id")
    require_positive_integer(release, "id", f"{context}.release")

    asset = receipt.get("asset")
    asset_keys = {"id", "filename", "sizeBytes", "sha256"}
    if not isinstance(asset, dict) or set(asset) != asset_keys:
        fail(f"{context}.asset keys differ from the reviewed schema")
    require_positive_integer(asset, "id", f"{context}.asset")
    filename = plain_filename(asset.get("filename"), f"{context}.asset.filename")
    validate_package_filename(filename, platform, version)
    size_bytes = require_positive_integer(asset, "sizeBytes", f"{context}.asset")
    sha256 = require_string(asset, "sha256", f"{context}.asset")
    if SHA256_PATTERN.fullmatch(sha256) is None:
        fail(f"{context}.asset.sha256 must be a lowercase SHA-256 digest")
    if expected_package is not None and (
        filename != expected_package.filename
        or size_bytes != expected_package.size_bytes
        or sha256 != expected_package.sha256
    ):
        fail(f"{context}.asset does not bind the expected package bytes")

    runner = receipt.get("runner")
    runner_keys = {
        "os",
        "architecture",
        "workflowRunId",
        "workflowRunAttempt",
    }
    if not isinstance(runner, dict) or set(runner) != runner_keys:
        fail(f"{context}.runner keys differ from the reviewed schema")
    if runner.get("os") != RUNNER_OS[platform]:
        fail(f"{context}.runner.os is not native for {platform}")
    runner_architecture = require_string(
        runner, "architecture", f"{context}.runner"
    )
    if normalize_architecture(runner_architecture) != architecture:
        fail(f"{context}.runner.architecture does not match package architecture")
    require_positive_integer(runner, "workflowRunId", f"{context}.runner")
    require_positive_integer(runner, "workflowRunAttempt", f"{context}.runner")

    smoke = receipt.get("smoke")
    if not isinstance(smoke, dict) or set(smoke) != {"status", "command"}:
        fail(f"{context}.smoke must contain exactly status and command")
    if smoke.get("status") != STATUS:
        fail(f"{context}.smoke.status must be {STATUS!r}")
    require_string(smoke, "command", f"{context}.smoke")

    generator = receipt.get("receiptGenerator")
    if not isinstance(generator, dict) or set(generator) != {"name", "version"}:
        fail(f"{context}.receiptGenerator must contain exactly name and version")
    if generator.get("name") != "Chaft unsigned-canary receipt generator":
        fail(f"{context}.receiptGenerator.name is unsupported")
    if generator.get("version") != "1":
        fail(f"{context}.receiptGenerator.version is unsupported")
