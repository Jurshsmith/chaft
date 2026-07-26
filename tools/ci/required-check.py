#!/usr/bin/env python3
"""Evaluate conditional CI jobs behind one stable required check.

Pass the JSON produced by the GitHub expression ``toJSON(needs)`` through the
``NEEDS_JSON`` environment variable:

    NEEDS_JSON='${{ toJSON(needs) }}' \
      python3 tools/ci/required-check.py

The evaluator accepts a skipped job only when its classifier scope was
disabled. Missing jobs, outputs, or results; unknown jobs or result values;
enabled-but-skipped jobs; failures; and cancellations all fail closed.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import sys
from typing import Mapping, Sequence


CLASSIFIER_JOB = "classify"
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

# Job identifiers are intentionally underscore-separated so workflow
# expressions can use needs.<job>.result without bracket syntax.
JOB_SCOPES: Mapping[str, tuple[str, ...]] = {
    "artifact_v7_producer": ("full",),
    "artifact_v8_consumer": ("full",),
    "website": ("website",),
    "rust_quality": ("rust",),
    "rust_tests": ("rust_test",),
    "rust_smokes": ("rust_smoke",),
    "benchmark_compile": ("benchmark",),
    "desktop_contracts": ("desktop_contract", "desktop"),
    "desktop": ("desktop",),
    "release_contracts": ("release_contract", "package"),
    "desktop_package": ("package",),
    "clean_package_smoke": ("package",),
}
KNOWN_RESULTS = frozenset({"success", "failure", "cancelled", "skipped"})


class RequiredCheckError(RuntimeError):
    """Raised when required-check input cannot be evaluated safely."""


@dataclass(frozen=True)
class JobEvaluation:
    job: str
    scopes: tuple[str, ...]
    expected: bool
    result: str
    passed: bool
    detail: str


@dataclass(frozen=True)
class RequiredEvaluation:
    passed: bool
    scopes: Mapping[str, bool]
    jobs: tuple[JobEvaluation, ...]
    errors: tuple[str, ...]


def _object(value: object, label: str) -> Mapping[str, object]:
    if not isinstance(value, dict):
        raise RequiredCheckError(f"{label} must be a JSON object")
    return value


def _result(job: str, value: object) -> str:
    entry = _object(value, f"needs.{job}")
    result = entry.get("result")
    if not isinstance(result, str) or not result:
        raise RequiredCheckError(f"needs.{job}.result is missing or invalid")
    if result not in KNOWN_RESULTS:
        raise RequiredCheckError(
            f"needs.{job}.result has unknown value {result!r}"
        )
    return result


def _classifier_outputs(needs: Mapping[str, object]) -> Mapping[str, bool]:
    if CLASSIFIER_JOB not in needs:
        raise RequiredCheckError(f"needs is missing {CLASSIFIER_JOB!r}")
    entry = _object(needs[CLASSIFIER_JOB], f"needs.{CLASSIFIER_JOB}")
    outputs = _object(
        entry.get("outputs"), f"needs.{CLASSIFIER_JOB}.outputs"
    )
    parsed: dict[str, bool] = {}
    for scope in SCOPE_NAMES:
        raw = outputs.get(scope)
        if raw not in {"true", "false"}:
            raise RequiredCheckError(
                f"classifier output {scope!r} must be 'true' or 'false'"
            )
        parsed[scope] = raw == "true"
    if parsed["full"] and not all(parsed[scope] for scope in RUNNABLE_SCOPES):
        raise RequiredCheckError(
            "classifier output 'full' is true but a runnable scope is false"
        )
    return parsed


def evaluate_needs(value: object) -> RequiredEvaluation:
    needs = _object(value, "needs")
    expected_jobs = {CLASSIFIER_JOB, *JOB_SCOPES}
    errors: list[str] = []

    missing = sorted(expected_jobs - set(needs))
    extra = sorted(set(needs) - expected_jobs)
    if missing:
        errors.append("needs is missing job(s): " + ", ".join(missing))
    if extra:
        errors.append("needs contains unmapped job(s): " + ", ".join(extra))

    try:
        classifier_result = _result(
            CLASSIFIER_JOB, needs.get(CLASSIFIER_JOB)
        )
    except RequiredCheckError as error:
        classifier_result = "invalid"
        errors.append(str(error))
    else:
        if classifier_result != "success":
            errors.append(
                f"classifier must succeed, observed {classifier_result!r}"
            )

    try:
        scopes = _classifier_outputs(needs)
    except RequiredCheckError as error:
        scopes = {scope: False for scope in SCOPE_NAMES}
        errors.append(str(error))

    jobs: list[JobEvaluation] = []
    for job, job_scopes in JOB_SCOPES.items():
        expected = any(scopes[scope] for scope in job_scopes)
        if job not in needs:
            jobs.append(
                JobEvaluation(
                    job=job,
                    scopes=job_scopes,
                    expected=expected,
                    result="missing",
                    passed=False,
                    detail="required dependency is missing",
                )
            )
            continue
        try:
            result = _result(job, needs[job])
        except RequiredCheckError as error:
            errors.append(str(error))
            jobs.append(
                JobEvaluation(
                    job=job,
                    scopes=job_scopes,
                    expected=expected,
                    result="invalid",
                    passed=False,
                    detail=str(error),
                )
            )
            continue

        if expected:
            passed = result == "success"
            detail = (
                "enabled scope completed successfully"
                if passed
                else "enabled scope did not complete successfully"
            )
        else:
            passed = result in {"success", "skipped"}
            detail = (
                "disabled scope was skipped"
                if result == "skipped"
                else (
                    "job ran successfully despite a disabled scope"
                    if result == "success"
                    else "disabled scope produced a failing result"
                )
            )
        if not passed:
            errors.append(
                f"{job}: expected {'success' if expected else 'success or skipped'}, "
                f"observed {result}"
            )
        jobs.append(
            JobEvaluation(
                job=job,
                scopes=job_scopes,
                expected=expected,
                result=result,
                passed=passed,
                detail=detail,
            )
        )

    return RequiredEvaluation(
        passed=not errors,
        scopes=scopes,
        jobs=tuple(jobs),
        errors=tuple(dict.fromkeys(errors)),
    )


def render_summary(evaluation: RequiredEvaluation) -> str:
    lines = [
        "### Required CI check",
        "",
        f"Overall result: **{'pass' if evaluation.passed else 'fail'}**",
        "",
        "| Job | Enabling scope(s) | Expected | Result | Verdict |",
        "| --- | --- | --- | --- | --- |",
    ]
    for job in evaluation.jobs:
        lines.append(
            "| `{job}` | {scopes} | {expected} | `{result}` | {verdict} |".format(
                job=job.job,
                scopes=", ".join(f"`{scope}`" for scope in job.scopes),
                expected="success" if job.expected else "success or skipped",
                result=job.result,
                verdict="pass" if job.passed else "fail",
            )
        )
    if evaluation.errors:
        lines.extend(["", "Failures:"])
        lines.extend(f"- {error}" for error in evaluation.errors)
    return "\n".join(lines) + "\n"


def _append(path: str, content: str) -> None:
    if not path:
        return
    with Path(path).open("a", encoding="utf-8") as handle:
        handle.write(content)


def emit_results(
    evaluation: RequiredEvaluation,
    *,
    github_output: str,
    step_summary: str,
) -> str:
    _append(
        github_output,
        f"required={'true' if evaluation.passed else 'false'}\n",
    )
    summary = render_summary(evaluation)
    _append(step_summary, summary)
    return summary


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group()
    source.add_argument(
        "--needs-json",
        default="",
        help="toJSON(needs) value (defaults to NEEDS_JSON)",
    )
    source.add_argument(
        "--needs-file",
        type=Path,
        help="path containing toJSON(needs)",
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


def _load_needs(args: argparse.Namespace) -> object:
    if args.needs_file is not None:
        try:
            raw = args.needs_file.read_text(encoding="utf-8")
        except OSError as error:
            raise RequiredCheckError(f"unable to read needs file: {error}") from error
    else:
        raw = args.needs_json or os.environ.get("NEEDS_JSON", "")
    if not raw:
        raise RequiredCheckError(
            "needs JSON is required via --needs-json, --needs-file, or NEEDS_JSON"
        )
    try:
        return json.loads(raw)
    except json.JSONDecodeError as error:
        raise RequiredCheckError(f"needs JSON is invalid: {error}") from error


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        needs = _load_needs(args)
        evaluation = evaluate_needs(needs)
    except RequiredCheckError as error:
        evaluation = RequiredEvaluation(
            passed=False,
            scopes={scope: False for scope in SCOPE_NAMES},
            jobs=(),
            errors=(str(error),),
        )
    summary = emit_results(
        evaluation,
        github_output=args.github_output,
        step_summary=args.step_summary,
    )
    sys.stdout.write(summary)
    return 0 if evaluation.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
