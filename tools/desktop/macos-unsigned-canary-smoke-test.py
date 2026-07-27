#!/usr/bin/env python3
"""Contract tests for the native macOS unsigned-canary inspection."""

from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SMOKE = ROOT / "tools" / "desktop" / "macos-unsigned-canary-smoke.sh"


def executable(path: Path, source: str) -> None:
    path.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


class MacosUnsignedCanarySmokeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="chaft-macos-unsigned-canary-test-"
        )
        self.root = Path(self.temporary.name)
        self.package_dir = self.root / "packages"
        self.package_dir.mkdir()
        self.dmg = self.package_dir / "Chaft-0.1.0-canary.1-macOS-x86_64.dmg"
        self.dmg.write_bytes(b"synthetic dmg")
        self.fake_bin = self.root / "bin"
        self.fake_bin.mkdir()
        executable(self.fake_bin / "uname", "#!/bin/sh\nprintf 'Darwin\\n'\n")
        executable(
            self.fake_bin / "hdiutil",
            """\
            #!/bin/sh
            action="$1"
            shift
            case "$action" in
              attach)
                mountpoint=
                while [ "$#" -gt 0 ]; do
                  if [ "$1" = "-mountpoint" ]; then
                    mountpoint="$2"
                    shift 2
                  else
                    shift
                  fi
                done
                mkdir -p "$mountpoint/Chaft.app/Contents/MacOS"
                : > "$mountpoint/Chaft.app/Contents/MacOS/ChaftDesktop"
                ;;
              detach) ;;
              *) exit 2 ;;
            esac
            """,
        )
        executable(
            self.fake_bin / "codesign",
            """\
            #!/bin/sh
            last=
            for argument in "$@"; do
              last="$argument"
            done
            case " $* " in
              *" --display "*)
                case "$last" in
                  *.dmg) exit 1 ;;
                  *)
                    printf '%s\\n' \
                      'Executable=ChaftDesktop' \
                      'Signature=adhoc' \
                      'TeamIdentifier=not set' >&2
                    ;;
                esac
                ;;
              *" --verify "*) ;;
              *) exit 2 ;;
            esac
            """,
        )
        executable(self.fake_bin / "xcrun", "#!/bin/sh\nexit 1\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_smoke(self, *, codesign_source: str | None = None) -> subprocess.CompletedProcess[str]:
        if codesign_source is not None:
            executable(self.fake_bin / "codesign", codesign_source)
        environment = dict(os.environ)
        environment["PATH"] = f"{self.fake_bin}:{environment['PATH']}"
        environment["TMPDIR"] = str(self.root)
        return subprocess.run(
            [str(SMOKE), str(self.package_dir)],
            cwd=ROOT,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_accepts_only_ad_hoc_bundle_without_team_or_notarization(self) -> None:
        result = self.run_smoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("ad-hoc app", result.stdout)

    def test_rejects_developer_id_authority(self) -> None:
        result = self.run_smoke(
            codesign_source="""\
            #!/bin/sh
            last=
            for argument in "$@"; do
              last="$argument"
            done
            case " $* " in
              *" --display "*)
                case "$last" in
                  *.dmg) exit 1 ;;
                  *)
                    printf '%s\\n' \
                      'Signature=adhoc' \
                      'Authority=Developer ID Application: Example' \
                      'TeamIdentifier=EXAMPLE123' >&2
                    ;;
                esac
                ;;
              *" --verify "*) ;;
              *) exit 2 ;;
            esac
            """
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("team identifier", result.stderr)


if __name__ == "__main__":
    unittest.main()
