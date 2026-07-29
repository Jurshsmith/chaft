#!/usr/bin/env python3
"""Render the unpublished Chaft Homebrew formula from immutable release inputs."""

from __future__ import annotations

import argparse
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TEMPLATE = ROOT / "packaging" / "homebrew" / "Formula" / "chaft.rb"

STABLE_VERSION = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
)
FULL_COMMIT = re.compile(r"[0-9a-f]{40}")

PLACEHOLDERS = {
    "UNRESOLVED_CHAFT_RELEASE_VERSION": 1,
    "UNRESOLVED_CHAFT_RELEASE_TAG": 1,
    "UNRESOLVED_CHAFT_RELEASE_COMMIT": 2,
}


def fail(message: str) -> None:
    raise SystemExit(message)


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Render the review-only formula template using a verified stable "
            "version, immutable tag, and full source commit."
        )
    )
    parser.add_argument("--version", required=True, help="Stable X.Y.Z version")
    parser.add_argument("--tag", required=True, help="Matching immutable vX.Y.Z tag")
    parser.add_argument(
        "--commit",
        required=True,
        help="Full lowercase 40-character release commit",
    )
    parser.add_argument(
        "--template",
        type=Path,
        default=DEFAULT_TEMPLATE,
        help="Unresolved formula template",
    )
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    arguments = parse_arguments()
    if arguments.output.resolve() == arguments.template.resolve():
        fail("--output must not overwrite the unresolved review template")
    if STABLE_VERSION.fullmatch(arguments.version) is None:
        fail("--version must be an exact stable X.Y.Z value without leading zeros")
    expected_tag = f"v{arguments.version}"
    if arguments.tag != expected_tag:
        fail(f"--tag must exactly match the version: {expected_tag}")
    if FULL_COMMIT.fullmatch(arguments.commit) is None:
        fail("--commit must be a full lowercase 40-character hexadecimal SHA")

    try:
        source = arguments.template.read_text(encoding="utf-8")
    except OSError as error:
        fail(f"unable to read formula template: {error}")

    for placeholder, expected_count in PLACEHOLDERS.items():
        actual_count = source.count(placeholder)
        if actual_count != expected_count:
            fail(
                f"formula template must contain {expected_count} {placeholder} "
                f"placeholder(s), found {actual_count}"
            )

    rendered = (
        source.replace("UNRESOLVED_CHAFT_RELEASE_VERSION", arguments.version)
        .replace("UNRESOLVED_CHAFT_RELEASE_TAG", arguments.tag)
        .replace("UNRESOLVED_CHAFT_RELEASE_COMMIT", arguments.commit)
    )
    if "UNRESOLVED_CHAFT_RELEASE_" in rendered:
        fail("rendered formula still contains unresolved release coordinates")

    try:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(rendered, encoding="utf-8")
    except OSError as error:
        fail(f"unable to write rendered formula: {error}")

    print(f"rendered immutable Chaft formula: {arguments.output}")


if __name__ == "__main__":
    main()
