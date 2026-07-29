---
title: Build Chaft Desktop
description: Build, launch, test, and package the native Qt desktop app from source.
section: getting-started
order: 10
audience: contributors
status: canary
draft: false
---

# Build Chaft Desktop

Chaft Desktop is a Qt 6/QML application backed by the Rust `chaft-ffi` library.
The build is suitable for development and evaluation, not as a substitute for
an official signed release. The release-input workflow has four exact native
targets: Windows x86-64, macOS Intel, macOS Apple Silicon, and Linux x86-64.
Rosetta is not the supported Apple Silicon build path.

Return to the [public guide index](../index.md), or continue with the
[workspace lifecycle](workspace-lifecycle.md) after the app launches.

## Prerequisites

Install these on every platform:

- Git;
- Rust 1.97.1;
- CMake 3.28 or newer;
- Ninja;
- Python 3;
- a Qt version allowed by the build mode described below.

Chaft deliberately has two Qt policies:

- `developer` is a native macOS-only path tested with Homebrew Qt 6.11.1. The
  accepted compatibility range is Qt 6.11.1 or newer within the 6.11 series,
  and the Qt prefix must belong to Homebrew.
- `release` requires exactly Chaft's verified, patched Qt 6.8.4 SDK for the
  current target. The SDK identity, toolchain fingerprint, architecture, and
  completed provenance must all verify. Homebrew Qt cannot enter this path.

The automated package builds use the open-source Qt 6.8.4 source release with
pinned security patches. The exact source inputs and SHA-256 digests are
recorded in `packaging/qt/QT-CORRESPONDING-SOURCE.json`.

Platform toolchains:

- **Linux:** a C/C++ compiler and the Qt desktop development/QML packages.
  `patchelf` and the pinned `linuxdeploy` tools are needed for AppImage
  packaging, but not for a normal debug build.
- **macOS:** a native terminal, Xcode command-line tools, and Homebrew. The
  guided source build can install missing Homebrew formulae only after asking
  for confirmation.
- **Windows:** Visual Studio 2022 Build Tools with the Desktop development with
  C++ workload, an x64 MSVC developer environment, and Git Bash. Use the Qt
  `win64_msvc2022_64` build so Qt and the compiler ABI match.

The public Qt installer repositories do not provide credential-free Qt 6.8.4
desktop binaries for these platforms, so `aqtinstall` cannot reproduce the
CI toolchain. CI builds the exact source modules instead of silently falling
back to an older patch release.

## Get the source

Build a named tag or full commit supplied by a reviewed release or testing
announcement. Do not use a moving `main` checkout as an immutable build input.

```sh
git clone https://github.com/Jurshsmith/chaft.git
cd chaft
git fetch --tags --force

tag=vX.Y.Z-canary.N
commit=<published-full-40-character-commit>
test "$(git rev-parse "refs/tags/${tag}^{commit}")" = "$commit"
git checkout --detach "$commit"
```

Commands below run from the repository root. On Windows, run the shell scripts
from Git Bash launched inside an x64 MSVC developer environment.

If the release announcement identifies the tag as cryptographically signed,
also run `git verify-tag "$tag"` with the maintainer key you independently
trust. A successful commit comparison does not by itself prove who created the
tag. When a source archive and SHA-256 are published, compare them before
extracting:

```sh
shasum -a 256 Chaft-source-vX.Y.Z-canary.N.tar.gz
```

Use only the digest published with that immutable release. If no reviewed
commit and source checksum are available, wait rather than guessing a revision.

## Guided macOS source build

On a native Intel or Apple Silicon Mac, run:

```sh
tools/macos/build-local.sh --expected-commit "$commit"
```

The script checks Xcode command-line tools, Homebrew, Git, CMake, Ninja,
Python, Rust, and the supported Homebrew Qt. It prints the exact proposed
`brew install` command and asks before installing missing formulae. Use
`--no-install-deps` to require a pre-provisioned machine. It never needs
administrator privileges.

The script builds and exercises the native app, verifies its name, icon,
metadata, Mach-O architecture, and local ad-hoc signature, then installs:

```text
~/Applications/Chaft.app
```

At completion it prints the source commit, source state, Qt version,
architecture, app path, and exact launch command. A cold build can take roughly
30–60 minutes depending on the Mac and Homebrew cache; allow at least 10 GB of
free disk for dependencies, Rust outputs, and the native app build. Later
builds are normally faster.

The resulting signature is only an ad-hoc signature created on that Mac. It is
not Apple Developer ID signing, notarization, or Apple verification. Treat the
app as a local technical-test build and do not redistribute it as a trusted
binary.

## Check the toolchain

The remaining commands use the strict release policy unless
`CHAFT_QT_POLICY=developer` is explicitly selected on macOS.

```sh
tools/desktop/preflight.sh
```

The preflight prints the detected Rust, CMake, Ninja, and Qt locations. Resolve
every missing or too-old dependency before building. In particular, installing
Qt without putting its tools on `PATH` is not enough; set `QT_ROOT_DIR` when
automatic discovery cannot find it.

## Build and launch

Build a debug desktop app and its Rust FFI library:

```sh
tools/desktop/build.sh debug
```

Launch with a fresh, isolated local runtime:

```sh
tools/desktop/launch.sh debug --fresh
```

The launcher keeps development data below `scratch/desktop-instances/` by
default. Re-running without `--fresh` preserves that instance. To run two peers
from the same checkout, give each launch a distinct instance name:

```sh
tools/desktop/launch.sh debug --instance alice
tools/desktop/launch.sh debug --instance bob
```

The debug binary is generated below `build/desktop-debug/`; the matching FFI
library is below `target/debug/`. Prefer the launcher over invoking the binary
directly because it resolves both paths and prepares an isolated runtime.

## Run focused checks

After a successful build, run the desktop lint and smoke checks:

```sh
tools/desktop/qml-lint.sh
tools/desktop/smoke.sh debug
```

Rust changes should also pass the repository Rust gate:

```sh
tools/ci/rust-gates.sh --offline
```

The broader `tools/desktop/ci-gates.sh` command also exercises live sync,
packaging, metadata, and platform-specific checks. It is slower and requires
the packaging dependencies for the selected platform.

## Build a local package

Official release contributors can create a release-mode package after
activating the exact verified Qt 6.8.4 SDK for the current target:

```sh
tools/desktop/package.sh release
```

Expected package formats are an AppImage on Linux x86_64, a DMG on macOS, and a
ZIP on Windows. Output is written below
`build/desktop-release/package/`.

Linux packaging additionally requires the repository’s checksum-pinned
AppImage tools:

```sh
appimage_tools_dir="${TMPDIR:-/tmp}/chaft-appimage-tools"
tools/desktop/fetch-appimage-tools.sh "$appimage_tools_dir"
export CHAFT_LINUXDEPLOY="$appimage_tools_dir/linuxdeploy"
export CHAFT_LINUXDEPLOY_PLUGIN_QT="$appimage_tools_dir/linuxdeploy-plugin-qt"
export CHAFT_LINUXDEPLOY_PLUGIN_APPIMAGE="$appimage_tools_dir/linuxdeploy-plugin-appimage"
tools/desktop/package.sh release
```

This command intentionally rejects the Homebrew developer lane. Locally
produced release-policy packages are still not public releases unless the
platform signing, verification, immutable-release, and promotion workflows
complete.

Every package also includes Chaft's AGPL license, the Qt third-party notice,
the LGPL and GPL license texts, and the exact Qt corresponding-source
manifest. They are installed under `usr/share/doc/Chaft` in the AppImage,
`share/doc/Chaft` in the Windows ZIP, and
`Chaft.app/Contents/Resources/doc/Chaft` in the macOS DMG.
For a public build, the immutable GitHub Release also carries the verified
`Chaft-Qt-6.8.4-corresponding-source.zip` bundle and its `.sha256` file named
by that manifest.

## Troubleshooting

### Qt cannot be found

For the guided macOS path, confirm that Homebrew's `qtpaths6 --qt-version`
reports a supported Qt 6.11 release and rerun `tools/macos/build-local.sh`.
For official packaging, activate the repository's exact target-specific Qt
6.8.4 SDK; merely pointing `QT_ROOT_DIR` at another Qt installation is
insufficient.

### The compiler and Qt do not match on Windows

Use the x64 MSVC developer environment and Qt’s `win64_msvc2022_64` target.
Avoid mixing MinGW Qt libraries with the MSVC build.

### The app cannot load the Rust FFI library

Launch through `tools/desktop/launch.sh`, which resolves the platform-specific
library name and build profile. Confirm that `tools/desktop/build.sh debug`
reported both the desktop binary and FFI library.

### A Linux AppImage does not start

First run the debug build to separate application failures from packaging
failures. On a system without FUSE, an AppImage can be evaluated with:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 build/desktop-release/package/Chaft-*-x86_64.AppImage
```

### The app opens with no workspaces

That is the expected first-run state, not a failed build. Continue with the
[workspace lifecycle](workspace-lifecycle.md) to create or join one.
