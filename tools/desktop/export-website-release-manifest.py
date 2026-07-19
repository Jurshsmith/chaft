#!/usr/bin/env python3
"""Export verified desktop packages as the website's published release manifest.

The platform-specific metadata verifier remains the authority for package-directory
shape, SBOM, provenance, and checksum coherence. This exporter invokes it for every
platform, then hashes the package files again when building the website manifest.

Native signing and notarization cannot be re-verified portably by this script. A
published Windows or macOS status therefore requires both the immutable public
platform-verification receipt and a trusted receipt produced by an independent native
rerun. Linux may be published as ``checksummed``; claiming ``signed`` requires the same
two-receipt binding. The public receipt remains the website evidence, while every
security-relevant claim must match the trusted native rerun.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
import tarfile
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Mapping, Sequence
from urllib.parse import quote, urlparse


PLATFORMS = ("windows", "macos", "linux")
PLATFORM_LABELS = {
    "windows": "Windows",
    "macos": "macOS",
    "linux": "Linux",
}
PACKAGE_SUFFIXES = (
    (".tar.gz", "linux", "linux-tgz", "tar.gz"),
    (".appimage", "linux", "linux-appimage", "appimage"),
    (".tgz", "linux", "linux-tgz", "tgz"),
    (".dmg", "macos", "macos-dmg", "dmg"),
    (".zip", "windows", "windows-zip", "zip"),
    (".msi", "windows", "windows-msi", "msi"),
    (".exe", "windows", "windows-exe", "exe"),
)
SIGNATURE_SUFFIXES = (".sig", ".asc")
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
VERIFICATION_TYPES = {
    "windows": "authenticode",
    "macos": "apple-notarization",
    "linux": "detached-signature",
}
SECURITY_RELEVANT_RECEIPT_FIELDS = (
    "schemaVersion",
    "platform",
    "verificationType",
    "status",
    "version",
    "tag",
    "commit",
    "architecture",
    "verificationPolicy",
    "artifacts",
    "signatures",
    "verificationDetails",
)
ALLOWED_SIGNING_STATES = {
    "windows": {"signed"},
    "macos": {"notarized"},
    "linux": {"checksummed", "signed"},
}
ARCHITECTURE_ALIASES = {
    "amd64": "x86_64",
    "x64": "x86_64",
    "x86-64": "x86_64",
    "x86_64": "x86_64",
    "aarch64": "arm64",
    "arm64": "arm64",
    "universal": "universal",
    "universal2": "universal",
}
SEMANTIC_VERSION = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-(?:(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"(?:\+(?:[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)
COMMIT_PATTERN = re.compile(r"^[0-9a-fA-F]{40,64}$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
AUTHENTICODE_THUMBPRINT_PATTERN = re.compile(r"^(?:[0-9A-F]{40}|[0-9A-F]{64})$")
APPLE_TEAM_ID_PATTERN = re.compile(r"^[A-Z0-9]{10}$")
OPENPGP_FINGERPRINT_PATTERN = re.compile(r"^(?:[0-9A-F]{40}|[0-9A-F]{64})$")
RFC3339_TIMESTAMP = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}"
    r"(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$"
)
REPOSITORY_PATTERN = re.compile(
    r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})/[A-Za-z0-9._-]{1,100}$"
)
MAX_TAR_ENTRIES = 20_000
MAX_TAR_ENTRY_BYTES = 2 * 1024 * 1024 * 1024
MAX_TAR_TOTAL_BYTES = 4 * 1024 * 1024 * 1024
ELF_MACHINE_ARCHITECTURES = {
    62: "x86_64",  # EM_X86_64
    183: "arm64",  # EM_AARCH64
}


class ManifestExportError(ValueError):
    """A release input cannot be represented as an honest published manifest."""


@dataclass(frozen=True)
class Artifact:
    name: str
    package_format: str
    website_format: str
    size_bytes: int
    sha256: str


@dataclass(frozen=True)
class EvidenceFile:
    name: str
    path: Path
    size_bytes: int
    sha256: str
    signature_format: str | None = None


@dataclass(frozen=True)
class PlatformRelease:
    platform: str
    version: str
    commit: str
    source_repository: str
    provenance_architecture: str | None
    artifacts: tuple[Artifact, ...]
    signatures: Mapping[str, EvidenceFile]
    metadata: Mapping[str, EvidenceFile]


@dataclass(frozen=True)
class ValidatedVerificationReceipt:
    evidence: EvidenceFile
    document: Mapping[str, object]


Verifier = Callable[[Path, str, Path, str], None]
TagResolver = Callable[[Path, str], str]


def fail(message: str) -> None:
    raise ManifestExportError(message)


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def evidence_file(path: Path, signature_format: str | None = None) -> EvidenceFile:
    if path.is_symlink():
        fail(f"release evidence must not be a symbolic link: {path}")
    if not path.is_file():
        fail(f"release evidence file not found: {path}")
    size_bytes = path.stat().st_size
    if size_bytes <= 0:
        fail(f"release evidence file is empty: {path.name}")
    return EvidenceFile(
        name=path.name,
        path=path,
        size_bytes=size_bytes,
        sha256=file_sha256(path),
        signature_format=signature_format,
    )


def load_json_object(path: Path, context: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        fail(f"cannot read {context} {path}: {error}")
    except json.JSONDecodeError as error:
        fail(f"{context} {path.name} is not valid JSON: {error}")
    if not isinstance(value, dict):
        fail(f"{context} {path.name} must contain a JSON object")
    return value


def require_string(record: Mapping[str, object], key: str, context: str) -> str:
    value = record.get(key)
    if not isinstance(value, str) or not value.strip():
        fail(f"{context}.{key} must be a non-empty string")
    return value


def require_aware_timestamp(value: object, context: str) -> datetime:
    if not isinstance(value, str) or RFC3339_TIMESTAMP.fullmatch(value) is None:
        fail(
            f"{context} must be an RFC 3339 date-time with an explicit timezone"
        )
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        fail(f"{context} is not a valid ISO 8601 timestamp: {value}")
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        fail(f"{context} must include an explicit timezone")
    return parsed


def normalized_timestamp(value: str, context: str) -> str:
    parsed = require_aware_timestamp(value, context).astimezone(timezone.utc)
    return parsed.isoformat(timespec="seconds").replace("+00:00", "Z")


def compare_semantic_versions(left: str, right: str) -> int:
    """Compare SemVer precedence, deliberately ignoring build metadata."""

    def parts(value: str) -> tuple[tuple[int, int, int], tuple[str, ...] | None]:
        if SEMANTIC_VERSION.fullmatch(value) is None:
            fail(f"cannot compare invalid semantic version {value!r}")
        precedence = value.split("+", 1)[0]
        core, separator, prerelease = precedence.partition("-")
        major, minor, patch = (int(item) for item in core.split("."))
        return (major, minor, patch), tuple(prerelease.split(".")) if separator else None

    left_core, left_prerelease = parts(left)
    right_core, right_prerelease = parts(right)
    if left_core != right_core:
        return 1 if left_core > right_core else -1
    if left_prerelease is None or right_prerelease is None:
        if left_prerelease is right_prerelease:
            return 0
        return 1 if left_prerelease is None else -1

    for left_identifier, right_identifier in zip(left_prerelease, right_prerelease):
        if left_identifier == right_identifier:
            continue
        left_numeric = left_identifier.isdigit()
        right_numeric = right_identifier.isdigit()
        if left_numeric and right_numeric:
            return 1 if int(left_identifier) > int(right_identifier) else -1
        if left_numeric != right_numeric:
            return -1 if left_numeric else 1
        return 1 if left_identifier > right_identifier else -1
    if len(left_prerelease) == len(right_prerelease):
        return 0
    return 1 if len(left_prerelease) > len(right_prerelease) else -1


def normalize_architecture(value: str, platform: str) -> str:
    normalized = ARCHITECTURE_ALIASES.get(value.strip().lower())
    if normalized is None:
        allowed = ", ".join(sorted(ARCHITECTURE_ALIASES))
        fail(f"unsupported {platform} architecture {value!r}; expected one of: {allowed}")
    if normalized == "universal" and platform != "macos":
        fail("universal architecture is supported only for macOS packages")
    return normalized


def elf_architecture_from_header(header: bytes, context: str) -> str:
    if len(header) < 20 or header[:4] != b"\x7fELF":
        fail(f"ELF payload has an invalid header: {context}")
    encoding = header[5]
    if encoding == 1:
        machine = struct.unpack_from("<H", header, 18)[0]
    elif encoding == 2:
        machine = struct.unpack_from(">H", header, 18)[0]
    else:
        fail(f"ELF payload has an invalid byte order: {context}")
    architecture = ELF_MACHINE_ARCHITECTURES.get(machine)
    if architecture is None:
        fail(f"ELF payload {context} uses unsupported machine {machine}")
    return architecture


def elf_file_architecture(path: Path) -> str:
    try:
        with path.open("rb") as handle:
            return elf_architecture_from_header(handle.read(20), path.name)
    except OSError as error:
        fail(f"could not read ELF payload {path.name}: {error}")


def tar_elf_architectures(path: Path) -> list[str]:
    """Inspect every ELF payload without extracting an untrusted Linux archive."""

    architectures: list[str] = []
    seen: set[str] = set()
    entry_count = 0
    total_bytes = 0
    try:
        archive_context = tarfile.open(path, mode="r:gz")
    except (OSError, tarfile.TarError) as error:
        fail(
            f"Linux package is not a valid gzip-compressed tar archive "
            f"({path.name}): {error}"
        )
    with archive_context as archive:
        try:
            for member in archive:
                entry_count += 1
                if entry_count > MAX_TAR_ENTRIES:
                    fail(f"Linux archive {path.name} has too many entries")
                normalized_name = member.name.replace("\\", "/")
                comparable = (
                    normalized_name[:-1]
                    if normalized_name.endswith("/")
                    else normalized_name
                )
                parts = comparable.split("/")
                if (
                    not normalized_name
                    or normalized_name.startswith("/")
                    or re.match(r"^[A-Za-z]:", normalized_name)
                    or any(part in {"", ".", ".."} for part in parts)
                ):
                    fail(
                        f"Linux archive {path.name} contains unsafe entry "
                        f"{member.name!r}"
                    )
                folded = normalized_name.casefold().rstrip("/")
                if folded in seen:
                    fail(
                        f"Linux archive {path.name} contains duplicate entry "
                        f"{member.name!r}"
                    )
                seen.add(folded)
                if member.isdev() or member.isfifo():
                    fail(
                        f"Linux archive {path.name} contains special entry "
                        f"{member.name!r}"
                    )
                if not member.isfile():
                    continue
                if member.size < 0 or member.size > MAX_TAR_ENTRY_BYTES:
                    fail(
                        f"Linux archive {path.name} entry is too large: {member.name}"
                    )
                total_bytes += member.size
                if total_bytes > MAX_TAR_TOTAL_BYTES:
                    fail(
                        f"Linux archive {path.name} expands beyond the verification limit"
                    )
                handle = archive.extractfile(member)
                if handle is None:
                    fail(f"Linux archive payload could not be read: {member.name}")
                with handle:
                    header = handle.read(20)
                if header.startswith(b"\x7fELF"):
                    architectures.append(
                        elf_architecture_from_header(
                            header, f"{path.name}:{member.name}"
                        )
                    )
        except (OSError, tarfile.TarError) as error:
            fail(f"could not inspect Linux archive {path.name}: {error}")
    if not architectures:
        fail(f"Linux archive contains no ELF payload: {path.name}")
    return architectures


def linux_package_architectures(path: Path) -> list[str]:
    lowered = path.name.lower()
    if lowered.endswith(".appimage"):
        return [elf_file_architecture(path)]
    if lowered.endswith((".tar.gz", ".tgz")):
        return tar_elf_architectures(path)
    fail(f"unsupported Linux package format for architecture inspection: {path.name}")


def verify_linux_package_architectures(
    package_dir: Path,
    artifacts: Sequence[Artifact],
    expected_architecture: str,
) -> None:
    """Bind an unsigned Linux release claim to its actual package payloads."""

    release_architectures: set[str] = set()
    for artifact in artifacts:
        observed = set(linux_package_architectures(package_dir / artifact.name))
        if len(observed) != 1:
            fail(
                f"Linux package {artifact.name} contains mixed payload architectures: "
                + ", ".join(sorted(observed))
            )
        actual = next(iter(observed))
        if actual != expected_architecture:
            fail(
                f"Linux package {artifact.name} payload architecture is {actual}, "
                f"not the requested {expected_architecture}"
            )
        release_architectures.add(actual)
    if release_architectures != {expected_architecture}:
        fail("Linux package set does not establish the requested payload architecture")


def normalize_publisher_identity(value: str, platform: str) -> str:
    normalized = re.sub(r"\s+", "", value).upper()
    if platform == "windows":
        if AUTHENTICODE_THUMBPRINT_PATTERN.fullmatch(normalized) is None:
            fail("Windows publisher identity must be a 40- or 64-hex certificate thumbprint")
    elif platform == "macos":
        if APPLE_TEAM_ID_PATTERN.fullmatch(normalized) is None:
            fail("macOS publisher identity must be a 10-character Apple Developer Team ID")
    elif OPENPGP_FINGERPRINT_PATTERN.fullmatch(normalized) is None:
        fail("Linux publisher identity must be a 40- or 64-hex OpenPGP fingerprint")
    return normalized


def package_description(name: str) -> tuple[str, str, str] | None:
    lowered = name.lower()
    for suffix, platform, package_format, website_format in PACKAGE_SUFFIXES:
        if lowered.endswith(suffix):
            return platform, package_format, website_format
    return None


def signature_description(name: str) -> tuple[str, str] | None:
    lowered = name.lower()
    for suffix in SIGNATURE_SUFFIXES:
        if lowered.endswith(suffix):
            return name[: -len(suffix)], suffix[1:]
    return None


def filename_contains_version(filename: str, version: str) -> bool:
    escaped = re.escape(version)
    return re.search(rf"(?:^|[^0-9A-Za-z]){escaped}(?:$|[^0-9A-Za-z])", filename) is not None


def parse_checksums(path: Path) -> dict[str, str]:
    rows: dict[str, str] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"cannot read checksum evidence {path}: {error}")
    for line_number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-f]{64})  ([^/\\\r\n]+)", line)
        if match is None:
            fail(f"invalid {path.name} line {line_number}: {line!r}")
        digest, name = match.groups()
        if name in rows:
            fail(f"duplicate {path.name} row for {name}")
        rows[name] = digest
    if not rows:
        fail(f"{path.name} must contain at least one artifact checksum")
    return rows


def normalize_github_repository(value: str) -> str | None:
    candidate = value.strip()
    if not candidate:
        return None
    if candidate.startswith("git@github.com:"):
        candidate = candidate[len("git@github.com:") :]
    elif "://" in candidate:
        parsed = urlparse(candidate)
        if (parsed.hostname or "").lower() != "github.com":
            return None
        candidate = parsed.path.lstrip("/")
    elif candidate.lower().startswith("github.com/"):
        candidate = candidate[len("github.com/") :]
    if candidate.endswith(".git"):
        candidate = candidate[:-4]
    candidate = candidate.strip("/")
    return candidate if REPOSITORY_PATTERN.fullmatch(candidate) else None


def validate_repository(value: str) -> str:
    if not REPOSITORY_PATTERN.fullmatch(value) or value.endswith(".git"):
        fail("repository must be a GitHub OWNER/REPOSITORY slug")
    return value


def resolve_git_tag_commit(source_root: Path, tag: str) -> str:
    completed = subprocess.run(
        [
            "git",
            "-C",
            str(source_root),
            "rev-parse",
            "--verify",
            f"refs/tags/{tag}^{{commit}}",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        suffix = f": {detail}" if detail else ""
        fail(f"cannot resolve Git release tag {tag!r} in source root{suffix}")
    commit = completed.stdout.strip().lower()
    if COMMIT_PATTERN.fullmatch(commit) is None:
        fail(f"Git release tag {tag!r} resolved to an invalid commit revision")
    return commit


def official_verifier(
    package_dir: Path,
    platform: str,
    source_root: Path,
    expected_commit: str,
) -> None:
    verifier = Path(__file__).with_name("verify-release-metadata.py")
    command = [
        sys.executable,
        str(verifier),
        "release",
        "--package-dir",
        str(package_dir),
        "--platform",
        platform,
        "--require-clean",
        "--source-root",
        str(source_root),
        "--expected-commit",
        expected_commit,
    ]
    completed = subprocess.run(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        suffix = f": {detail}" if detail else ""
        fail(f"{platform} package metadata verification failed{suffix}")


def _artifact_row_map(
    provenance: Mapping[str, object], platform: str
) -> dict[str, dict[str, object]]:
    rows = provenance.get("artifacts")
    if not isinstance(rows, list) or not rows:
        fail(f"{platform} provenance.artifacts must be a non-empty array")
    result: dict[str, dict[str, object]] = {}
    for index, value in enumerate(rows):
        if not isinstance(value, dict):
            fail(f"{platform} provenance.artifacts[{index}] must be an object")
        name = require_string(value, "name", f"{platform} provenance.artifacts[{index}]")
        if Path(name).name != name or "/" in name or "\\" in name:
            fail(f"{platform} provenance artifact name must be a filename: {name}")
        if name in result:
            fail(f"duplicate {platform} provenance artifact row for {name}")
        result[name] = value
    return result


def read_platform_release(
    package_dir: Path,
    platform: str,
    source_root: Path,
    expected_commit: str,
    verifier: Verifier = official_verifier,
) -> PlatformRelease:
    package_dir = package_dir.resolve()
    verifier(package_dir, platform, source_root, expected_commit)
    if not package_dir.is_dir():
        fail(f"{platform} package directory not found: {package_dir}")
    unexpected_directories = sorted(path.name for path in package_dir.iterdir() if path.is_dir())
    if unexpected_directories:
        fail(
            f"{platform} package directory contains unexpected directories: "
            + ", ".join(unexpected_directories)
        )

    names = METADATA_FILENAMES[platform]
    paths = {kind: package_dir / filename for kind, filename in names.items()}
    metadata = {kind: evidence_file(path) for kind, path in paths.items()}
    checksums = parse_checksums(paths["checksums"])
    provenance = load_json_object(paths["provenance"], f"{platform} provenance")

    if provenance.get("schemaVersion") != "chaft.desktop.provenance.v1":
        fail(f"{platform} provenance schemaVersion is unsupported")
    if provenance.get("profile") != "release":
        fail(f"{platform} provenance profile must be 'release'")
    if provenance.get("packagePlatform") != platform:
        fail(f"{platform} provenance packagePlatform does not match its directory")

    version = require_string(provenance, "version", f"{platform} provenance")
    if SEMANTIC_VERSION.fullmatch(version) is None:
        fail(f"{platform} provenance version is not semantic: {version}")
    source = provenance.get("source")
    if not isinstance(source, dict):
        fail(f"{platform} provenance.source must be an object")
    commit = require_string(source, "commit", f"{platform} provenance.source")
    if COMMIT_PATTERN.fullmatch(commit) is None:
        fail(f"{platform} provenance source commit is not a 40-to-64 character hex revision")
    source_repository = require_string(source, "repository", f"{platform} provenance.source")
    if normalize_github_repository(source_repository) is None:
        fail(f"{platform} provenance source repository is not a GitHub repository")
    build_platform = provenance.get("platform")
    provenance_architecture = None
    if isinstance(build_platform, dict):
        machine = build_platform.get("machine")
        if isinstance(machine, str) and machine.strip():
            provenance_architecture = ARCHITECTURE_ALIASES.get(machine.strip().lower())

    rows = _artifact_row_map(provenance, platform)
    if set(rows) != set(checksums):
        missing = sorted(set(rows) - set(checksums))
        extra = sorted(set(checksums) - set(rows))
        details = []
        if missing:
            details.append("missing checksums: " + ", ".join(missing))
        if extra:
            details.append("extra checksums: " + ", ".join(extra))
        fail(f"{platform} provenance/checksum artifact sets differ ({'; '.join(details)})")

    artifacts: list[Artifact] = []
    signatures: dict[str, EvidenceFile] = {}
    allowed_names = set(names.values()) | set(rows)
    actual_files = {path.name for path in package_dir.iterdir() if path.is_file()}
    if actual_files != allowed_names:
        unexpected = sorted(actual_files - allowed_names)
        missing = sorted(allowed_names - actual_files)
        details = []
        if unexpected:
            details.append("unexpected files: " + ", ".join(unexpected))
        if missing:
            details.append("missing files: " + ", ".join(missing))
        fail(f"{platform} package directory shape changed ({'; '.join(details)})")

    for name, row in rows.items():
        path = package_dir / name
        if path.is_symlink():
            fail(f"{platform} release artifact must not be a symbolic link: {name}")
        size_bytes = path.stat().st_size
        if size_bytes <= 0:
            fail(f"{platform} release artifact is empty: {name}")
        actual_sha256 = file_sha256(path)
        row_sha256 = row.get("sha256")
        row_size = row.get("sizeBytes")
        if row_sha256 != actual_sha256 or checksums[name] != actual_sha256:
            fail(f"{platform} verified metadata is stale for {name}")
        if not isinstance(row_size, int) or isinstance(row_size, bool) or row_size != size_bytes:
            fail(f"{platform} provenance sizeBytes is stale for {name}")

        package = package_description(name)
        signature = signature_description(name)
        if package is not None:
            artifact_platform, package_format, website_format = package
            if artifact_platform != platform:
                fail(f"{platform} directory contains package for {artifact_platform}: {name}")
            if row.get("packageFormat") != package_format:
                fail(f"{platform} provenance packageFormat is incoherent for {name}")
            if not filename_contains_version(name, version):
                fail(f"{platform} package filename does not contain version {version}: {name}")
            artifacts.append(
                Artifact(
                    name=name,
                    package_format=package_format,
                    website_format=website_format,
                    size_bytes=size_bytes,
                    sha256=actual_sha256,
                )
            )
        elif signature is not None:
            signed_artifact, signature_format = signature
            if row.get("packageFormat") != "detached-signature":
                fail(f"{platform} provenance signature packageFormat is incoherent for {name}")
            if row.get("signedArtifact") != signed_artifact:
                fail(f"{platform} provenance signedArtifact is incoherent for {name}")
            if row.get("signatureFormat") != signature_format:
                fail(f"{platform} provenance signatureFormat is incoherent for {name}")
            if signed_artifact in signatures:
                fail(f"multiple detached signatures found for {signed_artifact}")
            signatures[signed_artifact] = EvidenceFile(
                name=name,
                path=path,
                size_bytes=size_bytes,
                sha256=actual_sha256,
                signature_format=signature_format,
            )
        else:
            fail(f"{platform} provenance contains an unsupported artifact: {name}")

    if not artifacts:
        fail(f"{platform} package directory contains no installable package")
    package_names = {artifact.name for artifact in artifacts}
    orphaned = sorted(set(signatures) - package_names)
    if orphaned:
        fail(f"{platform} detached signatures reference missing packages: {', '.join(orphaned)}")

    return PlatformRelease(
        platform=platform,
        version=version,
        commit=commit.lower(),
        source_repository=source_repository,
        provenance_architecture=provenance_architecture,
        artifacts=tuple(sorted(artifacts, key=lambda artifact: artifact.name)),
        signatures=signatures,
        metadata=metadata,
    )


def release_asset_url(repository: str, tag: str, filename: str) -> str:
    return (
        f"https://github.com/{repository}/releases/download/"
        f"{quote(tag, safe='')}/{quote(filename, safe='')}"
    )


def evidence_json(file: EvidenceFile, repository: str, tag: str) -> dict[str, object]:
    value: dict[str, object] = {
        "filename": file.name,
        "url": release_asset_url(repository, tag, file.name),
        "sizeBytes": file.size_bytes,
        "sha256": file.sha256,
    }
    if file.signature_format is not None:
        value["format"] = file.signature_format
    return value


def require_plain_filename(value: object, context: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{context} must be a non-empty filename")
    if Path(value).name != value or "/" in value or "\\" in value:
        fail(f"{context} must be a plain filename")
    return value


def validate_verification_details(
    receipt: Mapping[str, object],
    context: str,
    platform_release: PlatformRelease,
    expected_architecture: str,
    expected_publisher_identity: str,
    policy: Mapping[str, object],
    signature_claims: Mapping[str, Mapping[str, str]],
) -> None:
    raw_details = receipt.get("verificationDetails")
    if not isinstance(raw_details, list) or not raw_details:
        fail(f"{context}.verificationDetails must be a non-empty array")

    detail_rows: dict[str, Mapping[str, object]] = {}
    for index, value in enumerate(raw_details):
        row_context = f"{context}.verificationDetails[{index}]"
        if not isinstance(value, dict):
            fail(f"{row_context} must be an object")
        filename = require_plain_filename(value.get("filename"), f"{row_context}.filename")
        if filename in detail_rows:
            fail(f"{context} has duplicate native verification details for {filename}")
        architecture = require_string(value, "architecture", row_context)
        if (
            normalize_architecture(architecture, platform_release.platform)
            != expected_architecture
        ):
            fail(f"{row_context}.architecture does not match the requested architecture")
        detail_rows[filename] = value

    expected_packages = {artifact.name for artifact in platform_release.artifacts}
    if set(detail_rows) != expected_packages:
        fail(f"{context}.verificationDetails does not cover the exact package set")

    if platform_release.platform == "windows":
        expected_algorithm = (
            "sha1" if len(expected_publisher_identity) == 40 else "sha256"
        )
        for filename, row in detail_rows.items():
            payloads = row.get("verifiedPayloads")
            row_context = f"{context}.verificationDetails[{filename}]"
            if not isinstance(payloads, list) or not payloads:
                fail(f"{row_context}.verifiedPayloads must be a non-empty array")
            for index, value in enumerate(payloads):
                payload_context = f"{row_context}.verifiedPayloads[{index}]"
                if not isinstance(value, dict):
                    fail(f"{payload_context} must be an object")
                require_string(value, "payload", payload_context)
                require_string(value, "signatureType", payload_context)
                thumbprint = require_string(value, "signerThumbprint", payload_context)
                if normalize_publisher_identity(thumbprint, "windows") != (
                    expected_publisher_identity
                ):
                    fail(f"{payload_context}.signerThumbprint does not match release policy")
                if value.get("signerThumbprintAlgorithm") != expected_algorithm:
                    fail(
                        f"{payload_context}.signerThumbprintAlgorithm must be "
                        f"{expected_algorithm}"
                    )
                certificate_sha1 = require_string(
                    value, "signerCertificateSha1", payload_context
                ).upper()
                certificate_sha256 = require_string(
                    value, "signerCertificateSha256", payload_context
                ).upper()
                if re.fullmatch(r"[0-9A-F]{40}", certificate_sha1) is None:
                    fail(f"{payload_context}.signerCertificateSha1 is invalid")
                if re.fullmatch(r"[0-9A-F]{64}", certificate_sha256) is None:
                    fail(f"{payload_context}.signerCertificateSha256 is invalid")
                expected_certificate = (
                    certificate_sha1
                    if expected_algorithm == "sha1"
                    else certificate_sha256
                )
                if expected_certificate != expected_publisher_identity:
                    fail(f"{payload_context} signer certificate does not match release policy")
                payload_architecture = value.get("architecture")
                if payload_architecture is not None:
                    if not isinstance(payload_architecture, str) or (
                        normalize_architecture(payload_architecture, "windows")
                        != expected_architecture
                    ):
                        fail(
                            f"{payload_context}.architecture does not match the requested "
                            "architecture"
                        )
                if "msiTemplate" in value:
                    require_string(value, "msiTemplate", payload_context)
    elif platform_release.platform == "macos":
        for filename, row in detail_rows.items():
            row_context = f"{context}.verificationDetails[{filename}]"
            team_identifier = require_string(row, "teamIdentifier", row_context)
            if normalize_publisher_identity(team_identifier, "macos") != (
                expected_publisher_identity
            ):
                fail(f"{row_context}.teamIdentifier does not match release policy")
            applications = row.get("verifiedApplications")
            if not isinstance(applications, list) or not applications:
                fail(f"{row_context}.verifiedApplications must be a non-empty array")
            for index, value in enumerate(applications):
                application_context = f"{row_context}.verifiedApplications[{index}]"
                if not isinstance(value, dict):
                    fail(f"{application_context} must be an object")
                require_string(value, "application", application_context)
                require_string(value, "executable", application_context)
                application_architecture = require_string(
                    value, "architecture", application_context
                )
                if normalize_architecture(application_architecture, "macos") != (
                    expected_architecture
                ):
                    fail(
                        f"{application_context}.architecture does not match the requested "
                        "architecture"
                    )
                application_team = require_string(
                    value, "teamIdentifier", application_context
                )
                if normalize_publisher_identity(application_team, "macos") != (
                    expected_publisher_identity
                ):
                    fail(
                        f"{application_context}.teamIdentifier does not match release policy"
                    )
    else:
        trusted_keyring = policy["trustedKeyring"]
        assert isinstance(trusted_keyring, dict)
        for filename, row in detail_rows.items():
            row_context = f"{context}.verificationDetails[{filename}]"
            raw_signature = row.get("signature")
            if not isinstance(raw_signature, dict):
                fail(f"{row_context}.signature must be an object")
            signature_claim = signature_claims.get(filename)
            if signature_claim is None:
                fail(f"{row_context}.signature is not bound to detached-signature evidence")
            for key in (
                "filename",
                "sha256",
                "signerFingerprint",
                "trustedFingerprint",
            ):
                value = require_string(raw_signature, key, f"{row_context}.signature")
                if value.upper() != signature_claim[key].upper():
                    fail(f"{row_context}.signature.{key} is incoherent")
            detail_keyring = raw_signature.get("trustedKeyring")
            if not isinstance(detail_keyring, dict) or detail_keyring != trusted_keyring:
                fail(f"{row_context}.signature.trustedKeyring is incoherent")


def read_verification_receipt(
    path: Path,
    platform_release: PlatformRelease,
    expected_tag: str,
    expected_type: str,
    expected_architecture: str,
    expected_publisher_identity: str,
    receipt_role: str = "public",
) -> ValidatedVerificationReceipt:
    expected_name = VERIFICATION_RECEIPT_FILENAMES[platform_release.platform]
    if path.name != expected_name:
        fail(
            f"{platform_release.platform} {receipt_role} verification receipt must be "
            f"named {expected_name}"
        )
    receipt_file = evidence_file(path)
    context = f"{platform_release.platform} {receipt_role} verification receipt"
    receipt = load_json_object(path, context)
    if receipt.get("schemaVersion") != "chaft.desktop.platform-verification.v1":
        fail(f"{context} schemaVersion is unsupported")
    if receipt.get("platform") != platform_release.platform:
        fail(f"{context} platform is incoherent")
    if receipt.get("verificationType") != expected_type:
        fail(f"{context} verificationType must be {expected_type!r}")
    if receipt.get("status") != "verified":
        fail(f"{context} status must be 'verified'")
    if receipt.get("version") != platform_release.version:
        fail(f"{context} version does not match package provenance")
    if receipt.get("tag") != expected_tag:
        fail(f"{context} tag does not match the requested release tag")
    receipt_architecture = require_string(receipt, "architecture", context)
    if (
        normalize_architecture(receipt_architecture, platform_release.platform)
        != expected_architecture
    ):
        fail(f"{context} architecture does not match the requested architecture")
    receipt_commit = require_string(receipt, "commit", context)
    if receipt_commit.lower() != platform_release.commit:
        fail(f"{context} commit does not match package provenance")
    require_aware_timestamp(receipt.get("verifiedAt"), f"{context}.verifiedAt")
    verifier = receipt.get("verifier")
    if not isinstance(verifier, dict):
        fail(f"{context}.verifier must be an object")
    require_string(verifier, "name", f"{context}.verifier")
    require_string(verifier, "version", f"{context}.verifier")
    receipt_generator = receipt.get("receiptGenerator")
    if not isinstance(receipt_generator, dict):
        fail(f"{context}.receiptGenerator must be an object")
    require_string(receipt_generator, "name", f"{context}.receiptGenerator")
    require_string(receipt_generator, "version", f"{context}.receiptGenerator")

    policy = receipt.get("verificationPolicy")
    if not isinstance(policy, dict):
        fail(f"{context}.verificationPolicy must be an object")
    publisher_identity = policy.get("publisherIdentity")
    if not isinstance(publisher_identity, dict):
        fail(f"{context}.verificationPolicy.publisherIdentity must be an object")
    expected_identity_type = {
        "windows": "authenticode-signer-certificate-thumbprint",
        "macos": "apple-developer-team-id",
        "linux": "openpgp-primary-key-fingerprint",
    }[platform_release.platform]
    if publisher_identity.get("type") != expected_identity_type:
        fail(f"{context} publisher identity type must be {expected_identity_type!r}")
    receipt_identity = require_string(
        publisher_identity, "value", f"{context}.verificationPolicy.publisherIdentity"
    )
    if normalize_publisher_identity(receipt_identity, platform_release.platform) != (
        expected_publisher_identity
    ):
        fail(f"{context} publisher identity does not match the protected release policy")
    if platform_release.platform == "windows":
        expected_algorithm = "sha1" if len(expected_publisher_identity) == 40 else "sha256"
        if publisher_identity.get("algorithm") != expected_algorithm:
            fail(f"{context} Authenticode thumbprint algorithm must be {expected_algorithm}")
    if platform_release.platform == "linux":
        trusted_keyring = policy.get("trustedKeyring")
        if not isinstance(trusted_keyring, dict):
            fail(f"{context}.verificationPolicy.trustedKeyring must be an object")
        keyring_name = require_plain_filename(
            trusted_keyring.get("filename"),
            f"{context}.verificationPolicy.trustedKeyring.filename",
        )
        keyring_digest = require_string(
            trusted_keyring, "sha256", f"{context}.verificationPolicy.trustedKeyring"
        )
        if SHA256_PATTERN.fullmatch(keyring_digest) is None:
            fail(f"{context} trusted keyring SHA-256 is invalid")

    rows = receipt.get("artifacts")
    if not isinstance(rows, list) or not rows:
        fail(f"{context}.artifacts must be a non-empty array")
    actual: dict[str, str] = {}
    for index, value in enumerate(rows):
        if not isinstance(value, dict):
            fail(f"{context}.artifacts[{index}] must be an object")
        name = require_plain_filename(
            value.get("filename"), f"{context}.artifacts[{index}].filename"
        )
        digest = require_string(value, "sha256", f"{context}.artifacts[{index}]")
        if SHA256_PATTERN.fullmatch(digest) is None:
            fail(f"{context}.artifacts[{index}].sha256 is invalid")
        if name in actual:
            fail(f"{context} has duplicate artifact receipt for {name}")
        actual[name] = digest
    expected = {
        artifact.name: artifact.sha256 for artifact in platform_release.artifacts
    }
    if actual != expected:
        fail(f"{context} does not cover the exact verified package set")

    raw_signatures = receipt.get("signatures")
    if not isinstance(raw_signatures, list):
        fail(f"{context}.signatures must be an array")
    signature_claims: dict[str, Mapping[str, str]] = {}
    if platform_release.platform != "linux":
        if raw_signatures:
            fail(f"{context}.signatures must be empty for embedded-signature platforms")
    else:
        actual_signatures: dict[str, tuple[str, str]] = {}
        for index, value in enumerate(raw_signatures):
            row_context = f"{context}.signatures[{index}]"
            if not isinstance(value, dict):
                fail(f"{row_context} must be an object")
            filename = require_plain_filename(value.get("filename"), f"{row_context}.filename")
            signed_artifact = require_plain_filename(
                value.get("signedArtifact"), f"{row_context}.signedArtifact"
            )
            digest = require_string(value, "sha256", row_context)
            signer = require_string(value, "signerFingerprint", row_context).upper()
            trusted = require_string(value, "trustedFingerprint", row_context).upper()
            if SHA256_PATTERN.fullmatch(digest) is None:
                fail(f"{row_context}.sha256 is invalid")
            if OPENPGP_FINGERPRINT_PATTERN.fullmatch(signer) is None:
                fail(f"{row_context}.signerFingerprint is invalid")
            if trusted != expected_publisher_identity:
                fail(f"{row_context}.trustedFingerprint does not match release policy")
            if signed_artifact in actual_signatures:
                fail(f"{context} has duplicate signature evidence for {signed_artifact}")
            actual_signatures[signed_artifact] = (filename, digest)
            signature_claims[signed_artifact] = {
                "filename": filename,
                "sha256": digest,
                "signerFingerprint": signer,
                "trustedFingerprint": trusted,
            }
        expected_signatures = {
            signed_artifact: (signature.name, signature.sha256)
            for signed_artifact, signature in platform_release.signatures.items()
        }
        if actual_signatures != expected_signatures:
            fail(f"{context} does not bind the exact detached-signature set")
    validate_verification_details(
        receipt,
        context,
        platform_release,
        expected_architecture,
        expected_publisher_identity,
        policy,
        signature_claims,
    )
    return ValidatedVerificationReceipt(receipt_file, receipt)


def require_matching_security_claims(
    public: ValidatedVerificationReceipt,
    trusted: ValidatedVerificationReceipt,
    platform: str,
) -> None:
    for field in SECURITY_RELEVANT_RECEIPT_FIELDS:
        if public.document.get(field) != trusted.document.get(field):
            fail(
                f"{platform} public verification receipt security-relevant claim "
                f"{field!r} does not match the trusted native verification receipt"
            )


def validate_signing_state(platform: str, state: str) -> None:
    if state not in ALLOWED_SIGNING_STATES[platform]:
        allowed = ", ".join(sorted(ALLOWED_SIGNING_STATES[platform]))
        fail(f"{platform} signing state must be one of: {allowed}")


def build_manifest(
    *,
    repository: str,
    tag: str,
    source_root: Path,
    published_at: str,
    channel: str,
    package_directories: Mapping[str, Path],
    architectures: Mapping[str, str],
    signing_states: Mapping[str, str],
    verification_receipts: Mapping[str, Path | None],
    trusted_verification_receipts: Mapping[str, Path | None],
    publisher_identities: Mapping[str, str | None],
    verifier: Verifier = official_verifier,
    tag_resolver: TagResolver = resolve_git_tag_commit,
) -> dict[str, object]:
    repository = validate_repository(repository)
    if not tag.startswith("v") or SEMANTIC_VERSION.fullmatch(tag[1:]) is None:
        fail("tag must be v followed by a semantic version (for example, v1.2.3)")
    source_root = Path(source_root).resolve()
    if not source_root.is_dir():
        fail(f"source root does not exist: {source_root}")
    tag_commit = tag_resolver(source_root, tag).lower()
    if COMMIT_PATTERN.fullmatch(tag_commit) is None:
        fail("resolved Git tag commit must be a 40-to-64 character hexadecimal revision")
    requested_version = tag[1:]
    published_at = normalized_timestamp(published_at, "published-at")
    if channel not in {"preview", "stable"}:
        fail("channel must be preview or stable")

    for mapping_name, mapping in (
        ("package directories", package_directories),
        ("architectures", architectures),
        ("signing states", signing_states),
        ("public verification receipts", verification_receipts),
        ("trusted verification receipts", trusted_verification_receipts),
        ("publisher identities", publisher_identities),
    ):
        if set(mapping) != set(PLATFORMS):
            fail(f"{mapping_name} must specify exactly: {', '.join(PLATFORMS)}")

    releases = {
        platform: read_platform_release(
            Path(package_directories[platform]),
            platform,
            source_root,
            tag_commit,
            verifier=verifier,
        )
        for platform in PLATFORMS
    }
    versions = {release.version for release in releases.values()}
    commits = {release.commit for release in releases.values()}
    if len(versions) != 1:
        fail("platform provenance versions do not agree")
    if versions != {requested_version}:
        fail(
            f"requested tag {tag} does not match platform provenance version "
            f"{next(iter(versions))}"
        )
    if len(commits) != 1:
        fail("platform provenance commits do not agree")
    commit = next(iter(commits))
    if commit != tag_commit:
        fail("platform provenance commit does not match the Git tag target commit")

    expected_repository = repository.lower()
    for platform, release in releases.items():
        provenance_repository = normalize_github_repository(release.source_repository)
        if provenance_repository is None or provenance_repository.lower() != expected_repository:
            fail(
                f"{platform} provenance repository does not match requested repository "
                f"{repository}"
            )

    normalized_architectures = {
        platform: normalize_architecture(architectures[platform], platform)
        for platform in PLATFORMS
    }
    receipts: dict[str, EvidenceFile | None] = {}
    for platform, release in releases.items():
        state = signing_states[platform]
        validate_signing_state(platform, state)
        public_receipt_path = verification_receipts[platform]
        trusted_receipt_path = trusted_verification_receipts[platform]
        requires_receipt = platform in {"windows", "macos"} or state == "signed"
        if requires_receipt and public_receipt_path is None:
            fail(
                f"{platform} signing state {state!r} requires a public "
                "platform-verification receipt"
            )
        if requires_receipt and trusted_receipt_path is None:
            fail(
                f"{platform} signing state {state!r} requires a trusted native "
                "platform-verification receipt"
            )
        if not requires_receipt and (
            public_receipt_path is not None or trusted_receipt_path is not None
        ):
            fail(
                f"{platform} checksummed state must not include public or trusted "
                "signing receipts"
            )
        raw_publisher_identity = publisher_identities[platform]
        if requires_receipt:
            if not isinstance(raw_publisher_identity, str) or not raw_publisher_identity.strip():
                fail(f"{platform} signed publication requires a protected publisher identity")
            publisher_identity = normalize_publisher_identity(
                raw_publisher_identity, platform
            )
        else:
            if raw_publisher_identity is not None:
                fail(f"{platform} checksummed state must not claim a publisher identity")
            publisher_identity = None
        if not requires_receipt:
            if platform == "linux" and release.signatures:
                fail(
                    "linux checksummed state must not include detached signatures; "
                    "publish a trusted native verification receipt and use signed state"
                )
            if platform == "linux":
                verify_linux_package_architectures(
                    Path(package_directories[platform]).resolve(),
                    release.artifacts,
                    normalized_architectures[platform],
                )
            if release.provenance_architecture is None:
                fail(
                    f"{platform} checksummed state requires provenance platform.machine "
                    "to identify a supported architecture"
                )
            if release.provenance_architecture != normalized_architectures[platform]:
                fail(
                    f"{platform} requested architecture does not match provenance "
                    "platform.machine"
                )
        if state == "signed" and platform == "linux":
            unsigned = sorted(
                artifact.name
                for artifact in release.artifacts
                if artifact.name not in release.signatures
            )
            if unsigned:
                fail(
                    "linux signed state requires a detached signature for every package: "
                    + ", ".join(unsigned)
                )
        if public_receipt_path is not None and trusted_receipt_path is not None:
            assert publisher_identity is not None
            public_receipt = read_verification_receipt(
                Path(public_receipt_path),
                release,
                tag,
                VERIFICATION_TYPES[platform],
                normalized_architectures[platform],
                publisher_identity,
                "public",
            )
            trusted_receipt = read_verification_receipt(
                Path(trusted_receipt_path),
                release,
                tag,
                VERIFICATION_TYPES[platform],
                normalized_architectures[platform],
                publisher_identity,
                "trusted native",
            )
            if os.path.samefile(
                public_receipt.evidence.path, trusted_receipt.evidence.path
            ):
                fail(
                    f"{platform} public and trusted native verification receipts "
                    "must be separate files"
                )
            require_matching_security_claims(
                public_receipt, trusted_receipt, platform
            )
            receipts[platform] = public_receipt.evidence
        else:
            receipts[platform] = None

    # GitHub release assets occupy a single filename namespace. Check everything that
    # the generated URLs expect the publisher to upload, not only installer packages.
    upload_owners: dict[str, str] = {}
    for platform, release in releases.items():
        files = [artifact.name for artifact in release.artifacts]
        files.extend(signature.name for signature in release.signatures.values())
        files.extend(item.name for item in release.metadata.values())
        if receipts[platform] is not None:
            files.append(receipts[platform].name)
        for filename in files:
            previous = upload_owners.get(filename)
            if previous is not None:
                fail(
                    f"release upload filename {filename!r} is shared by {previous} and {platform}"
                )
            upload_owners[filename] = platform

    assets: list[dict[str, object]] = []
    asset_ids: set[str] = set()
    asset_urls: set[str] = set()
    for platform in PLATFORMS:
        release = releases[platform]
        architecture = normalized_architectures[platform]
        for artifact in release.artifacts:
            asset_id = f"{platform}-{architecture}-{artifact.website_format.replace('.', '-')}"
            if asset_id in asset_ids:
                fail(
                    f"multiple {platform} packages map to website asset id {asset_id!r}"
                )
            asset_ids.add(asset_id)
            url = release_asset_url(repository, tag, artifact.name)
            if url in asset_urls:
                fail(f"multiple packages map to website asset URL {url}")
            asset_urls.add(url)
            signature = release.signatures.get(artifact.name)
            receipt = receipts[platform]
            assets.append(
                {
                    "id": asset_id,
                    "os": platform,
                    "platformLabel": PLATFORM_LABELS[platform],
                    "arch": architecture,
                    "format": artifact.website_format,
                    "filename": artifact.name,
                    "url": url,
                    "available": True,
                    "sizeBytes": artifact.size_bytes,
                    "sha256": artifact.sha256,
                    "signingStatus": signing_states[platform],
                    "evidence": {
                        "checksums": evidence_json(
                            release.metadata["checksums"], repository, tag
                        ),
                        "sbom": evidence_json(release.metadata["sbom"], repository, tag),
                        "provenance": evidence_json(
                            release.metadata["provenance"], repository, tag
                        ),
                        "signature": (
                            evidence_json(signature, repository, tag)
                            if signature is not None
                            else None
                        ),
                        "verification": (
                            evidence_json(receipt, repository, tag)
                            if receipt is not None
                            else None
                        ),
                    },
                }
            )

    return {
        "schemaVersion": 2,
        "channel": channel,
        "status": "published",
        "version": requested_version,
        "tag": tag,
        "publishedAt": published_at,
        "commit": commit,
        "releaseUrl": f"https://github.com/{repository}/releases/tag/{quote(tag, safe='')}",
        "sourceUrl": f"https://github.com/{repository}",
        "assets": assets,
    }


def atomic_write_json(path: Path, value: Mapping[str, object]) -> None:
    path = path.resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        text=True,
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, indent=2, ensure_ascii=False)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary_path, 0o644)
        os.replace(temporary_path, path)
        try:
            directory_descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        except (AttributeError, OSError):
            return
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def publish_manifest(
    output: Path,
    manifest: Mapping[str, object],
    history_directory: Path | None = None,
) -> None:
    """Publish without downgrading a channel or displacing stable with preview."""

    output = output.resolve()
    history_directory = (
        history_directory.resolve()
        if history_directory is not None
        else output.parent / "release-history"
    )
    if manifest.get("schemaVersion") != 2 or manifest.get("status") != "published":
        fail("new website release manifest must be a published schemaVersion 2 manifest")
    new_version = manifest.get("version")
    if not isinstance(new_version, str) or SEMANTIC_VERSION.fullmatch(new_version) is None:
        fail("new published website manifest has an invalid version")
    new_channel = manifest.get("channel")
    if new_channel not in {"preview", "stable"}:
        fail("new published website manifest has an invalid channel")

    current: dict[str, object] | None = None
    current_status: object = None
    if output.exists():
        current = load_json_object(output, "current website release manifest")
        if current.get("schemaVersion") != 2:
            fail("current website release manifest has an unsupported schemaVersion")
        current_status = current.get("status")
        if current_status not in {"coming-soon", "published"}:
            fail("current website release manifest has an unsupported status")
        if current_status == "published":
            version = current.get("version")
            if not isinstance(version, str) or SEMANTIC_VERSION.fullmatch(version) is None:
                fail("current published website manifest has an invalid version")
            if version == new_version:
                duplicate_archive = history_directory / f"{new_version}.json"
                if duplicate_archive.exists():
                    fail(
                        f"published website release {new_version} exists as both current and historical"
                    )
                if current != manifest:
                    fail(
                        f"refusing to mutate already-published website release {version}"
                    )
                return

    existing_new_archive = history_directory / f"{new_version}.json"
    if existing_new_archive.exists():
        archived_new = load_json_object(
            existing_new_archive, "historical website release manifest"
        )
        if archived_new != manifest:
            fail(f"refusing to mutate already-published website release {new_version}")
        if current_status == "published":
            return
        fail(
            f"historical website release {new_version} exists without a published current manifest"
        )

    if current_status == "published" and current is not None:
        current_version = current["version"]
        current_channel = current.get("channel")
        if current_channel not in {"preview", "stable"}:
            fail("current published website manifest has an invalid channel")

        if current_channel == "stable" and new_channel == "preview":
            atomic_write_json(existing_new_archive, manifest)
            return
        if current_channel == new_channel and compare_semantic_versions(
            new_version, current_version
        ) <= 0:
            fail(
                f"refusing to replace current {current_channel} release {current_version} "
                f"with non-newer release {new_version}"
            )

        archive = history_directory / f"{current_version}.json"
        if archive.exists():
            archived = load_json_object(archive, "historical website release manifest")
            if archived != current:
                fail(
                    f"historical manifest conflict for published release {current_version}"
                )
        else:
            atomic_write_json(archive, current)
    atomic_write_json(output, manifest)


def argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Export verified Linux, macOS, and Windows desktop packages to the "
            "Astro website release manifest."
        )
    )
    parser.add_argument("--repository", required=True, help="GitHub OWNER/REPOSITORY")
    parser.add_argument("--tag", required=True, help="Release tag, exactly v<semantic-version>")
    parser.add_argument(
        "--source-root",
        required=True,
        type=Path,
        help="Git source checkout containing the release tag and provenance inputs",
    )
    parser.add_argument("--published-at", required=True, help="Timezone-aware ISO 8601 timestamp")
    parser.add_argument(
        "--trusted-windows-signer-thumbprint",
        required=True,
        help="Protected SHA-1 or SHA-256 Authenticode signer thumbprint",
    )
    parser.add_argument(
        "--trusted-apple-team-id",
        required=True,
        help="Protected 10-character Apple Developer Team ID",
    )
    parser.add_argument(
        "--trusted-linux-signing-fingerprint",
        help="Protected OpenPGP fingerprint; required when Linux is signed",
    )
    parser.add_argument("--channel", required=True, choices=("preview", "stable"))
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument(
        "--history-dir",
        type=Path,
        help="Published release archive (defaults to release-history beside --output)",
    )
    for platform in PLATFORMS:
        parser.add_argument(
            f"--{platform}-package-dir",
            required=True,
            type=Path,
            help=f"Verified {PLATFORM_LABELS[platform]} package directory",
        )
        parser.add_argument(
            f"--{platform}-arch",
            required=True,
            help=f"Architecture shared by packages in the {platform} directory",
        )
        parser.add_argument(
            f"--{platform}-signing-state",
            required=True,
            choices=tuple(sorted(ALLOWED_SIGNING_STATES[platform])),
        )
        parser.add_argument(
            f"--{platform}-verification-receipt",
            type=Path,
            help=(
                f"Immutable public release verification receipt named "
                f"{VERIFICATION_RECEIPT_FILENAMES[platform]}"
            ),
        )
        parser.add_argument(
            f"--{platform}-trusted-verification-receipt",
            type=Path,
            help=(
                f"Trusted native-rerun receipt named "
                f"{VERIFICATION_RECEIPT_FILENAMES[platform]}; required whenever the "
                "public receipt is required"
            ),
        )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    try:
        package_directories = {
            platform: getattr(args, f"{platform}_package_dir")
            for platform in PLATFORMS
        }
        architectures = {
            platform: getattr(args, f"{platform}_arch") for platform in PLATFORMS
        }
        signing_states = {
            platform: getattr(args, f"{platform}_signing_state")
            for platform in PLATFORMS
        }
        verification_receipts = {
            platform: getattr(args, f"{platform}_verification_receipt")
            for platform in PLATFORMS
        }
        trusted_verification_receipts = {
            platform: getattr(args, f"{platform}_trusted_verification_receipt")
            for platform in PLATFORMS
        }
        publisher_identities = {
            "windows": args.trusted_windows_signer_thumbprint,
            "macos": args.trusted_apple_team_id,
            "linux": args.trusted_linux_signing_fingerprint,
        }
        manifest = build_manifest(
            repository=args.repository,
            tag=args.tag,
            source_root=args.source_root,
            published_at=args.published_at,
            channel=args.channel,
            package_directories=package_directories,
            architectures=architectures,
            signing_states=signing_states,
            verification_receipts=verification_receipts,
            trusted_verification_receipts=trusted_verification_receipts,
            publisher_identities=publisher_identities,
        )
        publish_manifest(args.output, manifest, args.history_dir)
    except ManifestExportError as error:
        parser.exit(2, f"release manifest export failed: {error}\n")
    print(f"website release manifest exported: {args.output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise SystemExit(1)
