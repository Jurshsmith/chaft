#!/usr/bin/env python3
"""Focused contracts for the guided native macOS source build."""

from __future__ import annotations

import json
import os
from pathlib import Path
import plistlib
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[2]
BUILD_LOCAL = ROOT / "tools" / "macos" / "build-local.sh"
VERIFY_APP = ROOT / "tools" / "macos" / "verify-local-app.sh"
PREFLIGHT = ROOT / "tools" / "desktop" / "preflight.sh"
CMAKE = ROOT / "apps" / "desktop-qt" / "CMakeLists.txt"
PATH_SAFETY = ROOT / "tools" / "desktop" / "validate-safe-path.py"


def executable(path: Path, body: str) -> None:
    path.write_text("#!/bin/sh\nset -eu\n" + body, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


class MacosBuildLocalContracts(unittest.TestCase):
    def run_script(
        self,
        script: Path,
        *arguments: str,
        path: Path,
        environment: dict[str, str] | None = None,
        stdin: str = "",
    ) -> subprocess.CompletedProcess[str]:
        env = {
            "CHAFT_DESKTOP_TOOL_DISCOVERY": "explicit",
            "CHAFT_HOMEBREW_EXECUTABLE": str(path / "brew"),
            "HOME": str(path.parent / "home"),
            "PATH": f"{path}:/usr/bin:/bin",
            "TMPDIR": str(path.parent),
        }
        if environment:
            env.update(environment)
        return subprocess.run(
            [str(script), *arguments],
            cwd=ROOT,
            env=env,
            input=stdin,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def base_native_tools(self, root: Path, *, translated: bool = False) -> Path:
        bin_dir = root / "bin"
        bin_dir.mkdir()
        tools_dir = root / "xcode-tools"
        tools_dir.mkdir()
        executable(
            bin_dir / "uname",
            """
            case "${1:-}" in
              -s) printf 'Darwin\\n' ;;
              -m) printf 'arm64\\n' ;;
              *) printf 'Darwin\\n' ;;
            esac
            """,
        )
        executable(
            bin_dir / "sysctl",
            f"printf '{1 if translated else 0}\\n'\n",
        )
        executable(
            bin_dir / "xcode-select",
            f"printf '%s\\n' '{tools_dir}'\n",
        )
        executable(bin_dir / "xcrun", "exit 0\n")
        return bin_dir

    def test_script_rejects_non_macos_and_translated_shells(self) -> None:
        with tempfile.TemporaryDirectory(prefix="chaft-macos-local-test-") as name:
            root = Path(name)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            executable(bin_dir / "uname", "printf 'Linux\\n'\n")
            non_macos = self.run_script(BUILD_LOCAL, path=bin_dir)
            self.assertNotEqual(non_macos.returncode, 0)
            self.assertIn("supported only on macOS", non_macos.stderr)

        with tempfile.TemporaryDirectory(prefix="chaft-macos-local-test-") as name:
            root = Path(name)
            bin_dir = self.base_native_tools(root, translated=True)
            translated = self.run_script(BUILD_LOCAL, path=bin_dir)
            self.assertNotEqual(translated.returncode, 0)
            self.assertIn("terminal is translated", translated.stderr)
            self.assertIn("native terminal", translated.stderr)

    def test_missing_formulae_require_confirmation_or_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="chaft-macos-local-test-") as name:
            root = Path(name)
            bin_dir = self.base_native_tools(root)
            brew_prefix = root / "brew"
            brew_prefix.mkdir()
            install_marker = root / "brew-install-ran"
            executable(
                bin_dir / "brew",
                textwrap.dedent(
                    f"""
                    if [ "${{1:-}}" = "--prefix" ]; then
                      printf '%s\\n' '{brew_prefix}'
                      exit 0
                    fi
                    if [ "${{1:-}}" = "config" ]; then
                      printf 'macOS: 26.0-arm64\\n'
                      exit 0
                    fi
                    if [ "${{1:-}}" = "list" ]; then
                      exit 1
                    fi
                    if [ "${{1:-}}" = "install" ]; then
                      : > '{install_marker}'
                      exit 0
                    fi
                    exit 1
                    """
                ),
            )

            declined = self.run_script(BUILD_LOCAL, path=bin_dir, stdin="n\n")
            self.assertNotEqual(declined.returncode, 0)
            self.assertIn("Install these formulae now? [y/N]", declined.stdout)
            self.assertIn("dependency installation declined", declined.stderr)
            self.assertFalse(install_marker.exists())

            disabled = self.run_script(
                BUILD_LOCAL, "--no-install-deps", path=bin_dir
            )
            self.assertNotEqual(disabled.returncode, 0)
            self.assertIn("dependency installation is disabled", disabled.stderr)
            self.assertFalse(install_marker.exists())

    def qt_environment(
        self, root: Path, version: str
    ) -> tuple[Path, dict[str, str]]:
        bin_dir = self.base_native_tools(root)
        brew_prefix = root / "brew"
        qt_prefix = brew_prefix / "Cellar" / "qtbase" / version
        (qt_prefix / "bin").mkdir(parents=True)
        executable(
            bin_dir / "brew",
            textwrap.dedent(
                f"""
                if [ "${{1:-}}" = "--prefix" ]; then
                  printf '%s\\n' '{brew_prefix}'
                  exit 0
                fi
                exit 1
                """
            ),
        )
        executable(bin_dir / "cargo", "printf 'cargo 1.97.1\\n'\n")
        executable(bin_dir / "cmake", "printf 'cmake version 3.28.0\\n'\n")
        executable(bin_dir / "ninja", "printf '1.12.0\\n'\n")
        os.symlink(sys.executable, bin_dir / "python3")
        executable(
            qt_prefix / "bin" / "qmake6",
            textwrap.dedent(
                f"""
                if [ "${{1:-}}" = "-query" ]; then
                  printf '%s\\n' '{qt_prefix}'
                else
                  printf 'QMake version 3.1\\n'
                  printf 'Using Qt version {version} in {qt_prefix}/lib\\n'
                fi
                """
            ),
        )
        executable(
            qt_prefix / "bin" / "qtpaths6",
            f"printf '{version}\\n'\n",
        )
        executable(qt_prefix / "bin" / "qt-cmake", "exit 0\n")
        return bin_dir, {
            "CHAFT_QT_POLICY": "developer",
            "QT_ROOT_DIR": str(qt_prefix),
        }

    def test_developer_policy_accepts_only_supported_homebrew_qt(self) -> None:
        with tempfile.TemporaryDirectory(prefix="chaft-qt-policy-test-") as name:
            root = Path(name)
            bin_dir, env = self.qt_environment(root, "6.11.1")
            accepted = self.run_script(PREFLIGHT, path=bin_dir, environment=env)
            self.assertEqual(
                accepted.returncode,
                0,
                msg=accepted.stdout + accepted.stderr,
            )
            self.assertIn("Qt policy: developer", accepted.stdout)

        with tempfile.TemporaryDirectory(prefix="chaft-qt-policy-test-") as name:
            root = Path(name)
            bin_dir, env = self.qt_environment(root, "6.12.0")
            rejected = self.run_script(PREFLIGHT, path=bin_dir, environment=env)
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn("Qt >=6.11.1 and <6.12.0", rejected.stderr)

        with tempfile.TemporaryDirectory(prefix="chaft-qt-policy-test-") as name:
            root = Path(name)
            bin_dir, env = self.qt_environment(root, "6.11.1")
            executable(bin_dir / "brew", "exit 1\n")
            unresolved = self.run_script(
                PREFLIGHT,
                path=bin_dir,
                environment=env,
            )
            self.assertNotEqual(unresolved.returncode, 0)
            self.assertIn(
                "could not resolve the selected Homebrew and Qt prefixes",
                unresolved.stderr,
            )

    def test_release_policy_rejects_minimal_or_missing_provenance(self) -> None:
        with tempfile.TemporaryDirectory(prefix="chaft-qt-policy-test-") as name:
            root = Path(name)
            bin_dir, _ = self.qt_environment(root, "6.8.4")
            executable(
                bin_dir / "uname",
                textwrap.dedent(
                    """
                    case "${1:-}" in
                      -s) printf '%s\\n' 'Darwin' ;;
                      -m) printf '%s\\n' 'arm64' ;;
                      *) printf '%s\\n' 'Darwin' ;;
                    esac
                    """
                ),
            )
            target = "macos-arm64"
            qt_prefix = root / "brew" / "Cellar" / "qtbase" / "6.8.4"
            fingerprint = "a" * 64
            identity = "qt-6.8.4-test"
            provenance = qt_prefix / "chaft-qt-sdk-provenance.json"
            provenance.write_text(
                json.dumps(
                    {
                        "schemaVersion": 2,
                        "qtVersion": "6.8.4",
                        "target": target,
                        "platform": "macos",
                        "architecture": "arm64",
                        "identity": identity,
                        "toolchainFingerprint": fingerprint,
                        "verification": {"completed": True},
                    }
                ),
                encoding="utf-8",
            )
            environment = {
                "CHAFT_QT_POLICY": "release",
                "QTDIR": str(qt_prefix),
                "QT_ROOT_DIR": str(qt_prefix),
                "CHAFT_QT_SDK_BUILD_TYPE": "Release",
                "CHAFT_QT_SDK_TARGET": target,
                "CHAFT_QT_SDK_PLATFORM": "macos",
                "CHAFT_QT_SDK_ARCHITECTURE": "arm64",
                "CHAFT_QT_SDK_VERSION": "6.8.4",
                "CHAFT_QT_SDK_IDENTITY": identity,
                "CHAFT_QT_SDK_TOOLCHAIN_FINGERPRINT": fingerprint,
                "CHAFT_QT_SDK_PROVENANCE": str(provenance),
            }
            minimal = self.run_script(
                PREFLIGHT, path=bin_dir, environment=environment
            )
            self.assertNotEqual(minimal.returncode, 0)
            self.assertIn("provenance", minimal.stderr)
            self.assertIn("missing", minimal.stderr)

            missing = dict(environment)
            missing.pop("CHAFT_QT_SDK_PROVENANCE")
            rejected = self.run_script(
                PREFLIGHT, path=bin_dir, environment=missing
            )
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn(
                "requires verified SDK activation variable "
                "CHAFT_QT_SDK_PROVENANCE",
                rejected.stderr,
            )

    def test_local_app_verifier_checks_brand_architecture_and_signature(self) -> None:
        with tempfile.TemporaryDirectory(prefix="chaft-app-verify-test-") as name:
            root = Path(name)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            app = root / "Chaft.app"
            macos = app / "Contents" / "MacOS"
            resources = app / "Contents" / "Resources"
            macos.mkdir(parents=True)
            resources.mkdir()
            executable(
                macos / "Chaft",
                """
                for name in \
                  CHAFT_FFI_LIBRARY \
                  CMAKE_PREFIX_PATH \
                  DYLD_FALLBACK_FRAMEWORK_PATH \
                  DYLD_FALLBACK_LIBRARY_PATH \
                  DYLD_FRAMEWORK_PATH \
                  DYLD_INSERT_LIBRARIES \
                  DYLD_LIBRARY_PATH \
                  DYLD_ROOT_PATH \
                  QML2_IMPORT_PATH \
                  QML_IMPORT_PATH \
                  QTDIR \
                  QT_PLUGIN_PATH \
                  QT_QPA_PLATFORM_PLUGIN_PATH \
                  QT_ROOT_DIR \
                  Qt6_DIR
                do
                  eval "value=\\${$name:-}"
                  if [ -n "$value" ]; then
                    printf 'inherited forbidden smoke variable: %s\\n' "$name" >&2
                    exit 91
                  fi
                done
                exit 0
                """,
            )
            (macos / "libchaft_ffi.dylib").write_bytes(b"ffi")
            with (app / "Contents" / "Info.plist").open("wb") as handle:
                plistlib.dump(
                    {
                        "CFBundleName": "Chaft",
                        "CFBundleExecutable": "Chaft",
                        "CFBundleIconFile": "Chaft.icns",
                        "CFBundleIdentifier": "app.chaft.desktop",
                        "CFBundleShortVersionString": "0.1.0",
                        "CFBundleVersion": "0.1.0",
                    },
                    handle,
                )
            icon_payload = b"payload"
            (resources / "Chaft.icns").write_bytes(
                b"icns" + (8 + len(icon_payload)).to_bytes(4, "big") + icon_payload
            )
            documentation = resources / "doc" / "Chaft"
            documentation.mkdir(parents=True)
            for filename in ("LICENSE", "LICENSE.LGPL3", "LICENSE.GPL3"):
                (documentation / filename).write_text(
                    f"{filename} fixture\n", encoding="utf-8"
                )
            (documentation / "QT-LOCAL-BUILD-NOTICE.txt").write_text(
                "Built with Homebrew's open-source Qt 6.11.1. "
                "This does not claim verified Chaft Qt 6.8.4 release SDK provenance.\n",
                encoding="utf-8",
            )
            offscreen = app / "Contents" / "PlugIns" / "platforms"
            offscreen.mkdir(parents=True)
            (offscreen / "libqoffscreen.dylib").write_bytes(b"plugin")
            executable(
                bin_dir / "uname",
                """
                case "${1:-}" in
                  -s) printf 'Darwin\\n' ;;
                  -m) printf 'arm64\\n' ;;
                  *) printf 'Darwin\\n' ;;
                esac
                """,
            )
            executable(bin_dir / "lipo", "printf 'arm64\\n'\n")
            executable(
                bin_dir / "codesign",
                """
                if [ "${1:-}" = "--display" ]; then
                  printf 'Signature=adhoc\\n'
                  printf 'TeamIdentifier=not set\\n'
                fi
                exit 0
                """,
            )
            os.symlink(sys.executable, bin_dir / "python3")

            verified = self.run_script(
                VERIFY_APP,
                "--expected-arch",
                "arm64",
                str(app),
                path=bin_dir,
            )
            self.assertEqual(
                verified.returncode,
                0,
                msg=verified.stdout + verified.stderr,
            )
            self.assertIn("verified native arm64", verified.stdout)

            relative_app = os.path.relpath(app, ROOT)
            launched = self.run_script(
                VERIFY_APP,
                "--expected-arch",
                "arm64",
                "--launch-smoke",
                relative_app,
                path=bin_dir,
                environment={
                    "CHAFT_FFI_LIBRARY": "/ambient/libchaft_ffi.dylib",
                    "CMAKE_PREFIX_PATH": "/ambient/cmake",
                    "DYLD_FALLBACK_FRAMEWORK_PATH": "/ambient/frameworks",
                    "DYLD_FALLBACK_LIBRARY_PATH": "/ambient/fallback-libraries",
                    "DYLD_FRAMEWORK_PATH": "/ambient/frameworks",
                    "DYLD_INSERT_LIBRARIES": "/ambient/injected.dylib",
                    "DYLD_LIBRARY_PATH": "/ambient/libraries",
                    "DYLD_ROOT_PATH": "/ambient/root",
                    "QML2_IMPORT_PATH": "/ambient/qml2",
                    "QML_IMPORT_PATH": "/ambient/qml",
                    "QTDIR": "/ambient/qt",
                    "QT_PLUGIN_PATH": "/ambient/plugins",
                    "QT_QPA_PLATFORM_PLUGIN_PATH": "/ambient/platforms",
                    "QT_ROOT_DIR": "/ambient/qt-root",
                    "Qt6_DIR": "/ambient/qt6",
                },
            )
            self.assertEqual(
                launched.returncode,
                0,
                msg=launched.stdout + launched.stderr,
            )
            self.assertIn(str(app), launched.stdout)

            wrong_arch = self.run_script(
                VERIFY_APP,
                "--expected-arch",
                "x86_64",
                str(app),
                path=bin_dir,
            )
            self.assertNotEqual(wrong_arch.returncode, 0)
            self.assertIn("Mach-O architecture mismatch", wrong_arch.stderr)

            executable(bin_dir / "lipo", "printf 'arm64e\\n'\n")
            arm64e = self.run_script(
                VERIFY_APP,
                "--expected-arch",
                "arm64",
                str(app),
                path=bin_dir,
            )
            self.assertNotEqual(arm64e.returncode, 0)
            self.assertIn("Mach-O architecture mismatch", arm64e.stderr)

            frameworks = app / "Contents" / "Frameworks"
            frameworks.mkdir()
            mixed = frameworks / "Mixed.dylib"
            mixed.write_bytes(b"\xcf\xfa\xed\xfe" + b"synthetic")
            executable(
                bin_dir / "lipo",
                """
                case "$2" in
                  *Mixed.dylib) printf 'x86_64\\n' ;;
                  *) printf 'arm64\\n' ;;
                esac
                """,
            )
            mixed_payload = self.run_script(
                VERIFY_APP,
                "--expected-arch",
                "arm64",
                str(app),
                path=bin_dir,
            )
            self.assertNotEqual(mixed_payload.returncode, 0)
            self.assertIn("Mixed.dylib", mixed_payload.stderr)
            self.assertIn("Mach-O architecture mismatch", mixed_payload.stderr)

    def test_path_validator_normalizes_and_rejects_symlink_ancestors(self) -> None:
        with tempfile.TemporaryDirectory(prefix="chaft-path-safety-test-") as name:
            root = Path(name)
            boundary = root / "workspace"
            boundary.mkdir()
            safe = subprocess.run(
                [
                    sys.executable,
                    str(PATH_SAFETY),
                    "--path",
                    str(boundary / "build" / ".." / "target"),
                    "--description",
                    "test path",
                    "--within",
                    str(boundary),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(safe.returncode, 0, msg=safe.stdout + safe.stderr)
            self.assertEqual(Path(safe.stdout.strip()), boundary / "target")

            root_path = subprocess.run(
                [
                    sys.executable,
                    str(PATH_SAFETY),
                    "--path",
                    "/tmp/..",
                    "--description",
                    "test path",
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertNotEqual(root_path.returncode, 0)
            self.assertIn("filesystem root", root_path.stderr)

            outside = root / "outside"
            outside.mkdir()
            symlink = boundary / "linked-build"
            symlink.symlink_to(outside, target_is_directory=True)
            linked = subprocess.run(
                [
                    sys.executable,
                    str(PATH_SAFETY),
                    "--path",
                    str(symlink / "nested"),
                    "--description",
                    "test path",
                    "--within",
                    str(boundary),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertNotEqual(linked.returncode, 0)
            self.assertIn("symbolic-link component", linked.stderr)

    def test_policy_and_safety_contracts_are_explicit(self) -> None:
        build_local = BUILD_LOCAL.read_text(encoding="utf-8")
        verifier = VERIFY_APP.read_text(encoding="utf-8")
        preflight = PREFLIGHT.read_text(encoding="utf-8")
        cmake = CMAKE.read_text(encoding="utf-8")
        for option in (
            "--yes",
            "--no-install-deps",
            "--install-dir",
            "--expected-commit",
            "--skip-launch",
            "--skip-open",
        ):
            self.assertIn(option, build_local)
        self.assertIn("CHAFT_QT_POLICY=developer", build_local)
        self.assertIn("cargo build --locked", (ROOT / "tools/desktop/build.sh").read_text())
        self.assertIn("QT-LOCAL-BUILD-NOTICE.txt", verifier)
        self.assertIn("Qt6::QOffscreenIntegrationPlugin", cmake)
        self.assertIn("QT_QPA_PLATFORM=offscreen", verifier)
        self.assertRegex(
            cmake,
            r'(?s)if\(CHAFT_QT_POLICY STREQUAL "release"\).*'
            r"QT-CORRESPONDING-SOURCE\.json.*else\(\).*"
            r"QT-LOCAL-BUILD-NOTICE\.txt",
        )
        self.assertIn("Signature=adhoc", verifier)
        self.assertIn("TeamIdentifier=not set", verifier)
        self.assertIn("CHAFT_QT_SDK_PROVENANCE", preflight)
        self.assertGreaterEqual(
            build_local.count("git rev-parse --verify HEAD^{commit}"),
            2,
        )
        self.assertIn(
            'source_commit_after" != "$source_commit',
            build_local,
        )
        self.assertIn('CHAFT_QT_POLICY STREQUAL "developer"', cmake)
        self.assertIn('CHAFT_QT_POLICY STREQUAL "release"', cmake)
        self.assertIn("macOS-${CHAFT_MACOS_ARCHITECTURE}", cmake)
        combined = build_local + verifier
        for forbidden in ("sudo", "xattr", "spctl", "Gatekeeper"):
            self.assertNotIn(forbidden, combined)
        self.assertNotRegex(combined, r"curl[^\n]*\|")


if __name__ == "__main__":
    unittest.main()
