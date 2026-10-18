#!/usr/bin/env python3
"""Verify SDK transport, provenance binding, safe extraction, and producer trust."""

import copy
from datetime import datetime, timezone
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("qt-sdk-artifact.py")
spec = importlib.util.spec_from_file_location("sdk_artifact", SCRIPT)
artifact = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifact)
qt = artifact.qt
TARGET = "linux-x86_64"


def contract(image="old", compiler="gcc 11.4.0"):
    return {"schemaVersion": 2, "target": TARGET, "platform": "linux",
            "runner": {"os": "Linux", "architecture": "x86_64", "imageOS": "ubuntu22", "imageVersion": image},
            "tools": {"cmake": "cmake 3.31.6", "ninja": "1.13.2", "compiler": compiler, "python": "3.13.3"}}


class SDKArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="test-sdk-artifact-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.manifest = qt.load_manifest()
        self.prefix = self.root / "producer"
        self.prefix.mkdir()
        (self.prefix / "bin").mkdir()
        executable = self.prefix / "bin" / "qtpaths"
        executable.write_bytes(b"fixture SDK executable")
        executable.chmod(0o755)
        provenance_path = qt.write_provenance(self.manifest, TARGET, self.prefix,
            qt.expected_source_materials(self.manifest, TARGET), [], contract())
        provenance = json.loads(provenance_path.read_text())
        provenance["verification"] = {"completed": True, "completedAt": "2026-10-18T00:00:00Z"}
        provenance_path.write_text(json.dumps(provenance))
        self.output = self.root / "transfer"
        self.expected = artifact.create(self.prefix, self.output, self.manifest, TARGET)
        self.destination = self.root / "consumer"

    def restore(self, **kwargs):
        return artifact.restore(self.output, self.destination, self.manifest, TARGET, **kwargs)

    def metadata(self, edit):
        path = self.output / artifact.METADATA
        data = json.loads(path.read_text())
        edit(data)
        path.write_text(json.dumps(data))

    def test_roundtrip_keeps_executable_permissions_and_original_identity(self):
        result = self.restore(current_contract=contract(image="new"), expected=self.expected_without_name())
        self.assertEqual(result, self.expected)
        self.assertEqual((self.destination / "bin/qtpaths").read_bytes(), b"fixture SDK executable")
        self.assertTrue((self.destination / "bin/qtpaths").stat().st_mode & 0o111)

    def expected_without_name(self):
        return {key: value for key, value in self.expected.items() if key != "artifact_name"}

    def test_artifact_name_tolerates_only_runner_image_revision(self):
        self.assertEqual(artifact.artifact_name(self.manifest, TARGET, contract("old")), artifact.artifact_name(self.manifest, TARGET, contract("new")))
        self.assertNotEqual(artifact.artifact_name(self.manifest, TARGET, contract()), artifact.artifact_name(self.manifest, TARGET, contract(compiler="gcc 12")))

    def test_changed_compiler_is_rejected_before_extraction(self):
        with self.assertRaisesRegex(ValueError, "incompatible"):
            self.restore(current_contract=contract(compiler="gcc 12"))
        self.assertFalse(self.destination.exists())

    def test_archive_byte_tampering_is_rejected(self):
        with (self.output / artifact.ARCHIVE).open("ab") as stream:
            stream.write(b"changed")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            self.restore()

    def test_metadata_target_schema_identity_and_digest_are_strict(self):
        original = (self.output / artifact.METADATA).read_text()
        for field, value in (("target", "windows-x86_64"), ("schemaVersion", True), ("identity", "different"),
                             ("toolchainFingerprint", "0" * 64), ("compatibilityFingerprint", "0" * 64), ("archiveSha256", "invalid")):
            with self.subTest(field=field):
                (self.output / artifact.METADATA).write_text(original)
                self.metadata(lambda data: data.update({field: value}))
                with self.assertRaises(ValueError):
                    self.restore()
                self.assertFalse(self.destination.exists())

    def test_consumer_rejects_a_different_provisioned_identity(self):
        with self.assertRaisesRegex(ValueError, "provisioned identity"):
            self.restore(expected={"identity": "different"})

    def test_wrapper_rejects_unexpected_files_and_symlinks(self):
        (self.output / "extra").write_text("unexpected")
        with self.assertRaises(ValueError):
            self.restore()
        (self.output / "extra").unlink()
        original = self.output / artifact.ARCHIVE
        original.rename(self.root / "archive")
        original.symlink_to(self.root / "archive")
        with self.assertRaisesRegex(ValueError, "regular file"):
            self.restore()

    def test_existing_destination_is_preserved(self):
        self.destination.mkdir()
        (self.destination / "keep").write_text("preserved")
        with self.assertRaisesRegex(ValueError, "already exist"):
            self.restore()
        self.assertEqual((self.destination / "keep").read_text(), "preserved")

    def rewrite_archive(self, name, kind=tarfile.REGTYPE, link=""):
        with tarfile.open(self.output / artifact.ARCHIVE, "w:gz") as archive:
            info = tarfile.TarInfo(name)
            info.type = kind
            info.linkname = link
            archive.addfile(info, io.BytesIO(b""))
        self.metadata(lambda data: data.update(archiveSha256=artifact.digest(self.output / artifact.ARCHIVE)))

    def test_archive_traversal_absolute_paths_and_special_files_are_rejected(self):
        for name, kind, link in (("../outside", tarfile.REGTYPE, ""), ("/absolute", tarfile.REGTYPE, ""),
                                 ("sdk/../../outside", tarfile.REGTYPE, ""), ("sdk\\outside", tarfile.REGTYPE, ""),
                                 ("sdk/device", tarfile.CHRTYPE, ""), ("sdk/link", tarfile.SYMTYPE, "../../outside"),
                                 ("sdk/link", tarfile.LNKTYPE, "/outside")):
            with self.subTest(name=name, kind=kind):
                self.rewrite_archive(name, kind, link)
                with self.assertRaises((ValueError, tarfile.TarError)):
                    self.restore()
                self.assertFalse(self.destination.exists())
                self.assertFalse((self.root / "outside").exists())

    def test_internal_framework_style_symlinks_survive(self):
        (self.prefix / "bin/current").symlink_to("qtpaths")
        artifact.create(self.prefix, self.output, self.manifest, TARGET)
        self.restore()
        self.assertTrue((self.destination / "bin/current").is_symlink())
        self.assertEqual((self.destination / "bin/current").read_bytes(), b"fixture SDK executable")

    def test_absolute_internal_links_are_relocated_and_relative_chains_are_preserved(self):
        (self.prefix / "bin/current").symlink_to("qtpaths")
        (self.prefix / "bin/alias").symlink_to("current")
        (self.prefix / "bin/absolute").symlink_to(self.prefix / "bin/qtpaths")
        artifact.create(self.prefix, self.output, self.manifest, TARGET)
        self.restore()
        self.assertEqual(os.readlink(self.destination / "bin/alias"), "current")
        self.assertFalse(os.path.isabs(os.readlink(self.destination / "bin/absolute")))
        self.assertEqual((self.destination / "bin/absolute").read_bytes(), b"fixture SDK executable")

    def test_archive_layout_rejects_duplicate_paths_and_writes_through_links(self):
        duplicate = tarfile.TarInfo("sdk/file")
        link = tarfile.TarInfo("sdk/alias")
        link.type = tarfile.SYMTYPE
        link.linkname = "."
        child = tarfile.TarInfo("sdk/alias/file")
        for members in ([duplicate, duplicate], [link, child]):
            with self.assertRaises(ValueError):
                artifact.validate_members(members)

    def test_hardlinks_cannot_reference_symlinks_and_links_cannot_walk_up_after_aliases(self):
        link = tarfile.TarInfo("sdk/alias")
        link.type = tarfile.SYMTYPE
        link.linkname = "."
        hardlink = tarfile.TarInfo("sdk/hard")
        hardlink.type = tarfile.LNKTYPE
        hardlink.linkname = "sdk/alias"
        with self.assertRaises(ValueError):
            artifact.validate_members([link, hardlink])
        link.linkname = "other/../../outside"
        with self.assertRaises(ValueError):
            artifact.validate_members([link])

    def test_incomplete_or_mismatched_embedded_provenance_is_rejected(self):
        path = self.prefix / qt.PROVENANCE_NAME
        original = json.loads(path.read_text())
        for field, value in (("identity", "mismatch"), ("verification", {"completed": False, "completedAt": None})):
            data = copy.deepcopy(original)
            data[field] = value
            path.write_text(json.dumps(data))
            # Modify the payload while retaining valid transport metadata.
            with tarfile.open(self.output / artifact.ARCHIVE, "w:gz") as archive:
                archive.add(self.prefix, arcname="sdk")
            self.metadata(lambda meta: meta.update(archiveSha256=artifact.digest(self.output / artifact.ARCHIVE)))
            with self.assertRaises(qt.QtSdkError):
                self.restore()
            self.assertFalse(self.destination.exists())

    def test_create_cannot_archive_into_itself(self):
        with self.assertRaisesRegex(ValueError, "outside"):
            artifact.create(self.prefix, self.prefix / "output", self.manifest, TARGET)


class SDKProducerTrustTests(unittest.TestCase):
    def setUp(self):
        self.manifest = qt.load_manifest()
        self.name = artifact.artifact_name(self.manifest, TARGET, contract())
        self.repository = {"id": 100, "default_branch": "main"}
        self.run = {"id": 200, "head_sha": "a" * 40, "head_repository": {"id": 100},
                    "repository": {"id": 100}, "head_branch": "main", "event": "push", "name": "CI",
                    "path": ".github/workflows/ci.yml", "status": "completed", "conclusion": "success"}
        self.candidate = {"id": 300, "name": self.name, "expired": False, "expires_at": "2099-01-01T00:00:00Z", "digest": "sha256:" + "b" * 64,
                          "workflow_run": {"id": 200, "repository_id": 100, "head_repository_id": 100, "head_sha": "a" * 40}}

    def api(self, path):
        if path == "repos/example/chaft":
            return self.repository
        if "/actions/artifacts?" in path:
            return {"artifacts": [self.candidate]}
        if path == "repos/example/chaft/actions/runs/200":
            return self.run
        if path.startswith("repos/example/chaft/actions/runs/200/jobs?"):
            return {"jobs": [{"name": artifact.SDK_JOB_NAMES[TARGET], "status": "completed", "conclusion": "success"}]}
        raise AssertionError(path)

    def test_lookup_selects_an_exact_successful_default_branch_push(self):
        result = artifact.lookup("example/chaft", self.manifest, TARGET, contract("new"), self.api)
        self.assertEqual(result, {"found": "true", "artifact_name": self.name, "upload_name": self.name, "refresh": "false", "artifact_id": "300", "run_id": "200"})

    def test_wrong_event_branch_workflow_status_and_repository_are_rejected(self):
        for field, value in (("event", "pull_request"), ("event", "workflow_dispatch"), ("event", "workflow_run"),
                             ("head_branch", "other"), ("path", ".github/workflows/other.yml"), ("name", "Other"),
                             ("status", "in_progress"), ("conclusion", "failure"), ("conclusion", "cancelled"),
                             ("head_repository", {"id": 999}), ("repository", {"id": 999}), ("head_sha", "c" * 40)):
            with self.subTest(field=field, value=value):
                altered = {**self.run, field: value}
                self.assertFalse(artifact.trusted_artifact(self.candidate, altered, self.repository, self.name))

    def test_expired_forked_wrong_name_and_unhashed_artifacts_are_rejected(self):
        for field, value in (("expired", True), ("name", "other"), ("digest", None), ("digest", "invalid"), ("id", True),
                             ("workflow_run", {**self.candidate["workflow_run"], "head_repository_id": 999})):
            with self.subTest(field=field):
                self.assertFalse(artifact.trusted_artifact({**self.candidate, field: value}, self.run, self.repository, self.name))

    def test_untrusted_candidate_is_a_miss_and_cannot_become_an_sdk(self):
        self.run["event"] = "pull_request"
        self.assertEqual(artifact.lookup("example/chaft", self.manifest, TARGET, contract(), self.api)["found"], "false")

    def test_a_passing_run_without_a_successful_sdk_job_cannot_seed_the_pool(self):
        for jobs in ([], [{"name": "Static website", "status": "completed", "conclusion": "success"}],
                     [{"name": artifact.SDK_JOB_NAMES[TARGET], "status": "completed", "conclusion": "skipped"}],
                     [{"name": artifact.SDK_JOB_NAMES[TARGET], "status": "completed", "conclusion": "failure"}]):
            def api(path):
                return {"jobs": jobs} if "/jobs?" in path else self.api(path)
            self.assertEqual(artifact.lookup("example/chaft", self.manifest, TARGET, contract(), api)["found"], "false")

    def test_empty_inventory_returns_a_named_miss(self):
        def api(path):
            return self.repository if path == "repos/example/chaft" else {"artifacts": []}
        self.assertEqual(artifact.lookup("example/chaft", self.manifest, TARGET, contract(), api), {"found": "false", "artifact_name": self.name, "upload_name": self.name, "refresh": "false"})

    def test_default_branch_nightly_ci_can_refresh_the_trusted_sdk_pool(self):
        self.run["event"] = "schedule"
        self.assertTrue(artifact.trusted_artifact(self.candidate, self.run, self.repository, self.name))
        self.run["head_branch"] = "other"
        self.assertFalse(artifact.trusted_artifact(self.candidate, self.run, self.repository, self.name))

    def test_archives_near_expiry_are_refreshed_without_a_source_rebuild(self):
        self.candidate["expires_at"] = "2026-10-19T00:00:00Z"
        now = datetime(2026, 10, 18, tzinfo=timezone.utc)
        result = artifact.lookup("example/chaft", self.manifest, TARGET, contract(), self.api, now=now)
        self.assertEqual(result["found"], "true")
        self.assertEqual(result["refresh"], "true")

    def test_reruns_use_unique_upload_names_without_deleting_earlier_artifacts(self):
        self.run["status"] = "in_progress"
        result = artifact.lookup("example/chaft", self.manifest, TARGET, contract(), self.api, current_run_id=200, attempt="2")
        self.assertEqual(result["found"], "false")
        self.assertEqual(result["upload_name"], self.name + "-retry-2")

    def test_invalid_repository_is_rejected_before_any_api_call(self):
        for name in ("../repo", "owner/repo\n", "owner/repo/extra"):
            with self.assertRaises(ValueError):
                artifact.lookup(name, self.manifest, TARGET, contract(), self.api)


if __name__ == "__main__":
    unittest.main()
