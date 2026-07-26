# Qt 6.8.4 source SDK

Chaft builds its desktop SDK from the official open-source Qt 6.8.4 source
archives. The manifest in `qt-6.8.4.json` is the authority for source and
security-patch URLs, SHA-256 digests, build order, platform configuration,
required plugins, and cache identities.

There is no credential-free prebuilt Qt 6.8.4 SDK in Qt's public online
repositories. `aqtinstall` and `install-qt-action` consume those repositories;
they cannot install a binary that Qt does not publish. The exact open-source
6.8.4 release is available as source from `download.qt.io`, so this tool builds
that source without commercial Qt credentials.

## Supported targets

| Platform | Required host | Architecture and compiler |
| --- | --- | --- |
| Linux | `ubuntu-22.04` | x86_64, GCC 11 |
| macOS | `macos-15-intel` | x86_64, Apple Clang, macOS 12.0 deployment target |
| Windows | `windows-latest` | x86_64, Visual Studio 2022 developer environment |

All hosts need Python 3, Git, CMake 3.28 or newer, and Ninja. Linux also needs
the XCB, desktop OpenGL/EGL, Wayland, font, input, and accessibility development
packages installed by Chaft's desktop jobs. Windows must initialize the VS 2022
x64 developer environment before invoking the tool.

## Commands

Choose one of `linux`, `macos`, or `windows` for `PLATFORM`.

Print the offline cache/release identity:

```sh
python3 tools/qt/build_qt.py identity --platform PLATFORM
```

Build into an empty install prefix:

```sh
python3 tools/qt/build_qt.py build \
  --platform PLATFORM \
  --prefix /absolute/path/to/qt
```

`--work-dir /absolute/path` may be supplied to retain verified downloads between
cold builds. The driver checks every digest before extraction, applies all six
official security patches in manifest order, builds shared Release libraries
with four Ninja workers, and excludes examples, tests, benchmarks, and
documentation. QtBase is configured with CMake; subsequent modules use Qt's
installed `qt-configure-module` frontend.

Verify a restored SDK:

```sh
python3 tools/qt/build_qt.py verify \
  --platform PLATFORM \
  --prefix /absolute/path/to/qt
```

Verification requires completed matching provenance, checks exact Qt 6.8.4 via
`qtpaths`, asserts the platform plugins, builds a CMake Quick/QML probe, and
runs a QML test with `qmltestrunner`.

Activate it for later GitHub Actions steps:

```sh
python3 tools/qt/build_qt.py activate \
  --prefix /absolute/path/to/qt \
  --github-env "$GITHUB_ENV" \
  --github-path "$GITHUB_PATH"
```

Without the GitHub file arguments, `activate` prints the environment values.

## Cache and provenance contract

Cache only the install prefix, never the source or build directories. A cold Qt
build is expected to take roughly 35–60 minutes per platform on standard public
GitHub-hosted runners; a prefix restore plus verification is substantially
faster.

The full per-platform identity is checked into the manifest and includes a
canonical hash of the manifest, the build driver, and every CMake/C++/QML
verification probe. Any source, patch, build, feature, platform, plugin,
recipe, or probe change makes the checked-in identities stale until
deliberately updated. `build` writes `chaft-qt-sdk-provenance.json` inside the
prefix and marks it complete only after all probes pass. `verify` rejects
incomplete provenance or any source-material, recipe-material, identity,
platform, or manifest mismatch.

Run the network-free tooling contracts with:

```sh
python3 tools/qt/build_qt_test.py
```
