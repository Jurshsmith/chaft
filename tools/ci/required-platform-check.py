#!/usr/bin/env python3
"""Reject missing, failed, cancelled, or unexpectedly skipped native stages."""

from __future__ import annotations

import json
import os
from pathlib import Path
import sys


TARGETS = {
    "linux-x86_64", "macos-x86_64", "macos-arm64", "windows-x86_64",
}
STAGE_INPUTS = {
    "sdk": None,
    "contracts": "run-contracts",
    "debug": "run-desktop",
    "package": "run-package",
    "smoke": "run-package",
}


def evaluate(needs: object, inputs: object) -> list[str]:
    if not isinstance(needs, dict) or not isinstance(inputs, dict):
        raise ValueError("platform inputs and dependency results must be objects")
    if not isinstance(inputs.get("target"), str) or inputs["target"] not in TARGETS:
        raise ValueError("unsupported native target")
    flags = {name: inputs.get(name) for name in set(STAGE_INPUTS.values()) - {None}}
    if any(type(value) is not bool for value in flags.values()):
        raise ValueError("platform stage selections must be booleans")
    if not any(flags.values()):
        raise ValueError("a platform run must enable at least one stage")
    if flags["run-contracts"] and inputs["target"] != "linux-x86_64":
        raise ValueError("desktop contracts must run on Linux")
    if set(needs) != set(STAGE_INPUTS):
        raise ValueError("platform dependencies are missing or unmapped")
    errors = []
    for stage, flag in STAGE_INPUTS.items():
        required = flag is None or flags[flag]
        entry = needs[stage]
        result = entry.get("result") if isinstance(entry, dict) else None
        allowed = {"success"} if required else {"success", "skipped"}
        if not isinstance(result, str) or result not in allowed:
            errors.append(f"{stage}: expected {'success' if required else 'success or skipped'}, observed {result!r}")
    return errors


def main() -> int:
    try:
        errors = evaluate(
            json.loads(os.environ["NEEDS_JSON"]),
            json.loads(os.environ["PLATFORM_INPUTS_JSON"]),
        )
    except (KeyError, TypeError, ValueError) as error:
        errors = [str(error)]
    summary = "### Native platform checks\n\n" + (
        "\n".join(f"- {error}" for error in errors) if errors else "Every requested platform stage passed."
    ) + "\n"
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as output:
            output.write(summary)
    print(summary)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
