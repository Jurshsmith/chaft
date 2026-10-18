#!/usr/bin/env python3
"""Exercise actual GnuPG signing through canary finalization and manifest export."""

import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
import linux_signing_policy as policy


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, TOOLS / filename)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


signer = load("linux_signer_test", "sign-linux-release.py")
fixtures = load("linux_signing_fixtures", "canary-release-assets-test.py")
native = load("linux_signing_native_test", "generate-platform-verification-receipt.py")
exporter = load("linux_signing_exporter_test", "export-website-release-manifest.py")
smoke_generator = load("linux_signing_smoke_test", "generate-unsigned-canary-receipt.py")
assets_tool = fixtures.assets_tool
TARGET = "linux-x86_64"
PASSPHRASE = "fixture-only-passphrase"


@unittest.skipUnless(shutil.which("gpg") and shutil.which("gpgconf"), "GnuPG is required")
class LinuxReleaseSigningTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.keys = tempfile.TemporaryDirectory(prefix="ls-", dir="/tmp")
        cls.key_home = Path(cls.keys.name)
        os.chmod(cls.key_home, 0o700)
        cls.gpg = ["gpg", "--batch", "--homedir", str(cls.key_home)]
        cls.command([
            "--pinentry-mode", "loopback", "--passphrase", PASSPHRASE,
            "--quick-generate-key", "Chaft signing fixture <fixture@example.invalid>",
            "ed25519", "cert", "1d",
        ])
        listing = cls.command(["--with-colons", "--list-keys"]).decode()
        cls.fingerprint = next(line.split(":")[9] for line in listing.splitlines() if line.startswith("fpr:"))
        cls.command([
            "--pinentry-mode", "loopback", "--passphrase", PASSPHRASE,
            "--quick-add-key", cls.fingerprint, "ed25519", "sign", "1d",
        ])
        cls.public = cls.command(["--export", cls.fingerprint])
        cls.private = cls.command([
            "--pinentry-mode", "loopback", "--passphrase", PASSPHRASE,
            "--export-secret-subkeys", cls.fingerprint,
        ])

    @classmethod
    def command(cls, args):
        result = subprocess.run([*cls.gpg, *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if result.returncode:
            raise AssertionError(result.stderr.decode())
        return result.stdout

    @classmethod
    def tearDownClass(cls):
        subprocess.run(["gpgconf", "--homedir", str(cls.key_home), "--kill", "gpg-agent"], check=True)
        cls.keys.cleanup()

    def setUp(self):
        self.fixture = fixtures.CanaryReleaseAssetsTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)
        self.root = self.fixture.root
        self.assets = self.fixture.assets
        self.package = self.assets / fixtures.PACKAGE_NAMES[TARGET]
        # An ELF x86-64 header is enough to exercise the verifier's architecture
        # inspection; release workflows separately launch the actual AppImage.
        header = bytearray(64)
        header[:6] = b"\x7fELF\x02\x01"
        header[18:20] = (62).to_bytes(2, "little")
        self.package.write_bytes(header + b"synthetic AppImage payload")
        names = assets_tool.stager.METADATA_FILENAMES[TARGET]
        digest = hashlib.sha256(self.package.read_bytes()).hexdigest()
        provenance_path = self.assets / names["provenance"]
        provenance = json.loads(provenance_path.read_text())
        provenance["artifacts"][0].update(sha256=digest, sizeBytes=self.package.stat().st_size)
        fixtures.write_json(provenance_path, provenance)
        sbom_path = self.assets / names["sbom"]
        sbom = json.loads(sbom_path.read_text())
        sbom["properties"][0]["value"] = digest
        fixtures.write_json(sbom_path, sbom)
        (self.assets / names["checksums"]).write_text(f"{digest}  {self.package.name}\n")
        self.directory = self.root / "linux-input"
        self.directory.mkdir()
        for filename in (self.package.name, *names.values()):
            shutil.copyfile(self.assets / filename, self.directory / filename)
        self.public_path = self.root / policy.PUBLIC_KEY_FILENAME
        self.environment = {
            "CHAFT_LINUX_SIGNING_PRIVATE_KEY_BASE64": base64.b64encode(self.private).decode(),
            "CHAFT_LINUX_SIGNING_PASSPHRASE": PASSPHRASE,
            "CHAFT_LINUX_SIGNING_KEYRING_BASE64": base64.b64encode(self.public).decode(),
            "CHAFT_LINUX_SIGNING_FINGERPRINT": self.fingerprint,
        }

    def sign(self, **overrides):
        with mock.patch.dict(os.environ, {**self.environment, **overrides}):
            signer.sign(self.directory, self.public_path)

    def verify(self, directory, output, fingerprint=None):
        return native.generate_receipt(
            target=TARGET, package_dir=directory, output=output,
            version=fixtures.VERSION, tag=fixtures.TAG, commit=fixtures.COMMIT,
            trusted_keyring=self.public_path,
            trusted_fingerprint=fingerprint or self.fingerprint,
            host_platform="linux",
        )

    def test_signed_canary_reaches_verified_manifest(self):
        self.sign()
        for path in self.directory.iterdir():
            shutil.copyfile(path, self.assets / path.name)
        shutil.copyfile(self.public_path, self.assets / self.public_path.name)
        assets_tool.verify_base_assets(
            self.assets, version=fixtures.VERSION, tag=fixtures.TAG,
            commit=fixtures.COMMIT, qt_verifier=self.fixture.qt_verifier,
        )
        fixtures.add_receipts(self.assets)
        receipt_name = assets_tool.release_targets.TARGET_BY_NAME[TARGET].verification_receipt_name
        public_receipt_path = self.assets / receipt_name
        smoke = json.loads(public_receipt_path.read_text())
        native_path = self.root / "native" / receipt_name
        self.verify(self.directory, native_path)
        bound_path = self.root / "bound" / receipt_name
        smoke_generator.generate_receipt(
            target=TARGET, package=self.directory / self.package.name,
            output=bound_path, version=fixtures.VERSION, tag=fixtures.TAG,
            commit=fixtures.COMMIT, repository=fixtures.REPOSITORY,
            release_id=fixtures.RELEASE_ID, asset_id=smoke["asset"]["id"],
            workflow_run_id=200, workflow_run_attempt=1,
            runner_os="Linux", runner_arch="x86_64", smoke_command="appimage-smoke.sh",
            linux_signing_receipt=native_path,
        )
        shutil.copyfile(bound_path, public_receipt_path)
        assets_tool.finalize_assets(
            self.assets, version=fixtures.VERSION, tag=fixtures.TAG,
            commit=fixtures.COMMIT, repository=fixtures.REPOSITORY,
            release_id=fixtures.RELEASE_ID, qt_verifier=self.fixture.qt_verifier,
        )
        self.assertEqual(len(list(self.assets.iterdir())), 26)
        staged = self.root / "staged"
        assets_tool.stager.stage_assets(self.assets, staged, channel="canary")
        trusted_path = self.root / "trusted" / receipt_name
        self.verify(staged / f"{TARGET}-package", trusted_path)
        arguments = dict(
            repository=fixtures.REPOSITORY, tag=fixtures.TAG,
            source_root=assets_tool.REPOSITORY_ROOT,
            published_at="2026-10-18T12:00:00Z", channel="canary",
            package_directories={name: staged / f"{name}-package" for name in fixtures.EXPECTED_TARGETS},
            architectures={target.name: target.architecture for target in assets_tool.release_targets.TARGETS},
            signing_states={name: "signed" if name == TARGET else "unsigned-canary" for name in fixtures.EXPECTED_TARGETS},
            verification_receipts={target.name: staged / "verification-receipts" / target.verification_receipt_name for target in assets_tool.release_targets.TARGETS},
            trusted_verification_receipts={name: trusted_path if name == TARGET else None for name in fixtures.EXPECTED_TARGETS},
            publisher_identities={"linux": self.fingerprint, "windows": None, "macos": None},
            release_evidence_directory=staged / "release-evidence",
            # The base asset audit above already ran the actual metadata verifier.
            verifier=lambda *_args: None, tag_resolver=lambda *_args: fixtures.COMMIT,
        )
        manifest = exporter.build_manifest(**arguments)
        linux = next(asset for asset in manifest["assets"] if asset["os"] == "linux")
        self.assertEqual(linux["signingStatus"], "signed")
        self.assertEqual(linux["evidence"]["signature"]["format"], "asc")
        arguments["trusted_verification_receipts"][TARGET] = None
        with self.assertRaisesRegex(exporter.ManifestExportError, "trusted native"):
            exporter.build_manifest(**arguments)

    def test_tampering_and_wrong_verification_key_are_rejected(self):
        self.sign()
        receipt_name = assets_tool.release_targets.TARGET_BY_NAME[TARGET].verification_receipt_name
        with self.assertRaises(native.VerificationError):
            self.verify(self.directory, self.root / "wrong" / receipt_name, "A" * 40)
        with (self.directory / self.package.name).open("ab") as handle:
            handle.write(b"tampered")
        with self.assertRaises(native.VerificationError):
            self.verify(self.directory, self.root / "tampered" / receipt_name)

    def test_wrong_passphrase_and_signing_identity_leave_no_signature(self):
        for overrides in (
            {"CHAFT_LINUX_SIGNING_PASSPHRASE": "incorrect"},
            {"CHAFT_LINUX_SIGNING_FINGERPRINT": "A" * 40},
        ):
            with self.subTest(overrides=list(overrides)), self.assertRaises(ValueError):
                self.sign(**overrides)
            self.assertFalse((self.directory / (self.package.name + ".asc")).exists())
            self.assertFalse(self.public_path.exists())

    def test_private_material_cannot_be_published_as_public_key(self):
        with self.assertRaisesRegex(ValueError, "private key packets"):
            self.sign(CHAFT_LINUX_SIGNING_KEYRING_BASE64=base64.b64encode(self.private).decode())
        self.assertFalse(self.public_path.exists())

    def test_missing_secrets_never_fall_back_to_unsigned(self):
        for secret in signer.SECRET_NAMES:
            with self.subTest(secret=secret), self.assertRaises(ValueError):
                self.sign(**{secret: ""})
            self.assertFalse((self.directory / (self.package.name + ".asc")).exists())
            self.assertFalse(self.public_path.exists())


if __name__ == "__main__":
    unittest.main()
