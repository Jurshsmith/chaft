#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
SMOKE = ROOT / "tools" / "desktop" / "macos-dmg-smoke.sh"
NOTICE_FILES = (
    "LICENSE",
    "THIRD_PARTY_NOTICES.txt",
    "LICENSE.LGPL3",
    "LICENSE.GPL3",
    "QT-CORRESPONDING-SOURCE.json",
)
RUN_NATIVE_DMG_TEST = (
    sys.platform == "darwin"
    and os.environ.get("CHAFT_RUN_NATIVE_DMG_TEST") == "1"
)


class MacosDmgSmokeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="chaft-macos-dmg-contract-"
        )
        self.root = Path(self.temporary.name)
        self.package_dir = self.root / "downloaded artifact"
        self.volume = self.root / "DMG payload"
        self.fake_bin = self.root / "fake bin"
        self.tmp_dir = self.root / "tmp"
        self.receipt = self.root / "launch-receipt.txt"
        self.hdiutil_log = self.root / "hdiutil.log"
        self.ditto_log = self.root / "ditto.log"
        for directory in (
            self.package_dir,
            self.volume,
            self.fake_bin,
            self.tmp_dir,
        ):
            directory.mkdir(parents=True)
        (self.package_dir / "Chaft-0.1.0-macOS-x86_64.dmg").write_bytes(
            b"fake dmg"
        )
        self.app = self.volume / "Chaft.app"
        self.binary = self.app / "Contents" / "MacOS" / "Chaft"
        self.icon = self.app / "Contents" / "Resources" / "Chaft.icns"
        self.compliance = (
            self.app / "Contents" / "Resources" / "doc" / "Chaft"
        )
        (self.app / "Contents" / "Frameworks").mkdir(parents=True)
        (self.app / "Contents" / "PlugIns" / "platforms").mkdir(parents=True)
        self.compliance.mkdir(parents=True)
        (self.app / "Contents" / "Info.plist").write_bytes(
            plistlib.dumps(
                {
                    "CFBundleName": "Chaft",
                    "CFBundleExecutable": "Chaft",
                    "CFBundleIconFile": "Chaft.icns",
                    "CFBundleShortVersionString": "0.1.0",
                    "CFBundleVersion": "0.1.0",
                }
            )
        )
        self.icon.write_bytes(b"synthetic Chaft icon")
        (
            self.app
            / "Contents"
            / "PlugIns"
            / "platforms"
            / "libqcocoa.dylib"
        ).write_bytes(b"synthetic cocoa plugin")
        (self.app / "Contents" / "Resources" / "dmg-origin-marker").write_text(
            "from mounted DMG\n", encoding="utf-8"
        )
        for filename in NOTICE_FILES:
            (self.compliance / filename).write_text(
                f"synthetic {filename}\n", encoding="utf-8"
            )
        self.write_fake_tools()
        self.write_passing_app()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def write_executable(path: Path, source: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
        path.chmod(0o755)

    def write_fake_tools(self) -> None:
        self.write_executable(
            self.fake_bin / "uname",
            """
            #!/usr/bin/env sh
            printf 'Darwin\\n'
            """,
        )
        self.write_executable(
            self.fake_bin / "hdiutil",
            """
            #!/usr/bin/env sh
            set -eu
            action="$1"
            shift
            case "$action" in
              attach)
                mountpoint=
                while [ "$#" -gt 0 ]; do
                  if [ "$1" = "-mountpoint" ]; then
                    shift
                    mountpoint="$1"
                  fi
                  shift
                done
                cp -R "$FAKE_DMG_VOLUME"/. "$mountpoint"/
                printf 'attach:%s\\n' "$mountpoint" >> "$FAKE_HDIUTIL_LOG"
                ;;
              detach)
                printf 'detach:%s\\n' "$*" >> "$FAKE_HDIUTIL_LOG"
                ;;
              *)
                printf 'unexpected hdiutil action: %s\\n' "$action" >&2
                exit 2
                ;;
            esac
            """,
        )
        self.write_executable(
            self.fake_bin / "ditto",
            """
            #!/usr/bin/env sh
            set -eu
            printf '%s|%s\\n' "$1" "$2" >> "$FAKE_DITTO_LOG"
            cp -R "$1" "$2"
            """,
        )

    def write_passing_app(self) -> None:
        self.write_executable(
            self.binary,
            """
            #!/usr/bin/env sh
            set -eu
            if [ "${QT_QPA_PLATFORM:-}" != "cocoa" ]; then
              printf 'expected Cocoa, got %s\\n' "${QT_QPA_PLATFORM:-}" >&2
              exit 10
            fi
            if [ "${CHAFT_DESKTOP_SMOKE:-}" != "1" ]; then
              printf 'desktop smoke flag is missing\\n' >&2
              exit 11
            fi
            if [ "${CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE:-}" != "1" ]; then
              printf 'clean DMG smoke must expect an empty runtime\\n' >&2
              exit 12
            fi
            bundle="$(CDPATH= cd "$(dirname "$0")/../.." && pwd)"
            for required in \
              Contents/Frameworks \
              Contents/Info.plist \
              Contents/PlugIns/platforms/libqcocoa.dylib \
              Contents/Resources/dmg-origin-marker
            do
              if [ ! -e "$bundle/$required" ]; then
                printf 'copied bundle path is missing: %s\\n' "$required" >&2
                exit 13
              fi
            done
            printf '%s\\n' "$0" > "$FAKE_LAUNCH_RECEIPT"
            """,
        )

    def environment(self) -> dict[str, str]:
        environment = os.environ.copy()
        for name in tuple(environment):
            if (
                name.startswith("CHAFT_")
                or name.startswith("DYLD_")
                or name.startswith("QML")
                or name.startswith("QT_")
                or name in {"QTDIR", "Qt6_DIR"}
            ):
                environment.pop(name)
        environment.update(
            {
                "PATH": f"{self.fake_bin}{os.pathsep}{environment['PATH']}",
                "TMPDIR": str(self.tmp_dir),
                "FAKE_DMG_VOLUME": str(self.volume),
                "FAKE_HDIUTIL_LOG": str(self.hdiutil_log),
                "FAKE_DITTO_LOG": str(self.ditto_log),
                "FAKE_LAUNCH_RECEIPT": str(self.receipt),
            }
        )
        return environment

    def run_smoke(
        self, *, extra_environment: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        environment = self.environment()
        if extra_environment:
            environment.update(extra_environment)
        return subprocess.run(
            [str(SMOKE), str(self.package_dir)],
            cwd=self.root,
            env=environment,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
        )

    def test_launches_complete_dmg_derived_bundle_after_detach(self) -> None:
        completed = self.run_smoke()
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("portable macOS DMG smoke passed", completed.stdout)
        launched_path = self.receipt.read_text(encoding="utf-8").strip()
        self.assertIn("/portable package/Chaft-dmg-smoke/", launched_path)
        self.assertNotIn(str(self.volume), launched_path)
        self.assertEqual(
            self.hdiutil_log.read_text(encoding="utf-8").splitlines()[0].split(
                ":", 1
            )[0],
            "attach",
        )
        self.assertIn(
            "detach:",
            self.hdiutil_log.read_text(encoding="utf-8"),
        )
        copied_from, copied_to = self.ditto_log.read_text(
            encoding="utf-8"
        ).strip().split("|", 1)
        self.assertIn("/mounted dmg/Chaft.app", copied_from)
        self.assertIn(
            "/portable package/Chaft-dmg-smoke",
            copied_to,
        )

    def test_rejects_missing_application_icon(self) -> None:
        self.icon.unlink()
        completed = self.run_smoke()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("application icon is missing", completed.stderr)

    def test_rejects_unpolished_application_bundle_name(self) -> None:
        self.app.rename(self.volume / "ChaftDesktop.app")
        completed = self.run_smoke()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("expected macOS application bundle is missing", completed.stderr)

    def test_rejects_icon_metadata_mismatch(self) -> None:
        info_path = self.app / "Contents" / "Info.plist"
        info = plistlib.loads(info_path.read_bytes())
        info["CFBundleIconFile"] = ""
        info_path.write_bytes(plistlib.dumps(info))
        completed = self.run_smoke()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("CFBundleIconFile must be", completed.stderr)

    def test_outer_watchdog_terminates_pre_event_loop_stall(self) -> None:
        self.write_executable(
            self.binary,
            """
            #!/usr/bin/env sh
            set -eu
            printf 'started\\n' > "$FAKE_LAUNCH_RECEIPT"
            while :; do :; done
            """,
        )
        started = time.monotonic()
        completed = self.run_smoke(
            extra_environment={
                "CHAFT_DESKTOP_SMOKE_TIMEOUT_MS": "60000",
                "CHAFT_DMG_SMOKE_WATCHDOG_SECONDS": "1",
            }
        )
        elapsed = time.monotonic() - started
        self.assertEqual(completed.returncode, 124, completed.stderr)
        self.assertLess(elapsed, 6)
        self.assertIn("macOS DMG smoke timed out after 1s", completed.stderr)
        self.assertEqual(self.receipt.read_text(encoding="utf-8"), "started\n")

    def test_accepts_prerelease_distribution_name_with_stable_embedded_version(
        self,
    ) -> None:
        stable_path = self.package_dir / "Chaft-0.1.0-macOS-x86_64.dmg"
        canary_path = self.package_dir / "Chaft-0.1.0-canary.1-macOS-x86_64.dmg"
        stable_path.rename(canary_path)
        completed = self.run_smoke(
            extra_environment={
                "CHAFT_SOURCE_VERSION": "0.1.0",
                "CHAFT_DISTRIBUTION_VERSION": "0.1.0-canary.1",
            }
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn(canary_path.name, completed.stdout)

    def test_rejects_filename_that_omits_distribution_version(self) -> None:
        stable_path = self.package_dir / "Chaft-0.1.0-macOS-x86_64.dmg"
        wrong_path = self.package_dir / "Chaft-test-macOS.dmg"
        stable_path.rename(wrong_path)
        completed = self.run_smoke()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("expected DMG filename", completed.stderr)

    @unittest.skipUnless(
        RUN_NATIVE_DMG_TEST,
        "set CHAFT_RUN_NATIVE_DMG_TEST=1 on macOS for native hdiutil",
    )
    def test_native_hdiutil_mount_copy_detach_and_launch(self) -> None:
        dmg_path = self.package_dir / "Chaft-0.1.0-macOS-x86_64.dmg"
        dmg_path.unlink()
        created = subprocess.run(
            [
                "/usr/bin/hdiutil",
                "create",
                "-quiet",
                "-volname",
                "Chaft DMG smoke contract",
                "-srcfolder",
                str(self.volume),
                "-format",
                "UDZO",
                str(dmg_path),
            ],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
        self.assertEqual(created.returncode, 0, created.stderr)
        environment = self.environment()
        environment["PATH"] = os.environ["PATH"]
        completed = subprocess.run(
            [str(SMOKE), str(self.package_dir)],
            cwd=self.root,
            env=environment,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("portable macOS DMG smoke passed", completed.stdout)
        launched_path = self.receipt.read_text(encoding="utf-8").strip()
        self.assertIn("/portable package/Chaft-dmg-smoke/", launched_path)


if __name__ == "__main__":
    unittest.main(verbosity=2)
