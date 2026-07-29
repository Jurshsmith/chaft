#!/usr/bin/env python3
"""Platform-independent tests for native verification receipt generation.

The native commands are represented by strict subprocess stubs. These tests cover
command construction, result parsing, fail-closed behavior, byte binding, and the
receipt contract; they do not claim to exercise unavailable platform-native tools.
"""

from __future__ import annotations

import hashlib
import io
import importlib.util
import json
import plistlib
import struct
import sys
import tarfile
import tempfile
import unittest
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Sequence


SCRIPT = Path(__file__).with_name("generate-platform-verification-receipt.py")
SPEC = importlib.util.spec_from_file_location("chaft_native_receipt", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load {SCRIPT}")
native = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = native
SPEC.loader.exec_module(native)


VERSION = "1.2.3"
TAG = f"v{VERSION}"
COMMIT = "0123456789abcdef0123456789abcdef01234567"
ARCHITECTURE = "x86_64"
SIGNER_FINGERPRINT = "A" * 40
PRIMARY_FINGERPRINT = "B" * 40
AUTHENTICODE_THUMBPRINT = "C" * 40
AUTHENTICODE_THUMBPRINT_SHA256 = "E" * 64
APPLE_TEAM_ID = "A1B2C3D4E5"
VERIFIED_AT = datetime(2026, 7, 18, 12, 34, 56, tzinfo=timezone.utc)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fake_pe(architecture: str = "x86_64") -> bytes:
    machines = {"x86_64": 0x8664, "arm64": 0xAA64}
    value = bytearray(256)
    value[:2] = b"MZ"
    struct.pack_into("<I", value, 0x3C, 0x80)
    value[0x80:0x84] = b"PE\x00\x00"
    struct.pack_into("<H", value, 0x84, machines[architecture])
    return bytes(value)


def fake_elf(architecture: str = "x86_64") -> bytes:
    machines = {"x86_64": 62, "arm64": 183}
    value = bytearray(64)
    value[:4] = b"\x7fELF"
    value[4] = 2
    value[5] = 1
    struct.pack_into("<H", value, 18, machines[architecture])
    return bytes(value)


class FakeRunner:
    def __init__(self, responder: Callable[[tuple[str, ...]], native.CommandResult]):
        self.responder = responder
        self.calls: list[tuple[str, ...]] = []

    def run(self, args: Sequence[str]) -> native.CommandResult:
        call = tuple(str(value) for value in args)
        self.calls.append(call)
        return self.responder(call)


def fake_which(name: str) -> str:
    return f"/native/{name}"


def authenticode_json(status: str = "Valid", msi_template: str = "") -> str:
    return json.dumps(
        {
            "Status": status,
            "StatusMessage": "Signature is valid" if status == "Valid" else "Unsigned",
            "SignatureType": "Authenticode" if status == "Valid" else "None",
            "SignerThumbprintSha1": (
                AUTHENTICODE_THUMBPRINT if status == "Valid" else ""
            ),
            "SignerThumbprintSha256": (
                AUTHENTICODE_THUMBPRINT_SHA256 if status == "Valid" else ""
            ),
            "MsiTemplate": msi_template,
        }
    )


class NativeReceiptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="chaft-native-receipt-test-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def package_dir(self, platform: str) -> Path:
        path = self.root / f"{platform}-packages"
        path.mkdir()
        return path

    def output(self, platform: str) -> Path:
        directory = self.root / "receipts"
        return directory / native.RECEIPT_FILENAMES[platform]

    def generate(self, platform: str, package_dir: Path, runner: FakeRunner, **values: object):
        arguments: dict[str, object] = {
            "platform": platform,
            "package_dir": package_dir,
            "output": self.output(platform),
            "version": VERSION,
            "tag": TAG,
            "commit": COMMIT,
            "architecture": ARCHITECTURE,
            "runner": runner,
            "which": fake_which,
            "host_platform": platform,
            "now": lambda: VERIFIED_AT,
        }
        if platform == "windows":
            arguments["trusted_windows_signer_thumbprint"] = AUTHENTICODE_THUMBPRINT
        elif platform == "macos":
            arguments["trusted_apple_team_id"] = APPLE_TEAM_ID
        arguments.update(values)
        return native.generate_receipt(**arguments)

    def test_windows_verifies_exe_and_every_pe_in_safe_zip(self) -> None:
        package_dir = self.package_dir("windows")
        executable = package_dir / f"Chaft-{VERSION}-Windows.exe"
        executable.write_bytes(fake_pe())
        archive_path = package_dir / f"Chaft-{VERSION}-Windows.zip"
        with zipfile.ZipFile(archive_path, "w") as archive:
            archive.writestr("Chaft/Chaft.exe", fake_pe())
            archive.writestr("Chaft/plugins/helper.dll", fake_pe())
            archive.writestr("Chaft/readme.txt", b"documentation")

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if "-File" in call:
                return native.CommandResult(0, authenticode_json())
            if "-Command" in call:
                return native.CommandResult(0, "5.1.22621.2506\n")
            return native.CommandResult(1, stderr=f"unexpected command: {call}")

        runner = FakeRunner(respond)
        receipt = self.generate("windows", package_dir, runner)

        self.assertEqual(receipt["schemaVersion"], native.SCHEMA_VERSION)
        self.assertEqual(receipt["verificationType"], "authenticode")
        self.assertEqual(receipt["status"], "verified")
        self.assertEqual(receipt["verifiedAt"], "2026-07-18T12:34:56Z")
        self.assertEqual(receipt["verifier"]["version"], "5.1.22621.2506")
        self.assertEqual(
            receipt["verificationPolicy"],
            {
                "publisherIdentity": {
                    "type": "authenticode-signer-certificate-thumbprint",
                    "value": AUTHENTICODE_THUMBPRINT,
                    "algorithm": "sha1",
                }
            },
        )
        self.assertEqual(
            receipt["artifacts"],
            [
                {"filename": executable.name, "sha256": sha256(executable)},
                {"filename": archive_path.name, "sha256": sha256(archive_path)},
            ],
        )
        self.assertEqual(receipt["signatures"], [])
        verification_calls = [call for call in runner.calls if "-File" in call]
        self.assertEqual(len(verification_calls), 3)
        self.assertTrue(all("-Path" in call for call in verification_calls))
        self.assertEqual(json.loads(self.output("windows").read_text()), receipt)

    def test_windows_rejects_unsigned_payload_and_does_not_write_receipt(self) -> None:
        package_dir = self.package_dir("windows")
        (package_dir / f"Chaft-{VERSION}-Windows.exe").write_bytes(fake_pe())

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if "-File" in call:
                return native.CommandResult(0, authenticode_json("NotSigned"))
            return native.CommandResult(0, "7.5.2\n")

        with self.assertRaisesRegex(native.VerificationError, "not valid"):
            self.generate("windows", package_dir, FakeRunner(respond))
        self.assertFalse(self.output("windows").exists())

    def test_windows_rejects_valid_signature_from_unpinned_signer(self) -> None:
        package_dir = self.package_dir("windows")
        (package_dir / f"Chaft-{VERSION}-Windows.exe").write_bytes(fake_pe())

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if "-File" in call:
                return native.CommandResult(0, authenticode_json())
            return native.CommandResult(0, "7.5.2\n")

        with self.assertRaisesRegex(native.VerificationError, "signer identity mismatch"):
            self.generate(
                "windows",
                package_dir,
                FakeRunner(respond),
                trusted_windows_signer_thumbprint="D" * 40,
            )
        self.assertFalse(self.output("windows").exists())

    def test_windows_accepts_exact_pinned_sha256_certificate_hash(self) -> None:
        package_dir = self.package_dir("windows")
        (package_dir / f"Chaft-{VERSION}-Windows.exe").write_bytes(fake_pe())

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if "-File" in call:
                return native.CommandResult(0, authenticode_json())
            return native.CommandResult(0, "7.5.2\n")

        receipt = self.generate(
            "windows",
            package_dir,
            FakeRunner(respond),
            trusted_windows_signer_thumbprint=AUTHENTICODE_THUMBPRINT_SHA256,
        )
        self.assertEqual(
            receipt["verificationPolicy"]["publisherIdentity"],
            {
                "type": "authenticode-signer-certificate-thumbprint",
                "value": AUTHENTICODE_THUMBPRINT_SHA256,
                "algorithm": "sha256",
            },
        )
        payload = receipt["verificationDetails"][0]["verifiedPayloads"][0]
        self.assertEqual(payload["signerThumbprintAlgorithm"], "sha256")

    def test_windows_rejects_one_mismatched_signer_among_zip_payloads(self) -> None:
        package_dir = self.package_dir("windows")
        archive_path = package_dir / f"Chaft-{VERSION}-Windows.zip"
        with zipfile.ZipFile(archive_path, "w") as archive:
            archive.writestr("Chaft/Chaft.exe", fake_pe())
            archive.writestr("Chaft/helper.dll", fake_pe())
        verification_count = 0

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            nonlocal verification_count
            if "-File" in call:
                verification_count += 1
                value = json.loads(authenticode_json())
                if verification_count == 2:
                    value["SignerThumbprintSha1"] = "D" * 40
                return native.CommandResult(0, json.dumps(value))
            return native.CommandResult(0, "7.5.2\n")

        with self.assertRaisesRegex(native.VerificationError, "signer identity mismatch"):
            self.generate("windows", package_dir, FakeRunner(respond))
        self.assertEqual(verification_count, 2)
        self.assertFalse(self.output("windows").exists())

    def test_windows_binds_msi_architecture_from_installer_metadata(self) -> None:
        package_dir = self.package_dir("windows")
        installer = package_dir / f"Chaft-{VERSION}-Windows.msi"
        installer.write_bytes(b"synthetic MSI bytes")

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if "-File" in call:
                return native.CommandResult(0, authenticode_json(msi_template="x64;1033"))
            return native.CommandResult(0, "5.1.22621.2506\n")

        receipt = self.generate("windows", package_dir, FakeRunner(respond))
        detail = receipt["verificationDetails"][0]
        self.assertEqual(detail["architecture"], "x86_64")
        self.assertEqual(
            detail["verifiedPayloads"][0]["msiTemplate"], "x64;1033"
        )

    def test_windows_rejects_claimed_architecture_not_found_in_signed_pe(self) -> None:
        package_dir = self.package_dir("windows")
        (package_dir / f"Chaft-{VERSION}-Windows.exe").write_bytes(fake_pe("arm64"))

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if "-File" in call:
                return native.CommandResult(0, authenticode_json())
            return native.CommandResult(0, "7.5.2\n")

        with self.assertRaisesRegex(native.VerificationError, "not the requested x86_64"):
            self.generate("windows", package_dir, FakeRunner(respond))
        self.assertFalse(self.output("windows").exists())

    def test_windows_rejects_unsafe_zip_before_running_authenticode(self) -> None:
        package_dir = self.package_dir("windows")
        archive_path = package_dir / f"Chaft-{VERSION}-Windows.zip"
        with zipfile.ZipFile(archive_path, "w") as archive:
            archive.writestr("../escape.exe", b"MZpayload")
        runner = FakeRunner(lambda _call: native.CommandResult(0, "7.5.2\n"))

        with self.assertRaisesRegex(native.VerificationError, "unsafe entry"):
            self.generate("windows", package_dir, runner)
        self.assertFalse(any("-File" in call for call in runner.calls))
        self.assertFalse((self.root / "escape.exe").exists())

    def test_windows_rejects_ntfs_alternate_stream_zip_entry(self) -> None:
        package_dir = self.package_dir("windows")
        archive_path = package_dir / f"Chaft-{VERSION}-Windows.zip"
        with zipfile.ZipFile(archive_path, "w") as archive:
            archive.writestr("Chaft/Chaft.exe:payload", fake_pe())
        runner = FakeRunner(lambda _call: native.CommandResult(0, "7.5.2\n"))

        with self.assertRaisesRegex(native.VerificationError, "unsafe entry"):
            self.generate("windows", package_dir, runner)
        self.assertFalse(any("-File" in call for call in runner.calls))

    def test_macos_runs_all_three_checks_for_dmg_and_app_then_detaches(self) -> None:
        package_dir = self.package_dir("macos")
        dmg = package_dir / f"Chaft-{VERSION}-macOS.dmg"
        dmg.write_bytes(b"synthetic disk image")

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if call[0].endswith("xcrun") and call[1:] == ("--version",):
                return native.CommandResult(0, "xcrun version 72.\n")
            if call[0].endswith("sw_vers"):
                return native.CommandResult(0, "15.5\n")
            if call[0].endswith("codesign") and "--display" in call:
                return native.CommandResult(0, stderr=f"TeamIdentifier={APPLE_TEAM_ID}\n")
            if call[0].endswith("hdiutil") and "attach" in call:
                mountpoint = Path(call[call.index("-mountpoint") + 1])
                contents = mountpoint / "Chaft.app" / "Contents"
                (contents / "MacOS").mkdir(parents=True)
                (contents / "Resources").mkdir()
                (contents / "Info.plist").write_bytes(
                    plistlib.dumps(
                        {
                            "CFBundleName": "Chaft",
                            "CFBundleExecutable": "Chaft",
                            "CFBundleIconFile": "Chaft.icns",
                        }
                    )
                )
                (contents / "MacOS" / "Chaft").write_bytes(b"Mach-O fixture")
                (contents / "Resources" / "Chaft.icns").write_bytes(b"icon fixture")
                plist = plistlib.dumps(
                    {"system-entities": [{"mount-point": str(mountpoint)}]}
                ).decode()
                return native.CommandResult(0, plist)
            if call[0].endswith("lipo"):
                return native.CommandResult(0, "x86_64\n")
            return native.CommandResult(0)

        runner = FakeRunner(respond)
        receipt = self.generate("macos", package_dir, runner)

        self.assertEqual(receipt["verificationType"], "apple-notarization")
        self.assertEqual(receipt["signatures"], [])
        commands = [" ".join(call) for call in runner.calls]
        self.assertEqual(sum("codesign --verify" in command for command in commands), 2)
        self.assertEqual(sum("spctl --assess" in command for command in commands), 2)
        self.assertEqual(sum("xcrun stapler validate" in command for command in commands), 2)
        self.assertEqual(sum("hdiutil detach" in command for command in commands), 1)
        self.assertEqual(sum("codesign --display" in command for command in commands), 2)
        self.assertEqual(
            receipt["verificationPolicy"]["publisherIdentity"],
            {"type": "apple-developer-team-id", "value": APPLE_TEAM_ID},
        )

    def test_macos_detaches_after_app_verification_failure(self) -> None:
        package_dir = self.package_dir("macos")
        (package_dir / f"Chaft-{VERSION}-macOS.dmg").write_bytes(b"disk image")

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if call[0].endswith("xcrun") and call[1:] == ("--version",):
                return native.CommandResult(0, "xcrun version 72.\n")
            if call[0].endswith("sw_vers"):
                return native.CommandResult(0, "15.5\n")
            if call[0].endswith("codesign") and "--display" in call:
                return native.CommandResult(0, stderr=f"TeamIdentifier={APPLE_TEAM_ID}\n")
            if call[0].endswith("hdiutil") and "attach" in call:
                mountpoint = Path(call[call.index("-mountpoint") + 1])
                app = mountpoint / "Chaft.app"
                contents = app / "Contents"
                (contents / "MacOS").mkdir(parents=True)
                (contents / "Resources").mkdir()
                (contents / "Info.plist").write_bytes(
                    plistlib.dumps(
                        {
                            "CFBundleName": "Chaft",
                            "CFBundleExecutable": "Chaft",
                            "CFBundleIconFile": "Chaft.icns",
                        }
                    )
                )
                (contents / "MacOS" / "Chaft").write_bytes(b"Mach-O fixture")
                (contents / "Resources" / "Chaft.icns").write_bytes(b"icon fixture")
                return native.CommandResult(
                    0,
                    plistlib.dumps(
                        {"system-entities": [{"mount-point": str(mountpoint)}]}
                    ).decode(),
                )
            if call[0].endswith("codesign") and call[-1].endswith("Chaft.app"):
                return native.CommandResult(1, stderr="invalid nested signature")
            return native.CommandResult(0)

        runner = FakeRunner(respond)
        with self.assertRaisesRegex(native.VerificationError, "codesign verification"):
            self.generate("macos", package_dir, runner)
        self.assertTrue(
            any(call[0].endswith("hdiutil") and "detach" in call for call in runner.calls)
        )
        self.assertFalse(self.output("macos").exists())

    def test_macos_rejects_valid_signature_from_unpinned_team(self) -> None:
        package_dir = self.package_dir("macos")
        (package_dir / f"Chaft-{VERSION}-macOS.dmg").write_bytes(b"disk image")

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if call[0].endswith("xcrun") and call[1:] == ("--version",):
                return native.CommandResult(0, "xcrun version 72.\n")
            if call[0].endswith("sw_vers"):
                return native.CommandResult(0, "15.5\n")
            if call[0].endswith("codesign") and "--display" in call:
                return native.CommandResult(0, stderr="TeamIdentifier=Z9Y8X7W6V5\n")
            return native.CommandResult(0)

        runner = FakeRunner(respond)
        with self.assertRaisesRegex(native.VerificationError, "Team ID mismatch"):
            self.generate("macos", package_dir, runner)
        self.assertFalse(any("attach" in call for call in runner.calls))
        self.assertFalse(self.output("macos").exists())

    def test_macos_rejects_app_team_that_differs_from_pinned_dmg_team(self) -> None:
        package_dir = self.package_dir("macos")
        (package_dir / f"Chaft-{VERSION}-macOS.dmg").write_bytes(b"disk image")
        display_count = 0

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            nonlocal display_count
            if call[0].endswith("xcrun") and call[1:] == ("--version",):
                return native.CommandResult(0, "xcrun version 72.\n")
            if call[0].endswith("sw_vers"):
                return native.CommandResult(0, "15.5\n")
            if call[0].endswith("codesign") and "--display" in call:
                display_count += 1
                team_id = APPLE_TEAM_ID if display_count == 1 else "Z9Y8X7W6V5"
                return native.CommandResult(0, stderr=f"TeamIdentifier={team_id}\n")
            if call[0].endswith("hdiutil") and "attach" in call:
                mountpoint = Path(call[call.index("-mountpoint") + 1])
                contents = mountpoint / "Chaft.app" / "Contents"
                (contents / "MacOS").mkdir(parents=True)
                (contents / "Resources").mkdir()
                (contents / "Info.plist").write_bytes(
                    plistlib.dumps(
                        {
                            "CFBundleName": "Chaft",
                            "CFBundleExecutable": "Chaft",
                            "CFBundleIconFile": "Chaft.icns",
                        }
                    )
                )
                (contents / "MacOS" / "Chaft").write_bytes(b"Mach-O fixture")
                (contents / "Resources" / "Chaft.icns").write_bytes(b"icon fixture")
                return native.CommandResult(
                    0,
                    plistlib.dumps(
                        {"system-entities": [{"mount-point": str(mountpoint)}]}
                    ).decode(),
                )
            return native.CommandResult(0)

        runner = FakeRunner(respond)
        with self.assertRaisesRegex(native.VerificationError, "Team ID mismatch"):
            self.generate("macos", package_dir, runner)
        self.assertEqual(display_count, 2)
        self.assertTrue(
            any(call[0].endswith("hdiutil") and "detach" in call for call in runner.calls)
        )
        self.assertFalse(self.output("macos").exists())

    def test_linux_verifies_every_signature_with_explicit_keyring_and_primary_key(self) -> None:
        package_dir = self.package_dir("linux")
        packages = [
            package_dir / f"Chaft-{VERSION}-Linux.AppImage",
            package_dir / f"Chaft-{VERSION}-Linux.tar.gz",
        ]
        for index, package in enumerate(packages):
            if package.name.lower().endswith(".appimage"):
                package.write_bytes(fake_elf())
            else:
                with tarfile.open(package, "w:gz") as archive:
                    payload = fake_elf()
                    info = tarfile.TarInfo("Chaft/bin/chaft")
                    info.size = len(payload)
                    archive.addfile(info, io.BytesIO(payload))
            package.with_name(f"{package.name}.asc").write_bytes(f"signature {index}".encode())
        keyring = self.root / "trusted-release-keys.gpg"
        keyring.write_bytes(b"synthetic test keyring")
        status = (
            "[GNUPG:] NEWSIG\n"
            f"[GNUPG:] VALIDSIG {SIGNER_FINGERPRINT} 2026-07-18 0 0 4 0 1 10 00 "
            f"{PRIMARY_FINGERPRINT}\n"
        )

        def respond(call: tuple[str, ...]) -> native.CommandResult:
            if call[1:] == ("--version",):
                return native.CommandResult(0, "gpg (GnuPG) 2.4.8\n")
            if "--verify" in call:
                return native.CommandResult(0, status)
            return native.CommandResult(1, stderr=f"unexpected command: {call}")

        runner = FakeRunner(respond)
        receipt = self.generate(
            "linux",
            package_dir,
            runner,
            trusted_keyring=keyring,
            trusted_fingerprint=PRIMARY_FINGERPRINT,
        )

        self.assertEqual(receipt["verificationType"], "detached-signature")
        self.assertEqual(
            receipt["verificationPolicy"]["publisherIdentity"],
            {
                "type": "openpgp-primary-key-fingerprint",
                "value": PRIMARY_FINGERPRINT,
            },
        )
        verify_calls = [call for call in runner.calls if "--verify" in call]
        self.assertEqual(len(verify_calls), 2)
        self.assertTrue(all("--no-default-keyring" in call for call in verify_calls))
        self.assertTrue(all("--no-auto-key-retrieve" in call for call in verify_calls))
        self.assertTrue(
            all(
                Path(call[call.index("--keyring") + 1]).name == keyring.name
                and call[call.index("--keyring") + 1] != str(keyring.resolve())
                for call in verify_calls
            )
        )
        details = receipt["verificationDetails"]
        self.assertTrue(
            all(
                row["signature"]["signerFingerprint"] == SIGNER_FINGERPRINT
                and row["signature"]["trustedFingerprint"] == PRIMARY_FINGERPRINT
                and row["signature"]["trustedKeyring"]["sha256"] == sha256(keyring)
                for row in details
            )
        )
        self.assertEqual(
            receipt["signatures"],
            [
                {
                    "filename": f"{package.name}.asc",
                    "signedArtifact": package.name,
                    "sha256": sha256(package.with_name(f"{package.name}.asc")),
                    "signerFingerprint": SIGNER_FINGERPRINT,
                    "trustedFingerprint": PRIMARY_FINGERPRINT,
                }
                for package in packages
            ],
        )

    def test_linux_rejects_missing_signature_and_wrong_fingerprint(self) -> None:
        package_dir = self.package_dir("linux")
        package = package_dir / f"Chaft-{VERSION}-Linux.AppImage"
        package.write_bytes(fake_elf())
        keyring = self.root / "trusted.gpg"
        keyring.write_bytes(b"keyring")
        runner = FakeRunner(lambda _call: native.CommandResult(0, "gpg (GnuPG) 2.4.8\n"))
        with self.assertRaisesRegex(native.VerificationError, "no detached"):
            self.generate(
                "linux",
                package_dir,
                runner,
                trusted_keyring=keyring,
                trusted_fingerprint=PRIMARY_FINGERPRINT,
            )

        package.with_name(f"{package.name}.sig").write_bytes(b"signature")

        def wrong_signer(call: tuple[str, ...]) -> native.CommandResult:
            if call[1:] == ("--version",):
                return native.CommandResult(0, "gpg (GnuPG) 2.4.8\n")
            return native.CommandResult(
                0,
                f"[GNUPG:] VALIDSIG {'D' * 40} 2026-07-18 0 0 4 0 1 10 00 {'E' * 40}\n",
            )

        with self.assertRaisesRegex(native.VerificationError, "trusted fingerprint"):
            self.generate(
                "linux",
                package_dir,
                FakeRunner(wrong_signer),
                trusted_keyring=keyring,
                trusted_fingerprint=PRIMARY_FINGERPRINT,
            )
        self.assertFalse(self.output("linux").exists())

    def test_linux_rejects_signature_bytes_changed_during_verification(self) -> None:
        package_dir = self.package_dir("linux")
        package = package_dir / f"Chaft-{VERSION}-Linux.AppImage"
        package.write_bytes(fake_elf())
        signature = package.with_name(f"{package.name}.sig")
        signature.write_bytes(b"original signature")
        keyring = self.root / "trusted.gpg"
        keyring.write_bytes(b"keyring")
        status = (
            f"[GNUPG:] VALIDSIG {SIGNER_FINGERPRINT} 2026-07-18 0 0 4 0 1 10 00 "
            f"{PRIMARY_FINGERPRINT}\n"
        )

        def mutate_signature(call: tuple[str, ...]) -> native.CommandResult:
            if call[1:] == ("--version",):
                return native.CommandResult(0, "gpg (GnuPG) 2.4.8\n")
            signature.write_bytes(b"substituted signature")
            return native.CommandResult(0, status)

        with self.assertRaisesRegex(native.VerificationError, "signature changed"):
            self.generate(
                "linux",
                package_dir,
                FakeRunner(mutate_signature),
                trusted_keyring=keyring,
                trusted_fingerprint=PRIMARY_FINGERPRINT,
            )
        self.assertFalse(self.output("linux").exists())

    def test_fails_closed_on_non_native_host_and_wrong_output_name(self) -> None:
        package_dir = self.package_dir("windows")
        (package_dir / f"Chaft-{VERSION}-Windows.exe").write_bytes(fake_pe())
        runner = FakeRunner(lambda _call: native.CommandResult(0, "7.5.2\n"))
        with self.assertRaisesRegex(native.VerificationError, "native windows host"):
            self.generate("windows", package_dir, runner, host_platform="linux")
        with self.assertRaisesRegex(native.VerificationError, "must be named"):
            self.generate(
                "windows",
                package_dir,
                runner,
                output=self.root / "receipts" / "wrong.json",
            )
        with self.assertRaisesRegex(native.VerificationError, "only for macOS"):
            self.generate(
                "windows",
                package_dir,
                runner,
                architecture="universal",
            )
        self.assertEqual(runner.calls, [])

    def test_publisher_identity_pins_are_mandatory_and_platform_scoped(self) -> None:
        windows_dir = self.package_dir("windows")
        (windows_dir / f"Chaft-{VERSION}-Windows.exe").write_bytes(fake_pe())
        macos_dir = self.package_dir("macos")
        (macos_dir / f"Chaft-{VERSION}-macOS.dmg").write_bytes(b"disk image")
        linux_dir = self.package_dir("linux")
        (linux_dir / f"Chaft-{VERSION}-Linux.AppImage").write_bytes(fake_elf())
        runner = FakeRunner(lambda _call: native.CommandResult(0))

        with self.assertRaisesRegex(native.VerificationError, "requires.*thumbprint"):
            self.generate(
                "windows",
                windows_dir,
                runner,
                trusted_windows_signer_thumbprint=None,
            )
        with self.assertRaisesRegex(native.VerificationError, "requires.*apple-team-id"):
            self.generate(
                "macos",
                macos_dir,
                runner,
                trusted_apple_team_id=None,
            )
        with self.assertRaisesRegex(native.VerificationError, "invalid for Linux"):
            self.generate(
                "linux",
                linux_dir,
                runner,
                trusted_windows_signer_thumbprint=AUTHENTICODE_THUMBPRINT,
            )
        self.assertEqual(runner.calls, [])

    def test_rejects_package_bytes_changed_during_native_verification(self) -> None:
        package_dir = self.package_dir("windows")
        package = package_dir / f"Chaft-{VERSION}-Windows.exe"
        package.write_bytes(fake_pe())

        def mutate_source(call: tuple[str, ...]) -> native.CommandResult:
            if "-File" in call:
                package.write_bytes(fake_pe("arm64"))
                return native.CommandResult(0, authenticode_json())
            return native.CommandResult(0, "7.5.2\n")

        with self.assertRaisesRegex(native.VerificationError, "changed during"):
            self.generate("windows", package_dir, FakeRunner(mutate_source))
        self.assertFalse(self.output("windows").exists())


if __name__ == "__main__":
    unittest.main()
