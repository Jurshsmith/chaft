#!/usr/bin/env python3
"""Sign an existing Linux AppImage with an isolated, short-lived GnuPG home."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

from linux_signing_policy import PUBLIC_KEY_FILENAME

SECRET_NAMES = (
    "CHAFT_LINUX_SIGNING_PRIVATE_KEY_BASE64",
    "CHAFT_LINUX_SIGNING_PASSPHRASE",
    "CHAFT_LINUX_SIGNING_KEYRING_BASE64",
    "CHAFT_LINUX_SIGNING_FINGERPRINT",
)


def command_environment():
    return {key: value for key, value in os.environ.items() if key not in SECRET_NAMES}


def run(args, *, data=None):
    result = subprocess.run(
        args, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        env=command_environment(),
        check=False,
    )
    if result.returncode:
        # Never echo imported key material, passphrases, or GnuPG diagnostics.
        raise ValueError("GnuPG operation failed; check the signing key, passphrase, and public keyring")
    return result.stdout


def secret_bytes(name):
    value = os.environ.get(name, "")
    if not value:
        raise ValueError(f"Configure {name}")
    try:
        decoded = base64.b64decode("".join(value.split()), validate=True)
    except ValueError:
        raise ValueError(f"{name} must contain base64-encoded key material") from None
    if not decoded:
        raise ValueError(f"{name} must not decode to empty key material")
    return decoded


def refresh_metadata(directory, package, signature):
    """Add signature evidence while preserving the original build provenance/SBOM."""
    prefix = "chaft-desktop-linux-x86_64"
    provenance_path = directory / f"{prefix}-provenance.json"
    sbom_path = directory / f"{prefix}-sbom.cdx.json"
    provenance = json.loads(provenance_path.read_text())
    sbom = json.loads(sbom_path.read_text())
    artifact = {
        "name": signature.name,
        "packageFormat": "detached-signature",
        "signatureFormat": "asc",
        "signedArtifact": package.name,
        "sizeBytes": signature.stat().st_size,
        "sha256": hashlib.sha256(signature.read_bytes()).hexdigest(),
    }
    provenance["artifacts"].append(artifact)
    provenance["artifacts"].sort(key=lambda row: row["name"])
    sbom["properties"].extend(
        {"name": f"chaft:artifact:{signature.name}:{key}", "value": artifact[key]}
        for key in ("sha256", "packageFormat", "signedArtifact", "signatureFormat")
    )
    for path, document in ((provenance_path, provenance), (sbom_path, sbom)):
        path.write_text(json.dumps(document, indent=2) + "\n")
    (directory / f"{prefix}-SHA256SUMS").write_text("".join(
        f"{row['sha256']}  {row['name']}\n" for row in provenance["artifacts"]
    ))


def sign(directory, public_key_output):
    directory = Path(directory).resolve(strict=True)
    packages = list(directory.glob("*.AppImage"))
    if len(packages) != 1 or packages[0].is_symlink() or not packages[0].is_file():
        raise ValueError("Signing requires exactly one regular AppImage")
    package = packages[0]
    signature = package.with_name(package.name + ".asc")
    public_key = Path(public_key_output)
    if public_key.name != PUBLIC_KEY_FILENAME:
        raise ValueError(f"Public key output must be named {PUBLIC_KEY_FILENAME}")
    if any(path.exists() or path.is_symlink() for path in (signature, public_key)):
        raise ValueError("Refusing to replace an existing signature or public key")
    fingerprint = os.environ.get("CHAFT_LINUX_SIGNING_FINGERPRINT", "").strip().upper()
    if re.fullmatch(r"(?:[0-9A-F]{40}|[0-9A-F]{64})", fingerprint) is None:
        raise ValueError("Configure CHAFT_LINUX_SIGNING_FINGERPRINT as a full OpenPGP fingerprint")
    private_bytes = secret_bytes("CHAFT_LINUX_SIGNING_PRIVATE_KEY_BASE64")
    public_bytes = secret_bytes("CHAFT_LINUX_SIGNING_KEYRING_BASE64")
    passphrase = os.environ.get("CHAFT_LINUX_SIGNING_PASSPHRASE", "")
    if not passphrase or "\n" in passphrase or "\r" in passphrase:
        raise ValueError("Configure a non-empty, single-line CHAFT_LINUX_SIGNING_PASSPHRASE")
    with tempfile.TemporaryDirectory(prefix="chaft-linux-signing-", dir=os.environ.get("RUNNER_TEMP")) as name:
        home = Path(name)
        os.chmod(home, 0o700)
        gpg = ["gpg", "--batch", "--no-tty", "--homedir", str(home)]
        keyring = home / "public.gpg"
        keyring.write_bytes(public_bytes)
        try:
            packets = run([*gpg, "--list-packets"], data=public_bytes)
            if re.search(rb"^:secret (?:sub )?key packet:", packets, re.MULTILINE):
                raise ValueError("The public signing keyring must not contain private key packets")
            run([*gpg, "--import"], data=private_bytes)
            run([
                *gpg, "--pinentry-mode", "loopback", "--passphrase-fd", "0",
                "--local-user", fingerprint, "--armor", "--detach-sign",
                "--output", str(signature), str(package),
            ], data=(passphrase + "\n").encode())
            # Verify using only the independently configured public keyring.
            status = run([
                *gpg, "--no-default-keyring", "--keyring", str(keyring),
                "--no-auto-key-retrieve", "--status-fd", "1", "--verify",
                str(signature), str(package),
            ]).decode()
            valid = [line.split() for line in status.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
            if len(valid) != 1 or fingerprint not in (valid[0][2], valid[0][-1]):
                raise ValueError("Signature does not match the protected Linux signing fingerprint")
            refresh_metadata(directory, package, signature)
            public_key.write_bytes(public_bytes)
        except BaseException:
            signature.unlink(missing_ok=True)
            raise
        finally:
            subprocess.run(
                ["gpgconf", "--homedir", str(home), "--kill", "gpg-agent"],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
                env=command_environment(),
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--public-key-output", required=True, type=Path)
    args = parser.parse_args()
    try:
        sign(args.package_dir, args.public_key_output)
    except (ValueError, OSError) as error:
        parser.exit(2, f"Linux release signing failed: {error}\n")


if __name__ == "__main__":
    main()
