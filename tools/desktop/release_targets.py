#!/usr/bin/env python3
"""Canonical native desktop release targets and artifact names."""

from __future__ import annotations

import os
import platform
from dataclasses import dataclass
from typing import Iterable


ARCHITECTURE_ALIASES = {
    "aarch64": "arm64",
    "amd64": "x86_64",
    "arm64": "arm64",
    "x64": "x86_64",
    "x86-64": "x86_64",
    "x86_64": "x86_64",
}
PLATFORM_ALIASES = {
    "darwin": "macos",
    "linux": "linux",
    "mac": "macos",
    "macos": "macos",
    "osx": "macos",
    "win32": "windows",
    "windows": "windows",
}


class ReleaseTargetError(ValueError):
    """A requested platform/architecture pair is outside the release contract."""


@dataclass(frozen=True)
class ReleaseTarget:
    name: str
    platform: str
    architecture: str
    display_name: str
    runner: str
    package_template: str

    def package_name(self, version: str) -> str:
        return self.package_template.format(version=version)

    @property
    def canonical_package_suffix(self) -> str:
        return self.package_template.format(version="")

    @property
    def metadata_prefix(self) -> str:
        return f"chaft-desktop-{self.name}"

    @property
    def metadata_names(self) -> dict[str, str]:
        return {
            "checksums": f"{self.metadata_prefix}-SHA256SUMS",
            "sbom": f"{self.metadata_prefix}-sbom.cdx.json",
            "provenance": f"{self.metadata_prefix}-provenance.json",
        }

    @property
    def verification_receipt_name(self) -> str:
        return f"{self.metadata_prefix}-verification.json"


TARGETS = (
    ReleaseTarget(
        name="windows-x86_64",
        platform="windows",
        architecture="x86_64",
        display_name="Windows x86-64",
        runner="windows-2022",
        package_template="Chaft-{version}-Windows-x86_64.zip",
    ),
    ReleaseTarget(
        name="macos-x86_64",
        platform="macos",
        architecture="x86_64",
        display_name="macOS Intel",
        runner="macos-15-intel",
        package_template="Chaft-{version}-macOS-x86_64.dmg",
    ),
    ReleaseTarget(
        name="macos-arm64",
        platform="macos",
        architecture="arm64",
        display_name="macOS Apple Silicon",
        runner="macos-15",
        package_template="Chaft-{version}-macOS-arm64.dmg",
    ),
    ReleaseTarget(
        name="linux-x86_64",
        platform="linux",
        architecture="x86_64",
        display_name="Linux x86-64",
        runner="ubuntu-22.04",
        package_template="Chaft-{version}-Linux-x86_64.AppImage",
    ),
)
TARGET_BY_NAME = {target.name: target for target in TARGETS}
TARGET_NAMES = tuple(target.name for target in TARGETS)
PLATFORMS = ("windows", "macos", "linux")


def normalize_platform(value: object) -> str:
    candidate = str(value or "").strip().lower()
    normalized = PLATFORM_ALIASES.get(candidate, candidate)
    if normalized not in PLATFORMS:
        raise ReleaseTargetError(
            f"unsupported desktop platform {value!r}; expected Linux, macOS, or Windows"
        )
    return normalized


def normalize_architecture(value: object) -> str:
    candidate = str(value or "").strip().lower()
    normalized = ARCHITECTURE_ALIASES.get(candidate)
    if normalized is None:
        raise ReleaseTargetError(
            f"unsupported desktop architecture {value!r}; expected x86_64 or arm64"
        )
    return normalized


def targets_for_platform(platform_name: object) -> tuple[ReleaseTarget, ...]:
    normalized = normalize_platform(platform_name)
    return tuple(target for target in TARGETS if target.platform == normalized)


def target_for(
    platform_name: object,
    architecture: object,
) -> ReleaseTarget:
    normalized_platform = normalize_platform(platform_name)
    normalized_architecture = normalize_architecture(architecture)
    name = f"{normalized_platform}-{normalized_architecture}"
    try:
        return TARGET_BY_NAME[name]
    except KeyError:
        raise ReleaseTargetError(
            "unsupported native desktop release target "
            f"{normalized_platform}/{normalized_architecture}"
        ) from None


def current_platform() -> str:
    return normalize_platform(os.environ.get("RUNNER_OS") or platform.system())


def current_architecture() -> str:
    return normalize_architecture(
        os.environ.get("RUNNER_ARCH") or platform.machine()
    )


def current_target() -> ReleaseTarget:
    return target_for(current_platform(), current_architecture())


def resolve_target(
    *,
    target_name: str | None = None,
    platform_name: str | None = None,
    architecture: str | None = None,
) -> ReleaseTarget:
    """Resolve one exact target without allowing contradictory selectors."""

    if target_name:
        try:
            target = TARGET_BY_NAME[target_name.strip().lower()]
        except KeyError:
            raise ReleaseTargetError(
                f"unsupported desktop release target {target_name!r}; "
                f"expected one of {', '.join(TARGET_NAMES)}"
            ) from None
        if platform_name and normalize_platform(platform_name) != target.platform:
            raise ReleaseTargetError(
                f"target {target.name} contradicts platform {platform_name!r}"
            )
        if architecture and normalize_architecture(architecture) != target.architecture:
            raise ReleaseTargetError(
                f"target {target.name} contradicts architecture {architecture!r}"
            )
        return target

    resolved_platform = (
        normalize_platform(platform_name)
        if platform_name is not None
        else current_platform()
    )
    resolved_architecture = (
        normalize_architecture(architecture)
        if architecture is not None
        else current_architecture()
    )
    return target_for(resolved_platform, resolved_architecture)


def metadata_names(target: ReleaseTarget | str) -> dict[str, str]:
    resolved = (
        TARGET_BY_NAME[target]
        if isinstance(target, str)
        else target
    )
    return resolved.metadata_names


def target_names(targets: Iterable[ReleaseTarget] = TARGETS) -> tuple[str, ...]:
    return tuple(target.name for target in targets)
