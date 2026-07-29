#!/usr/bin/env python3
import argparse
import hashlib
import importlib.util
import json
import os
import platform
import re
import sys
from datetime import datetime
from pathlib import Path

import release_targets

QT_TOOLS = Path(__file__).resolve().parents[1] / "qt"
sys.path.insert(0, str(QT_TOOLS))
import build_qt as qt_sdk  # noqa: E402
import source_bundle as qt_source  # noqa: E402

PACKAGE_FORMATS = (
    (".tar.gz", "linux", "linux-tgz"),
    (".tgz", "linux", "linux-tgz"),
    (".appimage", "linux", "linux-appimage"),
    (".dmg", "macos", "macos-dmg"),
    (".zip", "windows", "windows-zip"),
    (".msi", "windows", "windows-msi"),
    (".exe", "windows", "windows-exe"),
)
SIGNATURE_SUFFIXES = (".sig", ".asc")
SOURCE_MATERIALS = (
    "Cargo.lock",
    "Cargo.toml",
    "apps/desktop-qt/CMakeLists.txt",
    "tools/desktop/macos-adhoc-verify.cmake",
)


def load_release_version_module():
    path = Path(__file__).with_name("release-version.py")
    spec = importlib.util.spec_from_file_location(
        "chaft_release_version_verify", path
    )
    if spec is None or spec.loader is None:
        raise SystemExit(f"unable to load release version contract: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release_version = load_release_version_module()


def fail(message):
    raise SystemExit(message)


def repo_root():
    return Path(__file__).resolve().parents[2]


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def package_format(name):
    lowered = name.lower()
    for suffix, _, format_name in PACKAGE_FORMATS:
        if lowered.endswith(suffix):
            return format_name
    return "unknown"


def package_platform(name):
    lowered = name.lower()
    for suffix, platform_name, _ in PACKAGE_FORMATS:
        if lowered.endswith(suffix):
            return platform_name
    return None


def normalized_platform_name(value):
    try:
        return release_targets.normalize_platform(value)
    except release_targets.ReleaseTargetError:
        return (value or "").strip().lower()


def current_platform_name():
    return release_targets.current_platform()


def metadata_names(target):
    if isinstance(target, release_targets.ReleaseTarget):
        return target.metadata_names
    try:
        return release_targets.TARGET_BY_NAME[str(target)].metadata_names
    except KeyError:
        candidates = release_targets.targets_for_platform(target)
        if len(candidates) == 1:
            return candidates[0].metadata_names
        fail(f"package verification target must include architecture: {target!r}")


def load_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"{path.name} is not valid JSON: {error}")


def package_files(package_dir):
    return sorted(
        [
            path
            for path in package_dir.iterdir()
            if path.is_file() and package_format(path.name) != "unknown"
        ],
        key=lambda path: path.name,
    )


def signature_suffix(name):
    lowered = name.lower()
    return next(
        (suffix for suffix in SIGNATURE_SUFFIXES if lowered.endswith(suffix)),
        None,
    )


def signature_files(package_dir, packages):
    package_names = {path.name for path in packages}
    signatures = []
    for path in package_dir.iterdir():
        if not path.is_file():
            continue
        suffix = signature_suffix(path.name)
        if suffix is None:
            continue
        signed_artifact = path.name[: -len(suffix)]
        if signed_artifact not in package_names:
            fail(
                f"detached signature {path.name} does not correspond to a package file"
            )
        if path.stat().st_size <= 0:
            fail(f"detached signature is empty: {path.name}")
        signatures.append(path)
    return sorted(signatures, key=lambda path: path.name)


def artifact_rows(packages, signatures):
    package_rows = [
        {
            "name": path.name,
            "packageFormat": package_format(path.name),
            "sizeBytes": path.stat().st_size,
            "sha256": file_sha256(path),
        }
        for path in packages
    ]
    signature_rows = []
    for path in signatures:
        suffix = signature_suffix(path.name)
        signature_rows.append(
            {
                "name": path.name,
                "packageFormat": "detached-signature",
                "signatureFormat": suffix[1:],
                "signedArtifact": path.name[: -len(suffix)],
                "sizeBytes": path.stat().st_size,
                "sha256": file_sha256(path),
            }
        )
    return sorted(package_rows + signature_rows, key=lambda row: row["name"])


def source_material_rows(source_root):
    root = source_root
    rows = {}
    for relative in SOURCE_MATERIALS:
        try:
            path = qt_sdk.trusted_source_file(
                root,
                relative,
                f"release source material {relative}",
            )
        except qt_sdk.QtSdkError as error:
            fail(str(error))
        rows[relative] = {
            "sha256": file_sha256(path),
            "sizeBytes": path.stat().st_size,
        }
    return rows


def parse_checksums(path):
    rows = {}
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-f]{64})  ([^\r\n/]+)", line)
        if not match:
            fail(f"invalid {path.name} line {line_number}: {line!r}")
        digest, name = match.groups()
        if name in rows:
            fail(f"duplicate checksum row for {name}")
        rows[name] = digest
    return rows


def require_timestamp(value, field):
    if not isinstance(value, str) or not value:
        fail(f"{field} is missing")
    if not re.fullmatch(
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})",
        value,
    ):
        fail(f"{field} must be an RFC 3339 date-time with a timezone: {value}")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        fail(f"{field} is not a valid RFC 3339 date-time: {value}")
    if parsed.tzinfo is None:
        fail(f"{field} must include a timezone: {value}")


def verify_directory_shape(package_dir, names):
    if not package_dir.is_dir():
        fail(f"package directory not found: {package_dir}")

    staging_dir = package_dir / "_CPack_Packages"
    if staging_dir.exists():
        fail(f"CPack staging directory must not be uploaded: {staging_dir}")

    directories = sorted(path.name for path in package_dir.iterdir() if path.is_dir())
    if directories:
        fail(f"unexpected directories in package directory: {', '.join(directories)}")

    present = {path.name for path in package_dir.iterdir() if path.is_file()}
    required_metadata = set(names.values())
    missing = sorted(required_metadata - present)
    if missing:
        fail(f"missing release metadata file(s): {', '.join(missing)}")

    packages = package_files(package_dir)
    signatures = signature_files(package_dir, packages)
    allowed = (
        required_metadata
        | {path.name for path in packages}
        | {path.name for path in signatures}
    )
    unexpected = sorted(present - allowed)
    if unexpected:
        fail(f"unexpected file(s) in package directory: {', '.join(unexpected)}")
    return packages, signatures


def verify_checksums(package_dir, artifacts, names):
    checksum_name = names["checksums"]
    checksum_rows = parse_checksums(package_dir / checksum_name)
    artifact_names = {artifact["name"] for artifact in artifacts}
    checksum_names = set(checksum_rows)

    missing = sorted(artifact_names - checksum_names)
    extra = sorted(checksum_names - artifact_names)
    if missing:
        fail(f"{checksum_name} is missing artifact row(s): {', '.join(missing)}")
    if extra:
        fail(f"{checksum_name} has non-artifact row(s): {', '.join(extra)}")

    for artifact in artifacts:
        actual = artifact["sha256"]
        expected = checksum_rows[artifact["name"]]
        if actual != expected:
            fail(
                f"checksum mismatch for {artifact['name']}: "
                f"expected {expected}, got {actual}"
            )


def verify_platform_package_shape(artifacts, target):
    unexpected = [
        artifact["name"]
        for artifact in artifacts
        if artifact.get("packageFormat") != "detached-signature"
        and package_platform(artifact["name"]) != target.platform
    ]
    if unexpected:
        fail(
            f"{target.name} package directory contains unexpected package type(s): "
            + ", ".join(sorted(unexpected))
        )


def verify_distribution_package_shape(packages, target, distribution_version, prerelease):
    del prerelease
    expected = target.package_name(distribution_version)
    actual = [path.name for path in packages]
    if actual != [expected]:
        fail(
            f"{target.name} release package must be exactly {expected}; "
            f"found {', '.join(actual)}"
        )


def metadata_property_map(metadata):
    properties = metadata.get("properties")
    if not isinstance(properties, list):
        return {}
    return {
        item.get("name"): item.get("value")
        for item in properties
        if isinstance(item, dict)
    }


def verify_sbom(
    package_dir,
    artifacts,
    source_commit,
    source_version,
    distribution_version,
    target,
    names,
):
    sbom = load_json(package_dir / names["sbom"])
    if sbom.get("bomFormat") != "CycloneDX":
        fail("SBOM bomFormat must be CycloneDX")
    if str(sbom.get("specVersion", "")).split(".")[0] != "1":
        fail("SBOM specVersion must be a CycloneDX 1.x version")

    metadata = sbom.get("metadata")
    if not isinstance(metadata, dict):
        fail("SBOM metadata object is missing")
    require_timestamp(metadata.get("timestamp"), "SBOM metadata.timestamp")

    component = metadata.get("component")
    if not isinstance(component, dict) or component.get("name") != "Chaft Desktop":
        fail("SBOM metadata.component must describe Chaft Desktop")
    if component.get("version") != distribution_version:
        fail("SBOM component version does not match the distribution version")
    if (
        component.get("bom-ref")
        != f"pkg:generic/chaft-desktop@{distribution_version}"
    ):
        fail("SBOM component bom-ref does not match the distribution version")

    metadata_properties = metadata_property_map(metadata)
    if metadata_properties.get("chaft:sourceCommit") != source_commit:
        fail("SBOM metadata source commit does not match provenance source.commit")
    if metadata_properties.get("chaft:packageTarget") != target.name:
        fail("SBOM package target does not match its target-qualified filename")
    if metadata_properties.get("chaft:packagePlatform") != target.platform:
        fail("SBOM package platform does not match its platform-qualified filename")
    if (
        metadata_properties.get("chaft:packageArchitecture")
        != target.architecture
    ):
        fail("SBOM package architecture does not match its target-qualified filename")
    if metadata_properties.get("chaft:sourceVersion") != source_version:
        fail("SBOM source version does not match the release source version")
    if (
        metadata_properties.get("chaft:distributionVersion")
        != distribution_version
    ):
        fail("SBOM distribution version does not match provenance")

    components = sbom.get("components")
    if not isinstance(components, list) or not components:
        fail("SBOM components must include Cargo dependency components")

    properties = sbom.get("properties")
    if not isinstance(properties, list):
        fail("SBOM properties array is missing")
    property_map = {
        item.get("name"): item.get("value")
        for item in properties
        if isinstance(item, dict)
    }
    for artifact in artifacts:
        key = f"chaft:artifact:{artifact['name']}:sha256"
        if property_map.get(key) != artifact["sha256"]:
            fail(f"SBOM missing or stale artifact checksum property: {key}")
        key = f"chaft:artifact:{artifact['name']}:packageFormat"
        if property_map.get(key) != artifact["packageFormat"]:
            fail(f"SBOM missing or stale artifact packageFormat property: {key}")
        if artifact.get("signedArtifact"):
            key = f"chaft:artifact:{artifact['name']}:signedArtifact"
            if property_map.get(key) != artifact["signedArtifact"]:
                fail(f"SBOM missing or stale signedArtifact property: {key}")
        if artifact.get("signatureFormat"):
            key = f"chaft:artifact:{artifact['name']}:signatureFormat"
            if property_map.get(key) != artifact["signatureFormat"]:
                fail(f"SBOM missing or stale signatureFormat property: {key}")


def verify_provenance_materials(provenance, source_root):
    materials = provenance.get("materials")
    if not isinstance(materials, list) or not materials:
        fail("provenance materials array is missing")

    actual = {}
    for material in materials:
        if not isinstance(material, dict):
            fail("provenance material row is not an object")

        name = material.get("name")
        sha256 = material.get("sha256")
        size_bytes = material.get("sizeBytes")
        if not isinstance(name, str) or not name:
            fail("provenance material row is missing name")
        if name in actual:
            fail(f"duplicate provenance material row for {name}")
        if not re.fullmatch(r"[0-9a-f]{64}", str(sha256 or "")):
            fail(f"provenance material row has invalid sha256: {name}")
        if not isinstance(size_bytes, int) or isinstance(size_bytes, bool) or size_bytes <= 0:
            fail(f"provenance material row has invalid sizeBytes: {name}")

        actual[name] = {"sha256": sha256, "sizeBytes": size_bytes}

    if actual != source_material_rows(source_root):
        fail("provenance material rows do not match current source checkout")


def verify_qt_release_binding(
    provenance,
    source_root,
    target,
    qt_source_bundle=None,
    qt_source_checksum=None,
):
    qt_record = provenance.get("qt")
    if not isinstance(qt_record, dict) or set(qt_record) != {
        "schemaVersion",
        "sdk",
        "correspondingSource",
    }:
        fail("provenance Qt release binding is missing or malformed")
    if (
        type(qt_record["schemaVersion"]) is not int
        or qt_record["schemaVersion"] != 1
    ):
        fail("provenance Qt release binding schemaVersion is unsupported")

    sdk = qt_record["sdk"]
    if not isinstance(sdk, dict) or set(sdk) != {
        "identity",
        "provenance",
        "provenanceSha256",
    }:
        fail("provenance Qt SDK binding is missing or malformed")
    embedded = sdk["provenance"]
    manifest_path = source_root / "tools" / "qt" / "qt-6.8.4.json"
    manifest = qt_sdk.load_manifest(
        manifest_path, recipe_root=source_root
    )
    qt_sdk.validate_provenance_object(
        embedded,
        manifest,
        target.name,
        recipe_root=source_root,
    )
    if sdk["identity"] != embedded["identity"]:
        fail("provenance Qt SDK identity differs from embedded SDK provenance")
    expected_provenance_digest = qt_sdk.sha256_bytes(
        qt_sdk.canonical_json(embedded)
    )
    if sdk["provenanceSha256"] != expected_provenance_digest:
        fail("provenance Qt SDK provenanceSha256 is stale")

    expected_source = qt_source.release_contract(
        manifest_path,
        source_root / "packaging" / "qt",
        source_root / "tools" / "qt" / "source_bundle.py",
        recipe_root=source_root,
    )
    source = qt_record["correspondingSource"]
    if not isinstance(source, dict):
        fail("provenance Qt corresponding-source binding is malformed")
    if set(source) != set(expected_source) | {"bundleSha256"}:
        fail(
            "provenance Qt corresponding-source binding keys differ from "
            "the release contract"
        )
    bundle_sha256 = source.get("bundleSha256")
    contract_without_bundle = {
        key: value for key, value in source.items() if key != "bundleSha256"
    }
    if not qt_sdk.json_exact_equal(
        contract_without_bundle, expected_source
    ):
        fail(
            "provenance Qt corresponding-source contract differs from the "
            "release checkout"
        )
    if embedded["manifestSha256"] != source["sdkManifestSha256"]:
        fail("provenance Qt SDK and corresponding-source manifests differ")
    if embedded["contractSha256"] != source["sdkContractSha256"]:
        fail("provenance Qt SDK and corresponding-source recipes differ")
    if bundle_sha256 is not None and re.fullmatch(
        r"[0-9a-f]{64}", str(bundle_sha256)
    ) is None:
        fail("provenance Qt corresponding-source bundleSha256 is invalid")

    expected_bundle_digest = os.environ.get(
        "CHAFT_QT_SOURCE_BUNDLE_SHA256"
    )
    if expected_bundle_digest is not None:
        if re.fullmatch(r"[0-9a-f]{64}", expected_bundle_digest) is None:
            fail("CHAFT_QT_SOURCE_BUNDLE_SHA256 is invalid")
        if bundle_sha256 != expected_bundle_digest:
            fail(
                "provenance Qt corresponding-source bundleSha256 differs "
                "from the release workflow"
            )

    if qt_source_bundle is not None:
        bundle_path = qt_source_bundle.resolve()
        checksum_path = (
            qt_source_checksum.resolve()
            if qt_source_checksum is not None
            else None
        )
        qt_source.verify_bundle(
            bundle_path,
            checksum_path,
            manifest_path=manifest_path,
            package_dir=source_root / "packaging" / "qt",
            recipe_root=source_root,
        )
        actual_bundle_digest = file_sha256(bundle_path)
        if bundle_sha256 != actual_bundle_digest:
            fail(
                "provenance Qt corresponding-source bundleSha256 does not "
                "match the authenticated release bundle"
            )
    elif qt_source_checksum is not None:
        fail("--qt-source-checksum requires --qt-source-bundle")


def verify_provenance(
    package_dir,
    profile,
    artifacts,
    require_clean,
    target,
    names,
    source_root,
    expected_commit,
    source_version,
    expected_distribution_version,
    qt_source_bundle=None,
    qt_source_checksum=None,
):
    provenance = load_json(package_dir / names["provenance"])
    if provenance.get("schemaVersion") != "chaft.desktop.provenance.v2":
        fail("provenance schemaVersion is unsupported")
    if provenance.get("sourceVersion") != source_version:
        fail("provenance sourceVersion does not match the release checkout")
    distribution_version = provenance.get("distributionVersion")
    if not isinstance(distribution_version, str):
        fail("provenance distributionVersion is missing")
    distribution_version, prerelease = (
        release_version.validated_distribution_version(
            distribution_version, source_version
        )
    )
    if provenance.get("version") != distribution_version:
        fail("provenance version alias does not match distributionVersion")
    if (
        expected_distribution_version is not None
        and distribution_version != expected_distribution_version
    ):
        fail(
            "provenance distributionVersion does not match the expected "
            "distribution version"
        )
    if provenance.get("profile") != profile:
        fail(f"provenance profile must be {profile!r}")
    if provenance.get("packageTarget") != target.name:
        fail("provenance package target does not match its target-qualified filename")
    if provenance.get("packagePlatform") != target.platform:
        fail("provenance package platform does not match its platform-qualified filename")
    if provenance.get("packageArchitecture") != target.architecture:
        fail(
            "provenance package architecture does not match its "
            "target-qualified filename"
        )
    host = provenance.get("platform")
    if not isinstance(host, dict):
        fail("provenance platform object is missing")
    try:
        host_architecture = release_targets.normalize_architecture(
            host.get("machine")
        )
    except release_targets.ReleaseTargetError as error:
        fail(f"provenance host architecture is invalid: {error}")
    if host_architecture != target.architecture:
        fail(
            f"provenance host architecture {host_architecture} does not match "
            f"native target {target.architecture}"
        )
    require_timestamp(provenance.get("createdAt"), "provenance.createdAt")

    source = provenance.get("source")
    if not isinstance(source, dict) or not source.get("commit"):
        fail("provenance source.commit is missing")
    if expected_commit is not None and source.get("commit") != expected_commit:
        fail("provenance source.commit does not match the expected release commit")
    if require_clean and source.get("dirty") is not False:
        fail("CI release provenance must be generated from a clean worktree")

    if os.environ.get("GITHUB_ACTIONS") == "true":
        github = provenance.get("github")
        if not isinstance(github, dict):
            fail("CI provenance is missing github context")
        for key in ("GITHUB_REPOSITORY", "GITHUB_RUN_ID", "GITHUB_SHA"):
            if not github.get(key):
                fail(f"CI provenance is missing {key}")
        release_commit = github.get("CHAFT_RELEASE_COMMIT")
        if release_commit is not None:
            if not isinstance(release_commit, str) or re.fullmatch(
                r"[0-9a-fA-F]{40,64}", release_commit
            ) is None:
                fail("CI provenance CHAFT_RELEASE_COMMIT is invalid")
            release_commit = release_commit.lower()
            if expected_commit is not None and release_commit != expected_commit:
                fail(
                    "CI provenance CHAFT_RELEASE_COMMIT does not match the "
                    "expected release commit"
                )
            if source.get("commit") != release_commit:
                fail(
                    "CI provenance source.commit does not match "
                    "CHAFT_RELEASE_COMMIT"
                )
        else:
            ci_commit = expected_commit or os.environ.get("GITHUB_SHA")
            if github.get("GITHUB_SHA") != ci_commit:
                fail("CI provenance GITHUB_SHA does not match the expected CI commit")
            if source.get("commit") != github.get("GITHUB_SHA"):
                fail("CI provenance source.commit does not match GITHUB_SHA")

    provenance_artifacts = provenance.get("artifacts")
    if not isinstance(provenance_artifacts, list):
        fail("provenance artifacts array is missing")

    artifact_fields = (
        "packageFormat",
        "sha256",
        "sizeBytes",
        "signatureFormat",
        "signedArtifact",
    )
    expected = {
        artifact["name"]: {
            field: artifact[field]
            for field in artifact_fields
            if field in artifact
        }
        for artifact in artifacts
    }
    actual = {}
    for artifact in provenance_artifacts:
        if not isinstance(artifact, dict):
            fail("provenance artifact row is not an object")
        name = artifact.get("name")
        if not isinstance(name, str) or not name:
            fail("provenance artifact row is missing name")
        if name in actual:
            fail(f"duplicate provenance artifact row for {name}")
        actual[name] = {
            field: artifact[field]
            for field in artifact_fields
            if field in artifact
        }
    if actual != expected:
        fail("provenance artifact rows do not match package files")
    verify_provenance_materials(provenance, source_root)
    verify_qt_release_binding(
        provenance,
        source_root,
        target,
        qt_source_bundle,
        qt_source_checksum,
    )
    return provenance, distribution_version, prerelease


def main():
    parser = argparse.ArgumentParser(
        description="Verify Chaft desktop release package metadata."
    )
    parser.add_argument("profile", nargs="?", default="release", choices=("debug", "release"))
    parser.add_argument("--package-dir", type=Path)
    parser.add_argument(
        "--source-root",
        type=Path,
        default=repo_root(),
        help="Source checkout whose release inputs must match provenance materials.",
    )
    parser.add_argument(
        "--expected-commit",
        help="Exact release commit expected in provenance and GitHub CI context.",
    )
    parser.add_argument(
        "--expected-source-version",
        default=os.environ.get("CHAFT_SOURCE_VERSION"),
        help=(
            "Stable source version expected in provenance and the source checkout. "
            "Defaults to CHAFT_SOURCE_VERSION when set."
        ),
    )
    parser.add_argument(
        "--expected-distribution-version",
        default=os.environ.get("CHAFT_DISTRIBUTION_VERSION"),
        help=(
            "Exact SemVer package version expected in filenames and metadata. "
            "Defaults to CHAFT_DISTRIBUTION_VERSION when set."
        ),
    )
    parser.add_argument(
        "--require-clean",
        action="store_true",
        help="Require provenance to report a clean Git worktree.",
    )
    parser.add_argument(
        "--target",
        choices=release_targets.TARGET_NAMES,
        help="Exact native release target, including architecture.",
    )
    parser.add_argument(
        "--platform",
        help="Package platform selector; use with --architecture.",
    )
    parser.add_argument(
        "--architecture",
        help="Package architecture selector; use with --platform.",
    )
    parser.add_argument(
        "--qt-source-bundle",
        type=Path,
        help="Authenticated Qt corresponding-source bundle to cross-check.",
    )
    parser.add_argument(
        "--qt-source-checksum",
        type=Path,
        help="Checksum sidecar for --qt-source-bundle.",
    )
    args = parser.parse_args()

    try:
        source_root = qt_sdk.trusted_source_root(args.source_root)
    except qt_sdk.QtSdkError as error:
        fail(str(error))
    if args.expected_commit is not None and re.fullmatch(
        r"[0-9a-fA-F]{40,64}", args.expected_commit
    ) is None:
        fail("expected commit must be a 40-to-64 character hexadecimal revision")
    expected_commit = (
        args.expected_commit.lower() if args.expected_commit is not None else None
    )
    source_version, _ = release_version.validated_source_version(source_root)
    if (
        args.expected_source_version is not None
        and args.expected_source_version != source_version
    ):
        fail(
            "expected source version does not match the release checkout: "
            f"{args.expected_source_version} != {source_version}"
        )
    expected_distribution_version = args.expected_distribution_version
    if expected_distribution_version is not None:
        expected_distribution_version, _ = (
            release_version.validated_distribution_version(
                expected_distribution_version, source_version
            )
        )
    package_dir = args.package_dir or source_root / "build" / f"desktop-{args.profile}" / "package"
    try:
        target = release_targets.resolve_target(
            target_name=args.target,
            platform_name=args.platform,
            architecture=args.architecture,
        )
    except release_targets.ReleaseTargetError as error:
        fail(str(error))
    names = metadata_names(target)
    packages, signatures = verify_directory_shape(package_dir, names)

    if not packages:
        fail(f"no package artifacts found in {package_dir}")
    artifacts = artifact_rows(packages, signatures)

    verify_platform_package_shape(artifacts, target)
    verify_checksums(package_dir, artifacts, names)
    provenance, distribution_version, prerelease = verify_provenance(
        package_dir,
        args.profile,
        artifacts,
        args.require_clean or os.environ.get("GITHUB_ACTIONS") == "true",
        target,
        names,
        source_root,
        expected_commit,
        source_version,
        expected_distribution_version,
        args.qt_source_bundle,
        args.qt_source_checksum,
    )
    verify_distribution_package_shape(
        packages, target, distribution_version, prerelease
    )
    verify_sbom(
        package_dir,
        artifacts,
        provenance["source"]["commit"],
        source_version,
        distribution_version,
        target,
        names,
    )

    print(
        "release metadata verified: "
        f"{len(packages)} package(s), {len(signatures)} signature(s) in {package_dir}"
    )


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        sys.exit(1)
