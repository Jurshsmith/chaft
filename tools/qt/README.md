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

| Target | Required native host | Compiler and deployment contract |
| --- | --- | --- |
| `linux-x86_64` | `ubuntu-22.04`, x86_64 | GCC 11 |
| `macos-arm64` | `macos-15`, Apple Silicon | Apple Clang, arm64, macOS 12.0 deployment target |
| `macos-x86_64` | `macos-15-intel`, Intel | Apple Clang, x86_64, macOS 12.0 deployment target |
| `windows-x86_64` | `windows-2022`, x86_64 | Visual Studio 2022 x64 developer environment |

Each target is a first-class, architecture-specific SDK. The build driver
normalizes common host names such as `aarch64` to `arm64`, requires the native
host architecture to match the selected target, and rejects a
`macos-arm64` build running as a Rosetta-translated process. Rosetta is not the
supported Apple Silicon SDK path.

All hosts need Python 3, Git, CMake 3.28 or newer, and Ninja. Linux also needs
the XCB, desktop OpenGL/EGL, Wayland, font, input, and accessibility development
packages installed by Chaft's desktop jobs. Windows must initialize the VS 2022
x64 developer environment before invoking the tool.

Ubuntu dependency sets are centralized in
`tools/qt/install-linux-dependencies.sh`. Provisioning uses `sdk-build`,
restored SDK consumers use `sdk-consumer`, desktop packaging uses
`desktop-package`, release-input builds use `release-package`, and clean
AppImage runners use only `appimage-runtime`. The consumer profiles include
`libglvnd-dev` because Qt6Gui resolves the native OpenGL, GLX, and EGL CMake
targets again when a cached SDK is verified or linked on a fresh runner.

## Commands

Choose one of the four exact names above for `TARGET`.

Capture the native build-tool contract, then print the cache/release identity:

```sh
python3 tools/qt/build_qt.py toolchain-contract \
  --target TARGET \
  --output /absolute/path/to/qt-toolchain.json
python3 tools/qt/build_qt.py identity \
  --target TARGET \
  --toolchain-contract /absolute/path/to/qt-toolchain.json
```

Build into an empty install prefix:

```sh
python3 tools/qt/build_qt.py build \
  --target TARGET \
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
  --target TARGET \
  --prefix /absolute/path/to/qt \
  --toolchain-contract /absolute/path/to/qt-toolchain.json
```

Verification requires completed matching provenance, checks exact Qt 6.8.4 via
`qtpaths`, asserts the platform plugins, builds a CMake Quick/QML probe, and
runs a QML test with `qmltestrunner`. The target, logical platform, and
architecture must all match the manifest, captured toolchain contract, cache
identity, and provenance.

The legacy `--platform` selector remains accepted only when it resolves to one
target. It therefore resolves `linux` and `windows`, but fails for `macos`
because both Intel and Apple Silicon are supported. New automation should
always use `--target`.

Activate it for later GitHub Actions steps:

```sh
python3 tools/qt/build_qt.py activate \
  --prefix /absolute/path/to/qt \
  --github-env "$GITHUB_ENV" \
  --github-path "$GITHUB_PATH"
```

Without the GitHub file arguments, `activate` prints the environment values.
Activation first validates the completed provenance embedded in the prefix. In
addition to `QTDIR`, `QT_ROOT_DIR`, and `CMAKE_PREFIX_PATH`, it emits the exact
`CHAFT_QT_SDK_TARGET`, platform, architecture, Qt version, SDK identity,
toolchain fingerprint, and provenance path. Strict desktop preflight can use
those values without inferring architecture from a directory or package name.

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

The base per-target identity is checked into the manifest and includes its
logical platform and architecture plus a canonical hash of the manifest, build
driver, and every CMake/C++/QML verification probe. CI extends that identity
with a canonical build-provenance fingerprint of the target-bound hosted-runner
image, native architecture, CMake, Ninja, compiler, and Python versions. The
full runner contract includes both the hosted image family (`imageOS`) and the
provider's opaque image revision (`imageVersion`). The provisioning job's exact
identity remains the only cache key, so a new image revision still invalidates
the build cache and produces distinct SDK provenance.

Consumers use a separate, narrowly defined compatibility fingerprint before
restoring that exact producer cache:

```sh
python3 tools/ci/qt-toolchain-compatibility.py fingerprint \
  --target TARGET \
  --toolchain-contract /absolute/path/to/qt-toolchain.json
```

The compatibility helper first validates the complete target-bound contract
with `build_qt.py`. It then fingerprints the schema, target, platform, runner
OS, native architecture, hosted image family, and every captured tool version.
It omits only `runner.imageVersion`, because GitHub can schedule related jobs on
different rollout revisions of the same hosted image while their effective
build tools remain identical. No other runner or toolchain field is relaxed.

Each provisioner exports both fingerprints. A consumer independently captures
its contract and must match the producer's compatibility fingerprint before it
may restore the cache. Restore still uses the producer's exact identity, and
`verify --toolchain-fingerprint` validates the restored SDK against the full
producer fingerprint embedded in its provenance. Thus a harmless hosted-image
rollout does not strand consumers, while an OS, architecture, image family,
CMake, Ninja, compiler, Python, target, or SDK-provenance mismatch continues to
fail closed.

Any source, patch, build, feature, platform, plugin, recipe, probe, runner
image revision, architecture, target, or build-tool change therefore
invalidates the producer cache. `build` writes the complete toolchain contract
and `chaft-qt-sdk-provenance.json` inside the prefix and marks it complete only
after all probes pass. `verify` rejects incomplete provenance or any source
material, recipe material, toolchain, identity, target, platform, architecture,
or manifest mismatch.

Desktop release provenance embeds that complete, verified SDK provenance and
the corresponding-source recipe contract. Tag release inputs additionally
record the SHA-256 of the exact corresponding-source ZIP. The final release
audit and website promotion cross-check every Linux, macOS, and Windows
package against the authenticated ZIP and checksum sidecar.

Run the network-free tooling contracts with:

```sh
python3 tools/qt/build_qt_test.py
python3 tools/qt/source_bundle_test.py
python3 tools/ci/qt-toolchain-compatibility-test.py
```
