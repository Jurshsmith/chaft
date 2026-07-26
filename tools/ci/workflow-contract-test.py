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
GIT_ATTRIBUTES_PATH = ROOT / ".gitattributes"
WORKFLOWS = ROOT / ".github/workflows"
CI_PATH = WORKFLOWS / "ci.yml"
WEBSITE_PATH = WORKFLOWS / "website.yml"
RELEASE_INPUTS_PATH = WORKFLOWS / "build-desktop-release-inputs.yml"
PROMOTION_PATH = WORKFLOWS / "promote-desktop-release.yml"
CACHE_CLEANUP_PATH = WORKFLOWS / "cleanup-pull-request-caches.yml"
DUPLICATE_CHECK = Path(__file__).with_name("check-yaml-duplicates.rb")
REQUIRED_CHECK = Path(__file__).with_name("required-check.py")
RUST_CACHE_ACTION = (
    "Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4"
)
QT_CACHE_ACTION = (
    "actions/cache@27d5ce7f107fe9357f9df03efb73ab90386fccae"
)
QT_CACHE_RESTORE_ACTION = (
    "actions/cache/restore@27d5ce7f107fe9357f9df03efb73ab90386fccae"
)
QT_CACHE_KEY = "chaft-qt-sdk-${{ steps.qt-sdk.outputs.identity }}"
LINUX_DEPENDENCIES = "tools/qt/install-linux-dependencies.sh"
LINUX_DEPENDENCIES_PATH = ROOT / LINUX_DEPENDENCIES
LINUX_PACKAGE_DEPENDENCIES = (
    "tools/desktop/install-linux-package-dependencies.sh"
)
LINUX_PACKAGE_DEPENDENCIES_PATH = ROOT / LINUX_PACKAGE_DEPENDENCIES
QT_SOURCE_BUNDLE = "Chaft-Qt-6.8.4-corresponding-source.zip"
QT_SOURCE_CHECKSUM = f"{QT_SOURCE_BUNDLE}.sha256"
MAIN_CACHE_WRITER = (
    "${{ github.event_name != 'pull_request' && "
    "github.ref == 'refs/heads/main' }}"
)
RUST_VERSION = "1.97.1"


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


def action_inputs(block: str, action: str) -> dict[str, str]:
    match = re.search(
        rf"^        uses: {re.escape(action)}(?:\s+#.*)?$\n"
        r"^        with:$\n"
        r"(?P<inputs>(?:^          [a-z][a-z-]*:.*$\n?)+)",
        block,
        re.MULTILINE,
    )
    if match is None:
        raise AssertionError(f"action inputs not found: {action}")

    inputs = {}
    for line in match.group("inputs").splitlines():
        key, value = line.strip().split(":", 1)
        inputs[key] = value.strip()
    return inputs


class WorkflowYamlTests(unittest.TestCase):
    def test_checkout_text_is_canonical_across_runner_platforms(self) -> None:
        self.assertEqual(
            GIT_ATTRIBUTES_PATH.read_text(encoding="utf-8"),
            "* text=auto eol=lf\n",
        )

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

    def test_exact_rust_toolchain_is_consistent(self) -> None:
        workflows = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted(WORKFLOWS.glob("*.yml"))
        )
        installs = re.findall(r"rustup toolchain install ([0-9.]+)", workflows)
        defaults = re.findall(r"rustup default ([0-9.]+)", workflows)
        self.assertGreater(len(installs), 0)
        self.assertEqual(set(installs), {RUST_VERSION})
        self.assertEqual(defaults, installs)
        self.assertIn(
            f'rust-version = "{RUST_VERSION}"',
            (ROOT / "Cargo.toml").read_text(encoding="utf-8"),
        )
        self.assertIn(
            f'channel = "{RUST_VERSION}"',
            (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"),
        )

    def test_linux_qt_dependency_profiles_cover_each_runner_role(self) -> None:
        def packages(
            profile: str,
            installer: Path = LINUX_DEPENDENCIES_PATH,
        ) -> list[str]:
            completed = subprocess.run(
                [str(installer), "list", profile],
                cwd=ROOT,
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            result = completed.stdout.splitlines()
            self.assertEqual(
                len(result),
                len(set(result)),
                f"{profile} contains duplicate packages",
            )
            return result

        consumer = set(packages("sdk-consumer"))
        sdk_build = set(packages("sdk-build"))
        base_desktop_package = set(packages("desktop-package"))
        base_release_package = set(packages("release-package"))
        desktop_package = set(
            packages(
                "desktop-package",
                LINUX_PACKAGE_DEPENDENCIES_PATH,
            )
        )
        release_package = set(
            packages(
                "release-package",
                LINUX_PACKAGE_DEPENDENCIES_PATH,
            )
        )
        runtime = set(packages("appimage-runtime"))

        self.assertEqual(
            consumer,
            {"build-essential", "cmake", "libglvnd-dev", "ninja-build"},
        )
        self.assertTrue(
            {
                "libegl1-mesa-dev",
                "libgl1-mesa-dev",
                "libwayland-dev",
                "libx11-dev",
                "libxcb1-dev",
            }.issubset(sdk_build)
        )
        packaging = {"appstream", "desktop-file-utils", "patchelf"}
        package_host_libraries = {"libxcb-cursor0"}
        self.assertEqual(base_desktop_package, consumer | packaging)
        self.assertEqual(base_release_package, sdk_build | packaging)
        self.assertEqual(
            desktop_package,
            base_desktop_package | package_host_libraries,
        )
        self.assertEqual(
            release_package,
            base_release_package | package_host_libraries,
        )
        self.assertEqual(runtime, {"libegl1", "libglx0", "libopengl0"})
        qt_builder = (ROOT / "tools" / "qt" / "build_qt.py").read_text(
            encoding="utf-8"
        )
        self.assertNotIn(LINUX_PACKAGE_DEPENDENCIES, qt_builder)


class CiWorkflowContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.ci = CI_PATH.read_text(encoding="utf-8")
        cls.website = WEBSITE_PATH.read_text(encoding="utf-8")
        cls.release_inputs = RELEASE_INPUTS_PATH.read_text(encoding="utf-8")
        cls.promotion = PROMOTION_PATH.read_text(encoding="utf-8")
        cls.cache_cleanup = CACHE_CLEANUP_PATH.read_text(encoding="utf-8")

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
        self.assertIn("--component clippy", benchmark)
        self.assertIn("cargo clippy -p chaft-benchmarks", benchmark)
        self.assertIn("cargo bench -p chaft-benchmarks", benchmark)

    def test_rust_caches_share_stable_families_with_single_main_writers(
        self,
    ) -> None:
        expected_ci = {
            "rust_quality": {
                "shared-key": "rust",
                "save-if": MAIN_CACHE_WRITER,
            },
            "rust_tests_ffi": {
                "shared-key": "rust",
                "save-if": '"false"',
            },
            "rust_tests_runtime": {
                "shared-key": "rust",
                "save-if": '"false"',
            },
            "rust_tests_workspace": {
                "shared-key": "rust",
                "save-if": '"false"',
            },
            "rust_smokes": {
                "shared-key": "rust",
                "save-if": '"false"',
            },
            "benchmark_compile": {
                "shared-key": "rust",
                "save-if": '"false"',
            },
            "desktop": {
                "shared-key": "desktop",
                "save-if": '"false"',
            },
            "desktop_package": {
                "shared-key": "desktop",
                "save-if": MAIN_CACHE_WRITER,
            },
        }
        for job, expected in expected_ci.items():
            with self.subTest(workflow="ci", job=job):
                self.assertEqual(
                    action_inputs(job_block(self.ci, job), RUST_CACHE_ACTION),
                    expected,
                )

        self.assertEqual(
            action_inputs(
                job_block(self.release_inputs, "build"),
                RUST_CACHE_ACTION,
            ),
            {
                "shared-key": "desktop",
                "save-if": '"false"',
            },
        )

        all_workflows = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted(WORKFLOWS.glob("*.yml"))
        )
        self.assertEqual(all_workflows.count(f"uses: {RUST_CACHE_ACTION}"), 9)
        self.assertEqual(self.ci.count(f"save-if: {MAIN_CACHE_WRITER}"), 2)
        self.assertNotIn(f"save-if: {MAIN_CACHE_WRITER}", self.release_inputs)

        for job in (
            "rust_quality",
            "rust_tests_ffi",
            "rust_tests_runtime",
            "rust_tests_workspace",
            "rust_smokes",
            "benchmark_compile",
            "desktop_package",
            "build",
        ):
            self.assertNotIn(f"shared-key: {job}", all_workflows)

    def test_cache_writers_reject_pull_request_tag_and_non_main_refs(
        self,
    ) -> None:
        def writer_enabled(event_name: str, ref: str) -> bool:
            return event_name != "pull_request" and ref == "refs/heads/main"

        for event_name in ("push", "schedule", "workflow_dispatch"):
            with self.subTest(event_name=event_name, ref="main"):
                self.assertTrue(
                    writer_enabled(event_name, "refs/heads/main")
                )

        rejected = (
            ("pull_request", "refs/pull/7/merge"),
            ("push", "refs/tags/v0.1.0"),
            ("workflow_dispatch", "refs/tags/v0.1.0"),
            ("workflow_dispatch", "refs/heads/cache-experiment"),
        )
        for event_name, ref in rejected:
            with self.subTest(event_name=event_name, ref=ref):
                self.assertFalse(writer_enabled(event_name, ref))

    def test_closed_pr_cache_cleanup_has_one_least_privilege_trigger(
        self,
    ) -> None:
        trigger = self.cache_cleanup.split("\npermissions:\n", 1)[0]
        self.assertIn(
            "on:\n"
            "  pull_request:\n"
            "    types:\n"
            "      - closed\n",
            trigger,
        )
        self.assertNotIn("pull_request_target", trigger)
        self.assertNotIn("push:", trigger)
        self.assertNotIn("workflow_dispatch:", trigger)

        permissions = self.cache_cleanup.split(
            "\npermissions:\n", 1
        )[1].split("\nconcurrency:\n", 1)[0]
        self.assertEqual(permissions, "  actions: write\n")
        self.assertIn(
            "group: cleanup-pull-request-caches-"
            "${{ github.event.pull_request.number }}",
            self.cache_cleanup,
        )
        self.assertIn("cancel-in-progress: false", self.cache_cleanup)

    def test_closed_pr_cache_cleanup_deletes_only_validated_merge_ref(
        self,
    ) -> None:
        cleanup = job_block(self.cache_cleanup, "cleanup")
        self.assertIn(
            '[[ ! "${PR_NUMBER}" =~ ^[1-9][0-9]*$ ]]',
            cleanup,
        )
        self.assertIn(
            'cache_ref="refs/pull/${PR_NUMBER}/merge"',
            cleanup,
        )
        self.assertEqual(cleanup.count("gh cache delete"), 1)
        self.assertIn("gh cache delete --all \\", cleanup)
        self.assertIn('--ref "${cache_ref}" \\', cleanup)
        self.assertIn('--repo "${GH_REPO}" \\', cleanup)
        self.assertIn("--succeed-on-no-caches", cleanup)
        self.assertNotIn("gh api", cleanup)
        self.assertNotIn("curl ", cleanup)

    def test_closed_pr_cache_cleanup_uses_no_source_or_repository_secrets(
        self,
    ) -> None:
        cleanup = job_block(self.cache_cleanup, "cleanup")
        self.assertNotIn("uses:", cleanup)
        self.assertNotIn("checkout", cleanup.lower())
        self.assertNotIn("secrets.", self.cache_cleanup)
        self.assertIn("GH_TOKEN: ${{ github.token }}", cleanup)
        self.assertIn("GH_REPO: ${{ github.repository }}", cleanup)
        self.assertIn(
            "PR_NUMBER: ${{ github.event.pull_request.number }}",
            cleanup,
        )

    def test_desktop_stages_and_platform_invariants_are_explicit(self) -> None:
        contracts = job_block(self.ci, "desktop_contracts")
        desktop = job_block(self.ci, "desktop")
        package = job_block(self.ci, "desktop_package")
        clean = job_block(self.ci, "clean_package_smoke")
        release_contracts = job_block(self.ci, "release_contracts")
        self.assertIn("--stage contracts Linux", contracts)
        self.assertNotIn("rustup", contracts)
        self.assertIn("--stage debug", desktop)
        self.assertIn('smoke-timeout-ms: "60000"', desktop)
        self.assertIn("--stage package", package)
        self.assertIn("os: ubuntu-22.04", package)
        self.assertIn("compression-level: 0", package)
        self.assertIn("archive: true", package)
        self.assertIn("digest-mismatch: error", clean)
        self.assertIn("timeout-minutes: 15", clean)
        self.assertIn("os: macos-15-intel", clean)
        self.assertIn(
            "artifact-name: chaft-macOS-desktop-release",
            clean,
        )
        self.assertIn(
            "artifact-path: build/clean-macos-package",
            clean,
        )
        self.assertIn(
            "tools/desktop/macos-dmg-smoke.sh build/clean-macos-package",
            clean,
        )
        self.assertLess(
            clean.index("digest-mismatch: error"),
            clean.index(
                "tools/desktop/macos-dmg-smoke.sh build/clean-macos-package"
            ),
        )
        self.assertIn(
            "python3 tools/desktop/macos-dmg-smoke-test.py",
            release_contracts,
        )
        self.assertIn(
            f"{LINUX_DEPENDENCIES} install sdk-consumer",
            contracts,
        )
        self.assertIn(
            f"{LINUX_DEPENDENCIES} install sdk-consumer",
            desktop,
        )
        self.assertIn(
            f"{LINUX_PACKAGE_DEPENDENCIES} install desktop-package",
            package,
        )
        self.assertIn(
            f"{LINUX_DEPENDENCIES} install appimage-runtime",
            clean,
        )
        for block, installer, profile, later_step in (
            (
                contracts,
                LINUX_DEPENDENCIES,
                "sdk-consumer",
                "Match consumer toolchain to provisioned Qt SDK",
            ),
            (
                desktop,
                LINUX_DEPENDENCIES,
                "sdk-consumer",
                "Match consumer toolchain to provisioned Qt SDK",
            ),
            (
                package,
                LINUX_PACKAGE_DEPENDENCIES,
                "desktop-package",
                "Match consumer toolchain to provisioned Qt SDK",
            ),
            (
                clean,
                LINUX_DEPENDENCIES,
                "appimage-runtime",
                "tools/desktop/appimage-smoke.sh",
            ),
        ):
            with self.subTest(profile=profile):
                self.assertLess(
                    block.index(f"{installer} install {profile}"),
                    block.index(later_step),
                )

        self.assertNotIn("sudo apt-get", self.ci)
        self.assertNotIn("sudo apt-get", self.release_inputs)

    def test_windows_desktop_jobs_are_pinned_to_server_2022(self) -> None:
        desktop_workflows = "\n".join(
            (self.ci, self.release_inputs, self.promotion)
        )
        self.assertNotIn("windows-latest", desktop_workflows)
        self.assertEqual(desktop_workflows.count("windows-2022"), 7)

    def test_qt_sdk_is_built_once_per_needed_platform_then_restored_only(
        self,
    ) -> None:
        classify = job_block(self.ci, "classify")
        self.assertIn(
            "python3 tools/qt/build_qt_test.py",
            classify,
        )
        self.assertIn(
            "python3 tools/qt/source_bundle_test.py",
            classify,
        )
        provisioning = {
            "qt_sdk_linux": ("ubuntu-22.04", "linux"),
            "qt_sdk_macos": ("macos-15-intel", "macos"),
            "qt_sdk_windows": ("windows-2022", "windows"),
        }
        for job, (runner, platform) in provisioning.items():
            with self.subTest(job=job):
                block = job_block(self.ci, job)
                self.assertIn(f"runs-on: {runner}", block)
                self.assertIn("timeout-minutes: 120", block)
                self.assertIn(
                    "tools/qt/build_qt.py toolchain-contract",
                    block,
                )
                self.assertIn("tools/qt/build_qt.py toolchain-fingerprint", block)
                self.assertIn("tools/qt/build_qt.py identity", block)
                self.assertIn("--toolchain-contract", block)
                self.assertIn(
                    f"--platform {platform}",
                    block,
                )
                self.assertIn(
                    "if: steps.qt-cache.outputs.cache-hit != 'true'",
                    block,
                )
                self.assertEqual(
                    action_inputs(block, QT_CACHE_ACTION),
                    {
                        "path": "${{ runner.temp }}/chaft-qt-sdk",
                        "key": QT_CACHE_KEY,
                    },
                )

        linux = job_block(self.ci, "qt_sdk_linux")
        self.assertIn("desktop_contract", linux)
        self.assertIn("outputs.desktop", linux)
        self.assertIn("outputs.package", linux)
        self.assertIn(
            f"{LINUX_DEPENDENCIES} install sdk-build",
            linux,
        )
        self.assertLess(
            linux.index(f"{LINUX_DEPENDENCIES} install sdk-build"),
            linux.index("tools/qt/build_qt.py toolchain-contract"),
        )
        for job in ("qt_sdk_macos", "qt_sdk_windows"):
            block = job_block(self.ci, job)
            self.assertNotIn("desktop_contract", block)
            self.assertIn("outputs.desktop", block)
            self.assertIn("outputs.package", block)

        consumers = {
            "desktop_contracts": ("qt_sdk_linux",),
            "desktop": (
                "qt_sdk_linux",
                "qt_sdk_macos",
                "qt_sdk_windows",
            ),
            "desktop_package": (
                "qt_sdk_linux",
                "qt_sdk_macos",
                "qt_sdk_windows",
            ),
        }
        for job, dependencies in consumers.items():
            with self.subTest(consumer=job):
                block = job_block(self.ci, job)
                for dependency in dependencies:
                    self.assertIn(f"      - {dependency}", block)
                    self.assertIn(
                        f"needs.{dependency}.result == 'success'",
                        block,
                    )
                restore = action_inputs(block, QT_CACHE_RESTORE_ACTION)
                self.assertEqual(
                    restore,
                    {
                        "path": "${{ runner.temp }}/chaft-qt-sdk",
                        "key": QT_CACHE_KEY,
                        "fail-on-cache-miss": "true",
                    },
                )
                self.assertNotIn(f"uses: {QT_CACHE_ACTION}", block)
                self.assertIn(
                    "Match consumer toolchain to provisioned Qt SDK",
                    block,
                )
                self.assertIn("toolchain_fingerprint", block)
                self.assertIn(
                    "tools/qt/build_qt.py toolchain-contract",
                    block,
                )
                self.assertIn(
                    "tools/qt/build_qt.py toolchain-fingerprint",
                    block,
                )
                self.assertIn("consumer_fingerprint", block)
                self.assertIn("refusing the cache", block)
                self.assertIn(
                    "${RUNNER_TEMP}/chaft-qt-consumer-toolchain.json",
                    block,
                )
                self.assertIn("--toolchain-contract", block)
                self.assertNotIn("--toolchain-fingerprint", block)
                self.assertLess(
                    block.index("consumer_fingerprint"),
                    block.index(f"uses: {QT_CACHE_RESTORE_ACTION}"),
                )

        self.assertNotIn("jurplel/install-qt-action", self.ci)
        self.assertNotIn("aqtinstall", self.ci)
        self.assertEqual(self.ci.count(f"uses: {QT_CACHE_ACTION}"), 3)
        self.assertEqual(
            self.ci.count(f"uses: {QT_CACHE_RESTORE_ACTION}"),
            3,
        )

    def test_artifact_action_contract_is_full_only_and_digest_strict(self) -> None:
        producer = job_block(self.ci, "artifact_v7_producer")
        consumer = job_block(self.ci, "artifact_v8_consumer")
        self.assertIn("needs.classify.outputs.full == 'true'", producer)
        self.assertIn("needs.classify.outputs.full == 'true'", consumer)
        self.assertIn("artifact-digest", producer)
        self.assertIn("archive: true", producer)
        self.assertIn("digest-mismatch: error", consumer)
        self.assertIn("sha256sum --check SHA256SUMS", consumer)

    def test_release_inputs_run_contracts_once_inside_linux_package_job(
        self,
    ) -> None:
        build = job_block(self.release_inputs, "build")
        self.assertNotIn(
            "desktop_contracts",
            workflow_job_ids(self.release_inputs),
        )
        self.assertIn("      - validate", build)
        self.assertIn("      - qt_source_bundle", build)
        self.assertEqual(build.count("--stage contracts Linux"), 1)
        self.assertIn("if: runner.os == 'Linux'", build)
        self.assertIn("--stage package", build)
        self.assertNotIn("Smoke Linux AppImage", build)
        self.assertNotIn(
            'ci-gates.sh "${{ matrix.package-platform }}"',
            build,
        )

    def test_release_inputs_restore_main_qt_cache_and_build_without_saving(
        self,
    ) -> None:
        build = job_block(self.release_inputs, "build")
        self.assertIn("timeout-minutes: 180", build)
        self.assertEqual(
            action_inputs(build, QT_CACHE_RESTORE_ACTION),
            {
                "path": "${{ runner.temp }}/chaft-qt-sdk",
                "key": QT_CACHE_KEY,
            },
        )
        self.assertIn(
            "if: steps.qt-cache.outputs.cache-hit != 'true'",
            build,
        )
        self.assertIn("tools/qt/build_qt.py build", build)
        self.assertIn("tools/qt/build_qt.py verify", build)
        self.assertIn(
            f"{LINUX_PACKAGE_DEPENDENCIES} install release-package",
            build,
        )
        self.assertLess(
            build.index(
                f"{LINUX_PACKAGE_DEPENDENCIES} install release-package"
            ),
            build.index("tools/qt/build_qt.py toolchain-contract"),
        )
        self.assertEqual(
            build.count(
                'rm -rf -- "${RUNNER_TEMP:?RUNNER_TEMP must be non-empty}'
                '/chaft-qt-build"'
            ),
            1,
        )
        self.assertLess(
            build.index("tools/qt/build_qt.py verify"),
            build.index("Remove transient Qt build tree"),
        )
        self.assertLess(
            build.index("Remove transient Qt build tree"),
            build.index("Run platform-independent desktop contracts once"),
        )
        self.assertIn("tools/qt/build_qt.py toolchain-contract", build)
        self.assertIn("--toolchain-contract", build)
        self.assertNotIn(f"uses: {QT_CACHE_ACTION}", self.release_inputs)
        self.assertNotIn("actions/cache/save@", self.release_inputs)
        self.assertNotIn("jurplel/install-qt-action", self.release_inputs)
        self.assertNotIn("aqtinstall", self.release_inputs)

        clean = job_block(self.release_inputs, "clean-package-smoke")
        self.assertIn("timeout-minutes: 15", clean)
        self.assertIn("os: macos-15-intel", clean)
        self.assertIn(
            "artifact-prefix: unsigned-macos-x86_64-release-input",
            clean,
        )
        self.assertIn(
            "tools/desktop/macos-dmg-smoke.sh build/clean-package",
            clean,
        )
        self.assertLess(
            clean.index("digest-mismatch: error"),
            clean.index("tools/desktop/macos-dmg-smoke.sh build/clean-package"),
        )
        self.assertIn(
            f"{LINUX_DEPENDENCIES} install appimage-runtime",
            clean,
        )
        self.assertLess(
            clean.index(
                f"{LINUX_DEPENDENCIES} install appimage-runtime"
            ),
            clean.index("tools/desktop/appimage-smoke.sh"),
        )

    def test_release_input_provenance_and_security_invariants_are_preserved(
        self,
    ) -> None:
        trigger = self.release_inputs.split("\npermissions:\n", 1)[0]
        validate = job_block(self.release_inputs, "validate")
        build = job_block(self.release_inputs, "build")
        self.assertIn("  workflow_dispatch:\n", trigger)
        self.assertIn("  contents: read\n", self.release_inputs)
        self.assertIn("  cancel-in-progress: false\n", self.release_inputs)
        self.assertIn(
            "ref: ${{ github.event.repository.default_branch }}",
            validate,
        )
        self.assertNotIn("ref: ${{ inputs.tag }}", validate)
        self.assertIn(
            'git rev-parse --verify "refs/tags/${RELEASE_TAG}^{commit}"',
            validate,
        )
        self.assertIn(
            'git merge-base --is-ancestor "${tag_commit}" '
            '"${policy_commit}"',
            validate,
        )
        self.assertIn(
            'git worktree add --detach "${release_source}" '
            '"${tag_commit}"',
            validate,
        )
        self.assertIn(
            "python3 tools/desktop/release-version.py",
            validate,
        )
        self.assertIn('--root "${release_source}"', validate)
        self.assertNotIn(
            '"${release_source}/tools/desktop/release-version.py"',
            validate,
        )
        self.assertLess(
            validate.index("git merge-base --is-ancestor"),
            validate.index("git worktree add --detach"),
        )
        self.assertLess(
            validate.index("git worktree add --detach"),
            validate.index("python3 tools/desktop/release-version.py"),
        )
        self.assertIn("ref: ${{ needs.validate.outputs.commit }}", build)
        self.assertIn(
            "CHAFT_RELEASE_COMMIT: ${{ needs.validate.outputs.commit }}",
            build,
        )
        self.assertIn(
            "EXPECTED_COMMIT: ${{ needs.validate.outputs.commit }}",
            build,
        )
        self.assertIn(
            "CHAFT_QT_SOURCE_BUNDLE_SHA256: "
            "${{ needs.qt_source_bundle.outputs.bundle_sha256 }}",
            build,
        )
        self.assertIn("--expected-commit \"$EXPECTED_COMMIT\"", build)
        self.assertIn("--require-clean", build)

    def test_release_inputs_build_qt_source_once_with_exact_layout(
        self,
    ) -> None:
        source = job_block(self.release_inputs, "qt_source_bundle")
        self.assertIn("needs: validate", source)
        self.assertIn("runs-on: ubuntu-22.04", source)
        self.assertNotIn("matrix.", source)
        self.assertIn(
            "ref: ${{ needs.validate.outputs.commit }}",
            source,
        )
        self.assertIn(
            "--tag \"$RELEASE_TAG\"",
            source,
        )
        self.assertIn("--expected-commit HEAD", source)
        self.assertEqual(
            source.count("tools/qt/source_bundle.py create"),
            1,
        )
        self.assertIn("tools/qt/source_bundle.py verify", source)
        self.assertEqual(source.count(QT_SOURCE_CHECKSUM), 4)
        self.assertEqual(
            source.count(QT_SOURCE_BUNDLE)
            - source.count(QT_SOURCE_CHECKSUM),
            3,
        )
        self.assertIn(
            "artifact_digest: ${{ steps.upload.outputs.artifact-digest }}",
            source,
        )
        self.assertIn(
            "bundle_sha256: ${{ steps.source.outputs.bundle_sha256 }}",
            source,
        )
        self.assertIn(
            'echo "bundle_sha256=${bundle_sha256}" >> "${GITHUB_OUTPUT}"',
            source,
        )
        self.assertIn(
            "path: |\n"
            f"            build/qt-corresponding-source/{QT_SOURCE_BUNDLE}\n"
            f"            build/qt-corresponding-source/{QT_SOURCE_CHECKSUM}\n",
            source,
        )
        self.assertIn("retention-days: 7", source)
        self.assertIn("compression-level: 0", source)
        self.assertIn("archive: true", source)
        self.assertNotIn("gh release", source)
        self.assertEqual(
            self.release_inputs.count(
                "tools/qt/source_bundle.py create"
            ),
            1,
        )

    def test_qt_source_artifact_is_clean_verified_and_audited(
        self,
    ) -> None:
        clean = job_block(self.release_inputs, "clean-qt-source-bundle")
        self.assertIn("      - qt_source_bundle", clean)
        self.assertIn(
            "ref: ${{ needs.validate.outputs.commit }}",
            clean,
        )
        self.assertIn("digest-mismatch: error", clean)
        self.assertIn(QT_SOURCE_BUNDLE, clean)
        self.assertIn(QT_SOURCE_CHECKSUM, clean)
        self.assertIn("tools/qt/source_bundle.py verify", clean)
        self.assertNotIn("tools/qt/source_bundle.py create", clean)
        self.assertIn(
            "path: build/clean-qt-source\n"
            "          digest-mismatch: error",
            clean,
        )

        audit = job_block(self.release_inputs, "audit-release-inputs")
        self.assertIn("      - qt_source_bundle", audit)
        self.assertIn("      - clean-qt-source-bundle", audit)
        self.assertIn("digest-mismatch: error", audit)
        self.assertIn(QT_SOURCE_BUNDLE, audit)
        self.assertIn(QT_SOURCE_CHECKSUM, audit)
        self.assertIn("tools/qt/source_bundle.py verify", audit)
        self.assertIn("for platform in linux macos windows", audit)
        self.assertIn("--qt-source-bundle", audit)
        self.assertIn("--qt-source-checksum", audit)
        self.assertIn(
            "CHAFT_QT_SOURCE_BUNDLE_SHA256: "
            "${{ needs.qt_source_bundle.outputs.bundle_sha256 }}",
            audit,
        )
        self.assertIn(
            "path: build/release-input-audit/qt-source\n"
            "          digest-mismatch: error",
            audit,
        )

    def test_promotion_requires_verifies_and_isolates_qt_source_assets(
        self,
    ) -> None:
        prepare = job_block(self.promotion, "prepare")
        self.assertIn(
            "ref: ${{ github.event.repository.default_branch }}",
            prepare,
        )
        self.assertIn("gh release verify-asset", prepare)
        self.assertEqual(
            prepare.count(
                '[.assets[] | select(.name == $filename)] | length'
            ),
            1,
        )
        self.assertIn('for filename in "${bundle}" "${checksum}"', prepare)
        self.assertIn(f'bundle="{QT_SOURCE_BUNDLE}"', prepare)
        self.assertIn(f'checksum="{QT_SOURCE_CHECKSUM}"', prepare)
        self.assertIn(
            "python3 tools/qt/source_bundle.py verify",
            prepare,
        )
        self.assertIn('--source-root "${release_source}"', prepare)
        self.assertIn(
            'git worktree add --detach "${release_source}"',
            prepare,
        )
        self.assertIn(
            'source_assets="${RUNNER_TEMP}/qt-corresponding-source"',
            prepare,
        )
        self.assertEqual(prepare.count('mv -- "${assets}/'), 2)
        authenticate_position = prepare.index("gh release verify-asset")
        verify_position = prepare.index(
            "python3 tools/qt/source_bundle.py verify"
        )
        isolate_position = prepare.index('mv -- "${assets}/${bundle}"')
        stage_position = prepare.index(
            "tools/desktop/stage-website-release-assets.py"
        )
        self.assertLess(authenticate_position, verify_position)
        self.assertLess(verify_position, isolate_position)
        self.assertLess(isolate_position, stage_position)
        self.assertIn("for platform in linux macos windows", prepare)
        self.assertIn(
            "tools/desktop/verify-release-metadata.py release",
            prepare,
        )
        self.assertIn("--qt-source-bundle", prepare)
        self.assertIn("--qt-source-checksum", prepare)
        self.assertIn(
            '--package-dir "${staged}/${platform}-package"',
            prepare,
        )
        cross_check_position = prepare.index(
            "tools/desktop/verify-release-metadata.py release"
        )
        self.assertLess(stage_position, cross_check_position)
        for mutation in (
            "gh release upload",
            "gh release delete",
            "gh release edit",
        ):
            self.assertNotIn(mutation, self.promotion)

    def test_promotion_release_and_signing_invariants_are_preserved(
        self,
    ) -> None:
        trigger = self.promotion.split("\npermissions:\n", 1)[0]
        self.assertIn("  release:\n", trigger)
        self.assertIn("      - published\n", trigger)
        self.assertIn("  workflow_dispatch:\n", trigger)
        self.assertIn("permissions:\n  contents: read\n", self.promotion)
        self.assertIn("cancel-in-progress: false", self.promotion)

        prepare = job_block(self.promotion, "prepare")
        self.assertIn(
            '[[ "$(jq -r \'.immutable // false\' "${release_json}")" '
            '!= "true" ]]',
            prepare,
        )
        self.assertIn('gh release verify "${RELEASE_TAG}"', prepare)
        self.assertIn(
            'tag_commit="$(git rev-parse --verify '
            '"refs/tags/${RELEASE_TAG}^{commit}")"',
            prepare,
        )
        self.assertIn('policy_commit="$(git rev-parse HEAD)"', prepare)
        self.assertIn(
            'git merge-base --is-ancestor "${tag_commit}" '
            '"${policy_commit}"',
            prepare,
        )
        self.assertLess(
            prepare.index('tag_commit="$(git rev-parse'),
            prepare.index("git merge-base --is-ancestor"),
        )
        self.assertLess(
            prepare.index("git merge-base --is-ancestor"),
            prepare.index('echo "tag_commit=${tag_commit}"'),
        )

        windows = job_block(self.promotion, "verify_windows")
        macos = job_block(self.promotion, "verify_macos")
        linux = job_block(self.promotion, "verify_linux")
        self.assertIn("CHAFT_WINDOWS_SIGNER_THUMBPRINT", windows)
        self.assertIn("--trusted-windows-signer-thumbprint", windows)
        self.assertIn("CHAFT_APPLE_TEAM_ID", macos)
        self.assertIn("--trusted-apple-team-id", macos)
        self.assertIn("CHAFT_LINUX_SIGNING_FINGERPRINT", linux)
        self.assertIn("--trusted-fingerprint", linux)


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
