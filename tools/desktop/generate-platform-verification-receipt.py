#!/usr/bin/env python3
"""Create a fail-closed receipt from platform-native release verification.

This command is intentionally run *after* portable release-metadata verification.
It never accepts a caller's assertion that a package is signed. Instead, it stages
the exact package bytes, invokes the native platform verifier, checks that the
source bytes did not change during verification, and atomically writes a receipt
consumed by ``export-website-release-manifest.py``.

Run Windows verification on Windows, macOS verification on macOS, and Linux
verification on Linux. Linux verification uses only the explicitly supplied
OpenPGP keyring and requires every package signature to resolve to the explicitly
supplied public-key fingerprint.

Windows and macOS verification also require caller-pinned publisher identities.
Every Authenticode payload must use the pinned certificate thumbprint, and the
signed DMG plus every mounted app must use the pinned Apple Developer Team ID.
The normalized identity policy is persisted in the receipt for auditability.

The receipt architecture is evidence-derived rather than copied from the CLI:
Windows uses every signed PE COFF header (or the signed MSI Template Summary),
macOS uses ``lipo -archs`` on every regular Mach-O payload in each verified app,
and Linux uses the ELF machine field in the signed AppImage or every ELF in the
signed tar. The command rejects unsupported, mixed, universal, or
caller-mismatched architectures.
Linux receipts also expose the exact detached-signature set at top level so a
downstream publisher can reject signature omission or substitution directly.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import re
import shutil
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Callable, Mapping, Protocol, Sequence

import release_targets

SCHEMA_VERSION = "chaft.desktop.platform-verification.v2"
SCRIPT_VERSION = "1.4.0"
PLATFORMS = ("windows", "macos", "linux")
VERIFICATION_TYPES = {
    "windows": "authenticode",
    "macos": "apple-notarization",
    "linux": "detached-signature",
}
RECEIPT_FILENAMES = {
    target.name: target.verification_receipt_name
    for target in release_targets.TARGETS
}
RECEIPT_FILENAMES.update(
    {
        "windows": RECEIPT_FILENAMES["windows-x86_64"],
        "macos": RECEIPT_FILENAMES["macos-x86_64"],
        "linux": RECEIPT_FILENAMES["linux-x86_64"],
    }
)
PACKAGE_SUFFIXES = {
    "windows": (".zip", ".msi", ".exe"),
    "macos": (".dmg",),
    "linux": (".tar.gz", ".tgz", ".appimage"),
}
SIGNATURE_SUFFIXES = (".sig", ".asc")
WINDOWS_SIGNABLE_SUFFIXES = (
    ".exe",
    ".dll",
    ".sys",
    ".ocx",
    ".cpl",
    ".scr",
    ".efi",
    ".msi",
    ".cat",
    ".cab",
    ".ps1",
    ".psm1",
    ".psd1",
    ".ps1xml",
)
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
OPENPGP_FINGERPRINT_PATTERN = re.compile(r"^(?:[0-9A-F]{40}|[0-9A-F]{64})$")
AUTHENTICODE_THUMBPRINT_PATTERN = re.compile(r"^(?:[0-9A-F]{40}|[0-9A-F]{64})$")
APPLE_TEAM_ID_PATTERN = re.compile(r"^[A-Z0-9]{10}$")
MACOS_APPLICATION_BUNDLE_NAME = "Chaft.app"
MACOS_APPLICATION_EXECUTABLE_NAME = "Chaft"
MACOS_APPLICATION_ICON_NAME = "Chaft.icns"
MACOS_APPLICATION_BUNDLE_IDENTIFIER = "app.chaft.desktop"
MACH_O_MAGICS = frozenset(
    {
        b"\xca\xfe\xba\xbe",
        b"\xbe\xba\xfe\xca",
        b"\xca\xfe\xba\xbf",
        b"\xbf\xba\xfe\xca",
        b"\xfe\xed\xfa\xce",
        b"\xce\xfa\xed\xfe",
        b"\xfe\xed\xfa\xcf",
        b"\xcf\xfa\xed\xfe",
    }
)
WINDOWS_RESERVED_NAMES = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    *(f"COM{number}" for number in range(1, 10)),
    *(f"LPT{number}" for number in range(1, 10)),
}

# A release ZIP should be nowhere near these bounds. They are deliberately finite
# so verification does not become an unbounded ZIP decompression primitive.
MAX_ZIP_ENTRIES = 20_000
MAX_ZIP_ENTRY_BYTES = 2 * 1024 * 1024 * 1024
MAX_ZIP_TOTAL_BYTES = 4 * 1024 * 1024 * 1024
MAX_ZIP_COMPRESSION_RATIO = 200
ZIP_RATIO_ALLOWANCE_BYTES = 1024 * 1024
MAX_TAR_ENTRIES = 20_000
MAX_TAR_ENTRY_BYTES = 2 * 1024 * 1024 * 1024
MAX_TAR_TOTAL_BYTES = 4 * 1024 * 1024 * 1024

PE_MACHINE_ARCHITECTURES = {
    0x8664: "x86_64",  # IMAGE_FILE_MACHINE_AMD64
    0xAA64: "arm64",  # IMAGE_FILE_MACHINE_ARM64
    0xA641: "arm64",  # IMAGE_FILE_MACHINE_ARM64EC
    0xA64E: "arm64",  # IMAGE_FILE_MACHINE_ARM64X
}
ELF_MACHINE_ARCHITECTURES = {
    62: "x86_64",  # EM_X86_64
    183: "arm64",  # EM_AARCH64
}


class VerificationError(ValueError):
    """Native verification failed or its result could not be trusted."""


@dataclass(frozen=True)
class CommandResult:
    returncode: int
    stdout: str = ""
    stderr: str = ""


class Runner(Protocol):
    def run(self, args: Sequence[str]) -> CommandResult:
        """Run a command without a shell and return captured text output."""


class SubprocessRunner:
    def run(self, args: Sequence[str]) -> CommandResult:
        try:
            completed = subprocess.run(
                [str(value) for value in args],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                shell=False,
            )
        except OSError as error:
            raise VerificationError(f"could not execute {args[0]}: {error}") from error
        return CommandResult(completed.returncode, completed.stdout, completed.stderr)


@dataclass(frozen=True)
class StagedPackage:
    source: Path
    staged: Path
    filename: str
    sha256: str


@dataclass(frozen=True)
class MacApplicationBundle:
    executable: Path
    icon: Path
    bundle_identifier: str
    short_version: str
    bundle_version: str


def fail(message: str) -> None:
    raise VerificationError(message)


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_starts_with(path: Path, prefix: bytes) -> bool:
    with path.open("rb") as handle:
        return handle.read(len(prefix)) == prefix


def normalize_architecture(value: str) -> str:
    normalized = ARCHITECTURE_ALIASES.get(value.strip().lower())
    if normalized is None:
        fail(
            f"unsupported architecture {value!r}; expected one of: "
            + ", ".join(sorted(ARCHITECTURE_ALIASES))
        )
    return normalized


def normalize_authenticode_thumbprint(value: str) -> str:
    normalized = re.sub(r"\s+", "", value).upper()
    if AUTHENTICODE_THUMBPRINT_PATTERN.fullmatch(normalized) is None:
        fail(
            "trusted Authenticode signer thumbprint must be exactly 40 or 64 "
            "hexadecimal characters"
        )
    return normalized


def normalize_apple_team_id(value: str) -> str:
    normalized = value.strip().upper()
    if APPLE_TEAM_ID_PATTERN.fullmatch(normalized) is None:
        fail("trusted Apple Developer Team ID must be exactly 10 letters or digits")
    return normalized


def verified_architecture(observed: Sequence[str], expected: str, context: str) -> str:
    unique = set(observed)
    if not unique:
        fail(f"could not determine a signed payload architecture for {context}")
    if not unique <= {"x86_64", "arm64", "universal"}:
        fail(f"unsupported payload architecture for {context}: {', '.join(sorted(unique))}")
    if "universal" in unique or unique == {"x86_64", "arm64"}:
        actual = "universal"
    elif len(unique) == 1:
        actual = next(iter(unique))
    else:
        fail(f"incoherent payload architectures for {context}: {', '.join(sorted(unique))}")
    if actual != expected:
        fail(
            f"verified payload architecture for {context} is {actual}, "
            f"not the requested {expected}"
        )
    return actual


def pe_architecture(path: Path) -> str:
    try:
        with path.open("rb") as handle:
            header = handle.read(64)
            if len(header) < 64 or header[:2] != b"MZ":
                fail(f"PE payload has an invalid DOS header: {path.name}")
            pe_offset = struct.unpack_from("<I", header, 0x3C)[0]
            if pe_offset < 64 or pe_offset > path.stat().st_size - 6:
                fail(f"PE payload has an invalid PE offset: {path.name}")
            handle.seek(pe_offset)
            coff = handle.read(6)
    except OSError as error:
        fail(f"could not read PE payload {path.name}: {error}")
    if len(coff) != 6 or coff[:4] != b"PE\x00\x00":
        fail(f"PE payload has an invalid COFF header: {path.name}")
    machine = struct.unpack_from("<H", coff, 4)[0]
    architecture = PE_MACHINE_ARCHITECTURES.get(machine)
    if architecture is None:
        fail(f"PE payload {path.name} uses unsupported COFF machine 0x{machine:04x}")
    return architecture


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


def normalized_host_platform(value: str) -> str:
    lowered = value.strip().lower()
    if lowered.startswith("win"):
        return "windows"
    if lowered in {"darwin", "mac", "macos", "osx"}:
        return "macos"
    if lowered.startswith("linux"):
        return "linux"
    return lowered


def package_suffix(filename: str, platform: str) -> str | None:
    lowered = filename.lower()
    return next(
        (suffix for suffix in PACKAGE_SUFFIXES[platform] if lowered.endswith(suffix)),
        None,
    )


def discover_packages(package_dir: Path, platform: str) -> list[Path]:
    if not package_dir.is_dir():
        fail(f"package directory not found: {package_dir}")
    packages: list[Path] = []
    names: set[str] = set()
    for path in package_dir.iterdir():
        if package_suffix(path.name, platform) is None:
            continue
        if path.is_symlink() or not path.is_file():
            fail(f"package must be a regular, non-symlink file: {path.name}")
        folded = path.name.casefold()
        if folded in names:
            fail(f"package filenames are ambiguous when compared case-insensitively: {path.name}")
        names.add(folded)
        if path.stat().st_size <= 0:
            fail(f"package is empty: {path.name}")
        packages.append(path)
    if not packages:
        fail(f"{platform} package directory contains no supported package")
    return sorted(packages, key=lambda path: path.name)


def copy_and_hash(source: Path, destination: Path) -> str:
    digest = hashlib.sha256()
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with source.open("rb") as input_handle, os.fdopen(descriptor, "wb") as output_handle:
            descriptor = -1
            for chunk in iter(lambda: input_handle.read(1024 * 1024), b""):
                digest.update(chunk)
                output_handle.write(chunk)
            output_handle.flush()
            os.fsync(output_handle.fileno())
    finally:
        if descriptor >= 0:
            os.close(descriptor)
    return digest.hexdigest()


def stage_packages(packages: Sequence[Path], staging_dir: Path) -> list[StagedPackage]:
    staged: list[StagedPackage] = []
    for source in packages:
        if source.is_symlink() or not source.is_file():
            fail(f"package changed into a symlink or non-file before staging: {source.name}")
        before = source.stat()
        destination = staging_dir / "packages" / source.name
        digest = copy_and_hash(source, destination)
        after = source.stat()
        stable_fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
        if source.is_symlink() or any(
            getattr(before, field) != getattr(after, field) for field in stable_fields
        ):
            fail(f"package changed while it was staged: {source.name}")
        if file_sha256(source) != digest:
            fail(f"package changed while it was staged: {source.name}")
        staged.append(StagedPackage(source, destination, source.name, digest))
    return staged


def ensure_sources_unchanged(packages: Sequence[StagedPackage]) -> None:
    for package in packages:
        if package.source.is_symlink() or not package.source.is_file():
            fail(f"package changed into a symlink or non-file: {package.filename}")
        if file_sha256(package.source) != package.sha256:
            fail(f"package changed during native verification: {package.filename}")


def command_text(result: CommandResult) -> str:
    return (result.stderr or result.stdout).strip().replace("\n", " ")[:1000]


def run_checked(runner: Runner, args: Sequence[str], context: str) -> CommandResult:
    result = runner.run([str(value) for value in args])
    if result.returncode != 0:
        detail = command_text(result)
        suffix = f": {detail}" if detail else ""
        fail(f"{context} failed with exit code {result.returncode}{suffix}")
    return result


def require_tool(
    names: Sequence[str], which: Callable[[str], str | None]
) -> str:
    for name in names:
        resolved = which(name)
        if resolved:
            return resolved
    fail(f"required native verification tool not found: {' or '.join(names)}")


def authenticode_result(
    powershell: str,
    verifier_script: Path,
    path: Path,
    runner: Runner,
    trusted_signer_thumbprint: str,
) -> Mapping[str, str]:
    result = run_checked(
        runner,
        [
            powershell,
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-File",
            str(verifier_script),
            "-Path",
            str(path),
        ],
        f"Authenticode verification for {path.name}",
    )
    try:
        value = json.loads(result.stdout.lstrip("\ufeff"))
    except json.JSONDecodeError as error:
        fail(f"Authenticode verifier returned invalid JSON for {path.name}: {error}")
    if not isinstance(value, dict):
        fail(f"Authenticode verifier returned a non-object for {path.name}")
    status_value = value.get("Status")
    signature_type_value = value.get("SignatureType")
    thumbprint_sha1_value = value.get("SignerThumbprintSha1")
    thumbprint_sha256_value = value.get("SignerThumbprintSha256")
    msi_template_value = value.get("MsiTemplate")
    status = status_value.strip() if isinstance(status_value, str) else ""
    signature_type = (
        signature_type_value.strip() if isinstance(signature_type_value, str) else ""
    )
    thumbprint_sha1 = (
        re.sub(r"\s+", "", thumbprint_sha1_value).upper()
        if isinstance(thumbprint_sha1_value, str)
        else ""
    )
    thumbprint_sha256 = (
        re.sub(r"\s+", "", thumbprint_sha256_value).upper()
        if isinstance(thumbprint_sha256_value, str)
        else ""
    )
    if status != "Valid":
        message = value.get("StatusMessage")
        detail = message.strip() if isinstance(message, str) else ""
        fail(
            f"Authenticode signature is not valid for {path.name}: "
            f"{status or detail or 'unknown'}"
        )
    if not signature_type or signature_type.lower() == "none":
        fail(f"Authenticode signature type is missing for {path.name}")
    if re.fullmatch(r"[0-9A-F]{40}", thumbprint_sha1) is None:
        fail(f"Authenticode signer SHA-1 thumbprint is missing or invalid for {path.name}")
    if re.fullmatch(r"[0-9A-F]{64}", thumbprint_sha256) is None:
        fail(f"Authenticode signer SHA-256 thumbprint is missing or invalid for {path.name}")
    algorithm = "sha1" if len(trusted_signer_thumbprint) == 40 else "sha256"
    actual_thumbprint = (
        thumbprint_sha1 if algorithm == "sha1" else thumbprint_sha256
    )
    if actual_thumbprint != trusted_signer_thumbprint:
        fail(
            f"Authenticode signer identity mismatch for {path.name}: "
            f"got {actual_thumbprint}, "
            f"expected {trusted_signer_thumbprint}"
        )
    result_value = {
        "payload": path.name,
        "signatureType": signature_type,
        "signerThumbprint": actual_thumbprint,
        "signerThumbprintAlgorithm": algorithm,
        "signerCertificateSha1": thumbprint_sha1,
        "signerCertificateSha256": thumbprint_sha256,
    }
    if path.name.lower().endswith(".msi"):
        msi_template = (
            msi_template_value.strip() if isinstance(msi_template_value, str) else ""
        )
        if not msi_template:
            fail(f"Windows Installer architecture metadata is missing for {path.name}")
        result_value["msiTemplate"] = msi_template
    return result_value


def msi_architecture(template: str, filename: str) -> str:
    machine = template.split(";", 1)[0].strip().lower()
    if machine in {"x64", "amd64", "intel64"}:
        return "x86_64"
    if machine in {"arm64", "aarch64"}:
        return "arm64"
    fail(f"Windows Installer {filename} has unsupported template architecture {template!r}")


def safe_zip_entries(archive: zipfile.ZipFile, package_name: str) -> list[zipfile.ZipInfo]:
    infos = archive.infolist()
    if len(infos) > MAX_ZIP_ENTRIES:
        fail(f"Windows ZIP {package_name} has too many entries")
    total_size = 0
    seen: set[str] = set()
    result: list[zipfile.ZipInfo] = []
    for info in infos:
        raw_name = info.filename
        if "\x00" in raw_name:
            fail(f"Windows ZIP {package_name} contains a NUL in an entry name")
        normalized_name = raw_name.replace("\\", "/")
        comparable_name = normalized_name[:-1] if normalized_name.endswith("/") else normalized_name
        raw_parts = comparable_name.split("/")
        if (
            not normalized_name
            or normalized_name.startswith("/")
            or re.match(r"^[A-Za-z]:", normalized_name)
            or any(part in {"", ".", ".."} for part in raw_parts)
            or any(
                ":" in part
                or part.endswith((".", " "))
                or any(ord(character) < 32 for character in part)
                or part.split(".", 1)[0].upper() in WINDOWS_RESERVED_NAMES
                for part in raw_parts
            )
        ):
            fail(f"Windows ZIP {package_name} contains unsafe entry {raw_name!r}")
        folded = normalized_name.casefold().rstrip("/")
        if folded in seen:
            fail(f"Windows ZIP {package_name} contains duplicate entry {raw_name!r}")
        seen.add(folded)
        unix_mode = info.external_attr >> 16
        file_type = stat.S_IFMT(unix_mode)
        if file_type == stat.S_IFLNK:
            fail(f"Windows ZIP {package_name} contains symlink entry {raw_name!r}")
        if file_type not in {0, stat.S_IFREG, stat.S_IFDIR}:
            fail(f"Windows ZIP {package_name} contains special entry {raw_name!r}")
        if info.flag_bits & 0x1:
            fail(f"Windows ZIP {package_name} contains encrypted entry {raw_name!r}")
        if info.is_dir() or normalized_name.endswith("/"):
            continue
        if info.file_size < 0 or info.file_size > MAX_ZIP_ENTRY_BYTES:
            fail(f"Windows ZIP {package_name} entry is too large: {raw_name}")
        total_size += info.file_size
        if total_size > MAX_ZIP_TOTAL_BYTES:
            fail(f"Windows ZIP {package_name} expands beyond the verification limit")
        if info.file_size > ZIP_RATIO_ALLOWANCE_BYTES and info.file_size > max(
            1, info.compress_size
        ) * MAX_ZIP_COMPRESSION_RATIO:
            fail(f"Windows ZIP {package_name} has a suspicious compression ratio: {raw_name}")
        result.append(info)
    return result


def extract_windows_payloads(zip_path: Path, destination: Path) -> list[Path]:
    payloads: list[Path] = []
    try:
        archive_context = zipfile.ZipFile(zip_path)
    except (OSError, zipfile.BadZipFile, zipfile.LargeZipFile) as error:
        fail(f"Windows package is not a valid ZIP: {zip_path.name}: {error}")
    with archive_context as archive:
        for info in safe_zip_entries(archive, zip_path.name):
            lowered = info.filename.lower()
            with archive.open(info, "r") as input_handle:
                magic = input_handle.read(2)
            signable_suffix = lowered.endswith(WINDOWS_SIGNABLE_SUFFIXES)
            if signable_suffix and lowered.endswith(
                (".exe", ".dll", ".sys", ".ocx", ".cpl", ".scr", ".efi")
            ) and magic != b"MZ":
                fail(f"Windows ZIP signable PE payload has an invalid header: {info.filename}")
            if magic != b"MZ" and not signable_suffix:
                continue
            relative = PurePosixPath(info.filename.replace("\\", "/"))
            output = destination.joinpath(*relative.parts)
            output.parent.mkdir(parents=True, exist_ok=True)
            descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            try:
                with archive.open(info, "r") as input_handle, os.fdopen(
                    descriptor, "wb"
                ) as output_handle:
                    descriptor = -1
                    shutil.copyfileobj(input_handle, output_handle, length=1024 * 1024)
            finally:
                if descriptor >= 0:
                    os.close(descriptor)
            if output.stat().st_size != info.file_size:
                fail(f"Windows ZIP payload size changed during extraction: {info.filename}")
            payloads.append(output)
    if not payloads:
        fail(f"Windows ZIP contains no Authenticode-verifiable payload: {zip_path.name}")
    return sorted(payloads, key=lambda path: path.as_posix())


def verify_windows(
    packages: Sequence[StagedPackage],
    runner: Runner,
    which: Callable[[str], str | None],
    working_dir: Path,
    expected_architecture: str,
    trusted_signer_thumbprint: str,
) -> tuple[dict[str, str], list[dict[str, object]], dict[str, object]]:
    powershell = require_tool(("powershell.exe", "pwsh.exe", "powershell", "pwsh"), which)
    version_script = "$PSVersionTable.PSVersion.ToString()"
    version_result = run_checked(
        runner,
        [powershell, "-NoLogo", "-NoProfile", "-NonInteractive", "-Command", version_script],
        "PowerShell version query",
    )
    version = version_result.stdout.strip().splitlines()[0] if version_result.stdout.strip() else ""
    if not version:
        fail("PowerShell did not report a verifier version")

    verifier_script = working_dir / "verify-authenticode.ps1"
    verifier_script.write_text(
        "param([Parameter(Mandatory=$true)][string]$Path)\n"
        "$ErrorActionPreference = 'Stop'\n"
        "$Signature = Get-AuthenticodeSignature -LiteralPath $Path\n"
        "$ThumbprintSha1 = ''\n"
        "$ThumbprintSha256 = ''\n"
        "$MsiTemplate = ''\n"
        "if ($null -ne $Signature.SignerCertificate) {\n"
        "  $ThumbprintSha1 = [string]$Signature.SignerCertificate.Thumbprint\n"
        "  $Sha256 = [System.Security.Cryptography.SHA256]::Create()\n"
        "  $ThumbprintSha256 = [System.BitConverter]::ToString(\n"
        "    $Sha256.ComputeHash($Signature.SignerCertificate.RawData)).Replace('-', '')\n"
        "  $Sha256.Dispose()\n"
        "}\n"
        "if ([System.IO.Path]::GetExtension($Path) -ieq '.msi') {\n"
        "  $Installer = New-Object -ComObject WindowsInstaller.Installer\n"
        "  $Database = $Installer.GetType().InvokeMember(\n"
        "    'OpenDatabase', [System.Reflection.BindingFlags]::InvokeMethod,\n"
        "    $null, $Installer, @($Path, 0))\n"
        "  $Summary = $Database.GetType().InvokeMember(\n"
        "    'SummaryInformation', [System.Reflection.BindingFlags]::GetProperty,\n"
        "    $null, $Database, @(0))\n"
        "  $MsiTemplate = [string]$Summary.GetType().InvokeMember(\n"
        "    'Property', [System.Reflection.BindingFlags]::GetProperty,\n"
        "    $null, $Summary, @(7))\n"
        "}\n"
        "[pscustomobject]@{\n"
        "  Status = [string]$Signature.Status\n"
        "  StatusMessage = [string]$Signature.StatusMessage\n"
        "  SignatureType = [string]$Signature.SignatureType\n"
        "  SignerThumbprintSha1 = $ThumbprintSha1\n"
        "  SignerThumbprintSha256 = $ThumbprintSha256\n"
        "  MsiTemplate = $MsiTemplate\n"
        "} | ConvertTo-Json -Compress\n",
        encoding="utf-8",
    )
    os.chmod(verifier_script, 0o600)

    package_details: list[dict[str, object]] = []
    all_architectures: list[str] = []
    for package in packages:
        suffix = package_suffix(package.filename, "windows")
        payloads: list[Path]
        if suffix == ".zip":
            extraction_root = working_dir / "windows-zip" / package.sha256
            extraction_root.mkdir(parents=True, exist_ok=False)
            payloads = extract_windows_payloads(package.staged, extraction_root)
        else:
            payloads = [package.staged]
        verified_payloads: list[dict[str, str]] = []
        package_architectures: list[str] = []
        for payload in payloads:
            result = dict(
                authenticode_result(
                    powershell,
                    verifier_script,
                    payload,
                    runner,
                    trusted_signer_thumbprint,
                )
            )
            architecture: str | None = None
            if payload.name.lower().endswith(".msi"):
                architecture = msi_architecture(result["msiTemplate"], payload.name)
            elif payload.name.lower().endswith(
                (".exe", ".dll", ".sys", ".ocx", ".cpl", ".scr", ".efi")
            ) or file_starts_with(payload, b"MZ"):
                architecture = pe_architecture(payload)
            # Catalogs, cabinets, and signed PowerShell files contribute signing
            # evidence but cannot independently establish package architecture.
            if architecture is not None:
                result["architecture"] = architecture
                package_architectures.append(architecture)
            verified_payloads.append(result)
        package_architecture = verified_architecture(
            package_architectures, expected_architecture, package.filename
        )
        all_architectures.extend(package_architectures)
        package_details.append(
            {
                "filename": package.filename,
                "architecture": package_architecture,
                "verifiedPayloads": verified_payloads,
            }
        )
    verified_architecture(all_architectures, expected_architecture, "Windows package set")
    return (
        {"name": "PowerShell Get-AuthenticodeSignature", "version": version},
        package_details,
        {
            "publisherIdentity": {
                "type": "authenticode-signer-certificate-thumbprint",
                "value": trusted_signer_thumbprint,
                "algorithm": (
                    "sha1" if len(trusted_signer_thumbprint) == 40 else "sha256"
                ),
            }
        },
    )


def discover_mounted_apps(mountpoint: Path) -> list[Path]:
    apps: list[Path] = []
    for root, directories, _files in os.walk(mountpoint, topdown=True, followlinks=False):
        root_path = Path(root)
        retained: list[str] = []
        for name in directories:
            candidate = root_path / name
            if candidate.is_symlink():
                continue
            if name.lower().endswith(".app"):
                apps.append(candidate)
            else:
                retained.append(name)
        directories[:] = retained
    return sorted(apps, key=lambda path: path.as_posix())


def parse_hdiutil_mount(stdout: str, expected_mountpoint: Path, package_name: str) -> None:
    try:
        value = plistlib.loads(stdout.encode("utf-8"))
    except (ValueError, plistlib.InvalidFileException) as error:
        fail(f"hdiutil returned invalid plist for {package_name}: {error}")
    if not isinstance(value, dict):
        fail(f"hdiutil returned a non-object plist for {package_name}")
    entities = value.get("system-entities")
    if not isinstance(entities, list):
        fail(f"hdiutil did not report mounted entities for {package_name}")
    mountpoints = {
        Path(item["mount-point"]).resolve()
        for item in entities
        if isinstance(item, dict) and isinstance(item.get("mount-point"), str)
    }
    if expected_mountpoint.resolve() not in mountpoints:
        fail(f"hdiutil did not mount {package_name} at the requested mount point")


def inspect_application_bundle(
    app: Path,
    expected_version: str,
) -> MacApplicationBundle:
    if app.name != MACOS_APPLICATION_BUNDLE_NAME:
        fail(
            "macOS application bundle must be named "
            f"{MACOS_APPLICATION_BUNDLE_NAME}: {app.name}"
        )
    info_path = app / "Contents" / "Info.plist"
    if info_path.is_symlink() or not info_path.is_file():
        fail(f"application bundle has no regular Contents/Info.plist: {app.name}")
    try:
        info = plistlib.loads(info_path.read_bytes())
    except (OSError, ValueError, plistlib.InvalidFileException) as error:
        fail(f"application bundle has an invalid Info.plist ({app.name}): {error}")
    if not isinstance(info, dict):
        fail(f"application bundle Info.plist is not an object: {app.name}")
    expected_info = {
        "CFBundleName": "Chaft",
        "CFBundleExecutable": MACOS_APPLICATION_EXECUTABLE_NAME,
        "CFBundleIconFile": MACOS_APPLICATION_ICON_NAME,
        "CFBundleIdentifier": MACOS_APPLICATION_BUNDLE_IDENTIFIER,
        "CFBundleShortVersionString": expected_version,
        "CFBundleVersion": expected_version,
    }
    for key, expected_value in expected_info.items():
        value = info.get(key)
        if value != expected_value:
            fail(
                f"application bundle {key} must be {expected_value!r}: "
                f"{app.name}"
            )
    icon = app / "Contents" / "Resources" / MACOS_APPLICATION_ICON_NAME
    if icon.is_symlink() or not icon.is_file() or icon.stat().st_size == 0:
        fail(f"application bundle icon is missing or empty: {app.name}")
    try:
        with icon.open("rb") as handle:
            icon_header = handle.read(8)
    except OSError as error:
        fail(f"application bundle icon cannot be read ({app.name}): {error}")
    if len(icon_header) != 8 or icon_header[:4] != b"icns":
        fail(f"application bundle icon is not an ICNS file: {app.name}")
    declared_icon_size = struct.unpack(">I", icon_header[4:])[0]
    if declared_icon_size != icon.stat().st_size:
        fail(
            f"application bundle icon length is incoherent: {app.name}"
        )
    executable_name = info.get("CFBundleExecutable")
    if (
        not isinstance(executable_name, str)
        or not executable_name
        or executable_name in {".", ".."}
        or "/" in executable_name
        or "\\" in executable_name
    ):
        fail(f"application bundle has an invalid CFBundleExecutable: {app.name}")
    executable = app / "Contents" / "MacOS" / executable_name
    if executable.is_symlink() or not executable.is_file():
        fail(f"application bundle main executable is missing or a symlink: {app.name}")
    return MacApplicationBundle(
        executable=executable,
        icon=icon,
        bundle_identifier=MACOS_APPLICATION_BUNDLE_IDENTIFIER,
        short_version=expected_version,
        bundle_version=expected_version,
    )


def discover_macho_payloads(app: Path, executable: Path) -> list[Path]:
    candidates = {executable}
    for root, directories, filenames in os.walk(
        app,
        topdown=True,
        followlinks=False,
    ):
        root_path = Path(root)
        directories[:] = [
            name
            for name in directories
            if not (root_path / name).is_symlink()
        ]
        for name in filenames:
            path = root_path / name
            if path.is_symlink():
                continue
            try:
                mode = path.stat().st_mode
                if not stat.S_ISREG(mode):
                    continue
                with path.open("rb") as handle:
                    magic = handle.read(4)
            except OSError as error:
                fail(f"cannot inspect bundled file {path}: {error}")
            if magic in MACH_O_MAGICS:
                candidates.add(path)
    return sorted(candidates, key=lambda path: path.relative_to(app).as_posix())


def inspect_macho_payloads(
    app: Path,
    executable: Path,
    lipo: str,
    runner: Runner,
    expected_architecture: str,
) -> list[dict[str, str]]:
    details: list[dict[str, str]] = []
    for path in discover_macho_payloads(app, executable):
        relative = path.relative_to(app).as_posix()
        result = run_checked(
            runner,
            [lipo, "-archs", str(path)],
            f"Mach-O architecture inspection for {relative}",
        )
        architectures = parse_lipo_architectures(result.stdout, path)
        architecture = verified_architecture(
            architectures,
            expected_architecture,
            relative,
        )
        details.append(
            {
                "path": relative,
                "architecture": architecture,
            }
        )
    if not details:
        fail(f"application bundle contains no inspectable Mach-O payload: {app.name}")
    executable_relative = executable.relative_to(app).as_posix()
    if not any(row["path"] == executable_relative for row in details):
        fail(f"application bundle main executable was not inspected: {app.name}")
    return details


def parse_lipo_architectures(stdout: str, executable: Path) -> list[str]:
    aliases = {
        "x86_64": "x86_64",
        "arm64": "arm64",
    }
    tokens = stdout.strip().split()
    if not tokens:
        fail(f"lipo did not report an architecture for {executable.name}")
    architectures: list[str] = []
    for token in tokens:
        architecture = aliases.get(token.lower())
        if architecture is None:
            fail(f"lipo reported unsupported architecture {token!r} for {executable.name}")
        architectures.append(architecture)
    return architectures


def codesign_team_identifier(
    codesign: str,
    path: Path,
    runner: Runner,
    trusted_team_id: str,
) -> str:
    result = run_checked(
        runner,
        [codesign, "--display", "--verbose=4", str(path)],
        f"codesign identity inspection for {path.name}",
    )
    output = f"{result.stdout}\n{result.stderr}"
    matches = re.findall(r"(?m)^TeamIdentifier=([A-Za-z0-9]+)\s*$", output)
    if len(matches) != 1:
        fail(f"codesign did not report exactly one TeamIdentifier for {path.name}")
    team_id = matches[0].upper()
    if APPLE_TEAM_ID_PATTERN.fullmatch(team_id) is None:
        fail(f"codesign reported an invalid TeamIdentifier for {path.name}: {matches[0]}")
    if team_id != trusted_team_id:
        fail(
            f"Apple Developer Team ID mismatch for {path.name}: got {team_id}, "
            f"expected {trusted_team_id}"
        )
    return team_id


def verify_macos(
    packages: Sequence[StagedPackage],
    runner: Runner,
    which: Callable[[str], str | None],
    working_dir: Path,
    expected_version: str,
    expected_architecture: str,
    trusted_team_id: str,
) -> tuple[dict[str, str], list[dict[str, object]], dict[str, object]]:
    codesign = require_tool(("codesign",), which)
    spctl = require_tool(("spctl",), which)
    xcrun = require_tool(("xcrun",), which)
    hdiutil = require_tool(("hdiutil",), which)
    sw_vers = require_tool(("sw_vers",), which)
    lipo = require_tool(("lipo",), which)
    xcrun_version = run_checked(runner, [xcrun, "--version"], "xcrun version query")
    os_version = run_checked(
        runner, [sw_vers, "-productVersion"], "macOS version query"
    )
    version_parts = [
        xcrun_version.stdout.strip().splitlines()[0],
        f"macOS {os_version.stdout.strip().splitlines()[0]}",
    ]
    if any(not part.strip() for part in version_parts):
        fail("macOS native verifier did not report tool and operating-system versions")

    package_details: list[dict[str, object]] = []
    all_architectures: list[str] = []
    for index, package in enumerate(packages):
        run_checked(
            runner,
            [codesign, "--verify", "--deep", "--strict", "--verbose=4", str(package.staged)],
            f"codesign verification for {package.filename}",
        )
        dmg_team_id = codesign_team_identifier(
            codesign, package.staged, runner, trusted_team_id
        )
        run_checked(
            runner,
            [
                spctl,
                "--assess",
                "--type",
                "open",
                "--context",
                "context:primary-signature",
                "--verbose=4",
                str(package.staged),
            ],
            f"Gatekeeper assessment for {package.filename}",
        )
        run_checked(
            runner,
            [xcrun, "stapler", "validate", str(package.staged)],
            f"stapled notarization validation for {package.filename}",
        )

        mountpoint = working_dir / f"mounted-dmg-{index}"
        mountpoint.mkdir(mode=0o700)
        attached = False
        primary_error: VerificationError | None = None
        app_details: list[dict[str, object]] = []
        try:
            attach = run_checked(
                runner,
                [
                    hdiutil,
                    "attach",
                    "-readonly",
                    "-nobrowse",
                    "-noautoopen",
                    "-mountpoint",
                    str(mountpoint),
                    "-plist",
                    str(package.staged),
                ],
                f"read-only mount for {package.filename}",
            )
            attached = True
            parse_hdiutil_mount(attach.stdout, mountpoint, package.filename)
            apps = discover_mounted_apps(mountpoint)
            if not apps:
                fail(f"mounted macOS package contains no application bundle: {package.filename}")
            for app in apps:
                run_checked(
                    runner,
                    [codesign, "--verify", "--deep", "--strict", "--verbose=4", str(app)],
                    f"codesign verification for {app.name}",
                )
                app_team_id = codesign_team_identifier(
                    codesign, app, runner, trusted_team_id
                )
                run_checked(
                    runner,
                    [spctl, "--assess", "--type", "execute", "--verbose=4", str(app)],
                    f"Gatekeeper assessment for {app.name}",
                )
                run_checked(
                    runner,
                    [xcrun, "stapler", "validate", str(app)],
                    f"stapled notarization validation for {app.name}",
                )
                bundle = inspect_application_bundle(app, expected_version)
                macho_payloads = inspect_macho_payloads(
                    app,
                    bundle.executable,
                    lipo,
                    runner,
                    expected_architecture,
                )
                app_architecture = verified_architecture(
                    [
                        str(value["architecture"])
                        for value in macho_payloads
                    ],
                    expected_architecture,
                    app.name,
                )
                all_architectures.extend(
                    str(value["architecture"])
                    for value in macho_payloads
                )
                app_details.append(
                    {
                        "application": app.relative_to(mountpoint).as_posix(),
                        "executable": bundle.executable.name,
                        "architecture": app_architecture,
                        "teamIdentifier": app_team_id,
                        "bundleIdentifier": bundle.bundle_identifier,
                        "bundleShortVersion": bundle.short_version,
                        "bundleVersion": bundle.bundle_version,
                        "icon": {
                            "filename": bundle.icon.name,
                            "sizeBytes": bundle.icon.stat().st_size,
                            "sha256": file_sha256(bundle.icon),
                        },
                        "machOPayloads": macho_payloads,
                    }
                )
        except VerificationError as error:
            primary_error = error
        finally:
            if attached:
                try:
                    run_checked(
                        runner,
                        [hdiutil, "detach", str(mountpoint)],
                        f"unmount for {package.filename}",
                    )
                except VerificationError as detach_error:
                    if primary_error is not None:
                        raise VerificationError(
                            f"{primary_error}; additionally, {detach_error}"
                        ) from primary_error
                    raise
        if primary_error is not None:
            raise primary_error
        package_details.append(
            {
                "filename": package.filename,
                "architecture": verified_architecture(
                    [value["architecture"] for value in app_details],
                    expected_architecture,
                    package.filename,
                ),
                "teamIdentifier": dmg_team_id,
                "verifiedApplications": app_details,
            }
        )
    verified_architecture(all_architectures, expected_architecture, "macOS package set")
    return (
        {
            "name": "Apple codesign, Gatekeeper, and stapler",
            "version": "; ".join(version_parts),
        },
        package_details,
        {
            "publisherIdentity": {
                "type": "apple-developer-team-id",
                "value": trusted_team_id,
            }
        },
    )


def normalize_fingerprint(value: str) -> str:
    normalized = re.sub(r"\s+", "", value).upper()
    if OPENPGP_FINGERPRINT_PATTERN.fullmatch(normalized) is None:
        fail("trusted OpenPGP fingerprint must be exactly 40 or 64 hexadecimal characters")
    return normalized


def signature_for_package(package: Path) -> Path:
    matches = [
        package.with_name(f"{package.name}{suffix}")
        for suffix in SIGNATURE_SUFFIXES
        if package.with_name(f"{package.name}{suffix}").exists()
    ]
    if len(matches) != 1:
        if not matches:
            fail(f"Linux package has no detached .sig or .asc signature: {package.name}")
        fail(f"Linux package has ambiguous detached signatures: {package.name}")
    signature = matches[0]
    if signature.is_symlink() or not signature.is_file():
        fail(f"detached signature must be a regular, non-symlink file: {signature.name}")
    if signature.stat().st_size <= 0:
        fail(f"detached signature is empty: {signature.name}")
    return signature


def parse_validsig(stdout: str, expected_fingerprint: str, signature_name: str) -> str:
    terminal_failures = {
        "BADSIG",
        "ERRSIG",
        "NO_PUBKEY",
        "EXPKEYSIG",
        "EXPSIG",
        "REVKEYSIG",
    }
    valid_signatures: list[tuple[str, str | None]] = []
    for line in stdout.splitlines():
        if not line.startswith("[GNUPG:] "):
            continue
        fields = line[len("[GNUPG:] ") :].split()
        if not fields:
            continue
        if fields[0] in terminal_failures:
            fail(f"OpenPGP verifier reported {fields[0]} for {signature_name}")
        if fields[0] == "VALIDSIG" and len(fields) >= 2:
            signer = fields[1].upper()
            primary = fields[10].upper() if len(fields) >= 11 else None
            if OPENPGP_FINGERPRINT_PATTERN.fullmatch(signer) is None or (
                primary is not None
                and OPENPGP_FINGERPRINT_PATTERN.fullmatch(primary) is None
            ):
                fail(f"OpenPGP verifier reported an invalid fingerprint for {signature_name}")
            valid_signatures.append((signer, primary))
    if len(valid_signatures) != 1:
        rendered = (
            ", ".join(signer for signer, _primary in valid_signatures)
            if valid_signatures
            else "none"
        )
        fail(
            f"detached signature {signature_name} did not validate with the trusted "
            f"fingerprint {expected_fingerprint}; verifier reported: {rendered}"
        )
    signer, primary = valid_signatures[0]
    if expected_fingerprint not in {signer, primary}:
        fail(
            f"detached signature {signature_name} did not validate with the trusted "
            f"fingerprint {expected_fingerprint}; verifier reported signer {signer}"
        )
    return signer


def tar_elf_architectures(path: Path) -> list[str]:
    architectures: list[str] = []
    seen: set[str] = set()
    entry_count = 0
    total_bytes = 0
    try:
        archive_context = tarfile.open(path, mode="r:gz")
    except (OSError, tarfile.TarError) as error:
        fail(f"Linux package is not a valid gzip-compressed tar archive ({path.name}): {error}")
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
                    fail(f"Linux archive {path.name} contains unsafe entry {member.name!r}")
                folded = normalized_name.casefold().rstrip("/")
                if folded in seen:
                    fail(f"Linux archive {path.name} contains duplicate entry {member.name!r}")
                seen.add(folded)
                if member.isdev() or member.isfifo():
                    fail(f"Linux archive {path.name} contains special entry {member.name!r}")
                if not member.isfile():
                    continue
                if member.size < 0 or member.size > MAX_TAR_ENTRY_BYTES:
                    fail(f"Linux archive {path.name} entry is too large: {member.name}")
                total_bytes += member.size
                if total_bytes > MAX_TAR_TOTAL_BYTES:
                    fail(f"Linux archive {path.name} expands beyond the verification limit")
                handle = archive.extractfile(member)
                if handle is None:
                    fail(f"Linux archive payload could not be read: {member.name}")
                with handle:
                    header = handle.read(20)
                if header.startswith(b"\x7fELF"):
                    architectures.append(
                        elf_architecture_from_header(header, f"{path.name}:{member.name}")
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


def verify_linux(
    packages: Sequence[StagedPackage],
    runner: Runner,
    which: Callable[[str], str | None],
    working_dir: Path,
    trusted_keyring: Path | None,
    trusted_fingerprint: str | None,
    expected_architecture: str,
) -> tuple[dict[str, str], list[dict[str, object]], dict[str, object]]:
    if trusted_keyring is None or trusted_fingerprint is None:
        fail("Linux verification requires --trusted-keyring and --trusted-fingerprint")
    if trusted_keyring.is_symlink():
        fail(f"trusted OpenPGP keyring must not be a symlink: {trusted_keyring}")
    keyring = trusted_keyring.resolve()
    if not keyring.is_file() or keyring.stat().st_size <= 0:
        fail(f"trusted OpenPGP keyring must be a non-empty file: {keyring}")
    staged_keyring = working_dir / "trust" / keyring.name
    keyring_digest = copy_and_hash(keyring, staged_keyring)
    if file_sha256(keyring) != keyring_digest:
        fail("trusted OpenPGP keyring changed while it was staged")
    expected_fingerprint = normalize_fingerprint(trusted_fingerprint)
    gpg = require_tool(("gpg", "gpg2"), which)
    version_result = run_checked(runner, [gpg, "--version"], "GnuPG version query")
    version = version_result.stdout.strip().splitlines()[0] if version_result.stdout.strip() else ""
    if not version:
        fail("GnuPG did not report a verifier version")

    homedir = working_dir / "gnupg-home"
    homedir.mkdir(mode=0o700)
    package_details: list[dict[str, object]] = []
    source_by_name = {package.filename: package.source for package in packages}
    expected_signature_names: set[str] = set()
    all_architectures: list[str] = []
    for package in packages:
        signature_source = signature_for_package(source_by_name[package.filename])
        expected_signature_names.add(signature_source.name)
        staged_signature = working_dir / "signatures" / signature_source.name
        signature_digest = copy_and_hash(signature_source, staged_signature)
        if file_sha256(signature_source) != signature_digest:
            fail(f"detached signature changed while it was staged: {signature_source.name}")
        result = run_checked(
            runner,
            [
                gpg,
                "--batch",
                "--no-tty",
                "--no-auto-key-retrieve",
                "--no-default-keyring",
                "--homedir",
                str(homedir),
                "--keyring",
                str(staged_keyring),
                "--status-fd",
                "1",
                "--verify",
                str(staged_signature),
                str(package.staged),
            ],
            f"detached signature verification for {package.filename}",
        )
        signer_fingerprint = parse_validsig(
            result.stdout, expected_fingerprint, signature_source.name
        )
        if file_sha256(signature_source) != signature_digest:
            fail(f"detached signature changed during verification: {signature_source.name}")
        architectures = linux_package_architectures(package.staged)
        package_architecture = verified_architecture(
            architectures, expected_architecture, package.filename
        )
        all_architectures.extend(architectures)
        package_details.append(
            {
                "filename": package.filename,
                "architecture": package_architecture,
                "signature": {
                    "filename": signature_source.name,
                    "sha256": signature_digest,
                    "signerFingerprint": signer_fingerprint,
                    "trustedFingerprint": expected_fingerprint,
                    "trustedKeyring": {
                        "filename": keyring.name,
                        "sha256": keyring_digest,
                    },
                },
            }
        )

    orphaned = sorted(
        path.name
        for path in packages[0].source.parent.iterdir()
        if path.is_file()
        and path.name.lower().endswith(SIGNATURE_SUFFIXES)
        and path.name not in expected_signature_names
    )
    if orphaned:
        fail(
            "Linux package directory contains orphaned detached signatures: "
            + ", ".join(orphaned)
        )
    if file_sha256(keyring) != keyring_digest:
        fail("trusted OpenPGP keyring changed during verification")
    verified_architecture(all_architectures, expected_architecture, "Linux package set")
    return (
        {"name": "GnuPG detached-signature verification", "version": version},
        package_details,
        {
            "publisherIdentity": {
                "type": "openpgp-primary-key-fingerprint",
                "value": expected_fingerprint,
            },
            "trustedKeyring": {
                "filename": keyring.name,
                "sha256": keyring_digest,
            },
        },
    )


def validate_inputs(
    *,
    target: str | None,
    platform: str | None,
    package_dir: Path,
    output: Path,
    version: str,
    tag: str,
    commit: str,
    architecture: str | None,
    host_platform: str,
) -> tuple[release_targets.ReleaseTarget, Path, Path, str]:
    try:
        target_contract = release_targets.resolve_target(
            target_name=target,
            platform_name=platform,
            architecture=architecture,
        )
    except release_targets.ReleaseTargetError as error:
        fail(str(error))
    platform = target_contract.platform
    if normalized_host_platform(host_platform) != platform:
        fail(f"{platform} verification must run on a native {platform} host")
    if SEMANTIC_VERSION.fullmatch(version) is None:
        fail("version must be a semantic version")
    if tag != f"v{version}":
        fail("tag must be exactly v<version>")
    if COMMIT_PATTERN.fullmatch(commit) is None:
        fail("commit must be a 40-64 character hexadecimal object id")
    package_dir = package_dir.resolve()
    output = output.resolve()
    expected_name = RECEIPT_FILENAMES[target_contract.name]
    if output.name != expected_name:
        fail(
            f"{target_contract.name} verification receipt must be named "
            f"{expected_name}"
        )
    try:
        output.relative_to(package_dir)
    except ValueError:
        pass
    else:
        fail("verification receipt output must be outside the package directory")
    if output.exists() or output.is_symlink():
        fail("verification receipt output already exists; use a fresh receipt path")
    return target_contract, package_dir, output, commit.lower()


def atomic_write_json(path: Path, value: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        text=True,
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, indent=2, ensure_ascii=False)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, path)
        try:
            directory_descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        except (AttributeError, OSError):
            return
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    finally:
        if temporary.exists():
            temporary.unlink()


def generate_receipt(
    *,
    platform: str | None = None,
    target: str | None = None,
    package_dir: Path,
    output: Path,
    version: str,
    tag: str,
    commit: str,
    architecture: str | None = None,
    trusted_windows_signer_thumbprint: str | None = None,
    trusted_apple_team_id: str | None = None,
    trusted_keyring: Path | None = None,
    trusted_fingerprint: str | None = None,
    runner: Runner | None = None,
    which: Callable[[str], str | None] = shutil.which,
    host_platform: str = sys.platform,
    now: Callable[[], datetime] = lambda: datetime.now(timezone.utc),
) -> dict[str, object]:
    target_contract, package_dir, output, commit = validate_inputs(
        target=target,
        platform=platform,
        package_dir=package_dir,
        output=output,
        version=version,
        tag=tag,
        commit=commit,
        architecture=architecture,
        host_platform=host_platform,
    )
    platform = target_contract.platform
    architecture = target_contract.architecture
    windows_thumbprint: str | None = None
    apple_team_id: str | None = None
    if platform == "windows":
        if trusted_windows_signer_thumbprint is None:
            fail("Windows verification requires --trusted-windows-signer-thumbprint")
        windows_thumbprint = normalize_authenticode_thumbprint(
            trusted_windows_signer_thumbprint
        )
        if trusted_apple_team_id is not None:
            fail("--trusted-apple-team-id is valid only for macOS verification")
        if trusted_keyring is not None or trusted_fingerprint is not None:
            fail("trusted OpenPGP inputs are valid only for Linux verification")
    elif platform == "macos":
        if trusted_apple_team_id is None:
            fail("macOS verification requires --trusted-apple-team-id")
        apple_team_id = normalize_apple_team_id(trusted_apple_team_id)
        if trusted_windows_signer_thumbprint is not None:
            fail(
                "--trusted-windows-signer-thumbprint is valid only for Windows verification"
            )
        if trusted_keyring is not None or trusted_fingerprint is not None:
            fail("trusted OpenPGP inputs are valid only for Linux verification")
    else:
        if trusted_windows_signer_thumbprint is not None or trusted_apple_team_id is not None:
            fail("Windows and Apple publisher pins are invalid for Linux verification")
    packages = discover_packages(package_dir, platform)
    command_runner = runner or SubprocessRunner()
    with tempfile.TemporaryDirectory(prefix=f"chaft-{platform}-verification-") as name:
        working_dir = Path(name)
        staged = stage_packages(packages, working_dir)
        if platform == "windows":
            assert windows_thumbprint is not None
            verifier, details, verification_policy = verify_windows(
                staged,
                command_runner,
                which,
                working_dir,
                architecture,
                windows_thumbprint,
            )
            signature_evidence: list[dict[str, str]] = []
        elif platform == "macos":
            assert apple_team_id is not None
            verifier, details, verification_policy = verify_macos(
                staged,
                command_runner,
                which,
                working_dir,
                version,
                architecture,
                apple_team_id,
            )
            signature_evidence = []
        else:
            verifier, details, verification_policy = verify_linux(
                staged,
                command_runner,
                which,
                working_dir,
                trusted_keyring,
                trusted_fingerprint,
                architecture,
            )
            signature_evidence = [
                {
                    "filename": row["signature"]["filename"],
                    "signedArtifact": row["filename"],
                    "sha256": row["signature"]["sha256"],
                    "signerFingerprint": row["signature"]["signerFingerprint"],
                    "trustedFingerprint": row["signature"]["trustedFingerprint"],
                }
                for row in details
            ]
        ensure_sources_unchanged(staged)

    verified_at = now()
    if verified_at.tzinfo is None or verified_at.utcoffset() is None:
        fail("verification clock must return a timezone-aware timestamp")
    receipt: dict[str, object] = {
        "schemaVersion": SCHEMA_VERSION,
        "target": target_contract.name,
        "platform": platform,
        "verificationType": VERIFICATION_TYPES[platform],
        "status": "verified",
        "version": version,
        "tag": tag,
        "commit": commit,
        "architecture": architecture,
        "verifiedAt": verified_at.astimezone(timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z"),
        "verifier": verifier,
        "verificationPolicy": verification_policy,
        "artifacts": [
            {"filename": package.filename, "sha256": package.sha256}
            for package in staged
        ],
        "signatures": signature_evidence,
        "verificationDetails": details,
        "receiptGenerator": {
            "name": "Chaft platform verification receipt generator",
            "version": SCRIPT_VERSION,
        },
    }
    atomic_write_json(output, receipt)
    return receipt


def argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify desktop packages with native tools and emit a website release receipt.",
        epilog=(
            "Publisher identity pins are mandatory: Windows requires "
            "--trusted-windows-signer-thumbprint; macOS requires "
            "--trusted-apple-team-id; Linux requires both --trusted-keyring and "
            "--trusted-fingerprint. Pins for another platform are rejected."
        ),
    )
    parser.add_argument("--target", required=True, choices=release_targets.TARGET_NAMES)
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument(
        "--trusted-windows-signer-thumbprint",
        help=(
            "Windows only: exact SHA-1 or SHA-256 Authenticode signer certificate "
            "thumbprint required on every signed payload."
        ),
    )
    parser.add_argument(
        "--trusted-apple-team-id",
        help=(
            "macOS only: exact 10-character Apple Developer Team ID required on "
            "the signed DMG and every signed app."
        ),
    )
    parser.add_argument(
        "--trusted-keyring",
        type=Path,
        help="Linux only: explicit OpenPGP public keyring used with --no-default-keyring.",
    )
    parser.add_argument(
        "--trusted-fingerprint",
        help="Linux only: exact 40- or 64-hex OpenPGP signing-key fingerprint.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    try:
        generate_receipt(
            target=args.target,
            package_dir=args.package_dir,
            output=args.output,
            version=args.version,
            tag=args.tag,
            commit=args.commit,
            trusted_windows_signer_thumbprint=(
                args.trusted_windows_signer_thumbprint
            ),
            trusted_apple_team_id=args.trusted_apple_team_id,
            trusted_keyring=args.trusted_keyring,
            trusted_fingerprint=args.trusted_fingerprint,
        )
    except VerificationError as error:
        parser.exit(2, f"native verification failed: {error}\n")
    print(f"platform verification receipt written: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
