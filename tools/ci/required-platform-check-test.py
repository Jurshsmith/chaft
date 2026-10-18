#!/usr/bin/env python3
"""Exercise native pipeline selection and failure propagation independently."""

import importlib.util
import itertools
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest


SCRIPT = Path(__file__).with_name("required-platform-check.py")
spec = importlib.util.spec_from_file_location("required_platform_check", SCRIPT)
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


def fixture(target="linux-x86_64", contracts=False, desktop=True, package=False):
    inputs = {"target": target, "run-contracts": contracts, "run-desktop": desktop, "run-package": package}
    required = {"sdk": True, "contracts": contracts, "debug": desktop, "package": package, "smoke": package}
    return {name: {"result": "success" if enabled else "skipped"} for name, enabled in required.items()}, inputs


class PlatformCheckTests(unittest.TestCase):
    def test_every_valid_platform_and_stage_combination(self):
        for target in policy.TARGETS:
            for contracts, desktop, package in itertools.product((False, True), repeat=3):
                if not any((contracts, desktop, package)) or (contracts and target != "linux-x86_64"):
                    continue
                with self.subTest(target=target, contracts=contracts, desktop=desktop, package=package):
                    self.assertEqual(policy.evaluate(*fixture(target, contracts, desktop, package)), [])

    def test_every_required_stage_rejects_failure_cancellation_and_skip(self):
        for stage in ("sdk", "contracts", "debug", "package", "smoke"):
            for result in ("failure", "cancelled", "skipped", "unknown", None, {}):
                with self.subTest(stage=stage, result=result):
                    needs, inputs = fixture(contracts=True, package=True)
                    needs[stage]["result"] = result
                    errors = policy.evaluate(needs, inputs)
                    self.assertEqual(len(errors), 1)
                    self.assertIn(stage, errors[0])

    def test_disabled_stages_allow_success_but_reject_failures(self):
        for stage in ("contracts", "package", "smoke"):
            for result, passes in (("success", True), ("skipped", True), ("failure", False), ("cancelled", False)):
                with self.subTest(stage=stage, result=result):
                    needs, inputs = fixture()
                    needs[stage]["result"] = result
                    self.assertEqual(not policy.evaluate(needs, inputs), passes)

    def test_missing_or_unmapped_dependencies_fail(self):
        for missing in policy.STAGE_INPUTS:
            with self.subTest(missing=missing):
                needs, inputs = fixture()
                del needs[missing]
                with self.assertRaises(ValueError):
                    policy.evaluate(needs, inputs)
        needs, inputs = fixture()
        needs["extra"] = {"result": "success"}
        with self.assertRaises(ValueError):
            policy.evaluate(needs, inputs)

    def test_malformed_dependencies_fail(self):
        for entry in ({}, None, "success"):
            needs, inputs = fixture()
            needs["sdk"] = entry
            self.assertTrue(policy.evaluate(needs, inputs))

    def test_stage_inputs_are_strict_booleans(self):
        for flag in ("run-contracts", "run-desktop", "run-package"):
            for value in ("true", "false", None, 1):
                with self.subTest(flag=flag, value=value):
                    needs, inputs = fixture()
                    inputs[flag] = value
                    with self.assertRaises(ValueError):
                        policy.evaluate(needs, inputs)

    def test_empty_workload_and_non_linux_contracts_fail(self):
        for args in (("linux-x86_64", False, False, False), ("windows-x86_64", True, True, True)):
            with self.assertRaises(ValueError):
                policy.evaluate(*fixture(*args))

    def test_invalid_targets_and_non_objects_fail(self):
        for target in ("unknown", None, []):
            needs, inputs = fixture(target=target)
            with self.assertRaises(ValueError):
                policy.evaluate(needs, inputs)
        for needs, inputs in (([], {}), ({}, [])):
            with self.assertRaises(ValueError):
                policy.evaluate(needs, inputs)

    def test_cli_uses_real_json_boolean_inputs_and_rejects_invalid_json(self):
        needs, inputs = fixture(package=True)
        environment = dict(os.environ, NEEDS_JSON=json.dumps(needs), PLATFORM_INPUTS_JSON=json.dumps(inputs))
        environment.pop("GITHUB_STEP_SUMMARY", None)
        result = subprocess.run([sys.executable, str(SCRIPT)], env=environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Every requested platform stage passed", result.stdout)
        environment["NEEDS_JSON"] = "not-json"
        result = subprocess.run([sys.executable, str(SCRIPT)], env=environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)


if __name__ == "__main__":
    unittest.main()
