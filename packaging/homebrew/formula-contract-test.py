#!/usr/bin/env python3
"""Contracts for the unpublished Chaft source-building Homebrew formula."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FORMULA = ROOT / "packaging" / "homebrew" / "Formula" / "chaft.rb"
RENDERER = ROOT / "packaging" / "homebrew" / "render-formula.py"
BUILD_SCRIPT = ROOT / "tools" / "desktop" / "build.sh"

STABLE_VERSION = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
)
FULL_COMMIT = re.compile(r"[0-9a-f]{40}")
PLACEHOLDERS = {
    "UNRESOLVED_CHAFT_RELEASE_VERSION": 1,
    "UNRESOLVED_CHAFT_RELEASE_TAG": 1,
    "UNRESOLVED_CHAFT_RELEASE_COMMIT": 2,
}


def ruby_syntax_problem(formula: Path) -> str | None:
    ruby = shutil.which("ruby")
    if ruby is None:
        return None
    checked = subprocess.run(
        [ruby, "-c", str(formula)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if checked.returncode == 0:
        return None
    return checked.stdout + checked.stderr


def common_contract_problems(source: str) -> list[str]:
    problems: list[str] = []

    def require(fragment: str, description: str) -> None:
        if fragment not in source:
            problems.append(f"missing {description}: {fragment!r}")

    require("class Chaft < Formula", "Formula class")
    if re.search(r"\bCask\b|\bcask\b", source):
        problems.append("the source-building package must be a formula, never a cask")

    require("depends_on :macos", "macOS requirement")
    for formula in (
        "cmake",
        "git",
        "ninja",
        "python@3.14",
        "qtbase",
        "qtdeclarative",
        "rust",
    ):
        require(f'depends_on "{formula}" => :build', f"{formula} build dependency")

    brew_binding = (
        'ENV["CHAFT_HOMEBREW_EXECUTABLE"] = '
        'ENV.fetch("HOMEBREW_BREW_FILE")'
    )
    require(brew_binding, "exact parent Homebrew executable binding")
    require(
        'system "tools/macos/build-local.sh"',
        "reviewed source-build workflow invocation",
    )
    for argument in (
        "--yes",
        "--no-install-deps",
        "--install-dir",
        "--expected-commit",
        "--skip-launch",
    ):
        require(f'"{argument}"', f"{argument} source-build argument")
    require('(libexec/"Applications").to_s', "formula-private app destination")
    if (
        brew_binding in source
        and 'system "tools/macos/build-local.sh"' in source
        and source.index(brew_binding)
        > source.index('system "tools/macos/build-local.sh"')
    ):
        problems.append(
            "the exact parent Homebrew executable must be bound before the "
            "source-build workflow starts"
        )
    if re.search(r'\bsystem\s+"brew"', source):
        problems.append("the formula must not invoke brew directly")
    if re.search(r"\bcurl\b[^\n]*\|", source):
        problems.append("the formula must not use a curl-pipe-shell command")

    require('launcher = bin/"chaft"', "launcher")
    require(
        'exec /usr/bin/open -n "#{opt_libexec}/Applications/Chaft.app"',
        "launcher command",
    )
    require("local ad-hoc signature", "local-signing disclosure")
    if re.search(
        r"not Developer ID signed or\s+Apple notarized",
        source,
    ) is None:
        problems.append(
            "missing explicit disclosure that the app is neither Developer ID "
            "signed nor Apple notarized"
        )
    require(
        "should not be redistributed as a trusted binary",
        "redistribution disclosure",
    )

    require("assert_predicate binary, :executable?", "executable test")
    require("Contents/Resources/Chaft.icns", "icon test")
    require("CFBundleName", "bundle-name test")
    require('"Chaft"', "Chaft brand test")
    require('"--verify", "--deep", "--strict"', "signature verification")
    require("Signature=adhoc", "ad-hoc signature assertion")

    return problems


def template_contract_problems(source: str) -> list[str]:
    problems = common_contract_problems(source)
    for placeholder, expected_count in PLACEHOLDERS.items():
        actual_count = source.count(placeholder)
        if actual_count != expected_count:
            problems.append(
                f"template must contain {expected_count} {placeholder} "
                f"placeholder(s), found {actual_count}"
            )
    if "branch:" in source:
        problems.append("template must not track a moving branch")
    if re.search(r"\bhead\s+", source):
        problems.append("template must not offer a moving head build")
    if re.search(r"\bsha256\s+", source):
        problems.append("Git tag-plus-revision source must not declare sha256")
    return problems


def resolved_contract_problems(
    source: str,
    *,
    version: str,
    tag: str,
    commit: str,
) -> list[str]:
    problems = common_contract_problems(source)
    if STABLE_VERSION.fullmatch(version) is None:
        problems.append("resolved version must be an exact stable X.Y.Z value")
    if tag != f"v{version}":
        problems.append(f"resolved tag must exactly match v{version}")
    if FULL_COMMIT.fullmatch(commit) is None:
        problems.append(
            "resolved commit must be a full lowercase 40-character hexadecimal SHA"
        )

    unresolved = sorted(
        placeholder for placeholder in PLACEHOLDERS if placeholder in source
    )
    if unresolved:
        problems.append(
            "resolved formula contains placeholders: " + ", ".join(unresolved)
        )

    if f'version "{version}"' not in source:
        problems.append("resolved formula version does not match the reviewed input")
    coordinate_pattern = re.compile(
        r'url "https://github\.com/Jurshsmith/chaft\.git",\s+'
        rf'tag:\s+"{re.escape(tag)}",\s+'
        rf'revision:\s+"{re.escape(commit)}"'
    )
    if coordinate_pattern.search(source) is None:
        problems.append(
            "resolved formula must pin the reviewed Git tag and full commit"
        )
    if source.count(commit) != 2:
        problems.append(
            "resolved commit must appear exactly in the Git revision and "
            "--expected-commit argument"
        )
    if "branch:" in source:
        problems.append("resolved formula must not track a moving branch")
    if re.search(r"\bhead\s+", source):
        problems.append("resolved formula must not offer a moving head build")
    if re.search(r"\bsha256\s+", source):
        problems.append("Git tag-plus-revision source must not declare sha256")

    template = FORMULA.read_text(encoding="utf-8")
    expected_source = (
        template.replace("UNRESOLVED_CHAFT_RELEASE_VERSION", version)
        .replace("UNRESOLVED_CHAFT_RELEASE_TAG", tag)
        .replace("UNRESOLVED_CHAFT_RELEASE_COMMIT", commit)
    )
    if source != expected_source:
        problems.append(
            "resolved formula differs from the reviewed template beyond the "
            "three immutable release-coordinate substitutions"
        )
    return problems


def locked_cargo_problem() -> str | None:
    source = BUILD_SCRIPT.read_text(encoding="utf-8")
    cargo_build = re.search(r"cargo build(?:[^\n]|\\\n)*", source)
    if cargo_build is None or "--locked" not in cargo_build.group(0):
        return (
            "the shared desktop source-build workflow must invoke "
            "`cargo build --locked`"
        )
    return None


class ChaftFormulaContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = FORMULA.read_text(encoding="utf-8")

    def test_unresolved_review_template_contract(self) -> None:
        self.assertEqual([], template_contract_problems(self.source))
        self.assertIsNone(ruby_syntax_problem(FORMULA))

    def test_shared_source_workflow_locks_cargo_dependencies(self) -> None:
        self.assertIsNone(locked_cargo_problem())

    def test_renderer_produces_a_valid_separate_formula(self) -> None:
        version = "1.2.3"
        tag = "v1.2.3"
        commit = "0123456789abcdef0123456789abcdef01234567"
        with tempfile.TemporaryDirectory(prefix="chaft-formula-contract-") as name:
            rendered = Path(name) / "Formula" / "chaft.rb"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(RENDERER),
                    "--version",
                    version,
                    "--tag",
                    tag,
                    "--commit",
                    commit,
                    "--output",
                    str(rendered),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(
                0,
                completed.returncode,
                msg=completed.stdout + completed.stderr,
            )
            source = rendered.read_text(encoding="utf-8")
            self.assertEqual(
                [],
                resolved_contract_problems(
                    source,
                    version=version,
                    tag=tag,
                    commit=commit,
                ),
            )
            self.assertIsNone(ruby_syntax_problem(rendered))
            self.assertIn("UNRESOLVED_CHAFT_RELEASE_", self.source)
            tampered = source.replace(
                'desc "Native local-first peer-to-peer chat workspace"',
                'desc "Unreviewed replacement"',
            )
            self.assertTrue(
                any(
                    problem.startswith(
                        "resolved formula differs from the reviewed template"
                    )
                    for problem in resolved_contract_problems(
                        tampered,
                        version=version,
                        tag=tag,
                        commit=commit,
                    )
                )
            )

    def test_renderer_rejects_moving_or_mismatched_coordinates(self) -> None:
        with tempfile.TemporaryDirectory(prefix="chaft-formula-contract-") as name:
            rendered = Path(name) / "chaft.rb"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(RENDERER),
                    "--version",
                    "1.2.3",
                    "--tag",
                    "latest",
                    "--commit",
                    "a" * 40,
                    "--output",
                    str(rendered),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertNotEqual(0, completed.returncode)
            self.assertIn("--tag must exactly match", completed.stderr)
            self.assertFalse(rendered.exists())

            template = Path(name) / "template.rb"
            template.write_text(self.source, encoding="utf-8")
            overwrite = subprocess.run(
                [
                    sys.executable,
                    str(RENDERER),
                    "--version",
                    "1.2.3",
                    "--tag",
                    "v1.2.3",
                    "--commit",
                    "a" * 40,
                    "--template",
                    str(template),
                    "--output",
                    str(template),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertNotEqual(0, overwrite.returncode)
            self.assertIn("must not overwrite", overwrite.stderr)
            self.assertEqual(self.source, template.read_text(encoding="utf-8"))


def validate_resolved_formula(arguments: argparse.Namespace) -> int:
    formula = arguments.resolved_formula
    try:
        source = formula.read_text(encoding="utf-8")
    except OSError as error:
        print(f"unable to read resolved formula: {error}", file=sys.stderr)
        return 1

    problems = resolved_contract_problems(
        source,
        version=arguments.version,
        tag=arguments.tag,
        commit=arguments.commit,
    )
    syntax_problem = ruby_syntax_problem(formula)
    if syntax_problem is not None:
        problems.append(f"Ruby syntax check failed: {syntax_problem}")
    cargo_problem = locked_cargo_problem()
    if cargo_problem is not None:
        problems.append(cargo_problem)
    if problems:
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 1
    print(f"validated resolved immutable Chaft formula: {formula}")
    return 0


def parse_arguments() -> tuple[argparse.Namespace, list[str]]:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--resolved-formula", type=Path)
    parser.add_argument("--version")
    parser.add_argument("--tag")
    parser.add_argument("--commit")
    return parser.parse_known_args()


if __name__ == "__main__":
    parsed, unittest_arguments = parse_arguments()
    if parsed.resolved_formula is None:
        unittest.main(argv=[sys.argv[0], *unittest_arguments])
    else:
        missing = [
            name
            for name in ("version", "tag", "commit")
            if getattr(parsed, name) is None
        ]
        if missing:
            print(
                "--resolved-formula also requires "
                + ", ".join(f"--{name}" for name in missing),
                file=sys.stderr,
            )
            raise SystemExit(2)
        raise SystemExit(validate_resolved_formula(parsed))
