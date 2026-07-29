#!/usr/bin/env python3
"""Verify and finalize Chaft's exact unsigned-canary release asset namespace.

This command is intentionally offline. The publisher remains responsible for
querying GitHub's release API, downloading every asset by immutable identity,
and then invoking this tool on the resulting flat local directory.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Mapping, Sequence

import release_targets
import unsigned_canary_policy as unsigned_canary


TOOLS_DIRECTORY = Path(__file__).resolve().parent
REPOSITORY_ROOT = TOOLS_DIRECTORY.parent.parent
QT_TOOLS_DIRECTORY = REPOSITORY_ROOT / "tools" / "qt"
sys.path.insert(0, str(QT_TOOLS_DIRECTORY))
import build_qt as qt_sdk  # noqa: E402
import source_bundle as qt_source  # noqa: E402


def _load_stager():
    script = TOOLS_DIRECTORY / "stage-website-release-assets.py"
    spec = importlib.util.spec_from_file_location("chaft_canary_asset_stager", script)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {script}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


stager = _load_stager()


def _load_release_metadata_verifier():
    script = TOOLS_DIRECTORY / "verify-release-metadata.py"
    spec = importlib.util.spec_from_file_location(
        "chaft_canary_release_metadata_verifier",
        script,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {script}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


release_metadata = _load_release_metadata_verifier()

SCHEMA_VERSION = "chaft.desktop.canary-release-assets.v2"
INVENTORY_FILENAME = "chaft-desktop-release-inventory.json"
AGGREGATE_CHECKSUM_FILENAME = "chaft-desktop-release-SHA256SUMS"
BASE_ASSET_COUNT = 18
PREFINAL_ASSET_COUNT = 22
COMPLETE_ASSET_COUNT = 24
QT_SOURCE_BUNDLE = qt_source.BUNDLE_NAME
QT_SOURCE_CHECKSUM = qt_source.CHECKSUM_NAME
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
REPOSITORY_PATTERN = unsigned_canary.REPOSITORY_PATTERN


class CanaryReleaseAssetError(ValueError):
    """The local canary release namespace is incomplete or incoherent."""


@dataclass(frozen=True)
class AssetFingerprint:
    filename: str
    size_bytes: int
    sha256: str
    kind: str
    target: str | None = None
    platform: str | None = None


QtVerifier = Callable[[Path, Path], None]


def fail(message: str) -> None:
    raise CanaryReleaseAssetError(message)


def fingerprint(
    path: Path,
    *,
    kind: str,
    target: str | None = None,
    platform: str | None = None,
) -> AssetFingerprint:
    try:
        value = stager.fingerprint_file(path)
    except stager.AssetStagingError as error:
        fail(str(error))
    return AssetFingerprint(
        filename=value.name,
        size_bytes=value.size_bytes,
        sha256=value.sha256,
        kind=kind,
        target=target,
        platform=platform,
    )


def scan_assets(directory: Path) -> dict[str, Path]:
    try:
        return stager.scan_flat_directory(directory)
    except stager.AssetStagingError as error:
        fail(str(error))


def read_json(path: Path, context: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        fail(f"cannot read {context}: {error}")
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{context} is not valid UTF-8 JSON: {error}")
    if not isinstance(value, dict):
        fail(f"{context} must be a JSON object")
    return value


def normalize_repository(value: object, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        fail(f"{context} must be a GitHub repository")
    candidate = value.strip()
    if candidate.startswith("git@github.com:"):
        candidate = candidate[len("git@github.com:") :]
    elif candidate.startswith("https://github.com/"):
        candidate = candidate[len("https://github.com/") :]
    if candidate.endswith(".git"):
        candidate = candidate[:-4]
    candidate = candidate.strip("/")
    if REPOSITORY_PATTERN.fullmatch(candidate) is None:
        fail(f"{context} must be a GitHub OWNER/REPOSITORY slug")
    return candidate


def verify_qt_sidecar(bundle: Path, checksum: Path) -> None:
    expected = f"{stager.fingerprint_file(bundle).sha256}  {bundle.name}\n"
    try:
        actual = checksum.read_text(encoding="ascii")
    except OSError as error:
        fail(f"cannot read Qt corresponding-source checksum: {error}")
    except UnicodeDecodeError:
        fail("Qt corresponding-source checksum must contain ASCII only")
    if actual != expected:
        fail("Qt corresponding-source checksum does not bind the exact bundle")


def verify_qt_bundle(bundle: Path, checksum: Path) -> None:
    verify_qt_sidecar(bundle, checksum)
    try:
        qt_source.verify_bundle(bundle, checksum)
    except SystemExit as error:
        fail(f"Qt corresponding-source bundle verification failed: {error}")


def expected_qt_source_contract() -> dict[str, object]:
    return qt_source.release_contract(
        QT_TOOLS_DIRECTORY / qt_source.QT_MANIFEST_PATH.name,
        REPOSITORY_ROOT / "packaging" / "qt",
        QT_TOOLS_DIRECTORY / "source_bundle.py",
        recipe_root=REPOSITORY_ROOT,
    )


def provenance_identity(
    path: Path,
    *,
    target: str,
    version: str,
    commit: str,
    qt_bundle_sha256: str,
) -> str:
    value = read_json(path, f"{target} provenance")
    if value.get("version") != version:
        fail(f"{target} provenance version does not match {version}")
    source = value.get("source")
    if not isinstance(source, dict):
        fail(f"{target} provenance.source must be an object")
    if source.get("commit") != commit:
        fail(f"{target} provenance commit does not match {commit}")
    if source.get("dirty") is not False:
        fail(f"{target} provenance must record a clean source checkout")
    repository = normalize_repository(
        source.get("repository"), f"{target} provenance source repository"
    )
    qt = value.get("qt")
    if not isinstance(qt, dict):
        fail(f"{target} provenance.qt must be an object")
    corresponding_source = qt.get("correspondingSource")
    if not isinstance(corresponding_source, dict):
        fail(f"{target} provenance Qt corresponding-source binding is missing")
    expected_source = expected_qt_source_contract()
    if set(corresponding_source) != set(expected_source) | {"bundleSha256"}:
        fail(
            f"{target} provenance Qt corresponding-source keys differ "
            "from the release contract"
        )
    contract_without_bundle = {
        key: value
        for key, value in corresponding_source.items()
        if key != "bundleSha256"
    }
    if not qt_sdk.json_exact_equal(contract_without_bundle, expected_source):
        fail(
            f"{target} provenance Qt corresponding-source contract differs "
            "from the release checkout"
        )
    if corresponding_source.get("bundleSha256") != qt_bundle_sha256:
        fail(f"{target} provenance Qt source bundle digest is stale")
    return repository


def _plan(
    directory: Path,
    *,
    require_receipts: bool,
    require_finalization: bool,
):
    try:
        return stager.build_stage_plan(
            directory,
            channel="canary",
            require_receipts=require_receipts,
            require_finalization=require_finalization,
        )
    except stager.AssetStagingError as error:
        fail(str(error))


def target_package_fingerprints(
    plan,
    *,
    version: str,
) -> dict[str, AssetFingerprint]:
    packages: dict[str, AssetFingerprint] = {}
    for target_name in unsigned_canary.TARGETS:
        target = release_targets.TARGET_BY_NAME[target_name]
        rows = [
            item
            for item in plan.targets[target_name].package_files
            if stager.package_description(item.name) is not None
        ]
        signatures = [
            item
            for item in plan.targets[target_name].package_files
            if stager.signature_description(item.name) is not None
        ]
        if signatures:
            fail(f"{target_name} unsigned canary must not include detached signatures")
        if len(rows) != 1:
            fail(f"{target_name} unsigned canary must contain exactly one package")
        row = rows[0]
        try:
            unsigned_canary.validate_package_filename(
                row.name,
                target_name,
                version,
            )
        except unsigned_canary.UnsignedCanaryPolicyError as error:
            fail(f"{target_name} unsigned canary package: {error}")
        packages[target_name] = AssetFingerprint(
            filename=row.name,
            size_bytes=row.size_bytes,
            sha256=row.sha256,
            kind="package",
            target=target_name,
            platform=target.platform,
        )
    return packages


def verify_target_release_metadata(
    directory: Path,
    plan,
    *,
    target_name: str,
    version: str,
    commit: str,
) -> None:
    target = release_targets.TARGET_BY_NAME[target_name]
    names = stager.METADATA_FILENAMES[target_name]
    metadata_names = set(names.values())
    artifact_paths = [
        item.path
        for item in plan.targets[target_name].package_files
        if item.name not in metadata_names
    ]
    packages = sorted(
        (
            path
            for path in artifact_paths
            if release_metadata.package_format(path.name) != "unknown"
        ),
        key=lambda path: path.name,
    )
    signatures = sorted(
        (
            path
            for path in artifact_paths
            if release_metadata.signature_suffix(path.name) is not None
        ),
        key=lambda path: path.name,
    )
    artifacts = release_metadata.artifact_rows(packages, signatures)
    try:
        source_version, _ = (
            release_metadata.release_version.validated_source_version(
                REPOSITORY_ROOT
            )
        )
        release_metadata.verify_platform_package_shape(artifacts, target)
        release_metadata.verify_checksums(directory, artifacts, names)
        provenance, distribution_version, prerelease = (
            release_metadata.verify_provenance(
                directory,
                "release",
                artifacts,
                True,
                target,
                names,
                REPOSITORY_ROOT,
                commit,
                source_version,
                version,
            )
        )
        release_metadata.verify_distribution_package_shape(
            packages,
            target,
            distribution_version,
            prerelease,
        )
        release_metadata.verify_sbom(
            directory,
            artifacts,
            provenance["source"]["commit"],
            source_version,
            distribution_version,
            target,
            names,
        )
    except (SystemExit, qt_sdk.QtSdkError) as error:
        fail(f"{target_name} release metadata is invalid: {error}")


def verify_core_assets(
    directory: Path,
    *,
    version: str,
    tag: str,
    commit: str,
    require_receipts: bool,
    require_finalization: bool,
    repository: str | None = None,
    release_id: int | None = None,
    qt_verifier: QtVerifier = verify_qt_bundle,
) -> tuple[object, dict[str, Path], dict[str, AssetFingerprint]]:
    unsigned_canary.validate_release_identity(
        version=version,
        tag=tag,
        commit=commit,
        repository=repository or "Jurshsmith/chaft",
    )
    files = scan_assets(directory)
    expected_count = (
        COMPLETE_ASSET_COUNT
        if require_finalization
        else PREFINAL_ASSET_COUNT
        if require_receipts
        else BASE_ASSET_COUNT
    )
    if len(files) != expected_count:
        fail(
            f"release directory must contain exactly {expected_count} assets, "
            f"found {len(files)}"
        )
    plan = _plan(
        directory,
        require_receipts=require_receipts,
        require_finalization=require_finalization,
    )
    packages = target_package_fingerprints(plan, version=version)
    bundle = files[QT_SOURCE_BUNDLE]
    checksum = files[QT_SOURCE_CHECKSUM]
    qt_verifier(bundle, checksum)
    qt_digest = stager.fingerprint_file(bundle).sha256
    for target_name in unsigned_canary.TARGETS:
        verify_target_release_metadata(
            directory,
            plan,
            target_name=target_name,
            version=version,
            commit=commit,
        )

    repositories = {
        provenance_identity(
            files[stager.METADATA_FILENAMES[target_name]["provenance"]],
            target=target_name,
            version=version,
            commit=commit,
            qt_bundle_sha256=qt_digest,
        )
        for target_name in unsigned_canary.TARGETS
    }
    if len(repositories) != 1:
        fail("platform provenance repositories do not agree")
    observed_repository = next(iter(repositories))
    if repository is not None and observed_repository.lower() != repository.lower():
        fail("platform provenance repository does not match requested repository")

    if require_receipts:
        if repository is None or release_id is None:
            fail("receipt verification requires repository and release ID")
        if not isinstance(release_id, int) or isinstance(release_id, bool) or release_id <= 0:
            fail("release ID must be a positive integer")
        asset_ids: set[int] = set()
        for target_name, package in packages.items():
            target = release_targets.TARGET_BY_NAME[target_name]
            receipt_name = unsigned_canary.RECEIPT_FILENAMES[target_name]
            receipt = read_json(
                files[receipt_name], f"{target_name} unsigned receipt"
            )
            try:
                unsigned_canary.validate_receipt_document(
                    receipt,
                    expected_target=target_name,
                    expected_platform=target.platform,
                    expected_package=unsigned_canary.FileFingerprint(
                        filename=package.filename,
                        size_bytes=package.size_bytes,
                        sha256=package.sha256,
                    ),
                    expected_version=version,
                    expected_tag=tag,
                    expected_commit=commit,
                    expected_repository=repository,
                )
            except unsigned_canary.UnsignedCanaryPolicyError as error:
                fail(f"{target_name} unsigned receipt is invalid: {error}")
            release = receipt["release"]
            asset = receipt["asset"]
            assert isinstance(release, dict) and isinstance(asset, dict)
            if release.get("id") != release_id:
                fail(
                    f"{target_name} receipt release ID does not match "
                    "requested release"
                )
            asset_id = asset.get("id")
            assert isinstance(asset_id, int)
            if asset_id in asset_ids:
                fail("unsigned-canary receipts must bind distinct release asset IDs")
            asset_ids.add(asset_id)
    return plan, files, packages


def verify_base_assets(
    directory: Path,
    *,
    version: str,
    tag: str,
    commit: str,
    qt_verifier: QtVerifier = verify_qt_bundle,
) -> None:
    verify_core_assets(
        directory,
        version=version,
        tag=tag,
        commit=commit,
        require_receipts=False,
        require_finalization=False,
        qt_verifier=qt_verifier,
    )


def classify_assets(
    plan,
    files: Mapping[str, Path],
) -> list[AssetFingerprint]:
    result: list[AssetFingerprint] = []
    for target_name in unsigned_canary.TARGETS:
        target = release_targets.TARGET_BY_NAME[target_name]
        metadata = stager.METADATA_FILENAMES[target_name]
        for item in plan.targets[target_name].package_files:
            if stager.package_description(item.name) is not None:
                kind = "package"
            elif item.name == metadata["checksums"]:
                kind = "platform-checksums"
            elif item.name == metadata["sbom"]:
                kind = "sbom"
            elif item.name == metadata["provenance"]:
                kind = "provenance"
            else:
                fail(f"unsupported unsigned-canary target asset: {item.name}")
            result.append(
                AssetFingerprint(
                    filename=item.name,
                    size_bytes=item.size_bytes,
                    sha256=item.sha256,
                    kind=kind,
                    target=target_name,
                    platform=target.platform,
                )
            )
        receipt = plan.targets[target_name].receipt
        if receipt is None:
            fail(f"missing {target_name} unsigned-canary receipt")
        result.append(
            AssetFingerprint(
                filename=receipt.name,
                size_bytes=receipt.size_bytes,
                sha256=receipt.sha256,
                kind="unsigned-canary-verification",
                target=target_name,
                platform=target.platform,
            )
        )
    result.extend(
        (
            fingerprint(files[QT_SOURCE_BUNDLE], kind="qt-corresponding-source"),
            fingerprint(
                files[QT_SOURCE_CHECKSUM],
                kind="qt-corresponding-source-checksum",
            ),
        )
    )
    if len(result) != PREFINAL_ASSET_COUNT:
        fail(f"pre-final canary namespace must describe {PREFINAL_ASSET_COUNT} assets")
    return sorted(result, key=lambda item: item.filename)


def inventory_document(
    *,
    assets: Sequence[AssetFingerprint],
    repository: str,
    release_id: int,
    version: str,
    tag: str,
    commit: str,
) -> dict[str, object]:
    return {
        "schemaVersion": SCHEMA_VERSION,
        "channel": "canary",
        "signingStatus": "unsigned-canary",
        "warning": unsigned_canary.WARNING,
        "repository": repository,
        "releaseId": release_id,
        "version": version,
        "tag": tag,
        "commit": commit,
        "assetCount": len(assets),
        "assets": [
            {
                "filename": item.filename,
                "sizeBytes": item.size_bytes,
                "sha256": item.sha256,
                "kind": item.kind,
                **({"target": item.target} if item.target is not None else {}),
                **({"platform": item.platform} if item.platform is not None else {}),
            }
            for item in assets
        ],
    }


def serialized_json(value: Mapping[str, object]) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def atomic_write_bytes(path: Path, content: bytes) -> None:
    if path.exists() or path.is_symlink():
        fail(f"finalization output already exists: {path.name}")
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp"
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def finalize_assets(
    directory: Path,
    *,
    version: str,
    tag: str,
    commit: str,
    repository: str,
    release_id: int,
    qt_verifier: QtVerifier = verify_qt_bundle,
) -> None:
    plan, files, _packages = verify_core_assets(
        directory,
        version=version,
        tag=tag,
        commit=commit,
        require_receipts=True,
        require_finalization=False,
        repository=repository,
        release_id=release_id,
        qt_verifier=qt_verifier,
    )
    assets = classify_assets(plan, files)
    inventory = inventory_document(
        assets=assets,
        repository=repository,
        release_id=release_id,
        version=version,
        tag=tag,
        commit=commit,
    )
    inventory_bytes = serialized_json(inventory)
    inventory_digest = hashlib.sha256(inventory_bytes).hexdigest()
    checksum_rows = [
        f"{item.sha256}  {item.filename}\n" for item in assets
    ]
    checksum_rows.append(f"{inventory_digest}  {INVENTORY_FILENAME}\n")
    aggregate_bytes = "".join(sorted(checksum_rows)).encode("ascii")

    directory = Path(directory)
    atomic_write_bytes(directory / INVENTORY_FILENAME, inventory_bytes)
    atomic_write_bytes(
        directory / AGGREGATE_CHECKSUM_FILENAME, aggregate_bytes
    )
    verify_complete_assets(
        directory,
        version=version,
        tag=tag,
        commit=commit,
        repository=repository,
        release_id=release_id,
        qt_verifier=qt_verifier,
    )


def verify_complete_assets(
    directory: Path,
    *,
    version: str,
    tag: str,
    commit: str,
    repository: str,
    release_id: int,
    qt_verifier: QtVerifier = verify_qt_bundle,
) -> None:
    plan, files, _packages = verify_core_assets(
        directory,
        version=version,
        tag=tag,
        commit=commit,
        require_receipts=True,
        require_finalization=True,
        repository=repository,
        release_id=release_id,
        qt_verifier=qt_verifier,
    )
    assets = classify_assets(plan, files)
    expected_inventory = inventory_document(
        assets=assets,
        repository=repository,
        release_id=release_id,
        version=version,
        tag=tag,
        commit=commit,
    )
    actual_inventory = read_json(
        files[INVENTORY_FILENAME], "canary release inventory"
    )
    if actual_inventory != expected_inventory:
        fail(
            "canary release inventory does not describe the exact "
            f"{PREFINAL_ASSET_COUNT}-asset set"
        )
    inventory_fingerprint = fingerprint(
        files[INVENTORY_FILENAME], kind="release-inventory"
    )
    expected_rows = {
        item.filename: item.sha256 for item in assets
    }
    expected_rows[INVENTORY_FILENAME] = inventory_fingerprint.sha256
    try:
        aggregate = files[AGGREGATE_CHECKSUM_FILENAME].read_text(
            encoding="ascii"
        )
    except OSError as error:
        fail(f"cannot read aggregate checksums: {error}")
    except UnicodeDecodeError:
        fail("aggregate checksums must contain ASCII only")
    actual_rows: dict[str, str] = {}
    for line_number, line in enumerate(aggregate.splitlines(), 1):
        match = re.fullmatch(r"([0-9a-f]{64})  ([^/\\\r\n]+)", line)
        if match is None:
            fail(f"invalid aggregate checksum line {line_number}")
        digest, filename = match.groups()
        if filename in actual_rows:
            fail(f"duplicate aggregate checksum row for {filename}")
        actual_rows[filename] = digest
    if actual_rows != expected_rows:
        fail(
            "aggregate checksums do not bind the exact "
            f"{PREFINAL_ASSET_COUNT + 1} preceding assets"
        )
    if len(files) != COMPLETE_ASSET_COUNT:
        fail(f"complete canary namespace must contain exactly {COMPLETE_ASSET_COUNT} assets")


def add_identity_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--assets-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)


def argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify and finalize the exact Chaft unsigned-canary asset namespace."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    base = subparsers.add_parser(
        "verify-base",
        help=f"Verify exactly {BASE_ASSET_COUNT} build and Qt-source assets",
    )
    add_identity_arguments(base)
    for name, help_text in (
        (
            "finalize",
            f"Verify {PREFINAL_ASSET_COUNT} pre-final assets and "
            "create inventory/checksums",
        ),
        (
            "verify-complete",
            f"Verify the exact final {COMPLETE_ASSET_COUNT}-asset namespace",
        ),
    ):
        command = subparsers.add_parser(name, help=help_text)
        add_identity_arguments(command)
        command.add_argument("--repository", required=True)
        command.add_argument("--release-id", required=True, type=int)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    try:
        common = {
            "directory": args.assets_dir,
            "version": args.version,
            "tag": args.tag,
            "commit": args.commit,
        }
        if args.command == "verify-base":
            verify_base_assets(**common)
        elif args.command == "finalize":
            finalize_assets(
                **common,
                repository=args.repository,
                release_id=args.release_id,
            )
        else:
            verify_complete_assets(
                **common,
                repository=args.repository,
                release_id=args.release_id,
            )
    except (
        CanaryReleaseAssetError,
        unsigned_canary.UnsignedCanaryPolicyError,
    ) as error:
        parser.exit(2, f"canary release asset verification failed: {error}\n")
    print(f"{args.command} passed: {args.assets_dir}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise SystemExit(1)
