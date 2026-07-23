#!/usr/bin/env python3
import argparse
import json
import re
import subprocess
from pathlib import Path


VERSION_PATTERN = re.compile(
    r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)"
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


def validated_version(root):
    versions = source_versions(root)
    distinct = set(versions.values())
    if len(distinct) != 1:
        rendered = ", ".join(f"{name}={value}" for name, value in versions.items())
        fail(f"release version sources disagree: {rendered}")
    version = next(iter(distinct))
    if not VERSION_PATTERN.fullmatch(version):
        fail(
            f"release version must be a stable X.Y.Z value; prerelease and build "
            f"metadata are not supported yet: {version}"
        )
    return version, versions


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


def build_context(root, tag=None, expected_commit=None):
    version, versions = validated_version(root)
    if tag is not None and tag != f"v{version}":
        fail(f"release tag must exactly match the source version: expected v{version}")

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
        "schemaVersion": 1,
        "repository": optional_git_output(root, "config", "--get", "remote.origin.url"),
        "version": version,
        "tag": tag,
        "commit": commit,
        "sourceVersions": versions,
    }


def write_github_output(path, context):
    with path.open("a", encoding="utf-8", newline="\n") as output:
        for name in ("version", "tag", "commit"):
            value = context.get(name)
            if value is not None:
                output.write(f"{name}={value}\n")


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
    parser.add_argument("--tag", help="Exact release tag, such as v1.2.3")
    parser.add_argument(
        "--expected-commit",
        help="Revision that the tag must resolve to, such as HEAD",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Write the validated release context as JSON",
    )
    parser.add_argument(
        "--github-output",
        type=Path,
        help="Append version, tag, and commit to a GitHub Actions output file",
    )
    parser.add_argument(
        "--print-version",
        action="store_true",
        help="Print only the validated source version",
    )
    return parser.parse_args()


def main():
    args = parse_args()
    root = args.root.resolve()

    if (
        args.print_version
        and args.tag is None
        and args.expected_commit is None
        and args.output is None
        and args.github_output is None
    ):
        version, _ = validated_version(root)
        print(version)
        return

    context = build_context(root, args.tag, args.expected_commit)
    rendered = json.dumps(context, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    if args.github_output:
        write_github_output(args.github_output, context)
    if args.print_version:
        print(context["version"])
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
