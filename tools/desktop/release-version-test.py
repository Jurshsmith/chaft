#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("release-version.py")


class ReleaseVersionTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "apps" / "desktop-qt").mkdir(parents=True)
        self.write_versions("1.2.3", "1.2.3", "1.2.3")

    def tearDown(self):
        self.temporary.cleanup()

    def write_versions(self, cargo, root_cmake, desktop_cmake):
        (self.root / "Cargo.toml").write_text(
            f'[workspace]\n\n[workspace.package]\nversion = "{cargo}"\n',
            encoding="utf-8",
        )
        (self.root / "CMakeLists.txt").write_text(
            "cmake_minimum_required(VERSION 3.28)\n"
            f"project(Chaft VERSION {root_cmake} LANGUAGES CXX)\n",
            encoding="utf-8",
        )
        (self.root / "apps" / "desktop-qt" / "CMakeLists.txt").write_text(
            "cmake_minimum_required(VERSION 3.28)\n"
            f"project(ChaftDesktop VERSION {desktop_cmake} LANGUAGES CXX)\n",
            encoding="utf-8",
        )

    def run_script(self, *arguments, check=True):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root), *arguments],
            check=check,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

    def git_environment(self):
        environment = os.environ.copy()
        environment.update(
            {
                "GIT_AUTHOR_NAME": "Chaft test",
                "GIT_AUTHOR_EMAIL": "test@chaft.invalid",
                "GIT_COMMITTER_NAME": "Chaft test",
                "GIT_COMMITTER_EMAIL": "test@chaft.invalid",
            }
        )
        return environment

    def initialize_git(self):
        subprocess.run(["git", "init", "-q"], cwd=self.root, check=True)
        subprocess.run(
            [
                "git",
                "remote",
                "add",
                "origin",
                "https://github.com/Jurshsmith/chaft",
            ],
            cwd=self.root,
            check=True,
        )
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "test release version"],
            cwd=self.root,
            env=self.git_environment(),
            check=True,
        )
        subprocess.run(["git", "tag", "v1.2.3"], cwd=self.root, check=True)

    def test_prints_matching_version(self):
        result = self.run_script("--print-version")
        self.assertEqual(result.stdout.strip(), "1.2.3")

    def test_rejects_mismatched_sources(self):
        self.write_versions("1.2.3", "1.2.4", "1.2.3")
        result = self.run_script("--print-version", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("release version sources disagree", result.stderr)

    def test_rejects_prerelease_until_cmake_contract_supports_it(self):
        self.write_versions("1.2.3-rc.1", "1.2.3-rc.1", "1.2.3-rc.1")
        result = self.run_script("--print-version", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("stable X.Y.Z", result.stderr)

    def test_validates_tag_and_commit(self):
        self.initialize_git()
        output = self.root / "release-context.json"
        github_output = self.root / "github-output.txt"
        result = self.run_script(
            "--tag",
            "v1.2.3",
            "--expected-commit",
            "HEAD",
            "--output",
            str(output),
            "--github-output",
            str(github_output),
        )
        context = json.loads(result.stdout)
        self.assertEqual(context["version"], "1.2.3")
        self.assertEqual(context["tag"], "v1.2.3")
        self.assertEqual(
            context["repository"], "https://github.com/Jurshsmith/chaft"
        )
        self.assertRegex(context["commit"], r"^[0-9a-f]{40}$")
        self.assertEqual(json.loads(output.read_text(encoding="utf-8")), context)
        self.assertEqual(
            github_output.read_text(encoding="utf-8").splitlines(),
            [
                "version=1.2.3",
                "tag=v1.2.3",
                f"commit={context['commit']}",
            ],
        )

    def test_rejects_tag_that_resolves_away_from_expected_commit(self):
        self.initialize_git()
        (self.root / "after-tag.txt").write_text("new commit\n", encoding="utf-8")
        subprocess.run(["git", "add", "after-tag.txt"], cwd=self.root, check=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "commit after tag"],
            cwd=self.root,
            env=self.git_environment(),
            check=True,
        )
        result = self.run_script(
            "--tag",
            "v1.2.3",
            "--expected-commit",
            "HEAD",
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("but expected commit resolves to", result.stderr)

    def test_rejects_tag_that_does_not_match_sources(self):
        self.initialize_git()
        result = self.run_script("--tag", "v1.2.4", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("expected v1.2.3", result.stderr)


if __name__ == "__main__":
    unittest.main()
