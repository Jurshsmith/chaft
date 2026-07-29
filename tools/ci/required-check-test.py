#!/usr/bin/env python3
"""Truth-table tests for the stable required CI check."""

from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest


SCRIPT = Path(__file__).with_name("required-check.py")


def load_script() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location("required_check", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


required = load_script()


def needs_fixture(
    enabled_scopes: set[str] | None = None,
) -> dict[str, object]:
    enabled = enabled_scopes or set()
    outputs = {
        scope: "true" if scope in enabled else "false"
        for scope in required.SCOPE_NAMES
    }
    needs: dict[str, object] = {
        required.CLASSIFIER_JOB: {
            "result": "success",
            "outputs": outputs,
        }
    }
    for job, scopes in required.JOB_SCOPES.items():
        needs[job] = {
            "result": "success" if any(scope in enabled for scope in scopes) else "skipped",
            "outputs": {},
        }
    return needs


class RequiredTruthTableTests(unittest.TestCase):
    def test_all_legitimate_skips_pass(self) -> None:
        evaluation = required.evaluate_needs(needs_fixture())
        self.assertTrue(evaluation.passed, evaluation.errors)

    def test_each_scope_passes_when_mapped_jobs_succeed(self) -> None:
        for scope in required.RUNNABLE_SCOPES:
            with self.subTest(scope=scope):
                evaluation = required.evaluate_needs(needs_fixture({scope}))
                self.assertTrue(evaluation.passed, evaluation.errors)
                self.assertTrue(
                    any(job.expected for job in evaluation.jobs),
                    f"{scope} has no mapped required job",
                )

    def test_full_run_requires_and_accepts_every_job_success(self) -> None:
        enabled = set(required.SCOPE_NAMES)
        evaluation = required.evaluate_needs(needs_fixture(enabled))
        self.assertTrue(evaluation.passed, evaluation.errors)
        self.assertTrue(all(job.expected for job in evaluation.jobs))

    def test_artifact_contract_jobs_are_required_only_for_full_runs(self) -> None:
        scoped = required.evaluate_needs(needs_fixture({"rust"}))
        artifact_jobs = {
            job.job: job for job in scoped.jobs if job.job.startswith("artifact_")
        }
        self.assertEqual(set(artifact_jobs), {"artifact_v7_producer", "artifact_v8_consumer"})
        self.assertTrue(all(not job.expected for job in artifact_jobs.values()))

        needs = needs_fixture(set(required.SCOPE_NAMES))
        needs["artifact_v8_consumer"]["result"] = "skipped"
        full = required.evaluate_needs(needs)
        self.assertFalse(full.passed)
        self.assertTrue(
            any("artifact_v8_consumer" in error for error in full.errors)
        )

    def test_enabled_job_result_truth_table(self) -> None:
        for result, expected_pass in (
            ("success", True),
            ("skipped", False),
            ("failure", False),
            ("cancelled", False),
        ):
            with self.subTest(result=result):
                needs = needs_fixture({"rust"})
                needs["rust_quality"]["result"] = result
                evaluation = required.evaluate_needs(needs)
                self.assertEqual(
                    evaluation.passed,
                    expected_pass,
                    evaluation.errors,
                )

    def test_disabled_job_result_truth_table(self) -> None:
        for result, expected_pass in (
            ("success", True),
            ("skipped", True),
            ("failure", False),
            ("cancelled", False),
        ):
            with self.subTest(result=result):
                needs = needs_fixture()
                needs["rust_quality"]["result"] = result
                evaluation = required.evaluate_needs(needs)
                self.assertEqual(
                    evaluation.passed,
                    expected_pass,
                    evaluation.errors,
                )

    def test_desktop_contract_job_is_enabled_by_either_desktop_scope(self) -> None:
        for scope in ("desktop_contract", "desktop"):
            with self.subTest(scope=scope):
                needs = needs_fixture({scope})
                needs["desktop_contracts"]["result"] = "skipped"
                evaluation = required.evaluate_needs(needs)
                self.assertFalse(evaluation.passed)
                self.assertTrue(
                    any("desktop_contracts" in error for error in evaluation.errors)
                )

    def test_qt_sdk_provisioning_matches_platform_consumers(self) -> None:
        contracts = needs_fixture({"desktop_contract"})
        self.assertEqual(contracts["qt_sdk_linux"]["result"], "success")
        self.assertEqual(
            contracts["qt_sdk_macos_x86_64"]["result"], "skipped"
        )
        self.assertEqual(
            contracts["qt_sdk_macos_arm64"]["result"], "skipped"
        )
        self.assertEqual(contracts["qt_sdk_windows"]["result"], "skipped")

        for scope in ("desktop", "package"):
            with self.subTest(scope=scope):
                needs = needs_fixture({scope})
                for job in (
                    "qt_sdk_linux",
                    "qt_sdk_macos_x86_64",
                    "qt_sdk_macos_arm64",
                    "qt_sdk_windows",
                ):
                    self.assertEqual(needs[job]["result"], "success")
                    needs[job]["result"] = "skipped"
                    evaluation = required.evaluate_needs(needs)
                    self.assertFalse(evaluation.passed)
                    self.assertTrue(any(job in error for error in evaluation.errors))
                    needs[job]["result"] = "success"

    def test_macos_local_source_cancellation_fails_desktop_and_package(
        self,
    ) -> None:
        for scope in ("desktop", "package"):
            for result in ("skipped", "failure", "cancelled"):
                with self.subTest(scope=scope, result=result):
                    needs = needs_fixture({scope})
                    needs["macos_local_source"]["result"] = result
                    evaluation = required.evaluate_needs(needs)
                    self.assertFalse(evaluation.passed)
                    self.assertTrue(
                        any(
                            "macos_local_source" in error
                            for error in evaluation.errors
                        )
                    )

    def test_each_native_macos_lane_fails_closed_when_cancelled(self) -> None:
        macos_jobs = (
            "macos_local_source",
            "qt_sdk_macos_x86_64",
            "qt_sdk_macos_arm64",
            "desktop_package",
            "clean_package_smoke",
        )
        for job in macos_jobs:
            with self.subTest(job=job):
                needs = needs_fixture({"package"})
                needs[job]["result"] = "cancelled"
                evaluation = required.evaluate_needs(needs)
                self.assertFalse(evaluation.passed)
                self.assertTrue(
                    any(job in error for error in evaluation.errors)
                )

    def test_release_contract_job_is_enabled_by_package_scope(self) -> None:
        needs = needs_fixture({"package"})
        needs["release_contracts"]["result"] = "skipped"
        evaluation = required.evaluate_needs(needs)
        self.assertFalse(evaluation.passed)
        self.assertTrue(
            any("release_contracts" in error for error in evaluation.errors)
        )

    def test_classifier_must_succeed(self) -> None:
        for result in ("failure", "cancelled", "skipped"):
            with self.subTest(result=result):
                needs = needs_fixture()
                needs["classify"]["result"] = result
                evaluation = required.evaluate_needs(needs)
                self.assertFalse(evaluation.passed)
                self.assertTrue(
                    any("classifier must succeed" in error for error in evaluation.errors)
                )

    def test_missing_scope_output_fails(self) -> None:
        needs = needs_fixture()
        del needs["classify"]["outputs"]["rust"]
        evaluation = required.evaluate_needs(needs)
        self.assertFalse(evaluation.passed)
        self.assertTrue(
            any("classifier output 'rust'" in error for error in evaluation.errors)
        )

    def test_non_boolean_scope_output_fails(self) -> None:
        for value in ("yes", "", True, None, [], {}, ["true"]):
            with self.subTest(value=value):
                needs = needs_fixture()
                needs["classify"]["outputs"]["rust"] = value
                evaluation = required.evaluate_needs(needs)
                self.assertFalse(evaluation.passed)

    def test_extra_classifier_output_fails(self) -> None:
        needs = needs_fixture()
        needs["classify"]["outputs"]["unreviewed_scope"] = "false"
        evaluation = required.evaluate_needs(needs)
        self.assertFalse(evaluation.passed)
        self.assertTrue(
            any("unmapped output" in error for error in evaluation.errors)
        )

    def test_all_rust_test_groups_are_required_together(self) -> None:
        rust_test_jobs = {
            job
            for job, scopes in required.JOB_SCOPES.items()
            if scopes == ("rust_test",)
        }
        self.assertEqual(
            rust_test_jobs,
            {
                "rust_tests_ffi",
                "rust_tests_runtime",
                "rust_tests_workspace",
            },
        )
        for failed_job in rust_test_jobs:
            with self.subTest(job=failed_job):
                needs = needs_fixture({"rust_test"})
                needs[failed_job]["result"] = "skipped"
                evaluation = required.evaluate_needs(needs)
                self.assertFalse(evaluation.passed)
                self.assertTrue(
                    any(failed_job in error for error in evaluation.errors)
                )

    def test_inconsistent_full_output_fails(self) -> None:
        needs = needs_fixture()
        needs["classify"]["outputs"]["full"] = "true"
        evaluation = required.evaluate_needs(needs)
        self.assertFalse(evaluation.passed)
        self.assertTrue(
            any("'full' is true" in error for error in evaluation.errors)
        )

    def test_missing_job_fails_even_when_its_scope_is_disabled(self) -> None:
        needs = needs_fixture()
        del needs["benchmark_compile"]
        evaluation = required.evaluate_needs(needs)
        self.assertFalse(evaluation.passed)
        self.assertTrue(
            any("missing job" in error for error in evaluation.errors)
        )

    def test_unmapped_dependency_fails(self) -> None:
        needs = needs_fixture()
        needs["new_unreviewed_job"] = {"result": "success", "outputs": {}}
        evaluation = required.evaluate_needs(needs)
        self.assertFalse(evaluation.passed)
        self.assertTrue(
            any("unmapped job" in error for error in evaluation.errors)
        )

    def test_missing_or_unknown_result_fails(self) -> None:
        for value in (None, "", "neutral", "timed_out"):
            with self.subTest(value=value):
                needs = needs_fixture()
                if value is None:
                    del needs["website"]["result"]
                else:
                    needs["website"]["result"] = value
                evaluation = required.evaluate_needs(needs)
                self.assertFalse(evaluation.passed)


class RequiredOutputTests(unittest.TestCase):
    def test_cli_writes_passing_output_and_summary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            stdout = io.StringIO()
            with redirect_stdout(stdout):
                status = required.main(
                    [
                        "--needs-json",
                        json.dumps(needs_fixture({"website"})),
                        "--github-output",
                        str(output),
                        "--step-summary",
                        str(summary),
                    ]
                )
            self.assertEqual(status, 0)
            self.assertEqual(output.read_text(encoding="utf-8"), "required=true\n")
            rendered = summary.read_text(encoding="utf-8")
            self.assertIn("Overall result: **pass**", rendered)
            self.assertIn("| `website` |", rendered)
            self.assertEqual(stdout.getvalue(), rendered)

    def test_cli_uses_github_environment_inputs_and_output_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            completed = subprocess.run(
                [sys.executable, str(SCRIPT)],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env={
                    "PATH": os.environ.get("PATH", ""),
                    "NEEDS_JSON": json.dumps(needs_fixture({"website"})),
                    "GITHUB_OUTPUT": str(output),
                    "GITHUB_STEP_SUMMARY": str(summary),
                },
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(output.read_text(encoding="utf-8"), "required=true\n")
            self.assertIn(
                "Overall result: **pass**",
                summary.read_text(encoding="utf-8"),
            )

    def test_cli_writes_failing_output_and_summary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            needs = needs_fixture({"rust_test"})
            needs["rust_tests_ffi"]["result"] = "skipped"
            with redirect_stdout(io.StringIO()):
                status = required.main(
                    [
                        "--needs-json",
                        json.dumps(needs),
                        "--github-output",
                        str(output),
                        "--step-summary",
                        str(summary),
                    ]
                )
            self.assertEqual(status, 1)
            self.assertEqual(output.read_text(encoding="utf-8"), "required=false\n")
            rendered = summary.read_text(encoding="utf-8")
            self.assertIn("Overall result: **fail**", rendered)
            self.assertIn("rust_tests_ffi", rendered)

    def test_invalid_json_fails_closed_and_still_emits_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            with redirect_stdout(io.StringIO()):
                status = required.main(
                    [
                        "--needs-json",
                        "{not-json",
                        "--github-output",
                        str(output),
                        "--step-summary",
                        str(summary),
                    ]
                )
            self.assertEqual(status, 1)
            self.assertEqual(output.read_text(encoding="utf-8"), "required=false\n")
            self.assertIn(
                "needs JSON is invalid",
                summary.read_text(encoding="utf-8"),
            )

    def test_needs_file_is_supported_for_local_reproduction(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            needs_path = Path(directory) / "needs.json"
            needs_path.write_text(
                json.dumps(needs_fixture()), encoding="utf-8"
            )
            stdout = io.StringIO()
            with redirect_stdout(stdout):
                status = required.main(["--needs-file", str(needs_path)])
            self.assertEqual(status, 0)
            self.assertIn("Overall result: **pass**", stdout.getvalue())


if __name__ == "__main__":
    unittest.main()
