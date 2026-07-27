#!/usr/bin/env python3
"""Focused tests for explicit unsigned-canary packaged-smoke receipts."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
SCRIPT = TOOLS / "generate-unsigned-canary-receipt.py"
SPEC = importlib.util.spec_from_file_location("unsigned_canary_generator", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)
import unsigned_canary_policy as policy  # noqa: E402


class UnsignedCanaryReceiptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="chaft-unsigned-canary-receipt-test-"
        )
        self.root = Path(self.temporary.name)
        self.package = (
            self.root / "Chaft-0.1.0-canary.1-Windows-x86_64.zip"
        )
        self.package.write_bytes(b"reviewed canary package bytes")
        self.output = (
            self.root
            / policy.RECEIPT_FILENAMES["windows"]
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def arguments(self, **overrides: object) -> dict[str, object]:
        values: dict[str, object] = {
            "platform": "windows",
            "package": self.package,
            "output": self.output,
            "version": "0.1.0-canary.1",
            "tag": "v0.1.0-canary.1",
            "commit": "a" * 40,
            "repository": "Jurshsmith/chaft",
            "release_id": 41,
            "asset_id": 73,
            "workflow_run_id": 101,
            "workflow_run_attempt": 2,
            "runner_os": "Windows",
            "runner_arch": "X64",
            "smoke_command": "tools/desktop/windows-zip-smoke.ps1",
            "verified_at": "2026-07-26T12:34:56+01:00",
        }
        values.update(overrides)
        return values

    def test_generates_explicit_unsigned_native_smoke_receipt(self) -> None:
        receipt = generator.generate_receipt(**self.arguments())
        self.assertEqual(receipt["schemaVersion"], policy.SCHEMA_VERSION)
        self.assertEqual(receipt["signingStatus"], "unsigned-canary")
        self.assertEqual(receipt["signatureVerification"], "not-performed")
        self.assertEqual(
            receipt["signatureAndNotarization"],
            policy.SIGNATURE_AND_NOTARIZATION["windows"],
        )
        self.assertIs(receipt["productionEligible"], False)
        self.assertEqual(receipt["status"], "passed")
        self.assertEqual(receipt["verifiedAt"], "2026-07-26T11:34:56Z")
        self.assertEqual(receipt["architecture"], "x86_64")
        self.assertEqual(receipt["release"], {"id": 41})
        self.assertEqual(receipt["asset"]["id"], 73)
        self.assertEqual(receipt["asset"]["filename"], self.package.name)
        self.assertEqual(receipt["asset"]["sizeBytes"], self.package.stat().st_size)
        self.assertIn("Do not use", receipt["warning"])
        self.assertEqual(json.loads(self.output.read_text()), receipt)

    def test_rejects_stable_or_malformed_canary_versions(self) -> None:
        for version in (
            "0.1.0",
            "0.1.0-preview.1",
            "0.1.0-canary.0",
            "0.1.0-canary.01",
            "0.1.0-canary.1+build",
        ):
            with self.subTest(version=version):
                with self.assertRaises(policy.UnsignedCanaryPolicyError):
                    generator.generate_receipt(
                        **self.arguments(
                            version=version,
                            tag=f"v{version}",
                            output=self.root
                            / f"{version.replace('+', '-')}.json",
                        )
                    )

    def test_rejects_non_native_runner_and_wrong_package_format(self) -> None:
        with self.assertRaisesRegex(
            policy.UnsignedCanaryPolicyError, "runner OS"
        ):
            generator.generate_receipt(**self.arguments(runner_os="Linux"))
        wrong_package = self.root / "Chaft-0.1.0-canary.1-Windows-x86_64.dmg"
        wrong_package.write_bytes(b"not Windows")
        with self.assertRaisesRegex(
            policy.UnsignedCanaryPolicyError, "supported windows package"
        ):
            generator.generate_receipt(
                **self.arguments(package=wrong_package)
            )

    def test_rejects_existing_or_incorrectly_named_output(self) -> None:
        wrong = self.root / "receipt.json"
        with self.assertRaisesRegex(
            policy.UnsignedCanaryPolicyError, "must be named"
        ):
            generator.generate_receipt(**self.arguments(output=wrong))
        self.output.write_text("existing", encoding="utf-8")
        with self.assertRaisesRegex(
            policy.UnsignedCanaryPolicyError, "already exists"
        ):
            generator.generate_receipt(**self.arguments())

    def test_macos_discloses_native_inspected_ad_hoc_bundle(self) -> None:
        package = self.root / "Chaft-0.1.0-canary.1-macOS-x86_64.dmg"
        package.write_bytes(b"reviewed macOS canary bytes")
        receipt = generator.generate_receipt(
            **self.arguments(
                platform="macos",
                package=package,
                output=self.root / policy.RECEIPT_FILENAMES["macos"],
                runner_os="macOS",
                smoke_command=(
                    "tools/desktop/macos-dmg-smoke.sh + "
                    "tools/desktop/macos-unsigned-canary-smoke.sh"
                ),
            )
        )
        self.assertEqual(receipt["signatureVerification"], "native-inspected")
        self.assertEqual(
            receipt["signatureAndNotarization"]["appleCodeSigning"],
            "ad-hoc",
        )
        self.assertEqual(
            receipt["signatureAndNotarization"]["appleNotarization"],
            "not-performed",
        )

    def test_validator_rejects_forged_signing_claims_and_package_bytes(self) -> None:
        receipt = generator.generate_receipt(**self.arguments())
        receipt["signingStatus"] = "signed"
        with self.assertRaisesRegex(
            policy.UnsignedCanaryPolicyError, "unsigned-canary"
        ):
            policy.validate_receipt_document(receipt)

        receipt = json.loads(self.output.read_text(encoding="utf-8"))
        receipt["asset"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(
            policy.UnsignedCanaryPolicyError, "expected package bytes"
        ):
            policy.validate_receipt_document(
                receipt,
                expected_package=policy.fingerprint_file(self.package),
            )


if __name__ == "__main__":
    unittest.main()
