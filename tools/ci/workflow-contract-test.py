#!/usr/bin/env python3
"""Static contracts for path-scoped CI and immutable workflow actions."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import types
import unittest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github/workflows"
CI_PATH = WORKFLOWS / "ci.yml"
WEBSITE_PATH = WORKFLOWS / "website.yml"
RELEASE_INPUTS_PATH = WORKFLOWS / "build-desktop-release-inputs.yml"
DUPLICATE_CHECK = Path(__file__).with_name("check-yaml-duplicates.rb")
REQUIRED_CHECK = Path(__file__).with_name("required-check.py")


def load_required_check() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location(
        "workflow_required_check", REQUIRED_CHECK
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {REQUIRED_CHECK}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


required = load_required_check()


def workflow_job_ids(text: str) -> set[str]:
    jobs = text.split("\njobs:\n", 1)[1]
    return set(re.findall(r"^  ([a-zA-Z0-9_-]+):\s*$", jobs, re.MULTILINE))


def job_block(text: str, job: str) -> str:
    jobs = text.split("\njobs:\n", 1)[1]
    match = re.search(
        rf"^  {re.escape(job)}:\s*$\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\s*$|\Z)",
        jobs,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"workflow job not found: {job}")
    return match.group("body")


class WorkflowYamlTests(unittest.TestCase):
    def test_every_workflow_parses_without_duplicate_keys(self) -> None:
        paths = sorted(WORKFLOWS.glob("*.yml"))
        completed = subprocess.run(
            ["ruby", str(DUPLICATE_CHECK), *(str(path) for path in paths)],
            cwd=ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn(
            f"strict YAML parse passed: {len(paths)} file(s)", completed.stdout
        )

    def test_duplicate_key_detector_rejects_duplicate_mapping_key(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.yml"
            path.write_text("job:\n  runs-on: one\n  runs-on: two\n", encoding="utf-8")
            completed = subprocess.run(
                ["ruby", str(DUPLICATE_CHECK), str(path)],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        self.assertEqual(completed.returncode, 1)
        self.assertIn("duplicate YAML key \"runs-on\"", completed.stderr)

    def test_every_external_action_uses_an_immutable_sha(self) -> None:
        violations = []
        uses_pattern = re.compile(r"^\s*uses:\s*([^\s#]+)", re.MULTILINE)
        immutable = re.compile(r"^[^@]+@[0-9a-f]{40}$")
        for path in sorted(WORKFLOWS.glob("*.yml")):
            for action in uses_pattern.findall(path.read_text(encoding="utf-8")):
                if action.startswith("./"):
                    continue
                if immutable.fullmatch(action) is None:
                    violations.append(f"{path.name}: {action}")
        self.assertEqual(violations, [])


class CiWorkflowContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.ci = CI_PATH.read_text(encoding="utf-8")
        cls.website = WEBSITE_PATH.read_text(encoding="utf-8")
        cls.release_inputs = RELEASE_INPUTS_PATH.read_text(encoding="utf-8")

    def test_job_ids_exactly_match_aggregate_contract(self) -> None:
        expected = {
            required.CLASSIFIER_JOB,
            *required.JOB_SCOPES,
            "required",
        }
        self.assertEqual(workflow_job_ids(self.ci), expected)

        required_job = job_block(self.ci, "required")
        needs = set(re.findall(r"^      - ([a-zA-Z0-9_]+)$", required_job, re.MULTILINE))
        self.assertEqual(needs, expected - {"required"})
        self.assertIn("if: always()", required_job)
        self.assertIn("NEEDS_JSON: ${{ toJSON(needs) }}", required_job)

    def test_ci_triggers_and_concurrency_are_preserved(self) -> None:
        trigger_block = self.ci.split("\npermissions:\n", 1)[0]
        self.assertIn("  pull_request:\n", trigger_block)
        self.assertIn("  push:\n", trigger_block)
        self.assertIn("  schedule:\n", trigger_block)
        self.assertIn('    - cron: "17 3 * * *"\n', trigger_block)
        self.assertIn("  workflow_dispatch:\n", trigger_block)
        self.assertIn("  cancel-in-progress: true\n", self.ci)
        self.assertIn("  contents: read\n", self.ci)
        self.assertIn("  pull-requests: read\n", self.ci)

    def test_website_pr_validation_is_owned_by_ci(self) -> None:
        website_trigger = self.website.split("\npermissions:\n", 1)[0]
        self.assertNotIn("pull_request:", website_trigger)
        website_job = job_block(self.ci, "website")
        self.assertIn("pnpm install --frozen-lockfile", website_job)
        self.assertIn("pnpm validate", website_job)

    def test_rust_work_is_split_without_redundant_all_target_test_compile(
        self,
    ) -> None:
        self.assertNotIn("dorny/paths-filter", self.ci)
        self.assertNotIn("outputs.core", self.ci)
        self.assertNotIn("cargo check --workspace", self.ci)
        quality = job_block(self.ci, "rust_quality")
        ffi_tests = job_block(self.ci, "rust_tests_ffi")
        runtime_tests = job_block(self.ci, "rust_tests_runtime")
        workspace_tests = job_block(self.ci, "rust_tests_workspace")
        benchmark = job_block(self.ci, "benchmark_compile")
        self.assertIn("--all-targets", quality)
        self.assertIn("--exclude chaft-benchmarks", quality)
        self.assertIn("cargo test -p chaft-ffi --locked", ffi_tests)
        self.assertIn("cargo test -p chaft-runtime --locked", runtime_tests)
        self.assertIn("--exclude chaft-benchmarks", workspace_tests)
        self.assertIn("--exclude chaft-ffi", workspace_tests)
        self.assertIn("--exclude chaft-runtime", workspace_tests)
        for tests in (ffi_tests, runtime_tests, workspace_tests):
            self.assertNotIn("--all-targets", tests)
        self.assertIn("cargo clippy -p chaft-benchmarks", benchmark)
        self.assertIn("cargo bench -p chaft-benchmarks", benchmark)

    def test_desktop_stages_and_platform_invariants_are_explicit(self) -> None:
        contracts = job_block(self.ci, "desktop_contracts")
        desktop = job_block(self.ci, "desktop")
        package = job_block(self.ci, "desktop_package")
        clean = job_block(self.ci, "clean_package_smoke")
        self.assertIn("--stage contracts Linux", contracts)
        self.assertNotIn("rustup", contracts)
        self.assertNotIn("cmake", contracts)
        self.assertNotIn("ninja", contracts)
        self.assertIn("--stage debug", desktop)
        self.assertIn('smoke-timeout-ms: "60000"', desktop)
        self.assertIn("--stage package", package)
        self.assertIn("os: ubuntu-22.04", package)
        self.assertIn("compression-level: 0", package)
        self.assertIn("archive: true", package)
        self.assertIn("digest-mismatch: error", clean)
        self.assertIn("libopengl0", clean)
        self.assertIn("libegl1", clean)

    def test_artifact_action_contract_is_full_only_and_digest_strict(self) -> None:
        producer = job_block(self.ci, "artifact_v7_producer")
        consumer = job_block(self.ci, "artifact_v8_consumer")
        self.assertIn("needs.classify.outputs.full == 'true'", producer)
        self.assertIn("needs.classify.outputs.full == 'true'", consumer)
        self.assertIn("artifact-digest", producer)
        self.assertIn("archive: true", producer)
        self.assertIn("digest-mismatch: error", consumer)
        self.assertIn("sha256sum --check SHA256SUMS", consumer)

    def test_release_inputs_run_contracts_once_before_platform_packages(self) -> None:
        contracts = job_block(self.release_inputs, "desktop_contracts")
        build = job_block(self.release_inputs, "build")
        self.assertIn("needs: validate", contracts)
        self.assertIn("runs-on: ubuntu-22.04", contracts)
        self.assertIn("--stage contracts Linux", contracts)
        self.assertNotIn("rustup", contracts)
        self.assertNotIn("cmake", contracts)
        self.assertNotIn("ninja", contracts)
        self.assertIn("      - validate\n      - desktop_contracts", build)
        self.assertIn("--stage package", build)
        self.assertNotIn("Smoke Linux AppImage", build)
        self.assertNotIn(
            'ci-gates.sh "${{ matrix.package-platform }}"',
            build,
        )

    def test_release_input_provenance_and_security_invariants_are_preserved(
        self,
    ) -> None:
        trigger = self.release_inputs.split("\npermissions:\n", 1)[0]
        build = job_block(self.release_inputs, "build")
        self.assertIn("  workflow_dispatch:\n", trigger)
        self.assertIn("  contents: read\n", self.release_inputs)
        self.assertIn("  cancel-in-progress: false\n", self.release_inputs)
        self.assertIn("ref: ${{ needs.validate.outputs.commit }}", build)
        self.assertIn(
            "CHAFT_RELEASE_COMMIT: ${{ needs.validate.outputs.commit }}",
            build,
        )
        self.assertIn(
            "EXPECTED_COMMIT: ${{ needs.validate.outputs.commit }}",
            build,
        )
        self.assertIn("--expected-commit \"$EXPECTED_COMMIT\"", build)
        self.assertIn("--require-clean", build)


class DesktopGateScriptContractTests(unittest.TestCase):
    def test_stage_selection_and_build_reuse_are_explicit(self) -> None:
        gates = (ROOT / "tools/desktop/ci-gates.sh").read_text(encoding="utf-8")
        self.assertIn("stage=all", gates)
        self.assertIn("all|contracts|debug|package", gates)
        self.assertIn('run_contracts\n    run_debug\n    run_package', gates)
        self.assertIn("CHAFT_DESKTOP_SKIP_BUILD=1", gates)
        for name in (
            "empty-workspace-smoke.sh",
            "live-sync-smoke.sh",
            "smoke.sh",
        ):
            script = (ROOT / "tools/desktop" / name).read_text(encoding="utf-8")
            self.assertIn('CHAFT_DESKTOP_SKIP_BUILD:-0}" != "1"', script)

    def test_invalid_or_explicitly_skipped_stage_fails(self) -> None:
        gates = ROOT / "tools/desktop/ci-gates.sh"
        invalid = subprocess.run(
            [str(gates), "--stage", "invalid", "Linux"],
            cwd=ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(invalid.returncode, 2)
        self.assertIn("unsupported desktop CI stage", invalid.stderr)

        skipped = subprocess.run(
            [str(gates), "--stage", "package", "Linux"],
            cwd=ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={
                "PATH": os.environ.get("PATH", ""),
                "CHAFT_DESKTOP_SKIP_PACKAGE": "1",
            },
        )
        self.assertEqual(skipped.returncode, 2)
        self.assertIn("cannot be skipped", skipped.stderr)


if __name__ == "__main__":
    unittest.main()
