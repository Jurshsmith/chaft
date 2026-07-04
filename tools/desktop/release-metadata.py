#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path

PACKAGE_SUFFIXES = (".dmg", ".zip", ".tgz", ".tar.gz")


def package_format(name):
    if name.endswith((".tgz", ".tar.gz")):
        return "linux-tgz"
    if name.endswith(".dmg"):
        return "macos-dmg"
    if name.endswith(".zip"):
        return "windows-zip"
    return "unknown"


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
        if path.is_file() and path.name.endswith(PACKAGE_SUFFIXES)
    ]
    return sorted(files, key=lambda path: path.name)


def artifact_rows(files):
    return [
        {
            "name": path.name,
            "packageFormat": package_format(path.name),
            "path": path.as_posix(),
            "sizeBytes": path.stat().st_size,
            "sha256": file_sha256(path),
        }
        for path in files
    ]


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def project_version(root):
    cmake = root / "apps" / "desktop-qt" / "CMakeLists.txt"
    text = cmake.read_text(encoding="utf-8")
    match = re.search(r"project\(ChaftDesktop\s+VERSION\s+([^\s\)]+)", text)
    return match.group(1) if match else "0.0.0"


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
    for relative in ("Cargo.lock", "Cargo.toml", "apps/desktop-qt/CMakeLists.txt"):
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


def write_checksums(package_dir, artifacts):
    checksum_path = package_dir / "SHA256SUMS"
    checksum_path.write_text(
        "".join(f"{artifact['sha256']}  {artifact['name']}\n" for artifact in artifacts),
        encoding="utf-8",
    )
    return checksum_path


def write_sbom(root, package_dir, version, artifacts, tools):
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
                "version": version,
                "bom-ref": f"pkg:generic/chaft-desktop@{version}",
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
                {"name": "chaft:platform", "value": platform.platform()},
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
        ],
    }
    path = package_dir / "chaft-desktop-sbom.cdx.json"
    write_json(path, sbom)
    return path


def write_provenance(root, package_dir, profile, version, artifacts, tools):
    provenance = {
        "schemaVersion": "chaft.desktop.provenance.v1",
        "createdAt": datetime.now(timezone.utc).isoformat(),
        "name": "Chaft Desktop release package",
        "version": version,
        "profile": profile,
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
        "materials": material_rows(root),
        "artifacts": artifacts,
    }
    path = package_dir / "chaft-desktop-provenance.json"
    write_json(path, provenance)
    return path


def main():
    parser = argparse.ArgumentParser(
        description="Generate package checksums, SBOM, and provenance metadata."
    )
    parser.add_argument("profile", nargs="?", default="release", choices=("debug", "release"))
    parser.add_argument("--package-dir", type=Path)
    args = parser.parse_args()

    root = repo_root()
    package_dir = args.package_dir or root / "build" / f"desktop-{args.profile}" / "package"
    if not package_dir.is_dir():
        raise SystemExit(f"package directory not found: {package_dir}")

    files = package_files(package_dir)
    if not files:
        raise SystemExit(f"no package artifacts found in {package_dir}")

    artifacts = artifact_rows(files)
    tools = tool_versions()
    version = project_version(root)

    generated = [
        write_checksums(package_dir, artifacts),
        write_sbom(root, package_dir, version, artifacts, tools),
        write_provenance(root, package_dir, args.profile, version, artifacts, tools),
    ]

    print(f"release metadata generated in {package_dir}")
    for path in generated:
        print(path)


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        sys.exit(1)
