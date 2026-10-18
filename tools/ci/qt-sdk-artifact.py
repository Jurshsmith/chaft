#!/usr/bin/env python3
"""Transfer verified Qt SDKs and find reusable artifacts from trusted main CI."""

from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import json
import os
import posixpath
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import tempfile
from urllib.parse import quote


spec = importlib.util.spec_from_file_location(
    "artifact_qt_compatibility", Path(__file__).with_name("qt-toolchain-compatibility.py")
)
compatibility = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compatibility)
qt = compatibility.qt_sdk
ARCHIVE = "qt-sdk.tar.gz"
METADATA = "qt-sdk-artifact.json"
SHA256 = re.compile(r"[0-9a-f]{64}")
SDK_JOB_NAMES = {
    "linux-x86_64": "Linux / Provision verified Qt SDK",
    "macos-x86_64": "macOS Intel / Provision verified Qt SDK",
    "macos-arm64": "macOS Apple Silicon / Provision verified Qt SDK",
    "windows-x86_64": "Windows / Provision verified Qt SDK",
}


def read_json(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected a regular JSON file: {path.name}")
    return json.loads(path.read_text(encoding="utf-8"))


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def artifact_name(manifest, target, contract):
    base = qt.sdk_identity(manifest, target)
    compatible = compatibility.compatibility_fingerprint(contract, manifest, target)
    return f"chaft-qt-sdk-v1-{base}-{compatible}"


def traversal_after_name(path):
    named = False
    for part in PurePosixPath(path).parts:
        if part == ".." and named:
            return True
        named = named or part != ".."
    return False


def provenance_outputs(provenance, manifest, target):
    qt.validate_provenance_object(provenance, manifest, target)
    return {
        "identity": provenance["identity"],
        "toolchain_fingerprint": provenance["toolchainFingerprint"],
        "compatibility_fingerprint": compatibility.compatibility_fingerprint(
            provenance["toolchainContract"], manifest, target
        ),
        "artifact_name": artifact_name(manifest, target, provenance["toolchainContract"]),
    }


def create(prefix, output, manifest, target):
    if prefix.is_symlink() or not prefix.is_dir():
        raise ValueError("SDK prefix must be a real directory")
    provenance = read_json(prefix / qt.PROVENANCE_NAME)
    outputs = provenance_outputs(provenance, manifest, target)
    if prefix.resolve() == output.resolve() or prefix.resolve() in output.resolve().parents:
        raise ValueError("archive output must be outside the SDK prefix")
    output.mkdir(parents=True, exist_ok=True)
    def normalize_link(info):
        if info.issym():
            parent = (prefix / PurePosixPath(info.name).relative_to("sdk").parent).resolve()
            try:
                resolved = (parent / info.linkname).resolve()
                resolved.relative_to(prefix.resolve())
            except (OSError, RuntimeError, ValueError) as error:
                raise ValueError("SDK symlink escapes the installed prefix or loops") from error
            if os.path.isabs(info.linkname) or traversal_after_name(info.linkname):
                info.linkname = os.path.relpath(resolved, parent).replace(os.sep, "/")
        return info
    with tarfile.open(output / ARCHIVE, "w:gz", compresslevel=1) as archive:
        archive.add(prefix, arcname="sdk", recursive=True, filter=normalize_link)
    metadata = {
        "schemaVersion": 1,
        "target": target,
        "identity": outputs["identity"],
        "toolchainFingerprint": outputs["toolchain_fingerprint"],
        "compatibilityFingerprint": outputs["compatibility_fingerprint"],
        "toolchainContract": provenance["toolchainContract"],
        "archiveSha256": digest(output / ARCHIVE),
    }
    (output / METADATA).write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    return outputs


def validate_members(members):
    # Validate the complete layout before creating any files or links.
    paths = {}
    links = set()
    for member in members:
        path = PurePosixPath(member.name)
        if len(member.name) > 4096 or any(character in member.name for character in ("\\", ":", "\0")) or path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != "sdk":
            raise ValueError("SDK archive contains an unsafe path")
        if path.as_posix() in paths:
            raise ValueError("SDK archive contains duplicate paths")
        paths[path.as_posix()] = member
        if not (member.isfile() or member.isdir() or member.issym() or member.islnk()):
            raise ValueError("SDK archive contains a special file")
        if member.issym() or member.islnk():
            links.add(path)
    for name, member in paths.items():
        path = PurePosixPath(name)
        if any(parent in links for parent in path.parents):
            raise ValueError("SDK archive contains a member below a link")
        if member.issym() or member.islnk():
            target = member.linkname
            if not target or len(target) > 4096 or any(character in target for character in ("\\", ":", "\0")) or posixpath.isabs(target):
                raise ValueError("SDK archive contains an unsafe link")
            if traversal_after_name(target):
                raise ValueError("SDK link traversal must precede named components")
            resolved = posixpath.normpath(posixpath.join(str(path.parent), target) if member.issym() else target)
            if resolved != "sdk" and not resolved.startswith("sdk/"):
                raise ValueError("SDK archive link escapes the prefix")
            if member.islnk() and (resolved not in paths or not paths[resolved].isfile()):
                raise ValueError("SDK hard link must reference an archived regular file")


def restore(source, prefix, manifest, target, current_contract=None, expected=None):
    if source.is_symlink() or not source.is_dir():
        raise ValueError("artifact source must be a real directory")
    if {entry.name for entry in source.iterdir()} != {ARCHIVE, METADATA}:
        raise ValueError("SDK artifact must contain exactly its archive and metadata")
    metadata = read_json(source / METADATA)
    keys = {"schemaVersion", "target", "identity", "toolchainFingerprint", "compatibilityFingerprint", "toolchainContract", "archiveSha256"}
    if not isinstance(metadata, dict) or set(metadata) != keys:
        raise ValueError("SDK artifact metadata has missing or unexpected fields")
    if type(metadata["schemaVersion"]) is not int or metadata["schemaVersion"] != 1 or metadata["target"] != target:
        raise ValueError("SDK artifact schema or target mismatch")
    contract = metadata["toolchainContract"]
    fingerprint = qt.toolchain_fingerprint(contract, manifest, target)
    compatible = compatibility.compatibility_fingerprint(contract, manifest, target)
    identity = qt.sdk_identity(manifest, target, fingerprint)
    if (metadata["identity"], metadata["toolchainFingerprint"], metadata["compatibilityFingerprint"]) != (identity, fingerprint, compatible):
        raise ValueError("SDK artifact identity or toolchain binding mismatch")
    if current_contract is not None and compatibility.compatibility_fingerprint(current_contract, manifest, target) != compatible:
        raise ValueError("SDK artifact is incompatible with the consumer toolchain")
    actual = {"identity": identity, "toolchain_fingerprint": fingerprint, "compatibility_fingerprint": compatible}
    for key, value in (expected or {}).items():
        if value is not None and actual[key] != value:
            raise ValueError(f"SDK artifact does not match the provisioned {key}")
    archive_path = source / ARCHIVE
    if archive_path.is_symlink() or not archive_path.is_file():
        raise ValueError("SDK archive must be a regular file")
    if not isinstance(metadata["archiveSha256"], str) or not SHA256.fullmatch(metadata["archiveSha256"]) or digest(archive_path) != metadata["archiveSha256"]:
        raise ValueError("SDK archive SHA-256 mismatch")
    if prefix.exists() or prefix.is_symlink():
        raise ValueError("SDK extraction destination must not already exist")
    prefix.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="qt-sdk-extract-", dir=prefix.parent) as directory:
        staging = Path(directory)
        with tarfile.open(archive_path, "r:gz") as archive:
            members = archive.getmembers()
            if not members or len(members) > 250_000 or sum(member.size for member in members) > 20 * 1024**3:
                raise ValueError("SDK archive is empty or exceeds extraction limits")
            validate_members(members)
            archive.extractall(staging, members=members, filter="data")
        restored = read_json(staging / "sdk" / qt.PROVENANCE_NAME)
        outputs = provenance_outputs(restored, manifest, target)
        if any(outputs[key] != value for key, value in actual.items()):
            raise ValueError("SDK archive provenance does not match artifact metadata")
        (staging / "sdk").rename(prefix)
    return outputs


def gh_api(path):
    result = subprocess.run(["gh", "api", "-H", "X-GitHub-Api-Version: 2026-03-10", path], check=True, capture_output=True, text=True)
    return json.loads(result.stdout)


def trusted_artifact(artifact, run, repository, name):
    if not all(isinstance(value, dict) for value in (artifact, run, repository)):
        return False
    producer = artifact.get("workflow_run", {})
    if not all(isinstance(value, dict) for value in (producer, run.get("head_repository"), run.get("repository"))):
        return False
    if type(repository.get("id")) is not int or repository["id"] <= 0:
        return False
    if not isinstance(repository.get("default_branch"), str) or not repository["default_branch"]:
        return False
    if not isinstance(run.get("head_sha"), str) or not re.fullmatch(r"[0-9a-f]{40}", run["head_sha"]):
        return False
    return (
        artifact.get("name") == name
        and artifact.get("expired") is False
        and type(artifact.get("id")) is int and artifact["id"] > 0
        and isinstance(artifact.get("digest"), str)
        and re.fullmatch(r"sha256:[0-9a-f]{64}", artifact["digest"]) is not None
        and producer.get("repository_id") == repository["id"]
        and producer.get("head_repository_id") == repository["id"]
        and producer.get("id") == run.get("id")
        and producer.get("head_sha") == run.get("head_sha")
        and run.get("head_repository", {}).get("id") == repository["id"]
        and run.get("repository", {}).get("id") == repository["id"]
        and run.get("head_branch") == repository["default_branch"]
        and run.get("event") in {"push", "schedule"}
        and run.get("name") == "CI"
        and run.get("path") == ".github/workflows/ci.yml"
        and run.get("status") == "completed"
        and run.get("conclusion") == "success"
    )


def lookup(repository_name, manifest, target, contract, api=gh_api, current_run_id=None, attempt="1", now=None):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9-]*/[A-Za-z0-9_.-]+", repository_name) or repository_name.split("/")[-1] in {".", ".."}:
        raise ValueError("invalid repository name")
    repository = api(f"repos/{repository_name}")
    name = artifact_name(manifest, target, contract)
    if not re.fullmatch(r"[1-9][0-9]*", str(attempt)):
        raise ValueError("invalid workflow run attempt")
    result = {"artifact_name": name, "upload_name": name, "found": "false", "refresh": "false"}
    for page in range(1, 11):
        response = api(f"repos/{repository_name}/actions/artifacts?name={quote(name)}&per_page=100&page={page}")
        artifacts = response["artifacts"]
        if not isinstance(artifacts, list):
            raise ValueError("invalid artifact inventory")
        for artifact in artifacts:
            if not isinstance(artifact, dict):
                raise ValueError("invalid artifact entry")
            producer = artifact.get("workflow_run", {})
            if not isinstance(producer, dict):
                continue
            run_id = producer.get("id")
            if artifact.get("name") != name or artifact.get("expired") is not False or type(run_id) is not int or run_id <= 0:
                continue
            if str(run_id) == str(current_run_id):
                result["upload_name"] = f"{name}-retry-{attempt}"
            run = api(f"repos/{repository_name}/actions/runs/{run_id}")
            if trusted_artifact(artifact, run, repository, name) and successful_sdk_job(repository_name, run_id, target, api):
                try:
                    expires = datetime.fromisoformat(artifact["expires_at"].replace("Z", "+00:00"))
                    refresh = expires - (now or datetime.now(timezone.utc)) <= timedelta(days=2)
                except (KeyError, TypeError, ValueError, AttributeError):
                    refresh = True
                return {**result, "found": "true", "refresh": "true" if refresh else "false", "artifact_id": str(artifact["id"]), "run_id": str(run_id)}
        if len(artifacts) < 100:
            return result
    # A bounded lookup miss still permits a verified source build.
    return result


def successful_sdk_job(repository, run_id, target, api):
    for page in range(1, 11):
        response = api(f"repos/{repository}/actions/runs/{run_id}/jobs?filter=latest&per_page=100&page={page}")
        jobs = response["jobs"]
        if not isinstance(jobs, list):
            raise ValueError("invalid producer job inventory")
        if any(isinstance(job, dict) and job.get("name") == SDK_JOB_NAMES[target]
               and job.get("status") == "completed" and job.get("conclusion") == "success" for job in jobs):
            return True
        if len(jobs) < 100:
            return False
    return False


def emit(values, path):
    text = "".join(f"{key}={value}\n" for key, value in values.items())
    if any("\n" in str(value) or "\r" in str(value) for value in values.values()):
        raise ValueError("invalid multiline SDK output")
    if path:
        with Path(path).open("a", encoding="utf-8") as output:
            output.write(text)
    print(text, end="")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("lookup", "create", "restore", "describe"))
    parser.add_argument("--target", choices=qt.SUPPORTED_TARGETS, required=True)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--artifact-dir", type=Path)
    parser.add_argument("--repository")
    parser.add_argument("--toolchain-contract", type=Path)
    parser.add_argument("--expected-identity")
    parser.add_argument("--expected-fingerprint")
    parser.add_argument("--expected-compatibility")
    parser.add_argument("--github-output", default=os.environ.get("GITHUB_OUTPUT", ""))
    args = parser.parse_args()
    manifest = qt.load_manifest()
    contract = qt.load_toolchain_contract(args.toolchain_contract, manifest, args.target) if args.toolchain_contract else None
    if args.command == "lookup":
        if not args.repository or contract is None:
            parser.error("lookup requires --repository and --toolchain-contract")
        values = lookup(args.repository, manifest, args.target, contract,
                        current_run_id=os.environ.get("GITHUB_RUN_ID"),
                        attempt=os.environ.get("GITHUB_RUN_ATTEMPT", "1"))
    elif args.command == "create":
        if args.prefix is None or args.artifact_dir is None:
            parser.error("create requires --prefix and --artifact-dir")
        values = create(args.prefix, args.artifact_dir, manifest, args.target)
    elif args.command == "restore":
        if args.prefix is None or args.artifact_dir is None:
            parser.error("restore requires --prefix and --artifact-dir")
        values = restore(args.artifact_dir, args.prefix, manifest, args.target, contract, {
            "identity": args.expected_identity,
            "toolchain_fingerprint": args.expected_fingerprint,
            "compatibility_fingerprint": args.expected_compatibility,
        })
    else:
        if args.prefix is None:
            parser.error("describe requires --prefix")
        values = provenance_outputs(read_json(args.prefix / qt.PROVENANCE_NAME), manifest, args.target)
        if contract is not None and compatibility.compatibility_fingerprint(contract, manifest, args.target) != values["compatibility_fingerprint"]:
            raise ValueError("SDK is incompatible with the current toolchain")
        if args.expected_fingerprint is not None and values["toolchain_fingerprint"] != args.expected_fingerprint:
            raise ValueError("SDK does not match the expected producer toolchain")
    emit(values, args.github_output)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError, tarfile.TarError, qt.QtSdkError) as error:
        print(f"Qt SDK artifact operation failed: {error}", file=sys.stderr)
        sys.exit(1)
