#!/usr/bin/env python3
"""Tests for Qt SDK producer/consumer compatibility fingerprints."""

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest


SCRIPT = Path(__file__).with_name("qt-toolchain-compatibility.py")
MANIFEST = SCRIPT.parents[1] / "qt" / "qt-6.8.4.json"
TARGET = "linux-x86_64"
PRODUCER_IMAGE_VERSION = "20260726.241.1"
CONSUMER_IMAGE_VERSION = "20260720.234.2"


def load_script() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location(
        "qt_toolchain_compatibility", SCRIPT
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


compatibility = load_script()


def observed_linux_contract(image_version: str) -> dict[str, object]:
    return {
        "schemaVersion": 2,
        "target": TARGET,
        "platform": "linux",
        "runner": {
            "os": "Linux",
            "architecture": "x86_64",
            "imageOS": "ubuntu22",
            "imageVersion": image_version,
        },
        "tools": {
            "cmake": "cmake version 3.31.6",
            "ninja": "1.13.2",
            "compiler": (
                "gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0"
            ),
            "python": "3.13.3",
        },
    }


def set_path(value: dict[str, object], path: tuple[str, ...], replacement: object):
    cursor = value
    for component in path[:-1]:
        nested = cursor[component]
        if not isinstance(nested, dict):
            raise AssertionError(f"{component} is not an object")
        cursor = nested
    cursor[path[-1]] = replacement


class QtToolchainCompatibilityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest = compatibility.qt_sdk.load_manifest(MANIFEST)
        self.producer = observed_linux_contract(PRODUCER_IMAGE_VERSION)
        self.consumer = observed_linux_contract(CONSUMER_IMAGE_VERSION)

    def test_observed_runner_rollout_keeps_compatibility_only(self) -> None:
        producer_full = compatibility.qt_sdk.toolchain_fingerprint(
            self.producer, self.manifest, TARGET
        )
        consumer_full = compatibility.qt_sdk.toolchain_fingerprint(
            self.consumer, self.manifest, TARGET
        )
        self.assertNotEqual(producer_full, consumer_full)
        self.assertEqual(
            compatibility.compatibility_fingerprint(
                self.producer, self.manifest, TARGET
            ),
            compatibility.compatibility_fingerprint(
                self.consumer, self.manifest, TARGET
            ),
        )

    def test_payload_excludes_only_runner_image_version(self) -> None:
        expected_contract = copy.deepcopy(self.producer)
        del expected_contract["runner"]["imageVersion"]
        self.assertEqual(
            compatibility.compatibility_payload(
                self.producer, self.manifest, TARGET
            ),
            {
                "domain": compatibility.COMPATIBILITY_DOMAIN,
                "toolchainContract": expected_contract,
            },
        )

    def test_every_retained_leaf_affects_the_fingerprint(self) -> None:
        payload = compatibility.compatibility_payload(
            self.producer, self.manifest, TARGET
        )
        baseline = compatibility.fingerprint_payload(payload)
        mutations = {
            ("domain",): "different-domain",
            ("toolchainContract", "schemaVersion"): 3,
            ("toolchainContract", "target"): "macos-arm64",
            ("toolchainContract", "platform"): "macos",
            ("toolchainContract", "runner", "os"): "macOS",
            (
                "toolchainContract",
                "runner",
                "architecture",
            ): "arm64",
            (
                "toolchainContract",
                "runner",
                "imageOS",
            ): "ubuntu24",
            (
                "toolchainContract",
                "tools",
                "cmake",
            ): "cmake version 3.31.7",
            ("toolchainContract", "tools", "ninja"): "1.13.3",
            (
                "toolchainContract",
                "tools",
                "compiler",
            ): "gcc 11.4.1",
            ("toolchainContract", "tools", "python"): "3.13.4",
        }
        for path, replacement in mutations.items():
            with self.subTest(path=".".join(path)):
                changed = copy.deepcopy(payload)
                set_path(changed, path, replacement)
                self.assertNotEqual(
                    compatibility.fingerprint_payload(changed),
                    baseline,
                )

    def test_full_validation_rejects_extra_missing_and_wrong_contracts(self) -> None:
        mutations = (
            (
                "extra root field",
                lambda contract: contract.__setitem__("unexpected", True),
                "unexpected",
            ),
            (
                "extra runner field",
                lambda contract: contract["runner"].__setitem__(
                    "unexpected", True
                ),
                "unexpected",
            ),
            (
                "missing image version",
                lambda contract: contract["runner"].pop("imageVersion"),
                "imageVersion",
            ),
            (
                "wrong platform",
                lambda contract: contract.__setitem__("platform", "macos"),
                "platform mismatch",
            ),
            (
                "wrong target",
                lambda contract: contract.__setitem__(
                    "target", "macos-arm64"
                ),
                "target mismatch",
            ),
            (
                "wrong architecture",
                lambda contract: contract["runner"].__setitem__(
                    "architecture", "arm64"
                ),
                "architecture mismatch",
            ),
            (
                "multiline tool",
                lambda contract: contract["tools"].__setitem__(
                    "compiler", "gcc\nforged"
                ),
                "one non-empty line",
            ),
        )
        for name, mutate, message in mutations:
            with self.subTest(name=name):
                contract = copy.deepcopy(self.producer)
                mutate(contract)
                with self.assertRaisesRegex(
                    compatibility.qt_sdk.QtSdkError, message
                ):
                    compatibility.compatibility_fingerprint(
                        contract, self.manifest, TARGET
                    )

    def test_cli_prints_only_the_compatibility_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            contract_path = Path(temporary) / "contract.json"
            contract_path.write_text(
                json.dumps(self.producer), encoding="utf-8"
            )
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "fingerprint",
                    "--target",
                    TARGET,
                    "--toolchain-contract",
                    str(contract_path),
                ],
                check=True,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        self.assertRegex(result.stdout, r"\A[0-9a-f]{64}\n\Z")
        self.assertEqual(result.stderr, "")
        self.assertEqual(
            result.stdout.strip(),
            compatibility.compatibility_fingerprint(
                self.producer, self.manifest, TARGET
            ),
        )

    def test_cli_fails_closed_on_invalid_json_and_wrong_target(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            contract_path = Path(temporary) / "contract.json"
            contract_path.write_text("{", encoding="utf-8")
            invalid_json = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "fingerprint",
                    "--target",
                    TARGET,
                    "--toolchain-contract",
                    str(contract_path),
                ],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(invalid_json.returncode, 1)
            self.assertEqual(invalid_json.stdout, "")
            self.assertIn("invalid JSON", invalid_json.stderr)

            contract_path.write_text(
                json.dumps(self.producer), encoding="utf-8"
            )
            wrong_target = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "fingerprint",
                    "--target",
                    "macos-arm64",
                    "--toolchain-contract",
                    str(contract_path),
                ],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        self.assertEqual(wrong_target.returncode, 1)
        self.assertEqual(wrong_target.stdout, "")
        self.assertIn("platform mismatch", wrong_target.stderr)


if __name__ == "__main__":
    unittest.main()
