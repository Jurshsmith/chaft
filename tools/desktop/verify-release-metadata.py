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

PACKAGE_SUFFIXES = (".dmg", ".zip", ".tgz", ".tar.gz")
PLATFORM_PACKAGE_SUFFIXES = {
    "linux": (".tgz", ".tar.gz"),
    "macos": (".dmg",),
    "windows": (".zip",),
}
REQUIRED_METADATA = {
    "SHA256SUMS",
    "chaft-desktop-provenance.json",
    "chaft-desktop-sbom.cdx.json",
}


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
    if name.endswith((".tgz", ".tar.gz")):
        return "linux-tgz"
    if name.endswith(".dmg"):
        return "macos-dmg"
    if name.endswith(".zip"):
        return "windows-zip"
    return "unknown"


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
            if path.is_file() and path.name.endswith(PACKAGE_SUFFIXES)
        ],
        key=lambda path: path.name,
    )


def artifact_rows(files):
    return [
        {
            "name": path.name,
            "packageFormat": package_format(path.name),
            "sizeBytes": path.stat().st_size,
            "sha256": file_sha256(path),
        }
        for path in files
    ]


def parse_checksums(path):
    rows = {}
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-f]{64})  ([^\r\n/]+)", line)
        if not match:
            fail(f"invalid SHA256SUMS line {line_number}: {line!r}")
        digest, name = match.groups()
        if name in rows:
            fail(f"duplicate checksum row for {name}")
        rows[name] = digest
    return rows


def require_timestamp(value, field):
    if not isinstance(value, str) or not value:
        fail(f"{field} is missing")
    try:
        datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        fail(f"{field} is not an ISO timestamp: {value}")


def verify_directory_shape(package_dir):
    if not package_dir.is_dir():
        fail(f"package directory not found: {package_dir}")

    staging_dir = package_dir / "_CPack_Packages"
    if staging_dir.exists():
        fail(f"CPack staging directory must not be uploaded: {staging_dir}")

    directories = sorted(path.name for path in package_dir.iterdir() if path.is_dir())
    if directories:
        fail(f"unexpected directories in package directory: {', '.join(directories)}")

    present = {path.name for path in package_dir.iterdir() if path.is_file()}
    missing = sorted(REQUIRED_METADATA - present)
    if missing:
        fail(f"missing release metadata file(s): {', '.join(missing)}")

    allowed = REQUIRED_METADATA | {path.name for path in package_files(package_dir)}
    unexpected = sorted(present - allowed)
    if unexpected:
        fail(f"unexpected file(s) in package directory: {', '.join(unexpected)}")


def verify_checksums(package_dir, artifacts):
    checksum_rows = parse_checksums(package_dir / "SHA256SUMS")
    artifact_names = {artifact["name"] for artifact in artifacts}
    checksum_names = set(checksum_rows)

    missing = sorted(artifact_names - checksum_names)
    extra = sorted(checksum_names - artifact_names)
    if missing:
        fail(f"SHA256SUMS is missing artifact row(s): {', '.join(missing)}")
    if extra:
        fail(f"SHA256SUMS has non-artifact row(s): {', '.join(extra)}")

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
    expected_suffixes = PLATFORM_PACKAGE_SUFFIXES.get(normalized)
    if expected_suffixes is None:
        fail(
            "unsupported package verification platform "
            f"{platform_name!r}; expected Linux, macOS, or Windows"
        )

    unexpected = [
        artifact["name"]
        for artifact in artifacts
        if not artifact["name"].endswith(expected_suffixes)
    ]
    if unexpected:
        fail(
            f"{normalized} package directory contains unexpected package type(s): "
            + ", ".join(sorted(unexpected))
        )


def verify_sbom(package_dir, artifacts):
    sbom = load_json(package_dir / "chaft-desktop-sbom.cdx.json")
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


def verify_provenance(package_dir, profile, artifacts, require_clean):
    provenance = load_json(package_dir / "chaft-desktop-provenance.json")
    if provenance.get("schemaVersion") != "chaft.desktop.provenance.v1":
        fail("provenance schemaVersion is unsupported")
    if provenance.get("profile") != profile:
        fail(f"provenance profile must be {profile!r}")
    require_timestamp(provenance.get("createdAt"), "provenance.createdAt")

    source = provenance.get("source")
    if not isinstance(source, dict) or not source.get("commit"):
        fail("provenance source.commit is missing")
    if require_clean and source.get("dirty") is not False:
        fail("CI release provenance must be generated from a clean worktree")

    if os.environ.get("GITHUB_ACTIONS") == "true":
        github = provenance.get("github")
        if not isinstance(github, dict):
            fail("CI provenance is missing github context")
        for key in ("GITHUB_REPOSITORY", "GITHUB_RUN_ID", "GITHUB_SHA"):
            if not github.get(key):
                fail(f"CI provenance is missing {key}")

    provenance_artifacts = provenance.get("artifacts")
    if not isinstance(provenance_artifacts, list):
        fail("provenance artifacts array is missing")

    expected = {
        artifact["name"]: {
            "packageFormat": artifact["packageFormat"],
            "sha256": artifact["sha256"],
            "sizeBytes": artifact["sizeBytes"],
        }
        for artifact in artifacts
    }
    actual = {
        artifact.get("name"): {
            "packageFormat": artifact.get("packageFormat"),
            "sha256": artifact.get("sha256"),
            "sizeBytes": artifact.get("sizeBytes"),
        }
        for artifact in provenance_artifacts
        if isinstance(artifact, dict)
    }
    if actual != expected:
        fail("provenance artifact rows do not match package files")


def main():
    parser = argparse.ArgumentParser(
        description="Verify Chaft desktop release package metadata."
    )
    parser.add_argument("profile", nargs="?", default="release", choices=("debug", "release"))
    parser.add_argument("--package-dir", type=Path)
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

    package_dir = args.package_dir or repo_root() / "build" / f"desktop-{args.profile}" / "package"
    verify_directory_shape(package_dir)

    artifacts = artifact_rows(package_files(package_dir))
    if not artifacts:
        fail(f"no package artifacts found in {package_dir}")

    verify_platform_package_shape(artifacts, args.platform)
    verify_checksums(package_dir, artifacts)
    verify_sbom(package_dir, artifacts)
    verify_provenance(
        package_dir,
        args.profile,
        artifacts,
        args.require_clean or os.environ.get("GITHUB_ACTIONS") == "true",
    )

    print(
        f"release metadata verified: {len(artifacts)} artifact(s) in {package_dir}"
    )


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        sys.exit(1)
