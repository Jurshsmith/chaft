#!/usr/bin/env python3
"""Fingerprint the stable compatibility surface of a Qt SDK toolchain."""

from __future__ import annotations

import argparse
import copy
import importlib.util
from pathlib import Path
import sys
import types
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
QT_SDK_SCRIPT = SCRIPT_DIR.parent / "qt" / "build_qt.py"
COMPATIBILITY_DOMAIN = "chaft.qt-sdk.toolchain-compatibility.v1"


def load_qt_sdk() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location(
        "chaft_qt_sdk_for_compatibility", QT_SDK_SCRIPT
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {QT_SDK_SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


qt_sdk = load_qt_sdk()


def compatibility_payload(
    contract: dict[str, Any],
    manifest: dict[str, Any],
    target_name: str,
) -> dict[str, Any]:
    """Return the validated contract with only imageVersion excluded."""
    qt_sdk.validate_toolchain_contract(contract, manifest, target_name)
    compatible_contract = copy.deepcopy(contract)
    del compatible_contract["runner"]["imageVersion"]
    return {
        "domain": COMPATIBILITY_DOMAIN,
        "toolchainContract": compatible_contract,
    }


def fingerprint_payload(payload: dict[str, Any]) -> str:
    return qt_sdk.sha256_bytes(qt_sdk.canonical_json(payload))


def compatibility_fingerprint(
    contract: dict[str, Any],
    manifest: dict[str, Any],
    target_name: str,
) -> str:
    return fingerprint_payload(
        compatibility_payload(contract, manifest, target_name)
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description=(
            "Validate and fingerprint a Qt SDK consumer-compatibility contract"
        )
    )
    commands = result.add_subparsers(dest="command", required=True)
    fingerprint = commands.add_parser(
        "fingerprint",
        help=(
            "print a domain-separated compatibility SHA-256 that excludes "
            "only the hosted-runner image revision"
        ),
    )
    fingerprint.add_argument(
        "--target",
        choices=qt_sdk.SUPPORTED_TARGETS,
        required=True,
        help="exact operating-system and architecture target",
    )
    fingerprint.add_argument(
        "--toolchain-contract",
        type=Path,
        required=True,
        help="full toolchain contract captured by tools/qt/build_qt.py",
    )
    return result


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        manifest = qt_sdk.load_manifest()
        contract = qt_sdk.load_toolchain_contract(
            arguments.toolchain_contract,
            manifest,
            arguments.target,
        )
        if arguments.command != "fingerprint":
            raise AssertionError(f"unhandled command: {arguments.command}")
        print(
            compatibility_fingerprint(
                contract,
                manifest,
                arguments.target,
            )
        )
    except qt_sdk.QtSdkError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
