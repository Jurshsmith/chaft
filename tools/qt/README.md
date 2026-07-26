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

Capture the native build-tool contract, then print the cache/release identity:

```sh
python3 tools/qt/build_qt.py toolchain-contract \
  --platform PLATFORM \
  --output /absolute/path/to/qt-toolchain.json
python3 tools/qt/build_qt.py identity \
  --platform PLATFORM \
  --toolchain-contract /absolute/path/to/qt-toolchain.json
```

Build into an empty install prefix:

```sh
python3 tools/qt/build_qt.py build \
  --platform PLATFORM \
  --prefix /absolute/path/to/qt \
  --toolchain-contract /absolute/path/to/qt-toolchain.json
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
  --prefix /absolute/path/to/qt \
  --toolchain-contract /absolute/path/to/qt-toolchain.json
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

## Corresponding-source release assets

Every public desktop release must retain one exact copy of the Qt source
materials beside the Windows, macOS, and Linux binaries. Create the two
required assets with:

```sh
python3 tools/qt/source_bundle.py create \
  --output-dir dist/qt-corresponding-source
```

This downloads and SHA-verifies all five source archives and six security
patches, then creates the byte-deterministic
`Chaft-Qt-6.8.4-corresponding-source.zip` and its adjacent `.sha256` file. The
ZIP also contains both checked manifests, the packaged notices and license
texts, the exact SDK build driver and probes, an ordered patch guide, and
internal checksums.

Verify downloaded release assets without network access:

```sh
python3 tools/qt/source_bundle.py verify \
  --bundle dist/qt-corresponding-source/Chaft-Qt-6.8.4-corresponding-source.zip \
  --checksum dist/qt-corresponding-source/Chaft-Qt-6.8.4-corresponding-source.zip.sha256
```

Release-input automation builds the bundle once on Linux, verifies it again on
a clean runner, includes it in the release audit, and retains it for seven
days. Public release promotion fails closed unless the immutable GitHub Release
contains and verifies both assets.

## Cache and provenance contract

Cache only the install prefix, never the source or build directories. A cold Qt
build is expected to take roughly 35–60 minutes per platform on standard public
GitHub-hosted runners; a prefix restore plus verification is substantially
faster.

The base per-platform identity is checked into the manifest and includes a
canonical hash of the manifest, the build driver, and every CMake/C++/QML
verification probe. CI extends that identity with a canonical fingerprint of
the actual hosted-runner image, CMake, Ninja, compiler, and Python versions.
The provisioning job passes that exact identity and fingerprint to consumers,
so the producer identity remains the only cache key. After installing their
platform tools, every consumer independently captures its runner/toolchain
contract and must reproduce the provision job's fingerprint before it may
restore or use the cache. A hosted-image rollout between jobs therefore fails
clearly instead of mixing SDK and consumer toolchains.

Any source, patch, build, feature, platform, plugin, recipe, probe, runner
image, or build-tool change therefore invalidates the cache. `build` writes
the complete toolchain contract and `chaft-qt-sdk-provenance.json` inside the
prefix and marks it complete only after all probes pass. `verify` rejects
incomplete provenance or any source-material, recipe-material, toolchain,
identity, platform, or manifest mismatch.

Desktop release provenance embeds that complete, verified SDK provenance and
the corresponding-source recipe contract. Tag release inputs additionally
record the SHA-256 of the exact corresponding-source ZIP. The final release
audit and website promotion cross-check every Linux, macOS, and Windows
package against the authenticated ZIP and checksum sidecar.

Run the network-free tooling contracts with:

```sh
python3 tools/qt/build_qt_test.py
python3 tools/qt/source_bundle_test.py
```
