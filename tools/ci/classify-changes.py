#!/usr/bin/env python3
"""Classify repository changes into independently runnable CI scopes.

The classifier is deliberately fail closed:

* unknown repository paths enable every scope;
* malformed paths and unusable pull-request diff metadata are fatal;
* scheduled and manually dispatched runs enable every scope;
* main-branch pushes always add benchmark and release-package coverage.

GitHub Actions usage can rely entirely on environment variables:

    python3 tools/ci/classify-changes.py

The script reads ``GITHUB_EVENT_NAME``, ``GITHUB_EVENT_PATH``, ``GITHUB_REF``,
``GITHUB_OUTPUT``, and ``GITHUB_STEP_SUMMARY``. For local inspection, pass
changed paths directly:

    python3 tools/ci/classify-changes.py \
      --event-name pull_request README.md runtime/src/lib.rs
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
from typing import Iterable, Mapping, Sequence


SCOPE_NAMES = (
    "website",
    "rust",
    "rust_test",
    "rust_smoke",
    "benchmark",
    "desktop_contract",
    "desktop",
    "release_contract",
    "package",
    "full",
)
RUNNABLE_SCOPES = SCOPE_NAMES[:-1]
ALL_RUNNABLE_SCOPES = frozenset(RUNNABLE_SCOPES)

WEBSITE_ROOT_FILES = frozenset(
    {
        ".node-version",
        ".nvmrc",
        "CONTRIBUTING.md",
        "README.md",
        "SECURITY.md",
    }
)
RUST_WORKSPACE_PREFIXES = (
    "application/",
    "apps/chaft-cli/",
    "apps/chaft-node/",
    "bindings/ffi/",
    "domain/",
    "network/",
    "proto/",
    "runtime/",
    "security/",
    "storage/",
    "tests/",
)
RUST_TEST_ONLY_FILES = frozenset(
    {
        "bindings/ffi/ffi-exports.txt",
        "bindings/ffi/ffi-json-contract.snapshot.json",
        "bindings/ffi/src/tests.rs",
    }
)
DESKTOP_RUST_PREFIXES = tuple(
    prefix for prefix in RUST_WORKSPACE_PREFIXES if prefix != "tests/"
)
ROOT_RUST_RELEASE_INPUTS = frozenset(
    {
        "Cargo.lock",
        "Cargo.toml",
        "rust-toolchain.toml",
    }
)
ROOT_DESKTOP_INPUTS = frozenset({"CMakeLists.txt", "CMakePresets.json"})

DESKTOP_CONTRACT_TOOLS = frozenset(
    {
        "tools/desktop/instance-smoke.sh",
        "tools/desktop/invite-form-contract-check.py",
        "tools/desktop/qml-lint.sh",
        "tools/desktop/qml-module-check.py",
        "tools/desktop/style-lint.py",
        "tools/desktop/theme-contrast-check.py",
    }
)
DESKTOP_RUNTIME_TOOLS = frozenset(
    {
        "tools/desktop/empty-workspace-smoke.sh",
        "tools/desktop/launch-users.sh",
        "tools/desktop/launch.sh",
        "tools/desktop/live-sync-smoke.sh",
        "tools/desktop/screenshot-baseline.py",
        "tools/desktop/screenshot-smoke.sh",
        "tools/desktop/smoke.sh",
    }
)
DESKTOP_SHARED_TOOLS = frozenset(
    {
        "tools/desktop/ci-gates.sh",
        "tools/desktop/common.sh",
        "tools/desktop/preflight.sh",
        "tools/desktop/validate-safe-path.py",
    }
)
MACOS_LOCAL_TOOLS = frozenset(
    {
        "tools/macos/build-local-test.py",
        "tools/macos/build-local.sh",
        "tools/macos/verify-local-app.sh",
    }
)
RELEASE_TOOLS = frozenset(
    {
        "tools/desktop/appimage-smoke.sh",
        "tools/desktop/canary-release-assets-test.py",
        "tools/desktop/canary-release-assets.py",
        "tools/desktop/check-qt-xcb-runtime.sh",
        "tools/desktop/export-website-release-manifest-test.py",
        "tools/desktop/export-website-release-manifest.py",
        "tools/desktop/fetch-appimage-tools.sh",
        "tools/desktop/generate-platform-verification-receipt-test.py",
        "tools/desktop/generate-platform-verification-receipt.py",
        "tools/desktop/generate-unsigned-canary-receipt-test.py",
        "tools/desktop/generate-unsigned-canary-receipt.py",
        "tools/desktop/install-linux-package-dependencies.sh",
        "tools/desktop/linux-appimage-contract-test.py",
        "tools/desktop/macos-adhoc-verify.cmake",
        "tools/desktop/macos-dmg-smoke-test.py",
        "tools/desktop/macos-dmg-smoke.sh",
        "tools/desktop/macos-unsigned-canary-smoke-test.py",
        "tools/desktop/macos-unsigned-canary-smoke.sh",
        "tools/desktop/package-linux-appimage.sh",
        "tools/desktop/package-smoke.sh",
        "tools/desktop/package.sh",
        "tools/desktop/platform-verification-receipt-smoke.sh",
        "tools/desktop/qt-release-binding-test.py",
        "tools/desktop/release_targets.py",
        "tools/desktop/release-metadata-smoke.sh",
        "tools/desktop/release-metadata.py",
        "tools/desktop/release-version-test.py",
        "tools/desktop/release-version.py",
        "tools/desktop/stage-website-release-assets-test.py",
        "tools/desktop/stage-website-release-assets.py",
        "tools/desktop/unsigned_canary_policy.py",
        "tools/desktop/verify-release-metadata.py",
        "tools/desktop/windows-zip-smoke.ps1",
    }
)

ZERO_SHA = "0" * 40
SHA_PATTERN = re.compile(r"[0-9a-fA-F]{40}")
VALID_DIFF_STATUSES = frozenset({"A", "C", "D", "M", "R", "T", "U", "X", "B"})


class ClassificationError(RuntimeError):
    """Raised when reliable change classification is impossible."""


@dataclass(frozen=True)
class PathImpact:
    scopes: frozenset[str]
    recognized: bool = True
    force_full: bool = False


@dataclass(frozen=True)
class Classification:
    scopes: Mapping[str, bool]
    changed_paths: tuple[str, ...]
    unknown_paths: tuple[str, ...]
    reasons: tuple[str, ...]
    event_name: str
    ref: str


def _has_prefix(path: str, prefixes: Iterable[str]) -> bool:
    return any(path.startswith(prefix) for prefix in prefixes)


def _is_rust_test_only(path: str) -> bool:
    if path in RUST_TEST_ONLY_FILES:
        return True
    if not _has_prefix(path, RUST_WORKSPACE_PREFIXES):
        return False
    return path.startswith("tests/") or "/tests/" in path


def _validated_path(path: str) -> str:
    if not path:
        raise ClassificationError("changed path is empty")
    if "\\" in path or any(ord(character) < 32 or ord(character) == 127 for character in path):
        raise ClassificationError(f"changed path has unsupported characters: {path!r}")
    parsed = PurePosixPath(path)
    if parsed.is_absolute() or any(part in {"", ".", ".."} for part in parsed.parts):
        raise ClassificationError(f"changed path is not repository relative: {path!r}")
    normalized = parsed.as_posix()
    if normalized != path:
        raise ClassificationError(f"changed path is not normalized: {path!r}")
    return normalized


def classify_path(path: str) -> PathImpact:
    """Return the scopes affected by one normalized repository path."""

    path = _validated_path(path)

    if path.startswith(".github/"):
        return PathImpact(ALL_RUNNABLE_SCOPES, force_full=True)
    if path.startswith("tools/ci/") and path != "tools/ci/rust-gates.sh":
        return PathImpact(ALL_RUNNABLE_SCOPES, force_full=True)
    if path in {".gitattributes", "Makefile"}:
        return PathImpact(ALL_RUNNABLE_SCOPES, force_full=True)

    if (
        path.startswith("apps/website/")
        or path.startswith("guides/public/")
        or path in WEBSITE_ROOT_FILES
    ):
        return PathImpact(frozenset({"website"}))

    if path == ".gitignore" or (
        path.startswith("guides/") and not path.startswith("guides/public/")
    ):
        return PathImpact(frozenset())

    if path == "LICENSE":
        return PathImpact(frozenset({"release_contract", "package"}))

    if path in ROOT_RUST_RELEASE_INPUTS:
        return PathImpact(
            frozenset(
                {
                    "rust",
                    "rust_test",
                    "rust_smoke",
                    "benchmark",
                    "desktop_contract",
                    "desktop",
                    "release_contract",
                    "package",
                }
            )
        )
    if path == "rustfmt.toml":
        return PathImpact(frozenset({"rust"}))

    if path.startswith("benchmarks/"):
        return PathImpact(frozenset({"rust", "benchmark"}))

    if _is_rust_test_only(path):
        return PathImpact(frozenset({"rust", "rust_test"}))

    if _has_prefix(path, RUST_WORKSPACE_PREFIXES):
        scopes = {"rust", "rust_test", "rust_smoke"}
        if _has_prefix(path, DESKTOP_RUST_PREFIXES):
            scopes.update({"desktop_contract", "desktop"})
        return PathImpact(frozenset(scopes))

    if path in ROOT_DESKTOP_INPUTS:
        return PathImpact(
            frozenset(
                {
                    "desktop_contract",
                    "desktop",
                    "release_contract",
                    "package",
                }
            )
        )

    if path == "apps/desktop-qt/resources/branding/README.md":
        return PathImpact(frozenset())

    if path.startswith("apps/desktop-qt/"):
        if path.startswith("apps/desktop-qt/tests/"):
            return PathImpact(frozenset({"desktop_contract"}))
        scopes = {"desktop"}
        if (
            path.startswith("apps/desktop-qt/qml/")
            or path.startswith("apps/desktop-qt/src/")
            or path == "apps/desktop-qt/CMakeLists.txt"
        ):
            scopes.add("desktop_contract")
        if path == "apps/desktop-qt/CMakeLists.txt":
            scopes.update({"release_contract", "package"})
        return PathImpact(frozenset(scopes))

    if path.startswith("packaging/"):
        return PathImpact(frozenset({"release_contract", "package"}))

    if path == "tools/ci/rust-gates.sh":
        return PathImpact(
            frozenset({"rust", "rust_test", "rust_smoke", "benchmark"})
        )

    if path.startswith("tools/qt/"):
        return PathImpact(
            frozenset(
                {
                    "desktop_contract",
                    "desktop",
                    "release_contract",
                    "package",
                }
            )
        )

    if path == "tools/smoke/visual-workspace.sh":
        return PathImpact(frozenset({"rust_smoke", "desktop_contract", "desktop"}))
    if path.startswith("tools/smoke/"):
        return PathImpact(frozenset({"rust_smoke"}))

    if path in DESKTOP_CONTRACT_TOOLS:
        return PathImpact(frozenset({"desktop_contract"}))
    if path == "tools/desktop/build.sh":
        return PathImpact(
            frozenset({"desktop", "release_contract", "package"})
        )
    if path in DESKTOP_RUNTIME_TOOLS or (
        path.startswith("tools/desktop/screenshot-baseline")
        and path.endswith(".json")
    ):
        return PathImpact(frozenset({"desktop_contract", "desktop"}))
    if path in DESKTOP_SHARED_TOOLS:
        return PathImpact(
            frozenset(
                {
                    "desktop_contract",
                    "desktop",
                    "release_contract",
                    "package",
                }
            )
        )
    if path in MACOS_LOCAL_TOOLS:
        return PathImpact(frozenset({"desktop_contract", "desktop"}))
    if path in RELEASE_TOOLS:
        return PathImpact(frozenset({"release_contract", "package"}))

    return PathImpact(ALL_RUNNABLE_SCOPES, recognized=False, force_full=True)


def classify_paths(
    paths: Iterable[str],
    *,
    event_name: str,
    ref: str = "",
    force_full: bool = False,
    force_full_reason: str = "",
) -> Classification:
    """Classify a collection of paths and apply event-level coverage rules."""

    normalized_paths = tuple(sorted({_validated_path(path) for path in paths}))
    enabled: set[str] = set()
    unknown: list[str] = []
    reasons: list[str] = []
    full = force_full

    if event_name in {"schedule", "workflow_dispatch"}:
        full = True
        reasons.append(
            "scheduled runs require full coverage"
            if event_name == "schedule"
            else "manually dispatched runs require full coverage"
        )
    if force_full and force_full_reason:
        reasons.append(force_full_reason)

    for path in normalized_paths:
        impact = classify_path(path)
        enabled.update(impact.scopes)
        if not impact.recognized:
            unknown.append(path)
        if impact.force_full:
            full = True

    if unknown:
        reasons.append("unknown paths require full coverage")

    if event_name == "push" and ref == "refs/heads/main":
        enabled.update({"benchmark", "release_contract", "package"})
        reasons.append("main pushes require benchmark and release-package coverage")

    if full:
        enabled.update(ALL_RUNNABLE_SCOPES)

    scope_values = {scope: scope in enabled for scope in RUNNABLE_SCOPES}
    scope_values["full"] = full
    return Classification(
        scopes=scope_values,
        changed_paths=normalized_paths,
        unknown_paths=tuple(unknown),
        reasons=tuple(dict.fromkeys(reasons)),
        event_name=event_name,
        ref=ref,
    )


def parse_name_status_z(data: bytes) -> tuple[str, ...]:
    """Parse ``git diff --name-status -z`` output, retaining both rename paths."""

    fields = data.split(b"\0")
    if fields and fields[-1] == b"":
        fields.pop()
    paths: list[str] = []
    index = 0
    while index < len(fields):
        try:
            status = fields[index].decode("ascii")
        except UnicodeDecodeError as error:
            raise ClassificationError("git diff emitted a non-ASCII status") from error
        index += 1
        if not status or status[0] not in VALID_DIFF_STATUSES:
            raise ClassificationError(f"git diff emitted unsupported status: {status!r}")
        path_count = 2 if status[0] in {"C", "R"} else 1
        if index + path_count > len(fields):
            raise ClassificationError(
                f"git diff status {status!r} is missing path data"
            )
        for raw_path in fields[index : index + path_count]:
            try:
                path = raw_path.decode("utf-8")
            except UnicodeDecodeError as error:
                raise ClassificationError(
                    "git diff emitted a path that is not UTF-8"
                ) from error
            paths.append(_validated_path(path))
        index += path_count
    return tuple(sorted(set(paths)))


def _validated_sha(value: str, label: str) -> str:
    if not SHA_PATTERN.fullmatch(value):
        raise ClassificationError(f"{label} must be a full 40-character Git SHA")
    return value.lower()


def changed_paths_from_git(
    repository: Path,
    *,
    base: str,
    head: str,
    three_dot: bool,
) -> tuple[str, ...]:
    base = _validated_sha(base, "base SHA")
    head = _validated_sha(head, "head SHA")
    separator = "..." if three_dot else ".."
    command = [
        "git",
        "diff",
        "--name-status",
        "-z",
        "--find-renames",
        f"{base}{separator}{head}",
    ]
    try:
        completed = subprocess.run(
            command,
            cwd=repository,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise ClassificationError(f"unable to execute git diff: {error}") from error
    if completed.returncode != 0:
        stderr = completed.stderr.decode("utf-8", errors="replace").strip()
        raise ClassificationError(f"git diff failed: {stderr or 'unknown error'}")
    return parse_name_status_z(completed.stdout)


def _load_event(path: str) -> Mapping[str, object]:
    if not path:
        return {}
    try:
        value = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ClassificationError(f"unable to read GitHub event payload: {error}") from error
    if not isinstance(value, dict):
        raise ClassificationError("GitHub event payload must be a JSON object")
    return value


def _nested_string(
    value: Mapping[str, object], keys: Sequence[str], label: str
) -> str:
    current: object = value
    for key in keys:
        if not isinstance(current, dict) or key not in current:
            raise ClassificationError(f"GitHub event payload is missing {label}")
        current = current[key]
    if not isinstance(current, str) or not current:
        raise ClassificationError(f"GitHub event payload has invalid {label}")
    return current


def resolve_changes(
    *,
    repository: Path,
    event_name: str,
    event: Mapping[str, object],
    base: str,
    head: str,
) -> tuple[tuple[str, ...], bool, str]:
    """Resolve event diff paths and any intentional full-run fallback."""

    if event_name in {"schedule", "workflow_dispatch"}:
        return (), True, ""
    if event_name == "pull_request":
        resolved_base = base or _nested_string(
            event, ("pull_request", "base", "sha"), "pull request base SHA"
        )
        resolved_head = head or _nested_string(
            event, ("pull_request", "head", "sha"), "pull request head SHA"
        )
        return (
            changed_paths_from_git(
                repository,
                base=resolved_base,
                head=resolved_head,
                three_dot=True,
            ),
            False,
            "",
        )
    if event_name == "push":
        resolved_base = base or _nested_string(event, ("before",), "push base SHA")
        resolved_head = head or _nested_string(event, ("after",), "push head SHA")
        _validated_sha(resolved_base, "base SHA")
        _validated_sha(resolved_head, "head SHA")
        if resolved_base.lower() == ZERO_SHA:
            return (), True, "push base SHA is unavailable; full coverage is required"
        return (
            changed_paths_from_git(
                repository,
                base=resolved_base,
                head=resolved_head,
                three_dot=False,
            ),
            False,
            "",
        )
    raise ClassificationError(f"unsupported GitHub event: {event_name!r}")


def render_summary(classification: Classification) -> str:
    lines = [
        "### CI change classification",
        "",
        f"- Event: `{classification.event_name or 'local'}`",
        f"- Ref: `{classification.ref or 'not supplied'}`",
        f"- Changed paths: {len(classification.changed_paths)}",
        f"- Fail-closed full run: `{'yes' if classification.scopes['full'] else 'no'}`",
        "",
        "| Scope | Run |",
        "| --- | --- |",
    ]
    for scope in SCOPE_NAMES:
        lines.append(
            f"| `{scope}` | `{'true' if classification.scopes[scope] else 'false'}` |"
        )
    if classification.reasons:
        lines.extend(["", "Reasons:"])
        lines.extend(f"- {reason}" for reason in classification.reasons)
    if classification.unknown_paths:
        lines.extend(["", "Unknown paths that forced full coverage:"])
        lines.extend(f"- `{path}`" for path in classification.unknown_paths)
    if classification.changed_paths:
        lines.extend(["", "<details><summary>Changed paths</summary>", ""])
        lines.extend(f"- `{path}`" for path in classification.changed_paths)
        lines.extend(["", "</details>"])
    return "\n".join(lines) + "\n"


def _append(path: str, content: str) -> None:
    if not path:
        return
    with Path(path).open("a", encoding="utf-8") as handle:
        handle.write(content)


def emit_results(
    classification: Classification,
    *,
    github_output: str,
    step_summary: str,
) -> str:
    output = "".join(
        f"{scope}={'true' if classification.scopes[scope] else 'false'}\n"
        for scope in SCOPE_NAMES
    )
    summary = render_summary(classification)
    _append(github_output, output)
    _append(step_summary, summary)
    return summary


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        help="explicit changed paths; when supplied, no git diff is executed",
    )
    parser.add_argument(
        "--event-name",
        default=os.environ.get("GITHUB_EVENT_NAME", ""),
        help="GitHub event name (defaults to GITHUB_EVENT_NAME)",
    )
    parser.add_argument(
        "--event-path",
        default=os.environ.get("GITHUB_EVENT_PATH", ""),
        help="GitHub event JSON path (defaults to GITHUB_EVENT_PATH)",
    )
    parser.add_argument(
        "--ref",
        default=os.environ.get("GITHUB_REF", ""),
        help="Git ref (defaults to GITHUB_REF)",
    )
    parser.add_argument("--base", default="", help="explicit diff base SHA")
    parser.add_argument("--head", default="", help="explicit diff head SHA")
    parser.add_argument(
        "--repository",
        type=Path,
        default=Path.cwd(),
        help="repository used for git diff (defaults to current directory)",
    )
    parser.add_argument(
        "--github-output",
        default=os.environ.get("GITHUB_OUTPUT", ""),
        help="GitHub output file (defaults to GITHUB_OUTPUT)",
    )
    parser.add_argument(
        "--step-summary",
        default=os.environ.get("GITHUB_STEP_SUMMARY", ""),
        help="GitHub step-summary file (defaults to GITHUB_STEP_SUMMARY)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    event_name = args.event_name
    if not event_name:
        if args.paths:
            event_name = "local"
        else:
            raise ClassificationError(
                "event name is required when explicit paths are not supplied"
            )

    force_full = False
    force_full_reason = ""
    if args.paths:
        paths = tuple(args.paths)
    else:
        event = _load_event(args.event_path)
        paths, force_full, force_full_reason = resolve_changes(
            repository=args.repository,
            event_name=event_name,
            event=event,
            base=args.base,
            head=args.head,
        )

    classification = classify_paths(
        paths,
        event_name=event_name,
        ref=args.ref,
        force_full=force_full,
        force_full_reason=force_full_reason,
    )
    summary = emit_results(
        classification,
        github_output=args.github_output,
        step_summary=args.step_summary,
    )
    sys.stdout.write(summary)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ClassificationError as error:
        summary_path = os.environ.get("GITHUB_STEP_SUMMARY", "")
        message = f"### CI change classification failed\n\n{error}\n"
        try:
            _append(summary_path, message)
        except OSError:
            pass
        print(f"classify-changes: {error}", file=sys.stderr)
        raise SystemExit(2)
