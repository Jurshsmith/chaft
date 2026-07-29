#!/usr/bin/env python3
import argparse
import hashlib
import importlib.util
import json
import os
import platform
import re
import subprocess
import sys
import uuid
from datetime import datetime, timezone
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
def load_release_version_module():
    path = Path(__file__).with_name("release-version.py")
    spec = importlib.util.spec_from_file_location("chaft_release_version", path)
    if spec is None or spec.loader is None:
        raise SystemExit(f"unable to load release version contract: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release_version = load_release_version_module()


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
        raise SystemExit(
            f"package metadata target must include architecture: {target!r}"
        ) from None


def command_output(args):
    try:
        completed = subprocess.run(
            args,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return completed.stdout.strip()


def repo_root():
    root = command_output(["git", "rev-parse", "--show-toplevel"])
    if root:
        return Path(root)
    return Path(__file__).resolve().parents[2]


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def package_files(package_dir):
    files = [
        path
        for path in package_dir.iterdir()
        if path.is_file() and package_format(path.name) != "unknown"
    ]
    return sorted(files, key=lambda path: path.name)


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
            raise SystemExit(
                f"detached signature {path.name} does not correspond to a package file"
            )
        if path.stat().st_size <= 0:
            raise SystemExit(f"detached signature is empty: {path.name}")
        signatures.append(path)
    return sorted(signatures, key=lambda path: path.name)


def artifact_rows(packages, signatures):
    package_rows = [
        {
            "name": path.name,
            "packageFormat": package_format(path.name),
            "path": path.as_posix(),
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
                "path": path.as_posix(),
                "sizeBytes": path.stat().st_size,
                "sha256": file_sha256(path),
            }
        )
    return sorted(package_rows + signature_rows, key=lambda row: row["name"])


def verify_platform_packages(packages, target):
    platform_name = target.platform
    unexpected = [
        path.name for path in packages if package_platform(path.name) != platform_name
    ]
    if unexpected:
        raise SystemExit(
            f"{target.name} package directory contains unexpected package type(s): "
            + ", ".join(sorted(unexpected))
        )


def verify_distribution_packages(packages, target, distribution_version, prerelease):
    del prerelease
    expected = target.package_name(distribution_version)
    actual = [path.name for path in packages]
    if actual != [expected]:
        raise SystemExit(
            f"{target.name} release package must be exactly {expected}; "
            f"found {', '.join(actual)}"
        )


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def qt_release_record(root, target):
    if os.environ.get("CHAFT_QT_POLICY") != "release":
        raise SystemExit(
            "release metadata requires CHAFT_QT_POLICY=release; "
            "developer/Homebrew Qt cannot enter release metadata"
        )
    activated_target = os.environ.get("CHAFT_QT_SDK_TARGET")
    if activated_target != target.name:
        raise SystemExit(
            "release metadata requires the verified pinned Qt SDK target "
            f"{target.name}; activation reports {activated_target or 'none'}"
        )
    provenance_value = os.environ.get("CHAFT_QT_SDK_PROVENANCE")
    provenance_dir = os.environ.get("CHAFT_QT_SDK_PROVENANCE_DIR")
    if provenance_value and provenance_dir:
        raise SystemExit(
            "set only one of CHAFT_QT_SDK_PROVENANCE and "
            "CHAFT_QT_SDK_PROVENANCE_DIR"
        )
    if provenance_value:
        provenance_path = Path(provenance_value)
    elif provenance_dir:
        provenance_path = (
            Path(provenance_dir) / f"chaft-qt-sdk-{target.name}.json"
        )
    else:
        qt_prefix = os.environ.get("QTDIR")
        if not qt_prefix:
            raise SystemExit(
                "QTDIR or CHAFT_QT_SDK_PROVENANCE is required to bind "
                "desktop metadata to the verified Qt SDK"
            )
        provenance_path = Path(qt_prefix) / qt_sdk.PROVENANCE_NAME

    manifest_path = root / "tools" / "qt" / "qt-6.8.4.json"
    manifest = qt_sdk.load_manifest(manifest_path, recipe_root=root)
    sdk_provenance = qt_sdk.load_and_validate_provenance(
        provenance_path,
        manifest,
        target.name,
        recipe_root=root,
    )
    source_contract = qt_source.release_contract(
        manifest_path,
        root / "packaging" / "qt",
        root / "tools" / "qt" / "source_bundle.py",
        recipe_root=root,
    )
    if sdk_provenance["manifestSha256"] != source_contract["sdkManifestSha256"]:
        raise SystemExit(
            "Qt SDK provenance and corresponding-source manifest differ"
        )
    if sdk_provenance["contractSha256"] != source_contract["sdkContractSha256"]:
        raise SystemExit(
            "Qt SDK provenance and corresponding-source recipe differ"
        )

    bundle_sha256 = os.environ.get("CHAFT_QT_SOURCE_BUNDLE_SHA256")
    if bundle_sha256 is not None and re.fullmatch(
        r"[0-9a-f]{64}", bundle_sha256
    ) is None:
        raise SystemExit(
            "CHAFT_QT_SOURCE_BUNDLE_SHA256 must be a lowercase SHA-256 digest"
        )
    return {
        "schemaVersion": 1,
        "sdk": {
            "identity": sdk_provenance["identity"],
            "provenanceSha256": qt_sdk.sha256_bytes(
                qt_sdk.canonical_json(sdk_provenance)
            ),
            "provenance": sdk_provenance,
        },
        "correspondingSource": {
            **source_contract,
            "bundleSha256": bundle_sha256,
        },
    }


def release_versions(root, declared_distribution_version):
    source_version, _ = release_version.validated_source_version(root)
    distribution_version = declared_distribution_version or source_version
    distribution_version, prerelease = (
        release_version.validated_distribution_version(
            distribution_version, source_version
        )
    )
    return source_version, distribution_version, prerelease


def cargo_metadata(root):
    raw = command_output(["cargo", "metadata", "--locked", "--format-version", "1"])
    if raw is None:
        raw = command_output(["cargo", "metadata", "--format-version", "1"])
    if raw is None:
        return None
    return json.loads(raw)


def tool_component(name, version):
    component = {"type": "application", "name": name}
    if version:
        component["version"] = version.splitlines()[0]
    return component


def tool_versions():
    return {
        "rustc": command_output(["rustc", "--version"]),
        "cargo": command_output(["cargo", "--version"]),
        "cmake": command_output(["cmake", "--version"]),
        "cpack": command_output(["cpack", "--version"]),
        "qmake6": command_output(["qmake6", "-v"]),
        "qtCmake": command_output(["qt-cmake", "--version"]),
    }


def cargo_components(metadata):
    if metadata is None:
        return []

    components = []
    for package in sorted(
        metadata.get("packages", []),
        key=lambda item: (item.get("name", ""), item.get("version", ""), item.get("source") or ""),
    ):
        name = package.get("name", "")
        version = package.get("version", "")
        component = {
            "type": "library",
            "bom-ref": f"pkg:cargo/{name}@{version}",
            "name": name,
            "version": version,
            "purl": f"pkg:cargo/{name}@{version}",
        }
        if package.get("license"):
            component["licenses"] = [{"expression": package["license"]}]
        if package.get("source"):
            component["externalReferences"] = [
                {"type": "distribution", "url": package["source"]}
            ]
        components.append(component)
    return components


def git_dirty(root):
    status = command_output(["git", "status", "--porcelain=v1"])
    return bool(status)


def github_context():
    keys = [
        "GITHUB_ACTION",
        "GITHUB_ACTOR",
        "GITHUB_EVENT_NAME",
        "GITHUB_JOB",
        "GITHUB_REF",
        "GITHUB_REPOSITORY",
        "GITHUB_RUN_ATTEMPT",
        "GITHUB_RUN_ID",
        "GITHUB_RUN_NUMBER",
        "GITHUB_SHA",
        "GITHUB_WORKFLOW",
        "CHAFT_RELEASE_COMMIT",
    ]
    return {key: os.environ[key] for key in keys if key in os.environ}


def source_context(root):
    return {
        "repository": command_output(["git", "config", "--get", "remote.origin.url"]),
        "commit": command_output(["git", "rev-parse", "HEAD"]),
        "ref": command_output(["git", "rev-parse", "--abbrev-ref", "HEAD"]),
        "dirty": git_dirty(root),
    }


def material_rows(root):
    rows = []
    for relative in (
        "Cargo.lock",
        "Cargo.toml",
        "apps/desktop-qt/CMakeLists.txt",
        "tools/desktop/macos-adhoc-verify.cmake",
    ):
        path = root / relative
        if path.is_file():
            rows.append(
                {
                    "name": relative,
                    "sha256": file_sha256(path),
                    "sizeBytes": path.stat().st_size,
                }
            )
    return rows


def write_checksums(package_dir, artifacts, names):
    checksum_path = package_dir / names["checksums"]
    checksum_path.write_text(
        "".join(f"{artifact['sha256']}  {artifact['name']}\n" for artifact in artifacts),
        encoding="utf-8",
    )
    return checksum_path


def write_sbom(
    root,
    package_dir,
    source_version,
    distribution_version,
    artifacts,
    tools,
    target,
    names,
):
    metadata = cargo_metadata(root)
    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid4()}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "component": {
                "type": "application",
                "name": "Chaft Desktop",
                "version": distribution_version,
                "bom-ref": f"pkg:generic/chaft-desktop@{distribution_version}",
            },
            "tools": {
                "components": [
                    tool_component(name, version)
                    for name, version in tools.items()
                    if version
                ]
            },
            "properties": [
                {"name": "chaft:sourceCommit", "value": source_context(root).get("commit") or ""},
                {"name": "chaft:sourceVersion", "value": source_version},
                {
                    "name": "chaft:distributionVersion",
                    "value": distribution_version,
                },
                {"name": "chaft:platform", "value": platform.platform()},
                {"name": "chaft:packageTarget", "value": target.name},
                {"name": "chaft:packagePlatform", "value": target.platform},
                {
                    "name": "chaft:packageArchitecture",
                    "value": target.architecture,
                },
            ],
        },
        "components": cargo_components(metadata),
        "properties": [
            {"name": f"chaft:artifact:{artifact['name']}:sha256", "value": artifact["sha256"]}
            for artifact in artifacts
        ]
        + [
            {
                "name": f"chaft:artifact:{artifact['name']}:packageFormat",
                "value": artifact["packageFormat"],
            }
            for artifact in artifacts
        ]
        + [
            {
                "name": f"chaft:artifact:{artifact['name']}:signedArtifact",
                "value": artifact["signedArtifact"],
            }
            for artifact in artifacts
            if artifact.get("signedArtifact")
        ]
        + [
            {
                "name": f"chaft:artifact:{artifact['name']}:signatureFormat",
                "value": artifact["signatureFormat"],
            }
            for artifact in artifacts
            if artifact.get("signatureFormat")
        ],
    }
    path = package_dir / names["sbom"]
    write_json(path, sbom)
    return path


def write_provenance(
    root,
    package_dir,
    profile,
    source_version,
    distribution_version,
    artifacts,
    tools,
    target,
    names,
):
    qt = qt_release_record(root, target)
    provenance = {
        "schemaVersion": "chaft.desktop.provenance.v2",
        "createdAt": datetime.now(timezone.utc).isoformat(),
        "name": "Chaft Desktop release package",
        "version": distribution_version,
        "sourceVersion": source_version,
        "distributionVersion": distribution_version,
        "profile": profile,
        "packageTarget": target.name,
        "packagePlatform": target.platform,
        "packageArchitecture": target.architecture,
        "source": source_context(root),
        "github": github_context(),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "platform": platform.platform(),
            "python": platform.python_version(),
        },
        "tools": tools,
        "qt": qt,
        "materials": material_rows(root),
        "artifacts": artifacts,
    }
    path = package_dir / names["provenance"]
    write_json(path, provenance)
    return path


def main():
    parser = argparse.ArgumentParser(
        description="Generate package checksums, SBOM, and provenance metadata."
    )
    parser.add_argument("profile", nargs="?", default="release", choices=("debug", "release"))
    parser.add_argument("--package-dir", type=Path)
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
        "--distribution-version",
        default=os.environ.get("CHAFT_DISTRIBUTION_VERSION"),
        help=(
            "Exact SemVer package version. Defaults to "
            "CHAFT_DISTRIBUTION_VERSION, then the stable source version."
        ),
    )
    args = parser.parse_args()

    root = repo_root()
    package_dir = args.package_dir or root / "build" / f"desktop-{args.profile}" / "package"
    if not package_dir.is_dir():
        raise SystemExit(f"package directory not found: {package_dir}")

    packages = package_files(package_dir)
    if not packages:
        raise SystemExit(f"no package artifacts found in {package_dir}")
    try:
        target = release_targets.resolve_target(
            target_name=args.target,
            platform_name=args.platform,
            architecture=args.architecture,
        )
    except release_targets.ReleaseTargetError as error:
        raise SystemExit(str(error)) from None
    names = metadata_names(target)
    source_version, distribution_version, prerelease = release_versions(
        root, args.distribution_version
    )
    verify_platform_packages(packages, target)
    verify_distribution_packages(
        packages, target, distribution_version, prerelease
    )
    signatures = signature_files(package_dir, packages)

    artifacts = artifact_rows(packages, signatures)
    tools = tool_versions()
    generated = [
        write_checksums(package_dir, artifacts, names),
        write_sbom(
            root,
            package_dir,
            source_version,
            distribution_version,
            artifacts,
            tools,
            target,
            names,
        ),
        write_provenance(
            root,
            package_dir,
            args.profile,
            source_version,
            distribution_version,
            artifacts,
            tools,
            target,
            names,
        ),
    ]

    print(f"release metadata generated in {package_dir}")
    for path in generated:
        print(path)


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        sys.exit(1)
