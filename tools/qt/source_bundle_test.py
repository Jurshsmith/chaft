#!/usr/bin/env python3
"""Network-free tests for the deterministic Qt corresponding-source bundle."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock
from urllib.parse import urlparse
import zipfile

import build_qt as qt
import source_bundle as bundle


class SourceBundleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.manifest_path = self.root / "qt-6.8.4.json"
        self.package_dir = self.root / "packaging" / "qt"
        self.package_dir.mkdir(parents=True)

        manifest = json.loads(bundle.QT_MANIFEST_PATH.read_text(encoding="utf-8"))
        corresponding = json.loads(
            (
                bundle.PACKAGE_QT_DIR / bundle.CORRESPONDING_SOURCE_NAME
            ).read_text(encoding="utf-8")
        )
        self.material_bytes: dict[str, bytes] = {}

        for row in manifest["modules"]:
            filename = Path(urlparse(row["url"]).path).name
            content = f"small fake source archive: {row['name']}\n".encode()
            self.material_bytes[filename] = content
            row["sha256"] = qt.sha256_bytes(content)
        for row in manifest["patches"]:
            filename = Path(urlparse(row["url"]).path).name
            content = f"small fake security patch: {filename}\n".encode()
            self.material_bytes[filename] = content
            row["sha256"] = qt.sha256_bytes(content)

        digest = qt.unchecked_contract_digest(manifest)[:20]
        manifest["sdkIdentities"] = {
            platform_name: (
                f"qt-{manifest['qtVersion']}-r{manifest['sdkRevision']}-"
                f"{platform_name}-{platform_specification['architecture']}-"
                f"{platform_specification['toolchain']}-{digest}"
            )
            for platform_name, platform_specification in manifest[
                "platforms"
            ].items()
        }
        self.manifest = manifest

        module_hashes = {
            row["name"]: row["sha256"] for row in manifest["modules"]
        }
        for row in corresponding["sourceModules"]:
            row["sha256"] = module_hashes[row["name"]]
        patch_hashes = {
            row["url"]: row["sha256"] for row in manifest["patches"]
        }
        for row in corresponding["securityPatches"]:
            row["sha256"] = patch_hashes[row["url"]]
        self.corresponding = corresponding

        self.manifest_path.write_text(
            json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
        )
        for filename in bundle.PACKAGE_FILES:
            path = self.package_dir / filename
            if filename == bundle.CORRESPONDING_SOURCE_NAME:
                path.write_text(
                    json.dumps(corresponding, indent=2) + "\n",
                    encoding="utf-8",
                )
            else:
                path.write_text(f"fake package record: {filename}\n", encoding="utf-8")

        self.downloaded: list[str] = []

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def fake_download(self, row, download_dir):
        filename = Path(urlparse(row["url"]).path).name
        self.downloaded.append(filename)
        destination = Path(download_dir) / filename
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(self.material_bytes[filename])
        return destination

    def build_at(self, parent: Path) -> Path:
        output = parent / bundle.BUNDLE_NAME
        with mock.patch.object(
            bundle.qt, "download_verified", side_effect=self.fake_download
        ):
            bundle.build_bundle(
                output,
                parent / "downloads",
                manifest_path=self.manifest_path,
                package_dir=self.package_dir,
            )
        return output

    def refresh_external_checksum(self, output: Path) -> None:
        output.with_name(bundle.CHECKSUM_NAME).write_text(
            f"{qt.sha256_file(output)}  {output.name}\n",
            encoding="ascii",
        )

    def read_entries(self, output: Path) -> dict[str, bytes]:
        with zipfile.ZipFile(output, "r") as archive:
            return {
                info.filename: archive.read(info)
                for info in archive.infolist()
            }

    def verify(self, output: Path) -> None:
        bundle.verify_bundle(
            output,
            manifest_path=self.manifest_path,
            package_dir=self.package_dir,
        )

    def test_build_is_byte_deterministic_and_downloads_every_material(self) -> None:
        first = self.build_at(self.root / "first")
        second = self.build_at(self.root / "second")

        self.assertEqual(first.read_bytes(), second.read_bytes())
        self.assertEqual(
            first.with_name(bundle.CHECKSUM_NAME).read_bytes(),
            second.with_name(bundle.CHECKSUM_NAME).read_bytes(),
        )
        expected_downloads = {
            Path(urlparse(row["url"]).path).name
            for row in (
                self.corresponding["sourceModules"]
                + self.corresponding["securityPatches"]
            )
        }
        self.assertEqual(set(self.downloaded), expected_downloads)
        self.assertEqual(len(self.downloaded), 2 * 11)

    def test_exact_layout_stored_metadata_and_checksums_verify(self) -> None:
        output = self.build_at(self.root / "layout")
        with zipfile.ZipFile(output, "r") as archive:
            infos = archive.infolist()
            expected = bundle.expected_entry_names(self.corresponding)
            self.assertEqual([info.filename for info in infos], sorted(expected))
            for info in infos:
                with self.subTest(entry=info.filename):
                    self.assertEqual(info.date_time, bundle.FIXED_ZIP_TIMESTAMP)
                    self.assertEqual(info.compress_type, zipfile.ZIP_STORED)
                    self.assertEqual(info.create_system, 3)
                    self.assertEqual(
                        info.external_attr >> 16, bundle.REGULAR_FILE_MODE
                    )
            sums = bundle._parse_sha256sums(
                archive.read("SHA256SUMS"),
                sorted(expected - {"SHA256SUMS"}),
            )
            self.assertEqual(set(sums), expected - {"SHA256SUMS"})
        self.verify(output)

    def test_verify_is_network_free(self) -> None:
        output = self.build_at(self.root / "offline")
        with (
            mock.patch.object(
                bundle.qt,
                "download_verified",
                side_effect=AssertionError("offline verify attempted a download"),
            ),
            mock.patch.object(
                bundle.qt,
                "urlopen",
                side_effect=AssertionError("offline verify opened a URL"),
            ),
        ):
            self.verify(output)

    def test_external_tamper_is_rejected_before_zip_processing(self) -> None:
        output = self.build_at(self.root / "external-tamper")
        with output.open("ab") as handle:
            handle.write(b"tampered")
        with self.assertRaisesRegex(
            qt.QtSdkError, "external checksum is invalid"
        ):
            self.verify(output)

    def test_source_tamper_is_rejected_even_if_sums_and_sidecar_are_rebuilt(
        self,
    ) -> None:
        output = self.build_at(self.root / "source-tamper")
        entries = self.read_entries(output)
        source_name = next(
            name for name in sorted(entries) if name.startswith("source-archives/")
        )
        entries[source_name] = b"attacker-replaced-source"
        payload = {
            name: value for name, value in entries.items() if name != "SHA256SUMS"
        }
        entries["SHA256SUMS"] = bundle.sha256sums(payload)
        bundle.write_deterministic_zip(output, entries)
        self.refresh_external_checksum(output)

        with self.assertRaisesRegex(
            qt.QtSdkError, "authoritative digest"
        ):
            self.verify(output)

    def test_extra_path_is_rejected_even_with_valid_sums_and_sidecar(self) -> None:
        output = self.build_at(self.root / "extra-path")
        entries = self.read_entries(output)
        entries["unexpected.txt"] = b"not part of the contract"
        payload = {
            name: value for name, value in entries.items() if name != "SHA256SUMS"
        }
        entries["SHA256SUMS"] = bundle.sha256sums(payload)
        bundle.write_deterministic_zip(output, entries)
        self.refresh_external_checksum(output)

        with self.assertRaisesRegex(qt.QtSdkError, "layout differs"):
            self.verify(output)

    def test_symlink_metadata_is_rejected(self) -> None:
        output = self.build_at(self.root / "symlink")
        entries = self.read_entries(output)
        target_name = "README.md"
        with zipfile.ZipFile(
            output,
            "w",
            compression=zipfile.ZIP_STORED,
            allowZip64=True,
            strict_timestamps=True,
        ) as archive:
            for name in sorted(entries):
                value = entries[name]
                info = bundle._zip_info(name, len(value))
                if name == target_name:
                    info.external_attr = (stat.S_IFLNK | 0o777) << 16
                archive.writestr(info, value)
        self.refresh_external_checksum(output)

        with self.assertRaisesRegex(qt.QtSdkError, "no symlinks"):
            self.verify(output)

    def test_release_asset_names_are_authoritative(self) -> None:
        wrong = self.root / "wrong-name.zip"
        with self.assertRaisesRegex(qt.QtSdkError, "bundle filename must be"):
            with mock.patch.object(
                bundle.qt, "download_verified", side_effect=self.fake_download
            ):
                bundle.build_bundle(
                    wrong,
                    self.root / "downloads",
                    manifest_path=self.manifest_path,
                    package_dir=self.package_dir,
                )

        changed = copy.deepcopy(self.corresponding)
        changed["releaseAssets"]["bundle"] = "renamed.zip"
        with self.assertRaisesRegex(qt.QtSdkError, "releaseAssets"):
            bundle.validate_corresponding_source(changed, self.manifest)

    def test_cli_matches_declarative_release_workflows(self) -> None:
        create = bundle.parse_arguments(
            ["create", "--output-dir", str(self.root / "out")]
        )
        self.assertEqual(create.command, "create")
        self.assertEqual(create.output_dir, self.root / "out")

        bundle_path = self.root / bundle.BUNDLE_NAME
        checksum_path = self.root / bundle.CHECKSUM_NAME
        verify = bundle.parse_arguments(
            [
                "verify",
                "--bundle",
                str(bundle_path),
                "--checksum",
                str(checksum_path),
            ]
        )
        self.assertEqual(verify.command, "verify")
        self.assertEqual(verify.bundle, bundle_path)
        self.assertEqual(verify.checksum, checksum_path)

    def test_verify_rejects_a_non_adjacent_checksum_path(self) -> None:
        output = self.build_at(self.root / "checksum-path")
        wrong = self.root / "elsewhere" / bundle.CHECKSUM_NAME
        wrong.parent.mkdir()
        wrong.write_bytes(output.with_name(bundle.CHECKSUM_NAME).read_bytes())
        with self.assertRaisesRegex(qt.QtSdkError, "named sidecar"):
            bundle.verify_bundle(
                output,
                wrong,
                manifest_path=self.manifest_path,
                package_dir=self.package_dir,
            )


if __name__ == "__main__":
    unittest.main()
