---
title: Build Chaft Desktop
description: Build, launch, test, and package the native Qt desktop app from source.
section: getting-started
order: 10
audience: contributors
status: preview
draft: false
---

# Build Chaft Desktop

Chaft Desktop is a Qt 6/QML application backed by the Rust `chaft-ffi` library.
The build is suitable for development and evaluation, not as a substitute for
an official signed release. The current release-input workflow targets x86_64
Linux, macOS, and Windows; other local host architectures may build, but they
are not part of that release contract yet.

Return to the [public guide index](../index.md), or continue with the
[workspace lifecycle](workspace-lifecycle.md) after the app launches.

## Prerequisites

Install these on every platform:

- Git;
- Rust 1.97.1;
- CMake 3.28 or newer;
- Ninja;
- Python 3;
- Qt 6.8 or newer with the Network, QML, Quick, and Widgets components.

The automated builds currently use Qt 6.8.3. Keep `qt-cmake` or `qmake6` on
`PATH`, or set `QT_ROOT_DIR` to the Qt installation prefix before running the
repository scripts.

Platform toolchains:

- **Linux:** a C/C++ compiler and the Qt desktop development/QML packages.
  `patchelf` and the pinned `linuxdeploy` tools are needed for AppImage
  packaging, but not for a normal debug build.
- **macOS:** Xcode command-line tools. Homebrew can provide CMake and Ninja.
- **Windows:** Visual Studio 2022 Build Tools with the Desktop development with
  C++ workload, an x64 MSVC developer environment, and Git Bash. Use the Qt
  `win64_msvc2022_64` build so Qt and the compiler ABI match.

If you use `aqtinstall`, the repository’s CI-equivalent Qt targets are
`linux_gcc_64`, `clang_64`, and `win64_msvc2022_64` for Linux, macOS, and
Windows respectively.

## Get the source

```sh
git clone https://github.com/Jurshsmith/chaft.git
cd chaft
```

Commands below run from the repository root. On Windows, run the shell scripts
from Git Bash launched inside an x64 MSVC developer environment.

## Check the toolchain

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

Create a release-mode package on the current native OS:

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

These locally produced packages are unsigned development artifacts unless you
separately perform and verify the platform’s release-signing process.

## Troubleshooting

### Qt cannot be found

Confirm that `qmake6 --version` reports Qt 6.8 or newer. Put the matching Qt
`bin` directory on `PATH` or set `QT_ROOT_DIR` to its installation prefix, then
rerun `tools/desktop/preflight.sh`.

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
