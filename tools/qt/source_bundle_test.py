#!/usr/bin/env python3
"""Network-free tests for the deterministic Qt corresponding-source bundle."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import shutil
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
            target_name: (
                f"qt-{manifest['qtVersion']}-r{manifest['sdkRevision']}-"
                f"{target_specification['platform']}-"
                f"{target_specification['architecture']}-"
                f"{target_specification['toolchain']}-{digest}"
            )
            for target_name, target_specification in manifest[
                "targets"
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

    def copy_release_root(self, name: str) -> Path:
        release_root = self.root / name
        for logical, source in qt.recipe_file_paths():
            destination = release_root / logical
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        source_recipe = release_root / "tools" / "qt" / "source_bundle.py"
        shutil.copyfile(Path(bundle.__file__), source_recipe)
        manifest_path = release_root / "tools" / "qt" / "qt-6.8.4.json"
        shutil.copyfile(self.manifest_path, manifest_path)
        shutil.copytree(
            self.package_dir,
            release_root / "packaging" / "qt",
        )
        return release_root

    def rewrite_entry_metadata(
        self,
        output: Path,
        field: str,
        value: object,
    ) -> None:
        entries = self.read_entries(output)
        with zipfile.ZipFile(
            output,
            "w",
            compression=zipfile.ZIP_STORED,
            allowZip64=True,
            strict_timestamps=True,
        ) as archive:
            for name in sorted(entries):
                content = entries[name]
                info = bundle._zip_info(name, len(content))
                if name == "README.md":
                    setattr(info, field, value)
                archive.writestr(info, content)
        self.refresh_external_checksum(output)

    def rewrite_first_entry_flag_bits(
        self,
        output: Path,
        flag_bits: int,
    ) -> None:
        content = bytearray(output.read_bytes())
        for signature, offset in (
            (b"PK\x03\x04", 6),
            (b"PK\x01\x02", 8),
        ):
            position = content.find(signature)
            self.assertGreaterEqual(position, 0)
            content[position + offset : position + offset + 2] = (
                flag_bits.to_bytes(2, "little")
            )
        output.write_bytes(content)
        self.refresh_external_checksum(output)

    def rewrite_first_local_flag_bits(
        self,
        output: Path,
        flag_bits: int,
    ) -> None:
        content = bytearray(output.read_bytes())
        position = content.find(b"PK\x03\x04")
        self.assertGreaterEqual(position, 0)
        content[position + 6 : position + 8] = flag_bits.to_bytes(
            2, "little"
        )
        output.write_bytes(content)
        self.refresh_external_checksum(output)

    def swap_first_two_local_records(self, output: Path) -> None:
        content = bytearray(output.read_bytes())
        with zipfile.ZipFile(output, "r") as archive:
            infos = archive.infolist()
            central_offset = archive.start_dir
            physical_infos = sorted(
                infos, key=lambda info: info.header_offset
            )
        self.assertGreaterEqual(len(physical_infos), 2)
        first, second = physical_infos[:2]
        first_start = first.header_offset
        second_start = second.header_offset
        third_start = (
            physical_infos[2].header_offset
            if len(physical_infos) > 2
            else central_offset
        )
        first_record = bytes(content[first_start:second_start])
        second_record = bytes(content[second_start:third_start])
        content[first_start:third_start] = second_record + first_record
        new_offsets = {
            first.filename: first_start + len(second_record),
            second.filename: first_start,
        }

        cursor = central_offset
        for info in infos:
            record = bundle.CENTRAL_DIRECTORY_HEADER.unpack(
                content[cursor : cursor + bundle.CENTRAL_DIRECTORY_HEADER.size]
            )
            self.assertEqual(
                record[0], bundle.CENTRAL_DIRECTORY_SIGNATURE
            )
            filename_length = record[10]
            extra_length = record[11]
            comment_length = record[12]
            if info.filename in new_offsets:
                offset_position = cursor + 42
                content[offset_position : offset_position + 4] = new_offsets[
                    info.filename
                ].to_bytes(4, "little")
            cursor += (
                bundle.CENTRAL_DIRECTORY_HEADER.size
                + filename_length
                + extra_length
                + comment_length
            )

        output.write_bytes(content)
        self.refresh_external_checksum(output)

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
                    self.assertEqual(info.create_version, 20)
                    self.assertEqual(info.extract_version, 20)
                    self.assertEqual(info.internal_attr, 0)
                    self.assertEqual(
                        info.external_attr,
                        bundle.REGULAR_FILE_MODE << 16,
                    )
                    self.assertEqual(info.flag_bits, 0)
                    self.assertEqual(info.extra, b"")
                    self.assertEqual(info.comment, b"")
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

    def test_noncanonical_metadata_is_rejected_with_a_refreshed_sidecar(
        self,
    ) -> None:
        canonical = self.build_at(self.root / "metadata-canonical")
        mutations = {
            "comment": (b"hidden", "comment must be empty"),
            "extra": (b"\x99\x99\x00\x00", "extra metadata must be empty"),
            "create_system": (0, "normalized Unix metadata"),
            "create_version": (21, "create_version must be 20"),
            "extract_version": (21, "extract_version must be 20"),
            "internal_attr": (1, "internal_attr must be zero"),
            "external_attr": (
                (bundle.REGULAR_FILE_MODE << 16) | 1,
                "regular 0644 file",
            ),
            "compress_type": (
                zipfile.ZIP_DEFLATED,
                "stored compression",
            ),
            "date_time": (
                (1981, 1, 1, 0, 0, 0),
                "non-deterministic timestamp",
            ),
        }
        for field, (value, message) in mutations.items():
            with self.subTest(field=field):
                output = self.root / f"metadata-{field}" / bundle.BUNDLE_NAME
                output.parent.mkdir()
                shutil.copyfile(canonical, output)
                self.rewrite_entry_metadata(output, field, value)
                with self.assertRaisesRegex(qt.QtSdkError, message):
                    self.verify(output)

        output = self.root / "metadata-flag-bits" / bundle.BUNDLE_NAME
        output.parent.mkdir()
        shutil.copyfile(canonical, output)
        self.rewrite_first_entry_flag_bits(output, 8)
        with self.assertRaisesRegex(
            qt.QtSdkError, "flag_bits must be zero"
        ):
            self.verify(output)

    def test_local_header_only_tamper_is_rejected_with_a_refreshed_sidecar(
        self,
    ) -> None:
        canonical = self.build_at(self.root / "local-header-canonical")
        output = self.root / "local-header-flags" / bundle.BUNDLE_NAME
        output.parent.mkdir()
        shutil.copyfile(canonical, output)
        self.rewrite_first_local_flag_bits(output, 8)
        with self.assertRaisesRegex(
            qt.QtSdkError, "local header flag_bits is non-canonical"
        ):
            self.verify(output)

    def test_permuted_local_records_are_rejected_with_a_refreshed_sidecar(
        self,
    ) -> None:
        canonical = self.build_at(self.root / "local-order-canonical")
        output = self.root / "local-order" / bundle.BUNDLE_NAME
        output.parent.mkdir()
        shutil.copyfile(canonical, output)
        self.swap_first_two_local_records(output)
        with self.assertRaisesRegex(
            qt.QtSdkError, "local record order must match"
        ):
            self.verify(output)

    def test_hidden_container_bytes_are_rejected_with_a_refreshed_sidecar(
        self,
    ) -> None:
        canonical = self.build_at(self.root / "container-canonical")
        canonical_bytes = canonical.read_bytes()
        mutations = (
            (
                "prepended",
                b"HIDDEN" + canonical_bytes,
                "central directory|prepended|first local header",
            ),
            (
                "appended",
                canonical_bytes + b"HIDDEN",
                "EOCD must end exactly at EOF",
            ),
            (
                "concatenated",
                canonical_bytes + canonical_bytes,
                "central directory|prepended|concatenated|first local header",
            ),
        )
        for name, content, message in mutations:
            with self.subTest(name=name):
                output = self.root / f"container-{name}" / bundle.BUNDLE_NAME
                output.parent.mkdir()
                output.write_bytes(content)
                self.refresh_external_checksum(output)
                with self.assertRaisesRegex(qt.QtSdkError, message):
                    self.verify(output)

    def test_corresponding_source_requires_exact_json_number_types(self) -> None:
        changed = copy.deepcopy(self.corresponding)
        changed["schemaVersion"] = True
        with self.assertRaisesRegex(qt.QtSdkError, "schemaVersion"):
            bundle.validate_corresponding_source(changed, self.manifest)

        changed = copy.deepcopy(self.corresponding)
        patch = next(
            row for row in changed["securityPatches"] if "part" in row
        )
        patch["part"] = float(patch["part"])
        with self.assertRaisesRegex(qt.QtSdkError, "security patches"):
            bundle.validate_corresponding_source(changed, self.manifest)

    def test_corresponding_source_binds_all_architecture_targets(self) -> None:
        self.assertEqual(
            self.corresponding["targets"],
            [
                {
                    "name": "linux-x86_64",
                    "platform": "Linux",
                    "architecture": "x86_64",
                },
                {
                    "name": "macos-arm64",
                    "platform": "macOS",
                    "architecture": "arm64",
                },
                {
                    "name": "macos-x86_64",
                    "platform": "macOS",
                    "architecture": "x86_64",
                },
                {
                    "name": "windows-x86_64",
                    "platform": "Windows",
                    "architecture": "x86_64",
                },
            ],
        )
        changed = copy.deepcopy(self.corresponding)
        changed["targets"][1]["architecture"] = "x86_64"
        with self.assertRaisesRegex(qt.QtSdkError, "targets differ"):
            bundle.validate_corresponding_source(changed, self.manifest)

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

    def test_release_contract_binds_sdk_recipe_and_bundle_recipe(self) -> None:
        contract = bundle.release_contract(
            self.manifest_path,
            self.package_dir,
            Path(bundle.__file__),
        )
        self.assertEqual(contract["bundleName"], bundle.BUNDLE_NAME)
        self.assertEqual(contract["checksumName"], bundle.CHECKSUM_NAME)
        self.assertEqual(
            contract["sdkManifestSha256"],
            qt.manifest_digest(self.manifest),
        )
        self.assertEqual(
            contract["sdkContractSha256"],
            qt.contract_digest(self.manifest),
        )
        self.assertRegex(contract["contractSha256"], r"^[0-9a-f]{64}$")

        changed_recipe = self.root / "source_bundle.py"
        changed_recipe.write_text("changed recipe\n", encoding="utf-8")
        changed = bundle.release_contract(
            self.manifest_path,
            self.package_dir,
            changed_recipe,
        )
        self.assertNotEqual(
            contract["contractSha256"], changed["contractSha256"]
        )

    def test_trusted_verifier_hashes_detached_recipe_bytes_as_data(self) -> None:
        release_root = self.copy_release_root("detached-release")
        release_driver = release_root / "tools" / "qt" / "build_qt.py"
        release_driver.write_text(
            release_driver.read_text(encoding="utf-8")
            + "\nraise RuntimeError('must not execute detached recipe')\n",
            encoding="utf-8",
        )

        release_manifest_path = (
            release_root / "tools" / "qt" / "qt-6.8.4.json"
        )
        release_manifest = json.loads(
            release_manifest_path.read_text(encoding="utf-8")
        )
        digest = qt.unchecked_contract_digest(
            release_manifest, recipe_root=release_root
        )[:20]
        release_manifest["sdkIdentities"] = {
            target_name: (
                f"qt-{release_manifest['qtVersion']}-"
                f"r{release_manifest['sdkRevision']}-"
                f"{specification['platform']}-"
                f"{specification['architecture']}-"
                f"{specification['toolchain']}-{digest}"
            )
            for target_name, specification in release_manifest[
                "targets"
            ].items()
        }
        release_manifest_path.write_text(
            json.dumps(release_manifest, indent=2) + "\n",
            encoding="utf-8",
        )
        release_package_dir = release_root / "packaging" / "qt"

        output = self.root / "detached-bundle" / bundle.BUNDLE_NAME
        with mock.patch.object(
            bundle.qt, "download_verified", side_effect=self.fake_download
        ):
            bundle.build_bundle(
                output,
                self.root / "detached-downloads",
                manifest_path=release_manifest_path,
                package_dir=release_package_dir,
                recipe_root=release_root,
            )
        bundle.verify_bundle(
            output,
            manifest_path=release_manifest_path,
            package_dir=release_package_dir,
            recipe_root=release_root,
        )
        with self.assertRaisesRegex(qt.QtSdkError, "identities are stale"):
            bundle.verify_bundle(
                output,
                manifest_path=release_manifest_path,
                package_dir=release_package_dir,
            )

    def test_detached_root_rejects_absolute_symlink_inputs(self) -> None:
        cases = (
            (
                "manifest",
                "tools/qt/qt-6.8.4.json",
                self.manifest_path,
                lambda root: bundle.load_contracts(
                    root / "tools" / "qt" / "qt-6.8.4.json",
                    root / "packaging" / "qt",
                    root,
                ),
            ),
            (
                "build-recipe",
                "tools/qt/build_qt.py",
                Path(qt.__file__),
                lambda root: qt.recipe_materials(root),
            ),
            (
                "bundle-recipe",
                "tools/qt/source_bundle.py",
                Path(bundle.__file__),
                lambda root: bundle.release_contract(
                    root / "tools" / "qt" / "qt-6.8.4.json",
                    root / "packaging" / "qt",
                    root / "tools" / "qt" / "source_bundle.py",
                    recipe_root=root,
                ),
            ),
            (
                "compliance",
                "packaging/qt/README.md",
                self.package_dir / "README.md",
                lambda root: bundle.release_contract(
                    root / "tools" / "qt" / "qt-6.8.4.json",
                    root / "packaging" / "qt",
                    root / "tools" / "qt" / "source_bundle.py",
                    recipe_root=root,
                ),
            ),
        )
        for name, logical, outside, operation in cases:
            with self.subTest(name=name):
                release_root = self.copy_release_root(
                    f"symlink-{name}"
                )
                target = release_root / logical
                target.unlink()
                target.symlink_to(outside.resolve())
                with self.assertRaisesRegex(
                    qt.QtSdkError, "non-symlink regular file"
                ):
                    operation(release_root)

        release_root = self.copy_release_root("symlink-parent")
        probe = release_root / "tools" / "qt" / "probe"
        real_probe = release_root / "tools" / "qt" / "probe-real"
        probe.rename(real_probe)
        probe.symlink_to(real_probe.resolve(), target_is_directory=True)
        with self.assertRaisesRegex(
            qt.QtSdkError, "symlink path component"
        ):
            qt.recipe_materials(release_root)

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
