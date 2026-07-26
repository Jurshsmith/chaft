#!/usr/bin/env python3
"""Build and offline-verify Chaft's deterministic Qt corresponding source."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import struct
import sys
import tempfile
from typing import Any, BinaryIO
from urllib.parse import urlparse
import zipfile

import build_qt as qt


SCRIPT_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = SCRIPT_DIR.parents[1]
QT_MANIFEST_PATH = SCRIPT_DIR / "qt-6.8.4.json"
PACKAGE_QT_DIR = REPOSITORY_ROOT / "packaging" / "qt"
CORRESPONDING_SOURCE_NAME = "QT-CORRESPONDING-SOURCE.json"
BUNDLE_NAME = "Chaft-Qt-6.8.4-corresponding-source.zip"
CHECKSUM_NAME = f"{BUNDLE_NAME}.sha256"
DEFAULT_BUNDLE_PATH = REPOSITORY_ROOT / "dist" / BUNDLE_NAME
FIXED_ZIP_TIMESTAMP = (1980, 1, 1, 0, 0, 0)
REGULAR_FILE_MODE = stat.S_IFREG | 0o644
PACKAGE_FILES = (
    "README.md",
    "THIRD_PARTY_NOTICES.txt",
    "LICENSE.GPL3",
    "LICENSE.LGPL3",
    CORRESPONDING_SOURCE_NAME,
)
SHA256_LINE = re.compile(r"([0-9a-f]{64})  ([!-~]+)")
PLATFORM_LABELS = {
    "linux": "Linux",
    "macos": "macOS",
    "windows": "Windows",
}
LOCAL_FILE_HEADER = struct.Struct("<IHHHHHIIIHH")
CENTRAL_DIRECTORY_HEADER = struct.Struct("<I6H3I5H2I")
END_OF_CENTRAL_DIRECTORY = struct.Struct("<IHHHHIIH")
LOCAL_FILE_HEADER_SIGNATURE = 0x04034B50
CENTRAL_DIRECTORY_SIGNATURE = 0x02014B50
END_OF_CENTRAL_DIRECTORY_SIGNATURE = 0x06054B50
CANONICAL_CREATE_VERSION = (3 << 8) | 20


EntryValue = bytes | Path


def _read_json(path: Path, description: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        qt.fail(f"{description} not found: {path}")
    except json.JSONDecodeError as error:
        qt.fail(f"{description} is invalid JSON: {error}")
    if not isinstance(value, dict):
        qt.fail(f"{description} root must be an object")
    return value


def _authoritative_file(
    path: Path,
    description: str,
    recipe_root: Path | None,
) -> Path:
    if recipe_root is not None:
        return qt.trusted_source_file(recipe_root, path, description)
    if not path.is_file():
        qt.fail(f"{description} not found: {path}")
    return path


def _package_file(
    package_dir: Path,
    name: str,
    recipe_root: Path | None,
) -> Path:
    return _authoritative_file(
        package_dir / name,
        f"Qt corresponding-source package input {name}",
        recipe_root,
    )


def _expected_package_modules(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "name": row["name"],
            "platforms": [PLATFORM_LABELS[name] for name in row["platforms"]],
            "url": row["url"],
            "sha256": row["sha256"],
        }
        for row in manifest["modules"]
    ]


def _expected_package_patches(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    module_platforms = {
        row["name"]: [PLATFORM_LABELS[name] for name in row["platforms"]]
        for row in manifest["modules"]
    }
    expected: list[dict[str, Any]] = []
    for row in manifest["patches"]:
        cve = re.match(r"(CVE-\d{4}-\d+)", row["name"])
        if cve is None:
            qt.fail(f"security patch has no CVE identifier: {row['name']}")
        item: dict[str, Any] = {
            "id": cve.group(1),
            "module": row["module"],
        }
        part = re.search(r"-(\d{4})\.diff$", row["name"])
        if part is not None:
            item["part"] = int(part.group(1))
        item.update(
            {
                "platforms": module_platforms[row["module"]],
                "url": row["url"],
                "sha256": row["sha256"],
            }
        )
        expected.append(item)
    return expected


def validate_corresponding_source(
    source_manifest: dict[str, Any], sdk_manifest: dict[str, Any]
) -> None:
    """Validate the package record and its exact parity with the SDK recipe."""
    qt.require_exact_keys(
        source_manifest,
        {
            "schemaVersion",
            "component",
            "version",
            "instructions",
            "releaseAssets",
            "licenses",
            "sourceModules",
            "securityPatches",
        },
        "Qt corresponding-source manifest",
    )
    if (
        type(source_manifest["schemaVersion"]) is not int
        or source_manifest["schemaVersion"] != 1
    ):
        qt.fail("Qt corresponding-source schemaVersion must be 1")
    if source_manifest["component"] != "Qt":
        qt.fail("Qt corresponding-source component must be Qt")
    if source_manifest["version"] != sdk_manifest["qtVersion"]:
        qt.fail("Qt corresponding-source version differs from the SDK manifest")
    instructions = source_manifest["instructions"]
    if not isinstance(instructions, str) or not instructions.strip():
        qt.fail("Qt corresponding-source instructions must be non-empty")
    expected_assets = {
        "bundle": BUNDLE_NAME,
        "checksum": CHECKSUM_NAME,
    }
    if not qt.json_exact_equal(
        source_manifest["releaseAssets"], expected_assets
    ):
        qt.fail(
            "Qt corresponding-source releaseAssets must remain exactly "
            f"{expected_assets}"
        )
    expected_licenses = [
        {"spdx": "LGPL-3.0-only", "packageFile": "LICENSE.LGPL3"},
        {"spdx": "GPL-3.0-only", "packageFile": "LICENSE.GPL3"},
    ]
    if not qt.json_exact_equal(
        source_manifest["licenses"], expected_licenses
    ):
        qt.fail(
            "Qt corresponding-source license records must remain exactly "
            f"{expected_licenses}"
        )
    expected_modules = _expected_package_modules(sdk_manifest)
    if not qt.json_exact_equal(
        source_manifest["sourceModules"], expected_modules
    ):
        qt.fail(
            "Qt corresponding-source modules differ from the SDK manifest"
        )
    expected_patches = _expected_package_patches(sdk_manifest)
    if not qt.json_exact_equal(
        source_manifest["securityPatches"], expected_patches
    ):
        qt.fail(
            "Qt corresponding-source security patches differ from the SDK "
            "manifest or its required order"
        )


def load_contracts(
    manifest_path: Path = QT_MANIFEST_PATH,
    package_dir: Path = PACKAGE_QT_DIR,
    recipe_root: Path | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    manifest_path = _authoritative_file(
        manifest_path, "Qt SDK manifest", recipe_root
    )
    sdk_manifest = qt.load_manifest(
        manifest_path, recipe_root=recipe_root
    )
    source_manifest = _read_json(
        _package_file(
            package_dir, CORRESPONDING_SOURCE_NAME, recipe_root
        ),
        "Qt corresponding-source manifest",
    )
    validate_corresponding_source(source_manifest, sdk_manifest)
    return sdk_manifest, source_manifest


def release_contract(
    manifest_path: Path = QT_MANIFEST_PATH,
    package_dir: Path = PACKAGE_QT_DIR,
    bundle_recipe_path: Path | None = None,
    recipe_root: Path | None = None,
) -> dict[str, Any]:
    """Return the offline contract that every desktop provenance must bind."""
    sdk_manifest, source_manifest = load_contracts(
        manifest_path, package_dir, recipe_root
    )
    compliance_materials = []
    for name in PACKAGE_FILES:
        path = _package_file(package_dir, name, recipe_root)
        compliance_materials.append(
            {"path": f"packaging/qt/{name}", "sha256": qt.sha256_file(path)}
        )
    payload = {
        "schemaVersion": 1,
        "bundleName": BUNDLE_NAME,
        "checksumName": CHECKSUM_NAME,
        "qtVersion": sdk_manifest["qtVersion"],
        "sdkManifestSha256": qt.manifest_digest(
            sdk_manifest, recipe_root=recipe_root
        ),
        "sdkContractSha256": qt.contract_digest(
            sdk_manifest, recipe_root=recipe_root
        ),
        "sourceManifestSha256": qt.sha256_bytes(
            qt.canonical_json(source_manifest)
        ),
        "bundleRecipeSha256": qt.sha256_file(
            _authoritative_file(
                bundle_recipe_path or Path(__file__).resolve(),
                "Qt corresponding-source bundle recipe",
                recipe_root,
            )
        ),
        "complianceMaterials": compliance_materials,
    }
    return {
        **payload,
        "contractSha256": qt.sha256_bytes(qt.canonical_json(payload)),
    }


def _source_material_rows(
    source_manifest: dict[str, Any],
) -> list[tuple[str, dict[str, Any]]]:
    """Return authoritative package-manifest materials in application order."""
    rows: list[tuple[str, dict[str, Any]]] = []
    for row in source_manifest["sourceModules"]:
        filename = Path(urlparse(row["url"]).path).name
        rows.append(
            (
                f"source-archives/{filename}",
                {
                    "name": row["name"],
                    "url": row["url"],
                    "sha256": row["sha256"],
                },
            )
        )
    for row in source_manifest["securityPatches"]:
        filename = Path(urlparse(row["url"]).path).name
        rows.append(
            (
                f"security-patches/{filename}",
                {
                    "name": filename,
                    "module": row["module"],
                    "url": row["url"],
                    "sha256": row["sha256"],
                },
            )
        )
    return rows


def _recipe_entries(recipe_root: Path | None = None) -> dict[str, Path]:
    materials = {
        row["path"]: row["sha256"]
        for row in qt.recipe_materials(recipe_root)
    }
    recipe_files = dict(qt.recipe_file_paths(recipe_root))
    if set(materials) != set(recipe_files):
        qt.fail("Qt SDK recipe file list differs from recipe_materials()")
    entries: dict[str, Path] = {}
    for logical_path in sorted(materials):
        filesystem_path = recipe_files[logical_path]
        if qt.sha256_file(filesystem_path) != materials[logical_path]:
            qt.fail(f"Qt SDK recipe changed while bundling: {logical_path}")
        entries[f"recipe/{logical_path}"] = filesystem_path
    return entries


def bundle_readme(source_manifest: dict[str, Any]) -> bytes:
    patch_names = [
        Path(urlparse(row["url"]).path).name
        for row in source_manifest["securityPatches"]
    ]
    lines = [
        "# Chaft Qt 6.8.4 corresponding source",
        "",
        "This deterministic archive accompanies Chaft desktop release packages.",
        "It contains the exact official Qt source archives, the security patches",
        "applied in the checked order, Chaft's Qt build recipe, and the notices",
        "and license records shipped with the binaries.",
        "",
        "Verify this archive from a Chaft source checkout with:",
        "",
        "```text",
        "python3 tools/qt/source_bundle.py verify "
        f"--bundle {BUNDLE_NAME} --checksum {CHECKSUM_NAME}",
        "```",
        "",
        f"First verify `{CHECKSUM_NAME}` against `{BUNDLE_NAME}`. The",
        "offline verifier then checks the exact ZIP layout, deterministic metadata,",
        "every entry in `SHA256SUMS`, both checked manifests, every source digest,",
        "and every byte of the SDK recipe.",
        "",
        "Apply the security patches in this order:",
        "",
    ]
    lines.extend(
        f"{index}. `security-patches/{name}`"
        for index, name in enumerate(patch_names, start=1)
    )
    lines.extend(
        [
            "",
            "The source archives retain Qt's file-level license records and preferred",
            "source form. See `packaging/qt/QT-CORRESPONDING-SOURCE.json` for the",
            "authoritative source URLs, module/platform mapping, and SHA-256 values.",
            "",
        ]
    )
    return "\n".join(lines).encode("utf-8")


def _entry_size(value: EntryValue) -> int:
    return len(value) if isinstance(value, bytes) else value.stat().st_size


def _copy_entry(value: EntryValue, destination: BinaryIO) -> None:
    if isinstance(value, bytes):
        destination.write(value)
        return
    with value.open("rb") as source:
        shutil.copyfileobj(source, destination, length=1024 * 1024)


def _entry_digest(value: EntryValue) -> str:
    return qt.sha256_bytes(value) if isinstance(value, bytes) else qt.sha256_file(value)


def sha256sums(entries: dict[str, EntryValue]) -> bytes:
    lines = [
        f"{_entry_digest(entries[name])}  {name}"
        for name in sorted(entries)
    ]
    return ("\n".join(lines) + "\n").encode("ascii")


def _zip_info(name: str, size: int) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, FIXED_ZIP_TIMESTAMP)
    info.compress_type = zipfile.ZIP_STORED
    info.create_system = 3
    info.create_version = 20
    info.extract_version = 20
    info.external_attr = REGULAR_FILE_MODE << 16
    info.internal_attr = 0
    info.flag_bits = 0
    info.file_size = size
    info.extra = b""
    info.comment = b""
    return info


def write_deterministic_zip(
    destination: Path, entries: dict[str, EntryValue]
) -> None:
    """Write a stored, ordered ZIP without host timestamps or permissions."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(
        destination,
        mode="w",
        compression=zipfile.ZIP_STORED,
        allowZip64=True,
        strict_timestamps=True,
    ) as archive:
        archive.comment = b""
        for name in sorted(entries):
            value = entries[name]
            size = _entry_size(value)
            info = _zip_info(name, size)
            force_zip64 = size >= zipfile.ZIP64_LIMIT
            with archive.open(info, mode="w", force_zip64=force_zip64) as output:
                _copy_entry(value, output)


def _atomic_write(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp"
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        temporary.replace(path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def _assert_release_paths(
    bundle_path: Path, source_manifest: dict[str, Any]
) -> Path:
    assets = source_manifest["releaseAssets"]
    if bundle_path.name != assets["bundle"]:
        qt.fail(
            f"bundle filename must be {assets['bundle']}, got {bundle_path.name}"
        )
    checksum_path = bundle_path.with_name(assets["checksum"])
    if checksum_path.name != f"{bundle_path.name}.sha256":
        qt.fail("corresponding-source checksum must be named <bundle>.sha256")
    return checksum_path


def build_bundle(
    bundle_path: Path,
    download_dir: Path,
    *,
    manifest_path: Path = QT_MANIFEST_PATH,
    package_dir: Path = PACKAGE_QT_DIR,
    recipe_root: Path | None = None,
) -> tuple[Path, Path]:
    """Download verified materials and build the deterministic release assets."""
    manifest_path = _authoritative_file(
        manifest_path, "Qt SDK manifest", recipe_root
    )
    sdk_manifest, source_manifest = load_contracts(
        manifest_path, package_dir, recipe_root
    )
    del sdk_manifest  # Validation parity is the only build-time use.
    checksum_path = _assert_release_paths(bundle_path, source_manifest)

    entries: dict[str, EntryValue] = {
        "README.md": bundle_readme(source_manifest),
        "tools/qt/qt-6.8.4.json": manifest_path,
    }
    for filename in PACKAGE_FILES:
        path = _package_file(package_dir, filename, recipe_root)
        entries[f"packaging/qt/{filename}"] = path
    entries.update(_recipe_entries(recipe_root))

    for logical_path, row in _source_material_rows(source_manifest):
        material = qt.download_verified(row, download_dir)
        qt.verify_digest(material, row["sha256"], logical_path)
        entries[logical_path] = material

    expected_without_sums = expected_entry_names(
        source_manifest, include_sha256sums=False
    )
    if set(entries) != expected_without_sums:
        qt.fail("internal corresponding-source bundle layout construction failed")
    entries["SHA256SUMS"] = sha256sums(entries)

    bundle_path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        dir=bundle_path.parent,
        prefix=f".{bundle_path.name}.",
        suffix=".tmp",
    )
    os.close(descriptor)
    temporary = Path(temporary_name)
    try:
        write_deterministic_zip(temporary, entries)
        temporary.replace(bundle_path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise

    checksum = f"{qt.sha256_file(bundle_path)}  {bundle_path.name}\n".encode(
        "ascii"
    )
    _atomic_write(checksum_path, checksum)
    return bundle_path, checksum_path


def expected_entry_names(
    source_manifest: dict[str, Any], *, include_sha256sums: bool = True
) -> set[str]:
    names = {
        "README.md",
        "tools/qt/qt-6.8.4.json",
        *(f"packaging/qt/{name}" for name in PACKAGE_FILES),
        *(f"recipe/{logical}" for logical, _path in qt.RECIPE_FILES),
        *(name for name, _row in _source_material_rows(source_manifest)),
    }
    if include_sha256sums:
        names.add("SHA256SUMS")
    return names


def _parse_external_checksum(checksum_path: Path, bundle_path: Path) -> None:
    try:
        content = checksum_path.read_bytes()
    except FileNotFoundError:
        qt.fail(f"corresponding-source checksum not found: {checksum_path}")
    expected = f"{qt.sha256_file(bundle_path)}  {bundle_path.name}\n".encode(
        "ascii"
    )
    if content != expected:
        qt.fail(
            "corresponding-source external checksum is invalid or names the "
            "wrong bundle"
        )


def _validate_zip_info(info: zipfile.ZipInfo) -> None:
    if info.is_dir():
        qt.fail(f"bundle must not contain directory entries: {info.filename}")
    if info.date_time != FIXED_ZIP_TIMESTAMP:
        qt.fail(f"bundle entry has a non-deterministic timestamp: {info.filename}")
    if info.compress_type != zipfile.ZIP_STORED:
        qt.fail(f"bundle entry must use stored compression: {info.filename}")
    if info.compress_size != info.file_size:
        qt.fail(
            f"bundle stored entry sizes must be identical: {info.filename}"
        )
    if info.comment:
        qt.fail(f"bundle entry comment must be empty: {info.filename}")
    if info.extra:
        qt.fail(f"bundle entry extra metadata must be empty: {info.filename}")
    if info.create_system != 3:
        qt.fail(f"bundle entry must use normalized Unix metadata: {info.filename}")
    if info.create_version != 20:
        qt.fail(f"bundle entry create_version must be 20: {info.filename}")
    if info.extract_version != 20:
        qt.fail(f"bundle entry extract_version must be 20: {info.filename}")
    if info.internal_attr != 0:
        qt.fail(f"bundle entry internal_attr must be zero: {info.filename}")
    if info.external_attr != REGULAR_FILE_MODE << 16:
        qt.fail(
            f"bundle entry must be a regular 0644 file (no symlinks): "
            f"{info.filename}"
        )
    if info.flag_bits != 0:
        qt.fail(f"bundle entry flag_bits must be zero: {info.filename}")
    path_parts = Path(info.filename).parts
    if (
        not path_parts
        or info.filename.startswith(("/", "\\"))
        or ".." in path_parts
        or "\\" in info.filename
    ):
        qt.fail(f"bundle contains an unsafe path: {info.filename}")


def _read_exact(
    handle: BinaryIO,
    size: int,
    description: str,
) -> bytes:
    content = handle.read(size)
    if len(content) != size:
        qt.fail(f"bundle has a truncated {description}")
    return content


def _dos_timestamp(
    value: tuple[int, int, int, int, int, int]
) -> tuple[int, int]:
    year, month, day, hour, minute, second = value
    dos_time = (hour << 11) | (minute << 5) | (second // 2)
    dos_date = ((year - 1980) << 9) | (month << 5) | day
    return dos_time, dos_date


def _validate_raw_zip_layout(
    bundle_path: Path,
    archive: zipfile.ZipFile,
    infos: list[zipfile.ZipInfo],
) -> None:
    """Reject hidden bytes and require canonical local/central ZIP records."""
    file_size = bundle_path.stat().st_size
    if file_size < END_OF_CENTRAL_DIRECTORY.size:
        qt.fail("corresponding-source ZIP is too small for a canonical EOCD")
    eocd_offset = file_size - END_OF_CENTRAL_DIRECTORY.size
    expected_time, expected_date = _dos_timestamp(FIXED_ZIP_TIMESTAMP)

    with bundle_path.open("rb") as handle:
        handle.seek(eocd_offset)
        eocd = END_OF_CENTRAL_DIRECTORY.unpack(
            _read_exact(
                handle,
                END_OF_CENTRAL_DIRECTORY.size,
                "end-of-central-directory record",
            )
        )
        (
            signature,
            disk_number,
            central_disk,
            disk_entries,
            total_entries,
            central_size,
            central_offset,
            comment_length,
        ) = eocd
        if signature != END_OF_CENTRAL_DIRECTORY_SIGNATURE:
            qt.fail(
                "corresponding-source ZIP EOCD must end exactly at EOF"
            )
        if (
            disk_number != 0
            or central_disk != 0
            or disk_entries != len(infos)
            or total_entries != len(infos)
            or comment_length != 0
        ):
            qt.fail(
                "corresponding-source ZIP EOCD must be single-disk, "
                "comment-free, and contain the exact entry count"
            )
        if (
            disk_entries == 0xFFFF
            or total_entries == 0xFFFF
            or central_size == 0xFFFFFFFF
            or central_offset == 0xFFFFFFFF
        ):
            qt.fail(
                "corresponding-source ZIP must not use ZIP64 metadata"
            )
        if central_offset + central_size != eocd_offset:
            qt.fail(
                "corresponding-source ZIP central directory must end "
                "immediately before the EOCD"
            )
        if archive.start_dir != central_offset:
            qt.fail(
                "corresponding-source ZIP must not contain prepended or "
                "concatenated data"
            )

        physical_infos = sorted(infos, key=lambda info: info.header_offset)
        if [info.filename for info in physical_infos] != [
            info.filename for info in infos
        ]:
            qt.fail(
                "corresponding-source ZIP local record order must match "
                "the sorted central directory"
            )
        if not physical_infos or physical_infos[0].header_offset != 0:
            qt.fail(
                "corresponding-source ZIP first local header must start "
                "at byte zero"
            )
        previous_end = 0
        for info in physical_infos:
            if info.header_offset != previous_end:
                qt.fail(
                    "corresponding-source ZIP local records must be "
                    f"contiguous before {info.filename}"
                )
            if (
                info.file_size > 0xFFFFFFFF
                or info.compress_size > 0xFFFFFFFF
                or info.header_offset > 0xFFFFFFFF
            ):
                qt.fail(
                    "corresponding-source ZIP entries must not use ZIP64"
                )
            handle.seek(info.header_offset)
            local = LOCAL_FILE_HEADER.unpack(
                _read_exact(
                    handle,
                    LOCAL_FILE_HEADER.size,
                    f"local header for {info.filename}",
                )
            )
            (
                signature,
                extract_version,
                flag_bits,
                compress_type,
                modified_time,
                modified_date,
                crc,
                compressed_size,
                uncompressed_size,
                filename_length,
                extra_length,
            ) = local
            expected_filename = info.filename.encode("ascii")
            if signature != LOCAL_FILE_HEADER_SIGNATURE:
                qt.fail(
                    f"bundle entry has an invalid local header: {info.filename}"
                )
            local_contract = (
                (
                    extract_version,
                    info.extract_version,
                    "extract_version",
                ),
                (flag_bits, info.flag_bits, "flag_bits"),
                (compress_type, info.compress_type, "compression method"),
                (modified_time, expected_time, "DOS time"),
                (modified_date, expected_date, "DOS date"),
                (crc, info.CRC, "CRC"),
                (
                    compressed_size,
                    info.compress_size,
                    "compressed size",
                ),
                (
                    uncompressed_size,
                    info.file_size,
                    "uncompressed size",
                ),
                (
                    filename_length,
                    len(expected_filename),
                    "filename length",
                ),
                (extra_length, 0, "extra length"),
            )
            for actual, expected, field in local_contract:
                if actual != expected:
                    qt.fail(
                        f"bundle entry local header {field} is "
                        f"non-canonical: {info.filename}"
                    )
            filename = _read_exact(
                handle,
                filename_length,
                f"local filename for {info.filename}",
            )
            extra = _read_exact(
                handle,
                extra_length,
                f"local extra metadata for {info.filename}",
            )
            if filename != expected_filename:
                qt.fail(
                    f"bundle entry local filename differs: {info.filename}"
                )
            if extra:
                qt.fail(
                    f"bundle entry local extra metadata must be empty: "
                    f"{info.filename}"
                )
            data_start = handle.tell()
            data_end = data_start + info.compress_size
            if data_end > central_offset:
                qt.fail(
                    f"bundle entry payload is out of bounds: {info.filename}"
                )
            previous_end = data_end
        if previous_end != central_offset:
            qt.fail(
                "corresponding-source ZIP central directory must begin "
                "immediately after the final payload"
            )

        handle.seek(central_offset)
        central_end = central_offset + central_size
        for info in infos:
            record = CENTRAL_DIRECTORY_HEADER.unpack(
                _read_exact(
                    handle,
                    CENTRAL_DIRECTORY_HEADER.size,
                    f"central directory header for {info.filename}",
                )
            )
            (
                signature,
                create_version,
                extract_version,
                flag_bits,
                compress_type,
                modified_time,
                modified_date,
                crc,
                compressed_size,
                uncompressed_size,
                filename_length,
                extra_length,
                comment_length,
                disk_start,
                internal_attr,
                external_attr,
                local_offset,
            ) = record
            expected_filename = info.filename.encode("ascii")
            expected_central = (
                (signature, CENTRAL_DIRECTORY_SIGNATURE, "signature"),
                (
                    create_version,
                    CANONICAL_CREATE_VERSION,
                    "create_version",
                ),
                (
                    extract_version,
                    info.extract_version,
                    "extract_version",
                ),
                (flag_bits, info.flag_bits, "flag_bits"),
                (compress_type, info.compress_type, "compression method"),
                (modified_time, expected_time, "DOS time"),
                (modified_date, expected_date, "DOS date"),
                (crc, info.CRC, "CRC"),
                (
                    compressed_size,
                    info.compress_size,
                    "compressed size",
                ),
                (
                    uncompressed_size,
                    info.file_size,
                    "uncompressed size",
                ),
                (
                    filename_length,
                    len(expected_filename),
                    "filename length",
                ),
                (extra_length, 0, "extra length"),
                (comment_length, 0, "comment length"),
                (disk_start, 0, "disk number"),
                (internal_attr, info.internal_attr, "internal attributes"),
                (external_attr, info.external_attr, "external attributes"),
                (
                    local_offset,
                    info.header_offset,
                    "local header offset",
                ),
            )
            for actual, expected, field in expected_central:
                if actual != expected:
                    qt.fail(
                        f"bundle entry central directory {field} is "
                        f"non-canonical: {info.filename}"
                    )
            filename = _read_exact(
                handle,
                filename_length,
                f"central filename for {info.filename}",
            )
            extra = _read_exact(
                handle,
                extra_length,
                f"central extra metadata for {info.filename}",
            )
            comment = _read_exact(
                handle,
                comment_length,
                f"central comment for {info.filename}",
            )
            if filename != expected_filename:
                qt.fail(
                    f"bundle entry central filename differs: {info.filename}"
                )
            if extra or comment:
                qt.fail(
                    f"bundle entry central extra/comment must be empty: "
                    f"{info.filename}"
                )
        if handle.tell() != central_end:
            qt.fail(
                "corresponding-source ZIP central directory size is "
                "non-canonical"
            )


def _parse_sha256sums(
    content: bytes, expected_names: list[str]
) -> dict[str, str]:
    try:
        text = content.decode("ascii")
    except UnicodeDecodeError:
        qt.fail("SHA256SUMS must contain ASCII only")
    if not text.endswith("\n") or "\r" in text:
        qt.fail("SHA256SUMS must use canonical LF-terminated lines")
    rows: list[tuple[str, str]] = []
    for line in text.splitlines():
        match = SHA256_LINE.fullmatch(line)
        if match is None:
            qt.fail(f"SHA256SUMS contains an invalid line: {line!r}")
        rows.append((match.group(2), match.group(1)))
    actual_names = [name for name, _digest in rows]
    if actual_names != expected_names:
        qt.fail(
            "SHA256SUMS paths differ from the exact sorted bundle payload"
        )
    return dict(rows)


def _hash_zip_entry(
    archive: zipfile.ZipFile, info: zipfile.ZipInfo
) -> str:
    digest = hashlib.sha256()
    with archive.open(info, mode="r") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _read_small_entry(
    archive: zipfile.ZipFile, name: str, *, limit: int = 16 * 1024 * 1024
) -> bytes:
    info = archive.getinfo(name)
    if info.file_size > limit:
        qt.fail(f"bundle metadata entry is unexpectedly large: {name}")
    return archive.read(info)


def verify_bundle(
    bundle_path: Path,
    checksum_path: Path | None = None,
    *,
    manifest_path: Path = QT_MANIFEST_PATH,
    package_dir: Path = PACKAGE_QT_DIR,
    recipe_root: Path | None = None,
) -> None:
    """Verify a bundle and sidecar entirely offline against checked contracts."""
    manifest_path = _authoritative_file(
        manifest_path, "Qt SDK manifest", recipe_root
    )
    sdk_manifest, source_manifest = load_contracts(
        manifest_path, package_dir, recipe_root
    )
    expected_checksum_path = _assert_release_paths(bundle_path, source_manifest)
    if checksum_path is None:
        checksum_path = expected_checksum_path
    elif checksum_path.resolve() != expected_checksum_path.resolve():
        qt.fail(
            "corresponding-source checksum must be the named sidecar next to "
            "the bundle"
        )
    if not bundle_path.is_file():
        qt.fail(f"corresponding-source bundle not found: {bundle_path}")
    _parse_external_checksum(checksum_path, bundle_path)

    try:
        archive = zipfile.ZipFile(bundle_path, mode="r")
    except zipfile.BadZipFile as error:
        qt.fail(f"corresponding-source bundle is not a valid ZIP: {error}")
    with archive:
        if archive.comment:
            qt.fail("corresponding-source ZIP comment must be empty")
        infos = archive.infolist()
        names = [info.filename for info in infos]
        if len(names) != len(set(names)):
            qt.fail("corresponding-source bundle contains duplicate paths")
        expected_names = expected_entry_names(source_manifest)
        if set(names) != expected_names or names != sorted(expected_names):
            missing = sorted(expected_names - set(names))
            extra = sorted(set(names) - expected_names)
            qt.fail(
                "corresponding-source bundle layout differs from the exact "
                f"contract; missing={missing}, extra={extra}"
            )
        for info in infos:
            _validate_zip_info(info)
        _validate_raw_zip_layout(bundle_path, archive, infos)

        payload_names = sorted(expected_names - {"SHA256SUMS"})
        sums = _parse_sha256sums(
            _read_small_entry(archive, "SHA256SUMS"), payload_names
        )
        info_by_name = {info.filename: info for info in infos}
        actual_digests: dict[str, str] = {}
        for name in payload_names:
            actual = _hash_zip_entry(archive, info_by_name[name])
            actual_digests[name] = actual
            if actual != sums[name]:
                qt.fail(
                    f"SHA256SUMS mismatch for {name}: expected "
                    f"{sums[name]}, got {actual}"
                )

        manifest_entry = "tools/qt/qt-6.8.4.json"
        if _read_small_entry(archive, manifest_entry) != manifest_path.read_bytes():
            qt.fail("bundled Qt SDK manifest differs from the checked manifest")
        embedded_sdk = json.loads(
            _read_small_entry(archive, manifest_entry).decode("utf-8")
        )
        qt.validate_manifest(embedded_sdk, recipe_root=recipe_root)
        if embedded_sdk != sdk_manifest:
            qt.fail("bundled Qt SDK manifest is not the current contract")

        for filename in PACKAGE_FILES:
            logical = f"packaging/qt/{filename}"
            if _read_small_entry(archive, logical) != _package_file(
                package_dir, filename, recipe_root
            ).read_bytes():
                qt.fail(f"bundled Qt compliance file differs from checkout: {logical}")
        embedded_source = json.loads(
            _read_small_entry(
                archive,
                f"packaging/qt/{CORRESPONDING_SOURCE_NAME}",
            ).decode("utf-8")
        )
        if embedded_source != source_manifest:
            qt.fail("bundled corresponding-source manifest is not authoritative")
        validate_corresponding_source(embedded_source, embedded_sdk)

        if _read_small_entry(archive, "README.md") != bundle_readme(
            source_manifest
        ):
            qt.fail("bundled README differs from the deterministic contract")

        recipe_materials = {
            row["path"]: row["sha256"]
            for row in qt.recipe_materials(recipe_root)
        }
        recipe_files = dict(qt.recipe_file_paths(recipe_root))
        if set(recipe_materials) != set(recipe_files):
            qt.fail("current Qt SDK recipe file set is internally inconsistent")
        for logical, expected_digest in recipe_materials.items():
            entry = f"recipe/{logical}"
            if actual_digests[entry] != expected_digest:
                qt.fail(f"bundled Qt SDK recipe digest differs: {logical}")
            if _read_small_entry(archive, entry) != recipe_files[logical].read_bytes():
                qt.fail(f"bundled Qt SDK recipe bytes differ: {logical}")

        material_rows = dict(_source_material_rows(source_manifest))
        for logical, row in material_rows.items():
            if actual_digests[logical] != row["sha256"]:
                qt.fail(
                    f"bundled source material differs from authoritative digest: "
                    f"{logical}"
                )


def parse_arguments(arguments: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    create = subparsers.add_parser(
        "create", help="download verified inputs and create release assets"
    )
    create.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_BUNDLE_PATH.parent,
        help=f"output directory for {BUNDLE_NAME} and its checksum",
    )
    create.add_argument(
        "--download-dir",
        type=Path,
        help="verified material cache (default: sibling .qt-source-downloads)",
    )

    verify = subparsers.add_parser(
        "verify", help="verify the bundle and sidecar without network access"
    )
    verify.add_argument(
        "--bundle",
        type=Path,
        default=DEFAULT_BUNDLE_PATH,
        help=f"bundle path (basename must be {BUNDLE_NAME})",
    )
    verify.add_argument(
        "--checksum",
        type=Path,
        help=f"checksum path (must be the adjacent {CHECKSUM_NAME})",
    )
    verify.add_argument(
        "--source-root",
        type=Path,
        help=(
            "Release checkout whose manifest, compliance files, and recipe "
            "bytes are authoritative (the verifier code remains current)."
        ),
    )
    contract = subparsers.add_parser(
        "contract",
        help="print the exact offline corresponding-source contract",
    )
    contract.add_argument("--source-root", type=Path)
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    options = parse_arguments(sys.argv[1:] if arguments is None else arguments)
    try:
        if options.command == "create":
            output_dir = options.output_dir.resolve()
            output = output_dir / BUNDLE_NAME
            download_dir = (
                options.download_dir.resolve()
                if options.download_dir is not None
                else output_dir.parent / ".qt-source-downloads"
            )
            bundle, checksum = build_bundle(output, download_dir)
            print(bundle)
            print(checksum)
        elif options.command == "verify":
            checksum = (
                options.checksum.resolve()
                if options.checksum is not None
                else None
            )
            source_root = (
                qt.trusted_source_root(options.source_root)
                if options.source_root is not None
                else None
            )
            verify_bundle(
                options.bundle.resolve(),
                checksum,
                manifest_path=(
                    source_root / "tools" / "qt" / "qt-6.8.4.json"
                    if source_root is not None
                    else QT_MANIFEST_PATH
                ),
                package_dir=(
                    source_root / "packaging" / "qt"
                    if source_root is not None
                    else PACKAGE_QT_DIR
                ),
                recipe_root=source_root,
            )
            print(f"verified {options.bundle.resolve()}")
        else:
            source_root = (
                qt.trusted_source_root(options.source_root)
                if options.source_root is not None
                else None
            )
            print(
                json.dumps(
                    release_contract(
                        manifest_path=(
                            source_root
                            / "tools"
                            / "qt"
                            / "qt-6.8.4.json"
                            if source_root is not None
                            else QT_MANIFEST_PATH
                        ),
                        package_dir=(
                            source_root / "packaging" / "qt"
                            if source_root is not None
                            else PACKAGE_QT_DIR
                        ),
                        bundle_recipe_path=(
                            source_root / "tools" / "qt" / "source_bundle.py"
                            if source_root is not None
                            else None
                        ),
                        recipe_root=source_root,
                    ),
                    sort_keys=True,
                )
            )
    except (OSError, ValueError, zipfile.BadZipFile, qt.QtSdkError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
