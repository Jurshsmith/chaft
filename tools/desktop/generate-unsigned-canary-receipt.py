#!/usr/bin/env python3
"""Emit explicit unsigned-canary evidence after a native packaged-app smoke."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Mapping, Sequence

import release_targets
import unsigned_canary_policy as policy


def atomic_write_json(path: Path, value: Mapping[str, object]) -> None:
    path = Path(path)
    if path.name not in policy.RECEIPT_FILENAMES.values():
        policy.fail(
            "output must use the platform-qualified unsigned-canary receipt filename"
        )
    if path.exists() or path.is_symlink():
        policy.fail("receipt output already exists; use a fresh output path")
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp", text=True
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
    finally:
        if temporary.exists():
            temporary.unlink()


def generate_receipt(
    *,
    target: str | None = None,
    platform: str | None = None,
    package: Path,
    output: Path,
    version: str,
    tag: str,
    commit: str,
    repository: str,
    release_id: int,
    asset_id: int,
    workflow_run_id: int,
    workflow_run_attempt: int,
    runner_os: str,
    runner_arch: str,
    smoke_command: str,
    verified_at: str | None = None,
) -> dict[str, object]:
    try:
        target_contract = release_targets.resolve_target(
            target_name=target,
            platform_name=platform,
            architecture=runner_arch,
        )
    except release_targets.ReleaseTargetError as error:
        policy.fail(str(error))
    target = target_contract.name
    platform = target_contract.platform
    policy.validate_release_identity(
        version=version, tag=tag, commit=commit, repository=repository
    )
    if any(
        not isinstance(value, int) or isinstance(value, bool) or value <= 0
        for value in (
            release_id,
            asset_id,
            workflow_run_id,
            workflow_run_attempt,
        )
    ):
        policy.fail("release, asset, workflow run, and attempt IDs must be positive")
    if runner_os != policy.RUNNER_OS[platform]:
        policy.fail(f"runner OS must be {policy.RUNNER_OS[platform]!r} for {platform}")
    architecture = policy.normalize_architecture(runner_arch, "runner architecture")
    if architecture != target_contract.architecture:
        policy.fail(
            f"runner architecture must be {target_contract.architecture!r} "
            f"for {target}"
        )
    if not smoke_command.strip() or any(
        ord(character) < 32 or ord(character) == 127
        for character in smoke_command
    ):
        policy.fail("smoke command must be non-empty and contain no control characters")
    fingerprint = policy.fingerprint_file(package)
    policy.validate_package_filename(fingerprint.filename, target, version)
    output = Path(output)
    expected_output = policy.RECEIPT_FILENAMES[target]
    if output.name != expected_output:
        policy.fail(f"{target} receipt must be named {expected_output}")
    try:
        if output.resolve() == Path(package).resolve():
            policy.fail("receipt output must differ from the package")
    except OSError as error:
        policy.fail(f"cannot resolve receipt paths: {error}")

    timestamp = policy.normalize_timestamp(
        verified_at
        or datetime.now(timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z")
    )
    receipt: dict[str, object] = {
        "schemaVersion": policy.SCHEMA_VERSION,
        "target": target,
        "platform": platform,
        "verificationType": policy.VERIFICATION_TYPE,
        "status": policy.STATUS,
        "signingStatus": policy.SIGNING_STATUS,
        "signatureVerification": policy.SIGNATURE_VERIFICATION[platform],
        "signatureAndNotarization": dict(
            policy.SIGNATURE_AND_NOTARIZATION[platform]
        ),
        "productionEligible": False,
        "warning": policy.WARNING,
        "version": version,
        "tag": tag,
        "commit": commit,
        "repository": repository,
        "architecture": architecture,
        "verifiedAt": timestamp,
        "release": {"id": release_id},
        "asset": {
            "id": asset_id,
            "filename": fingerprint.filename,
            "sizeBytes": fingerprint.size_bytes,
            "sha256": fingerprint.sha256,
        },
        "runner": {
            "os": runner_os,
            "architecture": architecture,
            "workflowRunId": workflow_run_id,
            "workflowRunAttempt": workflow_run_attempt,
        },
        "smoke": {
            "status": policy.STATUS,
            "command": smoke_command,
        },
        "receiptGenerator": {
            "name": "Chaft unsigned-canary receipt generator",
            "version": "1",
        },
    }
    policy.validate_receipt_document(
        receipt,
        expected_target=target,
        expected_platform=platform,
        expected_package=fingerprint,
        expected_version=version,
        expected_tag=tag,
        expected_commit=commit,
        expected_repository=repository,
    )
    atomic_write_json(output, receipt)
    return receipt


def argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Record a passed packaged-app smoke as explicit unsigned-canary "
            "evidence. This command does not perform signing verification."
        )
    )
    parser.add_argument("--target", required=True, choices=policy.TARGETS)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--release-id", required=True, type=int)
    parser.add_argument("--asset-id", required=True, type=int)
    parser.add_argument("--workflow-run-id", required=True, type=int)
    parser.add_argument("--workflow-run-attempt", required=True, type=int)
    parser.add_argument("--runner-os", required=True)
    parser.add_argument("--runner-arch", required=True)
    parser.add_argument("--smoke-command", required=True)
    parser.add_argument(
        "--verified-at",
        help="RFC 3339 timestamp override for deterministic testing",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = argument_parser()
    args = parser.parse_args(argv)
    try:
        generate_receipt(
            target=args.target,
            package=args.package,
            output=args.output,
            version=args.version,
            tag=args.tag,
            commit=args.commit,
            repository=args.repository,
            release_id=args.release_id,
            asset_id=args.asset_id,
            workflow_run_id=args.workflow_run_id,
            workflow_run_attempt=args.workflow_run_attempt,
            runner_os=args.runner_os,
            runner_arch=args.runner_arch,
            smoke_command=args.smoke_command,
            verified_at=args.verified_at,
        )
    except policy.UnsignedCanaryPolicyError as error:
        parser.exit(2, f"unsigned-canary receipt generation failed: {error}\n")
    print(f"unsigned-canary receipt written: {args.output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise SystemExit(1)
