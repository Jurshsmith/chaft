#!/usr/bin/env python3
"""Build and verify Chaft's deterministic, open-source Qt SDK."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform as host_platform
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from datetime import datetime, timezone
from typing import Any
from urllib.error import URLError
from urllib.parse import urlparse
from urllib.request import Request, urlopen


SCRIPT_DIR = Path(__file__).resolve().parent
MANIFEST_PATH = SCRIPT_DIR / "qt-6.8.4.json"
PROBE_DIR = SCRIPT_DIR / "probe"
PROVENANCE_NAME = "chaft-qt-sdk-provenance.json"
TOOLCHAIN_CONTRACT_SCHEMA = 2
SUPPORTED_PLATFORMS = ("linux", "macos", "windows")
SUPPORTED_TARGETS = (
    "linux-x86_64",
    "macos-arm64",
    "macos-x86_64",
    "windows-x86_64",
)
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
RECIPE_FILES = (
    ("tools/qt/build_qt.py", SCRIPT_DIR / "build_qt.py"),
    (
        "tools/qt/install-linux-dependencies.sh",
        SCRIPT_DIR / "install-linux-dependencies.sh",
    ),
    ("tools/qt/probe/CMakeLists.txt", PROBE_DIR / "CMakeLists.txt"),
    ("tools/qt/probe/main.cpp", PROBE_DIR / "main.cpp"),
    ("tools/qt/probe/tst_QtSdk.qml", PROBE_DIR / "tst_QtSdk.qml"),
)
QT_SOURCE_PREFIX = (
    "https://download.qt.io/official_releases/qt/6.8/6.8.4/submodules/"
)
QT_PATCH_PREFIX = "https://download.qt.io/official_releases/qt/6.8/CVE-"


class QtSdkError(RuntimeError):
    """A user-actionable deterministic SDK failure."""


def fail(message: str) -> None:
    raise QtSdkError(message)


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def json_exact_equal(actual: Any, expected: Any) -> bool:
    """Compare JSON values without Python's bool/int or int/float coercions."""
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return set(actual) == set(expected) and all(
            json_exact_equal(actual[key], value)
            for key, value in expected.items()
        )
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            json_exact_equal(left, right)
            for left, right in zip(actual, expected, strict=True)
        )
    return actual == expected


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def trusted_source_root(source_root: Path) -> Path:
    """Resolve an explicit source root without accepting a symlink root."""
    root = Path(source_root)
    if root.is_symlink():
        fail(f"authoritative source root must not be a symlink: {root}")
    try:
        metadata = root.lstat()
    except FileNotFoundError:
        fail(f"authoritative source root not found: {root}")
    if not stat.S_ISDIR(metadata.st_mode):
        fail(f"authoritative source root must be a directory: {root}")
    return root.resolve(strict=True)


def trusted_source_file(
    source_root: Path,
    path: Path | str,
    description: str = "authoritative source input",
) -> Path:
    """Return a non-symlink regular file contained by an explicit source root."""
    supplied_root = Path(os.path.abspath(source_root))
    root = trusted_source_root(source_root)
    candidate = Path(path)
    if not candidate.is_absolute():
        candidate = root / candidate
    candidate = Path(os.path.abspath(candidate))
    try:
        supplied_relative = candidate.relative_to(supplied_root)
    except ValueError:
        pass
    else:
        candidate = root / supplied_relative
    try:
        relative = candidate.relative_to(root)
    except ValueError:
        fail(f"{description} escapes the authoritative source root: {candidate}")
    cursor = root
    for component in relative.parts[:-1]:
        cursor /= component
        try:
            parent_metadata = cursor.lstat()
        except FileNotFoundError:
            fail(f"{description} not found: {candidate}")
        if stat.S_ISLNK(parent_metadata.st_mode):
            fail(
                f"{description} has a symlink path component: {cursor}"
            )
        if not stat.S_ISDIR(parent_metadata.st_mode):
            fail(
                f"{description} parent must be a directory: {cursor}"
            )
    try:
        metadata = candidate.lstat()
    except FileNotFoundError:
        fail(f"{description} not found: {candidate}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        fail(f"{description} must be a non-symlink regular file: {candidate}")
    resolved = candidate.resolve(strict=True)
    try:
        resolved.relative_to(root)
    except ValueError:
        fail(f"{description} escapes the authoritative source root: {candidate}")
    return resolved


def load_manifest(
    path: Path = MANIFEST_PATH, *, recipe_root: Path | None = None
) -> dict[str, Any]:
    if recipe_root is not None:
        path = trusted_source_file(
            recipe_root, path, "Qt SDK manifest"
        )
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"Qt SDK manifest not found: {path}")
    except json.JSONDecodeError as error:
        fail(f"Qt SDK manifest is invalid JSON: {error}")
    validate_manifest(manifest, recipe_root=recipe_root)
    return manifest


def require_exact_keys(
    value: dict[str, Any], expected: set[str], description: str
) -> None:
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        fail(
            f"{description} keys differ from the contract; "
            f"missing={missing}, extra={extra}"
        )


def normalize_architecture(value: str) -> str:
    normalized = value.strip().lower()
    return {
        "aarch64": "arm64",
        "amd64": "x86_64",
        "arm64": "arm64",
        "x64": "x86_64",
        "x86-64": "x86_64",
        "x86_64": "x86_64",
    }.get(normalized, normalized)


def target_specification(
    manifest: dict[str, Any], target_name: str
) -> dict[str, Any]:
    specification = manifest["targets"].get(target_name)
    if specification is None:
        fail(f"unsupported Qt SDK target: {target_name}")
    return specification


def target_platform(manifest: dict[str, Any], target_name: str) -> str:
    return target_specification(manifest, target_name)["platform"]


def logical_platform(manifest: dict[str, Any], selector: str) -> str:
    if selector in SUPPORTED_PLATFORMS:
        return selector
    return target_platform(manifest, selector)


def resolve_target(
    manifest: dict[str, Any],
    *,
    target_name: str | None = None,
    platform_name: str | None = None,
) -> str:
    if target_name is not None:
        if target_name not in manifest["targets"]:
            fail(f"unsupported Qt SDK target: {target_name}")
        if (
            platform_name is not None
            and target_platform(manifest, target_name) != platform_name
        ):
            fail(
                f"Qt SDK target {target_name} does not belong to platform "
                f"{platform_name}"
            )
        return target_name
    if platform_name is None:
        fail("an exact Qt SDK --target is required")
    matches = [
        name
        for name, specification in manifest["targets"].items()
        if specification["platform"] == platform_name
    ]
    if len(matches) != 1:
        fail(
            f"Qt SDK platform {platform_name} is ambiguous; choose one of: "
            + ", ".join(matches)
        )
    return matches[0]


def validate_manifest(
    manifest: dict[str, Any], *, recipe_root: Path | None = None
) -> None:
    if not isinstance(manifest, dict):
        fail("Qt SDK manifest root must be an object")
    require_exact_keys(
        manifest,
        {
            "schemaVersion",
            "sdkRevision",
            "qtVersion",
            "identityHash",
            "sdkIdentities",
            "build",
            "targets",
            "modules",
            "patches",
        },
        "manifest",
    )
    if type(manifest["schemaVersion"]) is not int or manifest["schemaVersion"] != 2:
        fail("Qt SDK manifest schemaVersion must be 2")
    if type(manifest["sdkRevision"]) is not int or manifest["sdkRevision"] < 1:
        fail("Qt SDK manifest sdkRevision must be a positive integer")
    if manifest["qtVersion"] != "6.8.4":
        fail("Qt SDK manifest must describe exact Qt 6.8.4")
    if manifest["identityHash"] != "sha256":
        fail("Qt SDK manifest identityHash must be sha256")

    build = manifest["build"]
    if not isinstance(build, dict):
        fail("Qt SDK build configuration must be an object")
    require_exact_keys(
        build,
        {
            "buildType",
            "generator",
            "parallel",
            "shared",
            "buildExamples",
            "buildTests",
            "buildBenchmarks",
            "buildDocumentation",
        },
        "build configuration",
    )
    expected_build = {
        "buildType": "Release",
        "generator": "Ninja",
        "parallel": 4,
        "shared": True,
        "buildExamples": False,
        "buildTests": False,
        "buildBenchmarks": False,
        "buildDocumentation": False,
    }
    if not json_exact_equal(build, expected_build):
        fail(f"Qt SDK build configuration must remain {expected_build}")

    targets = manifest["targets"]
    if not isinstance(targets, dict) or tuple(sorted(targets)) != SUPPORTED_TARGETS:
        fail(
            "Qt SDK targets must be exactly "
            + ", ".join(SUPPORTED_TARGETS)
        )
    for name, specification in targets.items():
        if not isinstance(specification, dict):
            fail(f"target {name} must be an object")
        require_exact_keys(
            specification,
            {
                "platform",
                "runner",
                "architecture",
                "toolchain",
                "cmakeArguments",
                "moduleCmakeArguments",
                "requiredPlatformPlugins",
            },
            f"target {name}",
        )
        for field in ("platform", "runner", "architecture", "toolchain"):
            if not isinstance(specification[field], str) or not specification[field]:
                fail(f"target {name} {field} must be a non-empty string")
        if specification["platform"] not in SUPPORTED_PLATFORMS:
            fail(f"target {name} has unsupported platform")
        normalized_architecture = normalize_architecture(
            specification["architecture"]
        )
        if normalized_architecture not in {
            "x86_64",
            "arm64",
        }:
            fail(f"target {name} has unsupported architecture")
        if specification["architecture"] != normalized_architecture:
            fail(
                f"target {name} architecture must use canonical name "
                f"{normalized_architecture}"
            )
        expected_target_name = (
            f"{specification['platform']}-"
            f"{normalized_architecture}"
        )
        if name != expected_target_name:
            fail(
                f"target {name} must be named {expected_target_name} from its "
                "platform and architecture"
            )
        arguments = specification["cmakeArguments"]
        if not isinstance(arguments, list) or not all(
            isinstance(argument, str) and argument.startswith("-D")
            for argument in arguments
        ):
            fail(f"target {name} cmakeArguments must be -D strings")
        module_arguments = specification["moduleCmakeArguments"]
        if not isinstance(module_arguments, dict) or not all(
            isinstance(module_name, str)
            and isinstance(values, list)
            and all(
                isinstance(argument, str) and argument.startswith("-D")
                for argument in values
            )
            for module_name, values in module_arguments.items()
        ):
            fail(f"target {name} moduleCmakeArguments must contain -D strings")
        plugins = specification["requiredPlatformPlugins"]
        if not isinstance(plugins, list) or not plugins or not all(
            isinstance(plugin, str)
            and plugin
            and "/" not in plugin
            and "\\" not in plugin
            for plugin in plugins
        ):
            fail(f"target {name} requiredPlatformPlugins must be filenames")

    linux = targets["linux-x86_64"]
    if linux["moduleCmakeArguments"] != {
        "qtbase": [
            "-DFEATURE_xcb=ON",
            "-DFEATURE_opengl=ON",
            "-DFEATURE_egl=ON",
        ],
        "qtwayland": [
            "-DFEATURE_wayland_client=ON",
            "-DFEATURE_wayland_egl=ON",
        ],
    }:
        fail("linux Qt feature requirements must remain fail-closed")
    if linux["requiredPlatformPlugins"] != [
        "libqoffscreen.so",
        "libqwayland-egl.so",
        "libqxcb.so",
    ]:
        fail("linux Qt SDK must verify offscreen, Wayland, and XCB plugins")
    for macos_target in ("macos-arm64", "macos-x86_64"):
        specification = targets[macos_target]
        if "-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0" not in specification[
            "cmakeArguments"
        ]:
            fail(
                f"{macos_target} Qt SDK must target the Qt 6.8 supported "
                "12.0 floor"
            )
        expected_architecture_argument = (
            f"-DCMAKE_OSX_ARCHITECTURES={specification['architecture']}"
        )
        if expected_architecture_argument not in specification["cmakeArguments"]:
            fail(
                f"{macos_target} must configure its exact native architecture"
            )

    modules = manifest["modules"]
    if not isinstance(modules, list) or not modules:
        fail("Qt SDK modules must be a non-empty array")
    expected_modules = [
        "qtbase",
        "qtshadertools",
        "qtsvg",
        "qtdeclarative",
        "qtwayland",
    ]
    validate_ordered_rows(modules, "module")
    if [row["name"] for row in modules] != expected_modules:
        fail(f"Qt SDK module order must remain {expected_modules}")
    for row in modules:
        require_exact_keys(
            row, {"order", "name", "platforms", "url", "sha256"}, "module row"
        )
        if not row["url"].startswith(QT_SOURCE_PREFIX):
            fail(f"module {row['name']} must use Qt's official source URL")
        validate_url_filename_and_digest(row, row["name"])
        selected_platforms = row["platforms"]
        if (
            not isinstance(selected_platforms, list)
            or not selected_platforms
            or not set(selected_platforms).issubset(SUPPORTED_PLATFORMS)
        ):
            fail(f"module {row['name']} has invalid platforms")
        expected_platforms = (
            ["linux"]
            if row["name"] == "qtwayland"
            else ["linux", "macos", "windows"]
        )
        if selected_platforms != expected_platforms:
            fail(
                f"module {row['name']} platforms must remain {expected_platforms}"
            )

    patches = manifest["patches"]
    if not isinstance(patches, list) or len(patches) != 6:
        fail("Qt SDK manifest must contain exactly six security patches")
    validate_ordered_rows(patches, "patch")
    patch_names = [
        "CVE-2025-10728-qtsvg-6.8.diff",
        "CVE-2025-10729-qtsvg-6.8.diff",
        "CVE-2025-12385-qtdeclarative-6.8-0001.diff",
        "CVE-2025-12385-qtdeclarative-6.8-0002.diff",
        "CVE-2025-14576-qtdeclarative-6.8.diff",
        "CVE-2026-6210-qtsvg-6.8.diff",
    ]
    if [row["name"] for row in patches] != patch_names:
        fail(f"Qt SDK security patch order must remain {patch_names}")
    module_names = set(expected_modules)
    for row in patches:
        require_exact_keys(
            row,
            {"order", "name", "module", "url", "sha256"},
            "security patch row",
        )
        if row["module"] not in module_names:
            fail(f"patch {row['name']} targets an unknown module")
        if not row["url"].startswith(QT_PATCH_PREFIX):
            fail(f"patch {row['name']} must use Qt's official patch URL")
        validate_url_filename_and_digest(row, row["name"])

    identities = manifest["sdkIdentities"]
    if not isinstance(identities, dict) or set(identities) != set(targets):
        fail("Qt SDK identities must be checked in for every supported target")
    digest = unchecked_contract_digest(manifest, recipe_root=recipe_root)[:20]
    expected_identities = {
        target_name: (
            f"qt-{manifest['qtVersion']}-r{manifest['sdkRevision']}-"
            f"{target_specification['platform']}-"
            f"{target_specification['architecture']}-"
            f"{target_specification['toolchain']}-{digest}"
        )
        for target_name, target_specification in targets.items()
    }
    if identities != expected_identities:
        fail(
            "checked-in Qt SDK identities are stale; expected "
            f"{expected_identities}"
        )


def validate_ordered_rows(rows: list[dict[str, Any]], description: str) -> None:
    orders = [row.get("order") for row in rows if isinstance(row, dict)]
    if len(orders) != len(rows) or not all(
        type(order) is int and order > 0 for order in orders
    ):
        fail(f"every {description} row must have a positive integer order")
    if orders != sorted(orders) or len(set(orders)) != len(orders):
        fail(f"{description} rows must have unique ascending order values")


def validate_url_filename_and_digest(row: dict[str, Any], description: str) -> None:
    url = row.get("url")
    digest = row.get("sha256")
    if not isinstance(url, str) or urlparse(url).scheme != "https":
        fail(f"{description} URL must use HTTPS")
    filename = Path(urlparse(url).path).name
    expected_filename = (
        row.get("name")
        if description.startswith("CVE-")
        else f"{row.get('name')}-everywhere-opensource-src-6.8.4.tar.xz"
    )
    if filename != expected_filename:
        fail(
            f"{description} URL filename mismatch: expected "
            f"{expected_filename}, got {filename}"
        )
    if not isinstance(digest, str) or not SHA256_PATTERN.fullmatch(digest):
        fail(f"{description} must have a lowercase SHA-256 digest")


def unchecked_manifest_digest(manifest: dict[str, Any]) -> str:
    identity_payload = {
        key: value for key, value in manifest.items() if key != "sdkIdentities"
    }
    return sha256_bytes(canonical_json(identity_payload))


def recipe_file_paths(
    recipe_root: Path | None = None,
) -> tuple[tuple[str, Path], ...]:
    if recipe_root is None:
        return RECIPE_FILES
    root = trusted_source_root(recipe_root)
    return tuple(
        (
            logical_path,
            trusted_source_file(
                root,
                logical_path,
                f"Qt SDK recipe input {logical_path}",
            ),
        )
        for logical_path, _ in RECIPE_FILES
    )


def recipe_materials(
    recipe_root: Path | None = None,
) -> list[dict[str, str]]:
    materials = []
    for logical_path, filesystem_path in recipe_file_paths(recipe_root):
        materials.append(
            {
                "path": logical_path,
                "sha256": sha256_file(filesystem_path),
            }
        )
    return materials


def unchecked_contract_digest(
    manifest: dict[str, Any], *, recipe_root: Path | None = None
) -> str:
    payload = {
        "manifest": {
            key: value for key, value in manifest.items() if key != "sdkIdentities"
        },
        "recipeMaterials": recipe_materials(recipe_root),
    }
    return sha256_bytes(canonical_json(payload))


def manifest_digest(
    manifest: dict[str, Any], *, recipe_root: Path | None = None
) -> str:
    validate_manifest(manifest, recipe_root=recipe_root)
    return unchecked_manifest_digest(manifest)


def contract_digest(
    manifest: dict[str, Any], *, recipe_root: Path | None = None
) -> str:
    validate_manifest(manifest, recipe_root=recipe_root)
    return unchecked_contract_digest(manifest, recipe_root=recipe_root)


def validate_toolchain_fingerprint(value: str) -> str:
    if not isinstance(value, str) or SHA256_PATTERN.fullmatch(value) is None:
        fail("Qt SDK toolchain fingerprint must be a lowercase SHA-256 digest")
    return value


def validate_toolchain_contract(
    contract: dict[str, Any],
    manifest: dict[str, Any],
    target_name: str,
) -> None:
    specification = target_specification(manifest, target_name)
    platform_name = specification["platform"]
    if not isinstance(contract, dict):
        fail("Qt SDK toolchain contract root must be an object")
    require_exact_keys(
        contract,
        {"schemaVersion", "target", "platform", "runner", "tools"},
        "Qt SDK toolchain contract",
    )
    if (
        type(contract["schemaVersion"]) is not int
        or contract["schemaVersion"] != TOOLCHAIN_CONTRACT_SCHEMA
    ):
        fail(
            "Qt SDK toolchain contract schemaVersion must be "
            f"{TOOLCHAIN_CONTRACT_SCHEMA}"
        )
    if contract["platform"] != platform_name:
        fail(
            "Qt SDK toolchain contract platform mismatch: "
            f"expected {platform_name!r}, got {contract['platform']!r}"
        )
    if contract["target"] != target_name:
        fail(
            "Qt SDK toolchain contract target mismatch: "
            f"expected {target_name!r}, got {contract['target']!r}"
        )
    runner = contract["runner"]
    if not isinstance(runner, dict):
        fail("Qt SDK toolchain contract runner must be an object")
    require_exact_keys(
        runner,
        {"os", "architecture", "imageOS", "imageVersion"},
        "Qt SDK toolchain runner contract",
    )
    expected_runner_os = {
        "linux": "Linux",
        "macos": "macOS",
        "windows": "Windows",
    }[platform_name]
    if runner["os"] != expected_runner_os:
        fail(
            "Qt SDK toolchain contract runner OS mismatch: "
            f"expected {expected_runner_os!r}, got {runner['os']!r}"
        )
    for field in ("architecture", "imageOS", "imageVersion"):
        if not isinstance(runner[field], str) or not runner[field].strip():
            fail(f"Qt SDK toolchain runner {field} must be non-empty")
    runner_architecture = normalize_architecture(runner["architecture"])
    if runner_architecture != specification["architecture"]:
        fail(
            "Qt SDK toolchain contract runner architecture mismatch: "
            f"expected {specification['architecture']!r}, "
            f"got {runner_architecture!r}"
        )

    tools = contract["tools"]
    if not isinstance(tools, dict):
        fail("Qt SDK toolchain contract tools must be an object")
    require_exact_keys(
        tools,
        {"cmake", "ninja", "compiler", "python"},
        "Qt SDK toolchain tools contract",
    )
    for field, value in tools.items():
        if (
            not isinstance(value, str)
            or not value.strip()
            or "\n" in value
            or "\r" in value
        ):
            fail(f"Qt SDK toolchain {field} version must be one non-empty line")


def toolchain_fingerprint(
    contract: dict[str, Any],
    manifest: dict[str, Any],
    target_name: str,
) -> str:
    validate_toolchain_contract(contract, manifest, target_name)
    return sha256_bytes(canonical_json(contract))


def load_toolchain_contract(
    path: Path,
    manifest: dict[str, Any],
    target_name: str,
) -> dict[str, Any]:
    try:
        contract = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"Qt SDK toolchain contract not found: {path}")
    except json.JSONDecodeError as error:
        fail(f"Qt SDK toolchain contract is invalid JSON: {error}")
    validate_toolchain_contract(contract, manifest, target_name)
    return contract


def capture_toolchain_contract(
    manifest: dict[str, Any], target_name: str
) -> dict[str, Any]:
    specification = target_specification(manifest, target_name)
    platform_name = specification["platform"]
    validate_build_host(manifest, target_name)
    compiler_command = {
        "linux": ["gcc", "--version"],
        "macos": ["clang", "--version"],
        "windows": ["cl.exe"],
    }[platform_name]
    github_actions = os.environ.get("GITHUB_ACTIONS") == "true"
    image_os = os.environ.get("ImageOS")
    image_version = os.environ.get("ImageVersion")
    if github_actions and (not image_os or not image_version):
        fail(
            "GitHub-hosted Qt builds require ImageOS and ImageVersion in the "
            "runner contract"
        )
    runner_os = {
        "linux": "Linux",
        "macos": "macOS",
        "windows": "Windows",
    }[platform_name]
    contract = {
        "schemaVersion": TOOLCHAIN_CONTRACT_SCHEMA,
        "target": target_name,
        "platform": platform_name,
        "runner": {
            "os": os.environ.get("RUNNER_OS") or runner_os,
            "architecture": normalize_architecture(
                os.environ.get("RUNNER_ARCH") or normalized_machine()
            ),
            "imageOS": image_os or host_platform.system(),
            "imageVersion": image_version or host_platform.release(),
        },
        "tools": {
            "cmake": command_version(["cmake", "--version"]),
            "ninja": command_version(["ninja", "--version"]),
            "compiler": command_version(compiler_command),
            "python": host_platform.python_version(),
        },
    }
    validate_toolchain_contract(contract, manifest, target_name)
    return contract


def write_toolchain_contract(
    contract: dict[str, Any], path: Path | None
) -> None:
    rendered = json.dumps(contract, indent=2, sort_keys=True) + "\n"
    if path is None:
        print(rendered, end="")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(rendered, encoding="utf-8")


def sdk_identity(
    manifest: dict[str, Any],
    target_name: str,
    toolchain_fingerprint_value: str | None = None,
    *,
    recipe_root: Path | None = None,
) -> str:
    validate_manifest(manifest, recipe_root=recipe_root)
    resolved_target = resolve_target(
        manifest,
        target_name=(
            target_name if target_name in manifest["targets"] else None
        ),
        platform_name=(
            target_name if target_name in SUPPORTED_PLATFORMS else None
        ),
    )
    identity = manifest["sdkIdentities"][resolved_target]
    if toolchain_fingerprint_value is None:
        return identity
    fingerprint = validate_toolchain_fingerprint(toolchain_fingerprint_value)
    return f"{identity}-tc-{fingerprint[:20]}"


def selected_modules(
    manifest: dict[str, Any], target_or_platform: str
) -> list[dict[str, Any]]:
    platform_name = logical_platform(manifest, target_or_platform)
    return [
        row for row in manifest["modules"] if platform_name in row["platforms"]
    ]


def expected_source_materials(
    manifest: dict[str, Any], target_or_platform: str
) -> list[dict[str, Any]]:
    modules = selected_modules(manifest, target_or_platform)
    module_names = {row["name"] for row in modules}
    materials: list[dict[str, Any]] = [
        {
            "kind": "module",
            "order": row["order"],
            "name": row["name"],
            "url": row["url"],
            "sha256": row["sha256"],
        }
        for row in modules
    ]
    materials.extend(
        {
            "kind": "security-patch",
            "order": row["order"],
            "name": row["name"],
            "module": row["module"],
            "url": row["url"],
            "sha256": row["sha256"],
        }
        for row in manifest["patches"]
        if row["module"] in module_names
    )
    return materials


def normalized_host_platform() -> str:
    system = host_platform.system().lower()
    return {"darwin": "macos", "windows": "windows", "linux": "linux"}.get(
        system, system
    )


def normalized_machine() -> str:
    return normalize_architecture(host_platform.machine())


def macos_process_is_translated() -> bool:
    if normalized_host_platform() != "macos":
        return False
    try:
        result = subprocess.run(
            ["sysctl", "-in", "sysctl.proc_translated"],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
    except OSError:
        return False
    return result.returncode == 0 and result.stdout.strip() == "1"


def command_path(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        fail(f"required command is not on PATH: {name}")
    return path


def validate_build_host(
    manifest: dict[str, Any], target_name: str, *, check_tools: bool = True
) -> None:
    specification = target_specification(manifest, target_name)
    platform_name = specification["platform"]
    actual_platform = normalized_host_platform()
    if actual_platform != platform_name:
        fail(
            f"requested {platform_name} SDK on {actual_platform}; native builds only"
        )
    expected_machine = specification["architecture"]
    if platform_name == "macos" and macos_process_is_translated():
        if expected_machine == "arm64":
            fail(
                "macos-arm64 Qt SDK builds require a native Apple Silicon "
                "process; Rosetta translation is unsupported"
            )
        fail(
            "macos-x86_64 Qt SDK builds require a native Intel host; "
            "Rosetta translation is unsupported"
        )
    actual_machine = normalized_machine()
    if actual_machine != expected_machine:
        fail(
            f"{platform_name} SDK requires {expected_machine}, "
            f"found {actual_machine}"
        )
    if platform_name == "linux":
        os_release = Path("/etc/os-release")
        content = (
            os_release.read_text(encoding="utf-8", errors="replace")
            if os_release.is_file()
            else ""
        )
        if not re.search(r'^ID="?ubuntu"?$', content, re.MULTILINE) or not re.search(
            r'^VERSION_ID="?22\.04"?$', content, re.MULTILINE
        ):
            fail("linux Qt SDK builds require the ubuntu-22.04 runner")
    if not check_tools:
        return
    for tool in ("cmake", "ninja", "git"):
        command_path(tool)
    if platform_name == "linux":
        gcc = command_path("gcc")
        command_path("g++")
        version = subprocess.run(
            [gcc, "-dumpfullversion", "-dumpversion"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.strip()
        if version.split(".", 1)[0] != "11":
            fail(f"ubuntu-22.04 Qt SDK requires GCC 11, found {version}")
    elif platform_name == "macos":
        command_path("clang")
        command_path("clang++")
    else:
        command_path("cl.exe")
        visual_studio = os.environ.get("VisualStudioVersion", "")
        if not visual_studio.startswith("17."):
            fail(
                "windows Qt SDK builds require an initialized Visual Studio "
                "2022 (17.x) developer environment"
            )


def verify_digest(path: Path, expected: str, description: str) -> None:
    actual = sha256_file(path)
    if actual != expected:
        fail(
            f"SHA-256 mismatch for {description}: expected {expected}, got {actual}"
        )


def download_verified(row: dict[str, Any], download_dir: Path) -> Path:
    download_dir.mkdir(parents=True, exist_ok=True)
    filename = Path(urlparse(row["url"]).path).name
    destination = download_dir / filename
    if destination.is_file():
        try:
            verify_digest(destination, row["sha256"], filename)
            return destination
        except QtSdkError:
            destination.unlink()

    request = Request(row["url"], headers={"User-Agent": "Chaft-Qt-SDK/1"})
    partial = destination.with_suffix(destination.suffix + ".part")
    for attempt in range(1, 4):
        try:
            with urlopen(request, timeout=90) as response, partial.open("wb") as out:
                shutil.copyfileobj(response, out, length=1024 * 1024)
            verify_digest(partial, row["sha256"], filename)
            partial.replace(destination)
            return destination
        except (OSError, URLError, QtSdkError) as error:
            partial.unlink(missing_ok=True)
            if attempt == 3:
                fail(f"unable to download verified {filename}: {error}")
    raise AssertionError("download retry loop must return or fail")


def validate_tar_member(member: tarfile.TarInfo) -> None:
    path = PurePosixPath(member.name)
    if path.is_absolute() or ".." in path.parts:
        fail(f"archive contains an unsafe path: {member.name}")
    if member.isdev():
        fail(f"archive contains a device entry: {member.name}")
    if member.issym() or member.islnk():
        target = PurePosixPath(member.linkname)
        if target.is_absolute() or ".." in target.parts:
            fail(
                f"archive contains an unsafe link: "
                f"{member.name} -> {member.linkname}"
            )


def extract_archive(archive: Path, destination: Path) -> Path:
    if destination.exists():
        fail(f"archive extraction destination already exists: {destination}")
    destination.mkdir(parents=True)
    with tarfile.open(archive, mode="r:xz") as handle:
        members = handle.getmembers()
        if not members:
            fail(f"source archive is empty: {archive.name}")
        for member in members:
            validate_tar_member(member)
        try:
            handle.extractall(destination, members=members, filter="data")
        except TypeError:
            if any(member.issym() or member.islnk() for member in members):
                fail(
                    "this Python cannot safely extract source archives containing "
                    "links; use Python 3.11.4 or newer"
                )
            handle.extractall(destination, members=members)
    roots = [entry for entry in destination.iterdir()]
    if len(roots) != 1 or not roots[0].is_dir():
        fail(f"source archive must contain one top-level directory: {archive.name}")
    return roots[0]


def run(command: list[str], *, cwd: Path | None = None, env=None) -> None:
    rendered = subprocess.list2cmdline(command)
    print(f"+ {rendered}", flush=True)
    subprocess.run(command, cwd=cwd, env=env, check=True)


def clean_directory(path: Path, parent: Path) -> None:
    resolved = path.resolve()
    resolved_parent = parent.resolve()
    if resolved.parent != resolved_parent:
        fail(f"refusing to clean path outside expected work directory: {resolved}")
    if resolved.exists():
        shutil.rmtree(resolved)


def prepare_sources(
    manifest: dict[str, Any], target_name: str, work_dir: Path
) -> tuple[dict[str, Path], list[dict[str, str]]]:
    downloads = work_dir / "downloads"
    source_parent = work_dir / "sources"
    clean_directory(source_parent, work_dir)
    source_parent.mkdir(parents=True)

    source_paths: dict[str, Path] = {}
    materials: list[dict[str, str]] = []
    for module in selected_modules(manifest, target_name):
        archive = download_verified(module, downloads)
        verify_digest(archive, module["sha256"], module["name"])
        staging = source_parent / f".extract-{module['name']}"
        root = extract_archive(archive, staging)
        target = source_parent / module["name"]
        root.replace(target)
        staging.rmdir()
        source_paths[module["name"]] = target
        materials.append(
            {
                "kind": "module",
                "order": module["order"],
                "name": module["name"],
                "url": module["url"],
                "sha256": module["sha256"],
            }
        )

    for patch in manifest["patches"]:
        if patch["module"] not in source_paths:
            continue
        patch_path = download_verified(patch, downloads)
        verify_digest(patch_path, patch["sha256"], patch["name"])
        module_source = source_paths[patch["module"]]
        run(["git", "apply", "--check", str(patch_path)], cwd=module_source)
        run(
            ["git", "apply", "--whitespace=nowarn", str(patch_path)],
            cwd=module_source,
        )
        materials.append(
            {
                "kind": "security-patch",
                "order": patch["order"],
                "name": patch["name"],
                "module": patch["module"],
                "url": patch["url"],
                "sha256": patch["sha256"],
            }
        )
    return source_paths, materials


def cmake_configure_command(
    manifest: dict[str, Any],
    target_name: str,
    source_dir: Path,
    build_dir: Path,
    prefix: Path,
) -> list[str]:
    build = manifest["build"]
    arguments = [
        "cmake",
        "-S",
        str(source_dir),
        "-B",
        str(build_dir),
        "-G",
        build["generator"],
        f"-DCMAKE_BUILD_TYPE={build['buildType']}",
        f"-DCMAKE_INSTALL_PREFIX={prefix}",
        f"-DCMAKE_PREFIX_PATH={prefix}",
        "-DBUILD_SHARED_LIBS=ON",
        "-DQT_BUILD_EXAMPLES=OFF",
        "-DQT_BUILD_TESTS=OFF",
        "-DQT_BUILD_BENCHMARKS=OFF",
        "-DQT_BUILD_DOCS=OFF",
    ]
    specification = target_specification(manifest, target_name)
    arguments.extend(specification["cmakeArguments"])
    arguments.extend(
        specification["moduleCmakeArguments"].get(
            source_dir.name, []
        )
    )
    return arguments


def qt_configure_module_command(
    manifest: dict[str, Any],
    target_name: str,
    source_dir: Path,
    prefix: Path,
) -> list[str]:
    """Use Qt's supported installed-module configuration frontend."""
    specification = target_specification(manifest, target_name)
    suffix = ".bat" if specification["platform"] == "windows" else ""
    configure_module = prefix / "bin" / f"qt-configure-module{suffix}"
    arguments = [
        str(configure_module),
        str(source_dir),
        "--",
        "-G",
        manifest["build"]["generator"],
        f"-DCMAKE_BUILD_TYPE={manifest['build']['buildType']}",
        f"-DCMAKE_INSTALL_PREFIX={prefix}",
        f"-DCMAKE_PREFIX_PATH={prefix}",
        "-DBUILD_SHARED_LIBS=ON",
        "-DQT_BUILD_EXAMPLES=OFF",
        "-DQT_BUILD_TESTS=OFF",
        "-DQT_BUILD_BENCHMARKS=OFF",
        "-DQT_BUILD_DOCS=OFF",
    ]
    arguments.extend(specification["cmakeArguments"])
    arguments.extend(
        specification["moduleCmakeArguments"].get(
            source_dir.name, []
        )
    )
    return arguments


def build_modules(
    manifest: dict[str, Any],
    target_name: str,
    prefix: Path,
    work_dir: Path,
    source_paths: dict[str, Path],
) -> list[list[str]]:
    platform_name = target_platform(manifest, target_name)
    build_parent = work_dir / "build"
    clean_directory(build_parent, work_dir)
    build_parent.mkdir(parents=True)
    environment = os.environ.copy()
    environment["CMAKE_PREFIX_PATH"] = str(prefix)
    environment["QTDIR"] = str(prefix)
    commands: list[list[str]] = []
    for module in selected_modules(manifest, target_name):
        name = module["name"]
        build_dir = build_parent / name
        build_dir.mkdir()
        if name == "qtbase":
            configure = cmake_configure_command(
                manifest, target_name, source_paths[name], build_dir, prefix
            )
            configure_cwd = None
        else:
            configure_module = prefix / "bin" / (
                "qt-configure-module.bat"
                if platform_name == "windows"
                else "qt-configure-module"
            )
            if not configure_module.is_file():
                fail(
                    "qtbase did not install the supported module configuration "
                    f"frontend: {configure_module}"
                )
            configure = qt_configure_module_command(
                manifest, target_name, source_paths[name], prefix
            )
            configure_cwd = build_dir
        compile_command = [
            "cmake",
            "--build",
            str(build_dir),
            "--parallel",
            str(manifest["build"]["parallel"]),
        ]
        install_command = ["cmake", "--install", str(build_dir)]
        run(configure, cwd=configure_cwd, env=environment)
        commands.append(configure)
        for command in (compile_command, install_command):
            run(command, env=environment)
            commands.append(command)
    return commands


def command_version(command: list[str]) -> str:
    result = subprocess.run(
        command,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    return result.stdout.strip().splitlines()[0] if result.stdout.strip() else ""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def write_provenance(
    manifest: dict[str, Any],
    target_name: str,
    prefix: Path,
    materials: list[dict[str, str]],
    commands: list[list[str]],
    toolchain_contract: dict[str, Any],
) -> Path:
    specification = target_specification(manifest, target_name)
    platform_name = specification["platform"]
    fingerprint = toolchain_fingerprint(
        toolchain_contract, manifest, target_name
    )
    provenance = {
        "schemaVersion": 2,
        "identity": sdk_identity(manifest, target_name, fingerprint),
        "manifestSha256": manifest_digest(manifest),
        "contractSha256": contract_digest(manifest),
        "qtVersion": manifest["qtVersion"],
        "sdkRevision": manifest["sdkRevision"],
        "target": target_name,
        "platform": platform_name,
        "architecture": specification["architecture"],
        "targetSpecification": specification,
        "buildConfiguration": manifest["build"],
        "generatedAt": utc_now(),
        "host": {
            "system": host_platform.system(),
            "release": host_platform.release(),
            "machine": host_platform.machine(),
        },
        "toolchainFingerprint": fingerprint,
        "toolchainContract": toolchain_contract,
        "sourceMaterials": materials,
        "recipeMaterials": recipe_materials(),
        "commands": commands,
        "verification": {"completed": False, "completedAt": None},
    }
    path = prefix / PROVENANCE_NAME
    path.write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return path


def validate_provenance_object(
    provenance: dict[str, Any],
    manifest: dict[str, Any],
    target_name: str,
    *,
    recipe_root: Path | None = None,
    expected_toolchain_fingerprint: str | None = None,
    allow_incomplete: bool = False,
) -> dict[str, Any]:
    specification = target_specification(manifest, target_name)
    platform_name = specification["platform"]
    if not isinstance(provenance, dict):
        fail("Qt SDK provenance root must be an object")
    require_exact_keys(
        provenance,
        {
            "schemaVersion",
            "identity",
            "manifestSha256",
            "contractSha256",
            "qtVersion",
            "sdkRevision",
            "target",
            "platform",
            "architecture",
            "targetSpecification",
            "buildConfiguration",
            "generatedAt",
            "host",
            "toolchainFingerprint",
            "toolchainContract",
            "sourceMaterials",
            "recipeMaterials",
            "commands",
            "verification",
        },
        "Qt SDK provenance",
    )
    if type(provenance["schemaVersion"]) is not int or provenance["schemaVersion"] != 2:
        fail("Qt SDK provenance schemaVersion must be 2")
    if not json_exact_equal(
        provenance["targetSpecification"],
        specification,
    ):
        fail("Qt SDK provenance targetSpecification mismatch")
    if not json_exact_equal(
        provenance["buildConfiguration"], manifest["build"]
    ):
        fail("Qt SDK provenance buildConfiguration mismatch")
    if (
        not isinstance(provenance["generatedAt"], str)
        or not provenance["generatedAt"]
    ):
        fail("Qt SDK provenance generatedAt is missing")
    host = provenance["host"]
    if (
        not isinstance(host, dict)
        or set(host) != {"system", "release", "machine"}
        or not all(isinstance(value, str) and value for value in host.values())
    ):
        fail("Qt SDK provenance host is missing or malformed")
    commands = provenance["commands"]
    if not isinstance(commands, list) or not all(
        isinstance(command, list)
        and command
        and all(isinstance(argument, str) and argument for argument in command)
        for command in commands
    ):
        fail("Qt SDK provenance commands are malformed")
    toolchain_contract = provenance.get("toolchainContract")
    if not isinstance(toolchain_contract, dict):
        fail("Qt SDK provenance toolchainContract is missing or malformed")
    fingerprint = toolchain_fingerprint(
        toolchain_contract, manifest, target_name
    )
    if provenance.get("toolchainFingerprint") != fingerprint:
        fail(
            "Qt SDK provenance toolchainFingerprint does not match the "
            "canonical toolchain contract"
        )
    if expected_toolchain_fingerprint is not None:
        expected_fingerprint = validate_toolchain_fingerprint(
            expected_toolchain_fingerprint
        )
        if fingerprint != expected_fingerprint:
            fail(
                "Qt SDK provenance toolchainFingerprint mismatch: "
                f"expected {expected_fingerprint!r}, got {fingerprint!r}"
            )
    expected = {
        "identity": sdk_identity(
            manifest,
            target_name,
            fingerprint,
            recipe_root=recipe_root,
        ),
        "manifestSha256": manifest_digest(
            manifest, recipe_root=recipe_root
        ),
        "contractSha256": contract_digest(
            manifest, recipe_root=recipe_root
        ),
        "qtVersion": manifest["qtVersion"],
        "sdkRevision": manifest["sdkRevision"],
        "target": target_name,
        "platform": platform_name,
        "architecture": specification["architecture"],
    }
    for field, value in expected.items():
        if not json_exact_equal(provenance.get(field), value):
            fail(
                f"Qt SDK provenance {field} mismatch: "
                f"expected {value!r}, got {provenance.get(field)!r}"
            )
    expected_materials = expected_source_materials(manifest, target_name)
    if not json_exact_equal(
        provenance.get("sourceMaterials"), expected_materials
    ):
        fail(
            "Qt SDK provenance sourceMaterials mismatch: expected the exact "
            "platform-selected archives and ordered security patches"
        )
    if not json_exact_equal(
        provenance.get("recipeMaterials"), recipe_materials(recipe_root)
    ):
        fail(
            "Qt SDK provenance recipeMaterials mismatch: expected the exact "
            "build driver and verification probes"
        )
    verification = provenance.get("verification")
    if not isinstance(verification, dict) or set(verification) != {
        "completed",
        "completedAt",
    }:
        fail("Qt SDK provenance verification marker is missing or malformed")
    if allow_incomplete:
        is_provisional = (
            verification["completed"] is False
            and verification["completedAt"] is None
        )
        is_completed = (
            verification["completed"] is True
            and isinstance(verification["completedAt"], str)
            and bool(verification["completedAt"])
        )
        if not (is_provisional or is_completed):
            fail("Qt SDK provisional verification marker is malformed")
    elif verification.get("completed") is not True or not isinstance(
        verification.get("completedAt"), str
    ) or not verification["completedAt"]:
        fail("Qt SDK provenance does not record completed verification")
    return provenance


def load_and_validate_provenance(
    path: Path,
    manifest: dict[str, Any],
    target_name: str,
    *,
    recipe_root: Path | None = None,
    expected_toolchain_fingerprint: str | None = None,
    allow_incomplete: bool = False,
) -> dict[str, Any]:
    try:
        provenance = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"Qt SDK provenance not found: {path}")
    except json.JSONDecodeError as error:
        fail(f"Qt SDK provenance is invalid JSON: {error}")
    return validate_provenance_object(
        provenance,
        manifest,
        target_name,
        recipe_root=recipe_root,
        expected_toolchain_fingerprint=expected_toolchain_fingerprint,
        allow_incomplete=allow_incomplete,
    )


def prefix_tool(prefix: Path, *names: str) -> Path:
    for name in names:
        candidate = prefix / "bin" / name
        if candidate.is_file():
            return candidate
    fail(f"Qt SDK tool not found under {prefix / 'bin'}: {', '.join(names)}")
    raise AssertionError("fail always raises")


def verify_sdk(
    manifest: dict[str, Any],
    target_name: str,
    prefix: Path,
    provenance_path: Path | None = None,
    *,
    expected_toolchain_fingerprint: str | None = None,
    allow_incomplete_provenance: bool = False,
) -> None:
    specification = target_specification(manifest, target_name)
    platform_name = specification["platform"]
    validate_build_host(manifest, target_name)
    if not prefix.is_dir():
        fail(f"Qt SDK prefix not found: {prefix}")
    provenance_path = provenance_path or prefix / PROVENANCE_NAME
    load_and_validate_provenance(
        provenance_path,
        manifest,
        target_name,
        expected_toolchain_fingerprint=expected_toolchain_fingerprint,
        allow_incomplete=allow_incomplete_provenance,
    )

    executable_suffix = ".exe" if platform_name == "windows" else ""
    qtpaths = prefix_tool(
        prefix, f"qtpaths6{executable_suffix}", f"qtpaths{executable_suffix}"
    )
    version = subprocess.run(
        [str(qtpaths), "--qt-version"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    if version != manifest["qtVersion"]:
        fail(
            f"Qt SDK version mismatch: expected {manifest['qtVersion']}, got {version}"
        )
    platform_plugin_dir = prefix / "plugins" / "platforms"
    missing_plugins = [
        name
        for name in specification["requiredPlatformPlugins"]
        if not (platform_plugin_dir / name).is_file()
    ]
    if missing_plugins:
        fail(
            "Qt SDK is missing required platform plugin(s): "
            + ", ".join(missing_plugins)
        )

    environment = os.environ.copy()
    environment["QTDIR"] = str(prefix)
    environment["CMAKE_PREFIX_PATH"] = str(prefix)
    environment["QT_PLUGIN_PATH"] = str(prefix / "plugins")
    environment["QML2_IMPORT_PATH"] = str(prefix / "qml")
    environment["QT_QPA_PLATFORM"] = "offscreen"
    with tempfile.TemporaryDirectory(prefix="chaft-qt-sdk-probe-") as temporary:
        build_dir = Path(temporary) / "build"
        configure = [
            "cmake",
            "-S",
            str(PROBE_DIR),
            "-B",
            str(build_dir),
            "-G",
            manifest["build"]["generator"],
            f"-DCMAKE_BUILD_TYPE={manifest['build']['buildType']}",
            f"-DCMAKE_PREFIX_PATH={prefix}",
        ]
        configure.extend(specification["cmakeArguments"])
        run(configure, env=environment)
        run(
            [
                "cmake",
                "--build",
                str(build_dir),
                "--parallel",
                str(manifest["build"]["parallel"]),
            ],
            env=environment,
        )

        qmltestrunner = prefix_tool(
            prefix, f"qmltestrunner{executable_suffix}"
        )
        run(
            [
                str(qmltestrunner),
                "-input",
                str(PROBE_DIR / "tst_QtSdk.qml"),
                "-import",
                str(prefix / "qml"),
            ],
            cwd=PROBE_DIR,
            env=environment,
        )


def complete_provenance(path: Path) -> None:
    provenance = json.loads(path.read_text(encoding="utf-8"))
    provenance["verification"] = {
        "completed": True,
        "completedAt": utc_now(),
    }
    path.write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def build_sdk(
    manifest: dict[str, Any],
    target_name: str,
    prefix: Path,
    work_dir: Path | None,
    toolchain_contract: dict[str, Any],
) -> None:
    validate_build_host(manifest, target_name)
    prefix = prefix.resolve()
    fingerprint = toolchain_fingerprint(
        toolchain_contract, manifest, target_name
    )
    identity = sdk_identity(manifest, target_name, fingerprint)
    work_dir = (
        work_dir.resolve()
        if work_dir is not None
        else prefix.parent / f".chaft-qt-work-{identity}"
    )
    if prefix == work_dir or prefix in work_dir.parents or work_dir in prefix.parents:
        fail("Qt SDK prefix and work directory must not contain each other")
    if prefix.exists() and any(prefix.iterdir()):
        fail(f"Qt SDK prefix must be empty before a source build: {prefix}")
    prefix.mkdir(parents=True, exist_ok=True)
    work_dir.mkdir(parents=True, exist_ok=True)

    source_paths, materials = prepare_sources(
        manifest, target_name, work_dir
    )
    commands = build_modules(
        manifest, target_name, prefix, work_dir, source_paths
    )
    provenance_path = write_provenance(
        manifest,
        target_name,
        prefix,
        materials,
        commands,
        toolchain_contract,
    )
    verify_sdk(
        manifest,
        target_name,
        prefix,
        provenance_path,
        expected_toolchain_fingerprint=fingerprint,
        allow_incomplete_provenance=True,
    )
    complete_provenance(provenance_path)
    load_and_validate_provenance(
        provenance_path,
        manifest,
        target_name,
        expected_toolchain_fingerprint=fingerprint,
        allow_incomplete=False,
    )
    print(f"built and verified {identity} at {prefix}")


def activation_values(
    manifest: dict[str, Any], prefix: Path
) -> dict[str, str]:
    prefix = prefix.resolve()
    if not prefix.is_dir():
        fail(f"Qt SDK prefix not found: {prefix}")
    provenance_path = prefix / PROVENANCE_NAME
    try:
        untrusted = json.loads(provenance_path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"Qt SDK provenance not found: {provenance_path}")
    except json.JSONDecodeError as error:
        fail(f"Qt SDK provenance is invalid JSON: {error}")
    if not isinstance(untrusted, dict):
        fail("Qt SDK provenance root must be an object")
    target_name = untrusted.get("target")
    if not isinstance(target_name, str) or target_name not in manifest["targets"]:
        fail("Qt SDK provenance target is missing or unsupported")
    provenance = validate_provenance_object(
        untrusted, manifest, target_name
    )
    specification = target_specification(manifest, target_name)
    existing = os.environ.get("CMAKE_PREFIX_PATH")
    cmake_prefix = (
        os.pathsep.join((str(prefix), existing)) if existing else str(prefix)
    )
    return {
        "QTDIR": str(prefix),
        "QT_ROOT_DIR": str(prefix),
        "CMAKE_PREFIX_PATH": cmake_prefix,
        "CHAFT_QT_SDK_BUILD_TYPE": "Release",
        "CHAFT_QT_SDK_TARGET": target_name,
        "CHAFT_QT_SDK_PLATFORM": specification["platform"],
        "CHAFT_QT_SDK_ARCHITECTURE": specification["architecture"],
        "CHAFT_QT_SDK_VERSION": manifest["qtVersion"],
        "CHAFT_QT_SDK_IDENTITY": provenance["identity"],
        "CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT": provenance[
            "toolchainFingerprint"
        ],
        "CHAFT_QT_SDK_PROVENANCE": str(provenance_path),
    }


def append_github_file(path: Path, lines: list[str]) -> None:
    for line in lines:
        if "\n" in line or "\r" in line:
            fail("GitHub environment values must not contain newlines")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8", newline="\n") as handle:
        for line in lines:
            handle.write(line + "\n")


def activate_sdk(
    manifest: dict[str, Any],
    prefix: Path,
    github_env: Path | None,
    github_path: Path | None,
) -> None:
    values = activation_values(manifest, prefix)
    environment_lines = [f"{key}={value}" for key, value in values.items()]
    path_line = str(prefix.resolve() / "bin")
    if github_env is not None:
        append_github_file(github_env, environment_lines)
    if github_path is not None:
        append_github_file(github_path, [path_line])
    if github_env is None and github_path is None:
        print("\n".join(environment_lines + [f"PATH+={path_line}"]))


def add_target_selector(command: argparse.ArgumentParser) -> None:
    selector = command.add_mutually_exclusive_group(required=True)
    selector.add_argument(
        "--target",
        choices=SUPPORTED_TARGETS,
        help="exact operating-system and architecture target",
    )
    selector.add_argument(
        "--platform",
        choices=SUPPORTED_PLATFORMS,
        help=(
            "legacy logical-platform selector; accepted only when exactly one "
            "target exists for that platform"
        ),
    )


def command_target(
    manifest: dict[str, Any], arguments: argparse.Namespace
) -> str:
    return resolve_target(
        manifest,
        target_name=getattr(arguments, "target", None),
        platform_name=getattr(arguments, "platform", None),
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description="Build and verify Chaft's exact open-source Qt 6.8.4 SDK"
    )
    commands = result.add_subparsers(dest="command", required=True)

    identity = commands.add_parser(
        "identity", help="print the offline SDK cache/release identity"
    )
    add_target_selector(identity)
    identity_source = identity.add_mutually_exclusive_group()
    identity_source.add_argument("--toolchain-contract", type=Path)
    identity_source.add_argument("--toolchain-fingerprint")

    toolchain = commands.add_parser(
        "toolchain-contract",
        help="capture the native runner image and build-tool contract",
    )
    add_target_selector(toolchain)
    toolchain.add_argument("--output", type=Path)

    fingerprint = commands.add_parser(
        "toolchain-fingerprint",
        help="print the canonical SHA-256 of a captured toolchain contract",
    )
    add_target_selector(fingerprint)
    fingerprint.add_argument("--toolchain-contract", type=Path, required=True)

    build = commands.add_parser("build", help="build and verify the source SDK")
    add_target_selector(build)
    build.add_argument("--prefix", type=Path, required=True)
    build.add_argument("--work-dir", type=Path)
    build.add_argument("--toolchain-contract", type=Path, required=True)

    verify = commands.add_parser("verify", help="verify an installed SDK")
    add_target_selector(verify)
    verify.add_argument("--prefix", type=Path, required=True)
    verify.add_argument("--provenance", type=Path)
    verify_source = verify.add_mutually_exclusive_group(required=True)
    verify_source.add_argument("--toolchain-contract", type=Path)
    verify_source.add_argument("--toolchain-fingerprint")

    activate = commands.add_parser(
        "activate", help="emit or write deterministic SDK environment values"
    )
    activate.add_argument("--prefix", type=Path, required=True)
    activate.add_argument("--github-env", type=Path)
    activate.add_argument("--github-path", type=Path)
    return result


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        manifest = load_manifest()
        if arguments.command == "identity":
            target_name = command_target(manifest, arguments)
            fingerprint = arguments.toolchain_fingerprint
            if arguments.toolchain_contract is not None:
                contract = load_toolchain_contract(
                    arguments.toolchain_contract, manifest, target_name
                )
                fingerprint = toolchain_fingerprint(
                    contract, manifest, target_name
                )
            print(sdk_identity(manifest, target_name, fingerprint))
        elif arguments.command == "toolchain-contract":
            target_name = command_target(manifest, arguments)
            contract = capture_toolchain_contract(manifest, target_name)
            write_toolchain_contract(contract, arguments.output)
        elif arguments.command == "toolchain-fingerprint":
            target_name = command_target(manifest, arguments)
            contract = load_toolchain_contract(
                arguments.toolchain_contract, manifest, target_name
            )
            print(toolchain_fingerprint(contract, manifest, target_name))
        elif arguments.command == "build":
            target_name = command_target(manifest, arguments)
            contract = load_toolchain_contract(
                arguments.toolchain_contract, manifest, target_name
            )
            build_sdk(
                manifest,
                target_name,
                arguments.prefix,
                arguments.work_dir,
                contract,
            )
        elif arguments.command == "verify":
            target_name = command_target(manifest, arguments)
            fingerprint = arguments.toolchain_fingerprint
            if arguments.toolchain_contract is not None:
                contract = load_toolchain_contract(
                    arguments.toolchain_contract, manifest, target_name
                )
                fingerprint = toolchain_fingerprint(
                    contract, manifest, target_name
                )
            verify_sdk(
                manifest,
                target_name,
                arguments.prefix.resolve(),
                arguments.provenance,
                expected_toolchain_fingerprint=fingerprint,
            )
        elif arguments.command == "activate":
            activate_sdk(
                manifest,
                arguments.prefix,
                arguments.github_env,
                arguments.github_path,
            )
        else:
            raise AssertionError(f"unhandled command: {arguments.command}")
    except (QtSdkError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
