#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import platform
import re
import sys
from datetime import datetime
from pathlib import Path

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
)


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
    normalized = (value or "").strip().lower()
    if normalized in {"darwin", "mac", "macos", "osx"}:
        return "macos"
    if normalized in {"win32", "windows", "msys", "mingw", "cygwin"}:
        return "windows"
    if normalized == "linux":
        return "linux"
    return normalized


def current_platform_name():
    return normalized_platform_name(os.environ.get("RUNNER_OS") or platform.system())


def metadata_names(platform_name):
    normalized = normalized_platform_name(platform_name)
    if normalized not in {"linux", "macos", "windows"}:
        fail(
            "unsupported package verification platform "
            f"{platform_name!r}; expected Linux, macOS, or Windows"
        )
    prefix = f"chaft-desktop-{normalized}"
    return {
        "checksums": f"{prefix}-SHA256SUMS",
        "sbom": f"{prefix}-sbom.cdx.json",
        "provenance": f"{prefix}-provenance.json",
    }


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
    missing = []
    rows = {}
    for relative in SOURCE_MATERIALS:
        path = root / relative
        if not path.is_file():
            missing.append(relative)
            continue
        rows[relative] = {
            "sha256": file_sha256(path),
            "sizeBytes": path.stat().st_size,
        }
    if missing:
        fail(
            "source material file(s) missing from current checkout: "
            + ", ".join(missing)
        )
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


def verify_platform_package_shape(artifacts, platform_name):
    normalized = normalized_platform_name(platform_name)
    metadata_names(normalized)

    unexpected = [
        artifact["name"]
        for artifact in artifacts
        if artifact.get("packageFormat") != "detached-signature"
        and package_platform(artifact["name"]) != normalized
    ]
    if unexpected:
        fail(
            f"{normalized} package directory contains unexpected package type(s): "
            + ", ".join(sorted(unexpected))
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


def verify_sbom(package_dir, artifacts, source_commit, platform_name, names):
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

    metadata_properties = metadata_property_map(metadata)
    if metadata_properties.get("chaft:sourceCommit") != source_commit:
        fail("SBOM metadata source commit does not match provenance source.commit")
    if metadata_properties.get("chaft:packagePlatform") != platform_name:
        fail("SBOM package platform does not match its platform-qualified filename")

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


def verify_provenance(
    package_dir,
    profile,
    artifacts,
    require_clean,
    platform_name,
    names,
    source_root,
    expected_commit,
):
    provenance = load_json(package_dir / names["provenance"])
    if provenance.get("schemaVersion") != "chaft.desktop.provenance.v1":
        fail("provenance schemaVersion is unsupported")
    if provenance.get("profile") != profile:
        fail(f"provenance profile must be {profile!r}")
    if provenance.get("packagePlatform") != platform_name:
        fail("provenance package platform does not match its platform-qualified filename")
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
    return provenance


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
        "--require-clean",
        action="store_true",
        help="Require provenance to report a clean Git worktree.",
    )
    parser.add_argument(
        "--platform",
        default=current_platform_name(),
        help="Package platform to verify: Linux, macOS, or Windows.",
    )
    args = parser.parse_args()

    source_root = args.source_root.resolve()
    if not source_root.is_dir():
        fail(f"source root does not exist: {source_root}")
    if args.expected_commit is not None and re.fullmatch(
        r"[0-9a-fA-F]{40,64}", args.expected_commit
    ) is None:
        fail("expected commit must be a 40-to-64 character hexadecimal revision")
    expected_commit = (
        args.expected_commit.lower() if args.expected_commit is not None else None
    )
    package_dir = args.package_dir or source_root / "build" / f"desktop-{args.profile}" / "package"
    platform_name = normalized_platform_name(args.platform)
    names = metadata_names(platform_name)
    packages, signatures = verify_directory_shape(package_dir, names)

    if not packages:
        fail(f"no package artifacts found in {package_dir}")
    artifacts = artifact_rows(packages, signatures)

    verify_platform_package_shape(artifacts, platform_name)
    verify_checksums(package_dir, artifacts, names)
    provenance = verify_provenance(
        package_dir,
        args.profile,
        artifacts,
        args.require_clean or os.environ.get("GITHUB_ACTIONS") == "true",
        platform_name,
        names,
        source_root,
        expected_commit,
    )
    verify_sbom(
        package_dir,
        artifacts,
        provenance["source"]["commit"],
        platform_name,
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
