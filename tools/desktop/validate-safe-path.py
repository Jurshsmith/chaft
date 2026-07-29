#!/usr/bin/env python3
"""Validate a path before desktop build scripts write or remove content."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys


class UnsafePathError(ValueError):
    """Raised when a path is not safe for scripted mutation."""


def normalized_safe_path(
    raw_path: str,
    *,
    description: str,
    within: str | None = None,
) -> Path:
    candidate = Path(raw_path)
    if not candidate.is_absolute():
        raise UnsafePathError(f"{description} must be an absolute path: {raw_path}")

    normalized = Path(os.path.abspath(os.path.normpath(raw_path)))
    if normalized == Path(normalized.anchor):
        raise UnsafePathError(
            f"{description} must not resolve to the filesystem root: {raw_path}"
        )

    relative: Path | None = None
    boundary: Path | None = None
    if within is not None:
        boundary = Path(os.path.abspath(os.path.normpath(within)))
        if not boundary.is_absolute() or boundary == Path(boundary.anchor):
            raise UnsafePathError(f"{description} safety boundary is invalid: {within}")
        try:
            relative = normalized.relative_to(boundary)
        except ValueError as error:
            raise UnsafePathError(
                f"{description} must remain below {boundary}: {raw_path}"
            ) from error
        if relative == Path("."):
            raise UnsafePathError(
                f"{description} must be a child of {boundary}: {raw_path}"
            )

    if boundary is not None and relative is not None:
        current = boundary
        components = relative.parts
        if current.is_symlink():
            raise UnsafePathError(
                f"{description} safety boundary is a symbolic link: {current}"
            )
    else:
        current = Path(normalized.anchor)
        components = normalized.parts[1:]
    for component in components:
        current /= component
        if current.is_symlink():
            raise UnsafePathError(
                f"{description} contains a symbolic-link component: {current}"
            )

    if normalized.exists() and not normalized.is_dir():
        raise UnsafePathError(
            f"{description} exists but is not a directory: {normalized}"
        )
    return normalized


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--path", required=True)
    parser.add_argument("--description", required=True)
    parser.add_argument("--within")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    try:
        path = normalized_safe_path(
            args.path,
            description=args.description,
            within=args.within,
        )
    except UnsafePathError as error:
        print(error, file=sys.stderr)
        return 2
    print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
