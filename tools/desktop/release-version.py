#!/usr/bin/env python3
import argparse
import json
import re
import subprocess
from pathlib import Path


SOURCE_VERSION_PATTERN = re.compile(
    r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)"
)
SEMVER_PATTERN = re.compile(
    r"(?P<core>(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))"
    r"(?:-(?P<prerelease>"
    r"(?:0|[1-9]\d*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9]\d*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*"
    r"))?"
    r"(?:\+(?P<build>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)


def fail(message):
    raise SystemExit(message)


def cargo_workspace_version(path):
    in_workspace_package = False
    for line_number, raw_line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), 1
    ):
        line = raw_line.strip()
        if line.startswith("[") and line.endswith("]"):
            in_workspace_package = line == "[workspace.package]"
            continue
        if not in_workspace_package:
            continue
        match = re.fullmatch(r'version\s*=\s*"([^"]+)"(?:\s*#.*)?', line)
        if match:
            return match.group(1)
    fail(f"{path}: [workspace.package].version is missing")


def cmake_project_version(path, project_name):
    text = path.read_text(encoding="utf-8")
    match = re.search(
        rf"project\(\s*{re.escape(project_name)}\s+VERSION\s+([^\s\)]+)",
        text,
        flags=re.IGNORECASE,
    )
    if not match:
        fail(f"{path}: project({project_name} VERSION ...) is missing")
    return match.group(1)


def source_versions(root):
    return {
        "cargoWorkspace": cargo_workspace_version(root / "Cargo.toml"),
        "rootCmake": cmake_project_version(root / "CMakeLists.txt", "Chaft"),
        "desktopCmake": cmake_project_version(
            root / "apps" / "desktop-qt" / "CMakeLists.txt",
            "ChaftDesktop",
        ),
    }


def validated_source_version(root):
    versions = source_versions(root)
    distinct = set(versions.values())
    if len(distinct) != 1:
        rendered = ", ".join(f"{name}={value}" for name, value in versions.items())
        fail(f"release version sources disagree: {rendered}")
    version = next(iter(distinct))
    if not SOURCE_VERSION_PATTERN.fullmatch(version):
        fail(
            f"source version must be a stable X.Y.Z value; prerelease and build "
            f"metadata belong in the distribution version: {version}"
        )
    return version, versions


def validated_distribution_version(value, source_version):
    match = SEMVER_PATTERN.fullmatch(value)
    if not match:
        fail(f"distribution version must be an exact SemVer value: {value}")
    if match.group("core") != source_version:
        fail(
            "distribution version core must exactly match the source version: "
            f"expected {source_version}, got {match.group('core')}"
        )
    return value, match.group("prerelease")


def git_output(root, *arguments):
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=root,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        details = ""
        if isinstance(error, subprocess.CalledProcessError):
            details = error.stderr.strip()
        fail(f"git {' '.join(arguments)} failed: {details or error}")
    return result.stdout.strip()


def optional_git_output(root, *arguments):
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=root,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return result.stdout.strip() or None


def resolve_commit(root, revision):
    commit = git_output(root, "rev-parse", "--verify", f"{revision}^{{commit}}")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        fail(f"git resolved an invalid commit for {revision}: {commit}")
    return commit


def build_context(
    root,
    tag=None,
    distribution_version=None,
    expected_commit=None,
    allow_missing_tag=False,
):
    source_version, versions = validated_source_version(root)
    tag_distribution_version = tag[1:] if tag and tag.startswith("v") else None
    if tag is not None and tag_distribution_version is None:
        fail("release tag must start with v")
    if (
        distribution_version is not None
        and tag_distribution_version is not None
        and distribution_version != tag_distribution_version
    ):
        fail(
            "declared distribution version and release tag disagree: "
            f"{distribution_version} != {tag_distribution_version}"
        )

    declared_distribution_version = (
        distribution_version or tag_distribution_version or source_version
    )
    distribution_version, prerelease = validated_distribution_version(
        declared_distribution_version, source_version
    )
    if tag is not None and tag != f"v{distribution_version}":
        fail(
            "release tag must exactly match the distribution version: "
            f"expected v{distribution_version}"
        )

    if allow_missing_tag:
        if tag is None or expected_commit is None:
            fail("--allow-missing-tag requires both --tag and --expected-commit")
        if prerelease is None:
            fail("--allow-missing-tag is restricted to SemVer prerelease tags")
        expected = resolve_commit(root, expected_commit)
        revision = f"refs/tags/{tag}"
        existing_commit = optional_git_output(
            root, "rev-parse", "--verify", f"{revision}^{{commit}}"
        )
        if existing_commit is not None and not re.fullmatch(
            r"[0-9a-f]{40}", existing_commit
        ):
            fail(f"git resolved an invalid commit for {revision}: {existing_commit}")
        if existing_commit is not None and existing_commit != expected:
            fail(
                f"{revision} resolves to {existing_commit}, but expected commit "
                f"resolves to {expected}"
            )
        commit = expected
    else:
        revision = f"refs/tags/{tag}" if tag else "HEAD"
        commit = resolve_commit(root, revision)
        if expected_commit is not None:
            expected = resolve_commit(root, expected_commit)
            if commit != expected:
                fail(
                    f"{revision} resolves to {commit}, but expected commit resolves to "
                    f"{expected}"
                )

    return {
        "schemaVersion": 2,
        "repository": optional_git_output(root, "config", "--get", "remote.origin.url"),
        "sourceVersion": source_version,
        "distributionVersion": distribution_version,
        "tag": tag,
        "commit": commit,
        "sourceVersions": versions,
    }


def write_github_output(path, context):
    with path.open("a", encoding="utf-8", newline="\n") as output:
        for output_name, context_name in (
            ("source_version", "sourceVersion"),
            ("distribution_version", "distributionVersion"),
            ("tag", "tag"),
            ("commit", "commit"),
        ):
            value = context.get(context_name)
            if value is not None:
                output.write(f"{output_name}={value}\n")


def parse_args():
    parser = argparse.ArgumentParser(
        description="Validate Chaft's release version and optional Git tag."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--tag",
        help="Exact release tag, such as v1.2.3 or v1.2.3-canary.1",
    )
    parser.add_argument(
        "--distribution-version",
        help=(
            "Exact SemVer distribution version. Defaults to the tag version when a "
            "tag is supplied, otherwise to the stable source version."
        ),
    )
    parser.add_argument(
        "--expected-commit",
        help="Revision that the tag must resolve to, such as HEAD",
    )
    parser.add_argument(
        "--allow-missing-tag",
        action="store_true",
        help=(
            "Allow a declared prerelease tag to be validated before it exists. "
            "Requires --tag and --expected-commit."
        ),
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Write the validated release context as JSON",
    )
    parser.add_argument(
        "--github-output",
        type=Path,
        help=(
            "Append source_version, distribution_version, tag, and commit to a "
            "GitHub Actions output file"
        ),
    )
    print_group = parser.add_mutually_exclusive_group()
    print_group.add_argument(
        "--print-source-version",
        action="store_true",
        help="Print only the validated stable source version",
    )
    print_group.add_argument(
        "--print-distribution-version",
        action="store_true",
        help="Print only the validated distribution version",
    )
    print_group.add_argument(
        "--print-version",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    return parser.parse_args()


def main():
    args = parse_args()
    root = args.root.resolve()

    print_source_version = args.print_source_version or args.print_version
    print_distribution_version = args.print_distribution_version
    if (
        (print_source_version or print_distribution_version)
        and args.tag is None
        and args.expected_commit is None
        and not args.allow_missing_tag
        and args.output is None
        and args.github_output is None
    ):
        source_version, _ = validated_source_version(root)
        if args.distribution_version is not None:
            validated_distribution_version(
                args.distribution_version, source_version
            )
        if print_source_version:
            print(source_version)
        else:
            distribution_version = args.distribution_version or source_version
            validated_distribution_version(distribution_version, source_version)
            print(distribution_version)
        return

    context = build_context(
        root,
        args.tag,
        args.distribution_version,
        args.expected_commit,
        args.allow_missing_tag,
    )
    rendered = json.dumps(context, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    if args.github_output:
        write_github_output(args.github_output, context)
    if print_source_version:
        print(context["sourceVersion"])
    elif print_distribution_version:
        print(context["distributionVersion"])
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
