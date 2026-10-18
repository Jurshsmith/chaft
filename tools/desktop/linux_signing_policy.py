"""Shared policy for optional OpenPGP-signed Linux canaries."""

from __future__ import annotations

from typing import Mapping

import unsigned_canary_policy as canary

PUBLIC_KEY_FILENAME = "chaft-linux-release-signing-key.gpg"
WARNING = "Canary. Do not use Chaft canary builds for sensitive or production communication."
SMOKE_FIELDS = ("repository", "release", "asset", "runner", "smoke")


def is_signed_linux(receipt: Mapping[str, object]) -> bool:
    return (
        receipt.get("target") == "linux-x86_64"
        and receipt.get("schemaVersion") == "chaft.desktop.platform-verification.v2"
    )


def validate_canary_receipt(receipt: Mapping[str, object], **expected) -> None:
    """Validate release/download smoke binding; native signature validation is separate."""
    if not is_signed_linux(receipt):
        canary.validate_receipt_document(receipt, **expected)
        return
    if (
        receipt.get("status") != "verified"
        or receipt.get("signingStatus") != "signed"
        or receipt.get("productionEligible") is not False
        or receipt.get("warning") != WARNING
    ):
        canary.fail("signed Linux canary must retain verified signing and canary eligibility")
    # Reuse the established smoke/download identity validator without publishing
    # any of the unsigned-signature claims in the signed native receipt.
    smoke = {
        "schemaVersion": canary.SCHEMA_VERSION,
        "verificationType": canary.VERIFICATION_TYPE,
        "status": canary.STATUS,
        "signingStatus": canary.SIGNING_STATUS,
        "signatureVerification": canary.SIGNATURE_VERIFICATION["linux"],
        "signatureAndNotarization": canary.SIGNATURE_AND_NOTARIZATION["linux"],
        "productionEligible": False,
        "warning": canary.WARNING,
        "receiptGenerator": {"name": "Chaft unsigned-canary receipt generator", "version": "1"},
        **{key: receipt.get(key) for key in (
            "target", "platform", "version", "tag", "commit", "architecture", "verifiedAt",
            *SMOKE_FIELDS,
        )},
    }
    canary.validate_receipt_document(smoke, **expected)


def bind_smoke(native: dict, smoke: dict) -> dict:
    canary.validate_receipt_document(smoke, expected_target="linux-x86_64")
    if not is_signed_linux(native):
        canary.fail("Linux signing requires a native Linux verification receipt")
    for key in ("target", "platform", "version", "tag", "commit", "architecture"):
        if native.get(key) != smoke.get(key):
            canary.fail(f"signed Linux receipt {key} differs from smoke evidence")
    artifacts = native.get("artifacts", [])
    asset = smoke["asset"]
    if not isinstance(artifacts, list) or len(artifacts) != 1 or any(
        artifacts[0].get(key) != asset.get(asset_key)
        for key, asset_key in (("filename", "filename"), ("sha256", "sha256"))
    ):
        canary.fail("signed Linux receipt does not bind the smoked package bytes")
    bound = {
        **native,
        **{key: smoke[key] for key in SMOKE_FIELDS},
        "signingStatus": "signed",
        "productionEligible": False,
        "warning": WARNING,
    }
    validate_canary_receipt(bound)
    return bound
