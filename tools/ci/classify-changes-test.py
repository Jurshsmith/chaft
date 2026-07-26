#!/usr/bin/env python3
"""Tests for the fail-closed CI change classifier."""

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


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).with_name("classify-changes.py")


def load_script() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location("classify_changes", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


classifier = load_script()


def enabled(result: object) -> set[str]:
    return {
        scope
        for scope, value in result.scopes.items()
        if value and scope != "full"
    }


class PathClassificationTests(unittest.TestCase):
    def classify(
        self, *paths: str, event_name: str = "pull_request", ref: str = ""
    ):
        return classifier.classify_paths(paths, event_name=event_name, ref=ref)

    def test_each_public_website_input_is_website_only(self) -> None:
        paths = (
            "apps/website/src/pages/index.astro",
            "guides/public/index.md",
            "README.md",
            "SECURITY.md",
            "CONTRIBUTING.md",
            ".node-version",
            ".nvmrc",
        )
        for path in paths:
            with self.subTest(path=path):
                result = self.classify(path)
                self.assertEqual(enabled(result), {"website"})
                self.assertFalse(result.scopes["full"])

    def test_internal_guides_are_known_documentation_without_expensive_jobs(
        self,
    ) -> None:
        result = self.classify("guides/workspace-admin-policy.md")
        self.assertEqual(enabled(result), set())
        self.assertFalse(result.scopes["full"])
        self.assertEqual(result.unknown_paths, ())

    def test_runtime_security_and_domain_sources_reach_desktop(self) -> None:
        for path in (
            "runtime/src/lib.rs",
            "security/identity/src/lib.rs",
            "domain/types/src/lib.rs",
        ):
            with self.subTest(path=path):
                result = self.classify(path)
                self.assertEqual(
                    enabled(result),
                    {
                        "rust",
                        "rust_test",
                        "rust_smoke",
                        "desktop_contract",
                        "desktop",
                    },
                )

    def test_node_source_reaches_desktop_live_sync(self) -> None:
        result = self.classify("apps/chaft-node/src/main.rs")
        self.assertEqual(
            enabled(result),
            {
                "rust",
                "rust_test",
                "rust_smoke",
                "desktop_contract",
                "desktop",
            },
        )

    def test_cli_source_reaches_desktop_smoke(self) -> None:
        result = self.classify("apps/chaft-cli/src/main.rs")
        self.assertEqual(
            enabled(result),
            {
                "rust",
                "rust_test",
                "rust_smoke",
                "desktop_contract",
                "desktop",
            },
        )

    def test_benchmark_change_has_dedicated_coverage(self) -> None:
        result = self.classify("benchmarks/benches/hot_paths.rs")
        self.assertEqual(enabled(result), {"rust", "benchmark"})

    def test_workspace_test_change_runs_quality_and_tests_without_smokes(self) -> None:
        result = self.classify("tests/protocol-golden/new-vector.json")
        self.assertEqual(enabled(result), {"rust", "rust_test"})

    def test_crate_tests_and_reviewed_contract_fixtures_avoid_desktop(self) -> None:
        paths = (
            "apps/chaft-cli/tests/secret_input_subprocess.rs",
            "bindings/ffi/src/tests.rs",
            "bindings/ffi/ffi-exports.txt",
            "bindings/ffi/ffi-json-contract.snapshot.json",
            "network/direct/tests/direct_peer_sync.rs",
            "network/sync/tests/two_peer_sync.rs",
            "runtime/tests/fixtures/portable-workspace-v2.json",
            "runtime/tests/sync_efficiency.rs",
        )
        for path in paths:
            with self.subTest(path=path):
                result = self.classify(path)
                self.assertEqual(enabled(result), {"rust", "rust_test"})
                self.assertFalse(result.scopes["desktop_contract"])
                self.assertFalse(result.scopes["desktop"])
                self.assertFalse(result.scopes["rust_smoke"])

    def test_uncertain_rust_source_and_build_inputs_remain_desktop_impacting(
        self,
    ) -> None:
        for path in (
            "bindings/ffi/src/snapshot.rs",
            "network/direct/build.rs",
            "runtime/src/runtime_validation.rs",
        ):
            with self.subTest(path=path):
                result = self.classify(path)
                self.assertEqual(
                    enabled(result),
                    {
                        "rust",
                        "rust_test",
                        "rust_smoke",
                        "desktop_contract",
                        "desktop",
                    },
                )

    def test_rust_smoke_script_only_runs_smoke_job(self) -> None:
        result = self.classify("tools/smoke/local-p2p.sh")
        self.assertEqual(enabled(result), {"rust_smoke"})

    def test_visual_smoke_script_also_reaches_desktop(self) -> None:
        result = self.classify("tools/smoke/visual-workspace.sh")
        self.assertEqual(
            enabled(result), {"rust_smoke", "desktop_contract", "desktop"}
        )

    def test_qml_change_avoids_release_packaging(self) -> None:
        result = self.classify(
            "apps/desktop-qt/qml/Chaft/features/timeline/TimelineView.qml"
        )
        self.assertEqual(enabled(result), {"desktop_contract", "desktop"})

    def test_desktop_test_change_runs_once_in_linux_contracts(self) -> None:
        result = self.classify(
            "apps/desktop-qt/tests/check_reactivity_contract.py"
        )
        self.assertEqual(enabled(result), {"desktop_contract"})

    def test_desktop_branding_readme_is_documentation_only(self) -> None:
        result = self.classify(
            "apps/desktop-qt/resources/branding/README.md"
        )
        self.assertEqual(enabled(result), set())
        self.assertFalse(result.scopes["full"])

    def test_desktop_cmake_change_reaches_package_validation(self) -> None:
        result = self.classify("apps/desktop-qt/CMakeLists.txt")
        self.assertEqual(
            enabled(result),
            {"desktop_contract", "desktop", "release_contract", "package"},
        )

    def test_packaging_and_release_tools_avoid_rust_and_debug_desktop(self) -> None:
        for path in (
            "packaging/linux/appimage-tools.lock",
            "tools/desktop/release-metadata.py",
            "tools/desktop/windows-zip-smoke.ps1",
        ):
            with self.subTest(path=path):
                result = self.classify(path)
                self.assertEqual(enabled(result), {"release_contract", "package"})

    def test_desktop_build_script_reaches_debug_and_package_consumers(self) -> None:
        result = self.classify("tools/desktop/build.sh")
        self.assertEqual(
            enabled(result),
            {"desktop", "release_contract", "package"},
        )

    def test_root_cargo_inputs_cover_benchmark_desktop_and_package(self) -> None:
        for path in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            with self.subTest(path=path):
                result = self.classify(path)
                self.assertEqual(
                    enabled(result),
                    {
                        "rust",
                        "rust_test",
                        "rust_smoke",
                        "benchmark",
                        "desktop_contract",
                        "desktop",
                        "release_contract",
                        "package",
                    },
                )

    def test_mixed_website_and_rust_change_is_the_union(self) -> None:
        result = self.classify("README.md", "runtime/src/lib.rs")
        self.assertEqual(
            enabled(result),
            {
                "website",
                "rust",
                "rust_test",
                "rust_smoke",
                "desktop_contract",
                "desktop",
            },
        )

    def test_ci_and_classifier_changes_force_every_scope(self) -> None:
        for path in (
            ".github/workflows/ci.yml",
            "tools/ci/classify-changes.py",
            "Makefile",
        ):
            with self.subTest(path=path):
                result = self.classify(path)
                self.assertEqual(enabled(result), set(classifier.RUNNABLE_SCOPES))
                self.assertTrue(result.scopes["full"])

    def test_unknown_path_forces_every_scope_and_is_reported(self) -> None:
        result = self.classify("new-subsystem/implementation.xyz")
        self.assertEqual(enabled(result), set(classifier.RUNNABLE_SCOPES))
        self.assertTrue(result.scopes["full"])
        self.assertEqual(
            result.unknown_paths, ("new-subsystem/implementation.xyz",)
        )
        self.assertIn("unknown paths require full coverage", result.reasons)

    def test_main_push_adds_benchmark_and_release_coverage(self) -> None:
        result = self.classify(
            "README.md",
            event_name="push",
            ref="refs/heads/main",
        )
        self.assertEqual(
            enabled(result),
            {"website", "benchmark", "release_contract", "package"},
        )
        self.assertFalse(result.scopes["full"])

    def test_non_main_push_has_no_event_override(self) -> None:
        result = self.classify(
            "README.md",
            event_name="push",
            ref="refs/heads/topic",
        )
        self.assertEqual(enabled(result), {"website"})

    def test_nightly_and_manual_events_force_every_scope(self) -> None:
        for event_name in ("schedule", "workflow_dispatch"):
            with self.subTest(event_name=event_name):
                result = self.classify(event_name=event_name)
                self.assertEqual(enabled(result), set(classifier.RUNNABLE_SCOPES))
                self.assertTrue(result.scopes["full"])

    def test_invalid_paths_are_fatal_instead_of_becoming_false(self) -> None:
        for path in (
            "",
            "../README.md",
            "/README.md",
            "apps\\website\\README.md",
            "README.md\npackage=true",
        ):
            with self.subTest(path=path):
                with self.assertRaises(classifier.ClassificationError):
                    self.classify(path)


class GitDiffParsingTests(unittest.TestCase):
    def test_rename_and_delete_retain_all_affected_paths(self) -> None:
        data = (
            b"R100\0README.md\0runtime/src/renamed.rs\0"
            b"D\0packaging/linux/removed.desktop\0"
            b"M\0apps/website/src/pages/index.astro\0"
        )
        paths = classifier.parse_name_status_z(data)
        self.assertEqual(
            paths,
            (
                "README.md",
                "apps/website/src/pages/index.astro",
                "packaging/linux/removed.desktop",
                "runtime/src/renamed.rs",
            ),
        )
        result = classifier.classify_paths(
            paths, event_name="pull_request"
        )
        self.assertEqual(
            enabled(result),
            {
                "website",
                "rust",
                "rust_test",
                "rust_smoke",
                "desktop_contract",
                "desktop",
                "release_contract",
                "package",
            },
        )

    def test_copy_status_also_retains_both_paths(self) -> None:
        paths = classifier.parse_name_status_z(
            b"C087\0guides/public/index.md\0runtime/src/copied.rs\0"
        )
        self.assertEqual(
            paths, ("guides/public/index.md", "runtime/src/copied.rs")
        )

    def test_malformed_name_status_is_fatal(self) -> None:
        for data in (b"R100\0only-one-path\0", b"Q\0README.md\0"):
            with self.subTest(data=data):
                with self.assertRaises(classifier.ClassificationError):
                    classifier.parse_name_status_z(data)

    def test_zero_push_base_requests_full_coverage_without_diff(self) -> None:
        paths, force_full, reason = classifier.resolve_changes(
            repository=ROOT,
            event_name="push",
            event={
                "before": classifier.ZERO_SHA,
                "after": "1" * 40,
            },
            base="",
            head="",
        )
        self.assertEqual(paths, ())
        self.assertTrue(force_full)
        self.assertIn("base SHA", reason)

    def test_missing_pull_request_metadata_is_fatal(self) -> None:
        with self.assertRaises(classifier.ClassificationError):
            classifier.resolve_changes(
                repository=ROOT,
                event_name="pull_request",
                event={},
                base="",
                head="",
            )


class OutputAndInventoryTests(unittest.TestCase):
    def test_cli_writes_github_outputs_and_readable_summary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            stdout = io.StringIO()
            with redirect_stdout(stdout):
                status = classifier.main(
                    [
                        "--event-name",
                        "pull_request",
                        "--github-output",
                        str(output),
                        "--step-summary",
                        str(summary),
                        "README.md",
                    ]
                )
            self.assertEqual(status, 0)
            output_rows = dict(
                line.split("=", 1)
                for line in output.read_text(encoding="utf-8").splitlines()
            )
            self.assertEqual(set(output_rows), set(classifier.SCOPE_NAMES))
            self.assertEqual(output_rows["website"], "true")
            self.assertEqual(output_rows["rust"], "false")
            rendered = summary.read_text(encoding="utf-8")
            self.assertIn("### CI change classification", rendered)
            self.assertIn("| `website` | `true` |", rendered)
            self.assertEqual(stdout.getvalue(), rendered)

    def test_cli_uses_github_environment_output_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--event-name",
                    "pull_request",
                    "README.md",
                ],
                cwd=ROOT,
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env={
                    "PATH": os.environ.get("PATH", ""),
                    "GITHUB_OUTPUT": str(output),
                    "GITHUB_STEP_SUMMARY": str(summary),
                },
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn("website=true\n", output.read_text(encoding="utf-8"))
            self.assertIn(
                "### CI change classification",
                summary.read_text(encoding="utf-8"),
            )

    def test_cli_failure_is_nonzero_and_writes_environment_summary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--event-name",
                    "pull_request",
                    "../README.md",
                ],
                cwd=ROOT,
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env={
                    "PATH": os.environ.get("PATH", ""),
                    "GITHUB_OUTPUT": str(output),
                    "GITHUB_STEP_SUMMARY": str(summary),
                },
            )
            self.assertEqual(completed.returncode, 2)
            self.assertFalse(output.exists())
            self.assertIn(
                "classification failed",
                summary.read_text(encoding="utf-8"),
            )

    def test_event_payload_resolution_uses_pull_request_shas(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(
                ["git", "config", "user.email", "ci@example.invalid"],
                cwd=repo,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "CI Test"],
                cwd=repo,
                check=True,
            )
            (repo / "README.md").write_text("before\n", encoding="utf-8")
            subprocess.run(["git", "add", "README.md"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "-qm", "base"], cwd=repo, check=True)
            base = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=repo,
                check=True,
                text=True,
                stdout=subprocess.PIPE,
            ).stdout.strip()
            (repo / "runtime/src").mkdir(parents=True)
            subprocess.run(
                ["git", "mv", "README.md", "runtime/src/renamed.rs"],
                cwd=repo,
                check=True,
            )
            subprocess.run(
                ["git", "commit", "-qam", "rename"], cwd=repo, check=True
            )
            head = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=repo,
                check=True,
                text=True,
                stdout=subprocess.PIPE,
            ).stdout.strip()
            paths, force_full, reason = classifier.resolve_changes(
                repository=repo,
                event_name="pull_request",
                event={
                    "pull_request": {
                        "base": {"sha": base},
                        "head": {"sha": head},
                    }
                },
                base="",
                head="",
            )
            self.assertEqual(paths, ("README.md", "runtime/src/renamed.rs"))
            self.assertFalse(force_full)
            self.assertEqual(reason, "")

    def test_every_tracked_file_has_an_explicit_classification(self) -> None:
        completed = subprocess.run(
            ["git", "ls-files"],
            cwd=ROOT,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        )
        unknown = [
            path
            for path in completed.stdout.splitlines()
            if not classifier.classify_path(path).recognized
        ]
        self.assertEqual(
            unknown,
            [],
            "tracked paths require explicit classification: "
            + json.dumps(unknown),
        )


if __name__ == "__main__":
    unittest.main()
