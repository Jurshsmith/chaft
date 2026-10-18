---
title: Test Chaft changes
description: Choose and run the Rust, replication, desktop, website, and release checks that protect Chaft.
section: development
order: 10
audience: contributors
status: canary
draft: false
---

# Test Chaft changes

Chaft spans a Rust workspace, peer-to-peer transports, a Qt/QML desktop
application, and a static Astro website. Run the smallest focused test while
developing, then run every gate relevant to the boundary you changed.

## Start with the repository gate

From the repository root, run:

```sh
tools/ci/rust-gates.sh --locked
```

This checks formatting, compiles every workspace target, runs Clippy with
warnings denied, compiles the public benchmark target, and runs all workspace
tests. If dependencies are already cached and the machine must stay offline,
use:

```sh
tools/ci/rust-gates.sh --offline
```

Use a package filter for a fast feedback loop before the full gate:

```sh
cargo test -p chaft-runtime
cargo test -p chaft-ffi
cargo test -p chaft-cli
```

Run a named regression directly when iterating:

```sh
cargo test -p chaft-runtime workspace_recovery_bundle_wrong_passphrase
```

## Match checks to the change

| Changed area | Minimum additional validation |
| --- | --- |
| Runtime, sync, network, replica node, or CLI | `tools/smoke/local-p2p.sh --locked` |
| Access requests or join transport | `tools/smoke/access-transport.sh --locked` |
| Workspace lifecycle and recovery | `tools/smoke/workspace-lifecycle.sh --locked` |
| Snapshot, timeline, search, attachments, reactions, or navigation | `tools/smoke/visual-workspace.sh --locked` |
| Qt/QML desktop behavior | Desktop checks and smoke tests below |
| Static website or public guides | `make website-validate` |
| Packaging or release metadata | Release checks below |

The smoke scripts create isolated temporary runtimes. They do not require a
public relay or production service.

## Choose GitHub Actions coverage

Pull requests and main-branch pushes select checks from their changed paths.
Website and public-documentation changes run website validation without native
builds. Native desktop changes retain package checks on main, and Rust test
changes retain benchmark compilation on main. Known website workflows run
website checks; release publishers run release contracts and package checks;
release-promotion workflows run website and release-contract checks.
Mac developer-install checks run for installer, Homebrew, and relevant CMake
changes, plus nightly and full runs. Ordinary desktop and package changes use
the official native SDK checks without repeating the developer-install builds.

Changes to the CI orchestrator or classifier, unknown paths, and nightly runs
still require the complete suite. Every CI run starts with the classifier and
workflow contract tests. The required aggregate check rejects missing or
unexpectedly skipped checks.

The manual **CI** workflow offers these scopes:

| Scope | Coverage |
| --- | --- |
| `full` | Complete suite, including artifact transfer contracts; the default |
| `website` | Static website and public guides |
| `rust` | Formatting, Clippy, Rust tests, and Rust smoke tests |
| `desktop` | Desktop contracts and all native debug builds |
| `local-install` | Mac guided developer-install checks on Intel and Apple Silicon |
| `release-contract` | Platform-independent release-tool regression checks |
| `package` | Release contracts, all native packages, and clean package smokes |
| `benchmarks` | Benchmark lint and compilation |

For example, run release-tool checks without provisioning Qt SDKs:

```sh
gh workflow run ci.yml --ref main -f scope=release-contract
```

Run the complete suite explicitly:

```sh
gh workflow run ci.yml --ref main -f scope=full
```

Different manual scopes on the same branch have separate cancellation groups.
Scope selection applies only to manual runs; it cannot narrow pull-request,
push, or nightly coverage. A successful focused manual run does not replace
the successful main-push CI check required for canary publication.

Native checks use a reusable workflow per platform. Each platform provisions
its own verified SDK, then starts its selected contracts, debug checks, and
package build independently of other platforms. A clean package smoke starts
after that platform's package upload. A platform result succeeds only when
every requested stage succeeds; missing results, failures, cancellations, and
unexpected skips fail the required check. SDK compatibility, provenance, and
strict artifact digest checks apply throughout this workflow.

Verified SDK archives from successful main-push or nightly CI runs can be reused across
branches. The lookup binds the exact SDK recipe and toolchain compatibility
fingerprint, then downloads one immutable artifact ID. Archive checksums,
embedded provenance, executable permissions, and native SDK probes are checked
again. The original producer fingerprint is preserved when only the hosted
runner image revision differs; compiler, tools, architecture, source, and build
configuration changes still require a matching SDK.

An SDK archive is uploaded when no trusted matching archive is available, or
when a main-push or nightly run refreshes an archive within two days of expiry.
Main-push and nightly archives are retained for 14 days; other runs retain their own SDK
archives for one day. Other jobs in the same run consume that exact SDK artifact
without relying on cache writes. A missing trusted archive falls back to the
exact cache and then a verified source build. Each platform summary records
whether an archive or cache was reused and whether a source build ran.
Reruns use a fresh upload name when an earlier attempt already uploaded the
canonical artifact; existing artifact IDs are preserved. Canary and stable
release builds use the same trusted archive lookup, with a cache/source-build
fallback for older tags that do not contain the SDK artifact helper.

## Test desktop changes

Desktop development requires Rust 1.97.1 or newer, CMake 3.28 or newer, Ninja,
and a policy-compatible Qt. Official package testing uses the exact verified
Qt 6.8.4 SDK. Native macOS developer testing may use the reviewed Homebrew Qt
6.11 compatibility range. Confirm the selected policy first:

```sh
tools/desktop/preflight.sh
```

For a typical UI or FFI change, run:

```sh
tools/desktop/qml-lint.sh
python3 tools/desktop/style-lint.py
python3 tools/desktop/invite-form-contract-check.py
python3 tools/desktop/theme-contrast-check.py
tools/desktop/build.sh debug
tools/desktop/smoke.sh debug
tools/desktop/live-sync-smoke.sh debug
```

Linux contributors should also run the screenshot baseline when visual output
changes:

```sh
tools/desktop/screenshot-smoke.sh debug
```

The platform-aware aggregate gate runs these checks, builds a release package,
smokes the installed package, and verifies its metadata:

```sh
tools/desktop/ci-gates.sh Linux
```

Use `macOS` or `Windows` on those platforms. The package stage requires the
exact target-specific Qt 6.8.4 activation. For a quicker local pass that skips
release packaging:

```sh
CHAFT_DESKTOP_SKIP_PACKAGE=1 tools/desktop/ci-gates.sh Linux
```

## Test replication and recovery

The local P2P smoke builds the CLI and headless node, creates independent
devices, exercises invitation and encrypted replication, verifies incomplete
backup behavior, restores full history, imports key material, and checks
decrypted search:

```sh
tools/smoke/local-p2p.sh --locked
```

Passphrases used by tests are supplied through owner-only files rather than
command arguments. Follow the same rule in manual testing; the
[CLI reference](../reference/cli.md) documents the supported prompt, standard
input, and file modes.

## Test the website and public guides

Install the pinned website dependencies once:

```sh
corepack enable
make website-install
```

Then run the complete static-site gate:

```sh
make website-validate
```

That command runs Astro and TypeScript checks, unit tests, root-domain and
path-prefixed static builds, route and asset validation, and a route-less
Wrangler dry run. Public guide front matter and links are part of the same
validation boundary.

Use the smaller commands while iterating:

```sh
make website-check
make website-test
SITE_URL=https://example.com make website-build
```

No website validation command publishes a production deployment.

## Test packages and release tooling

Create and smoke a local release package on the current platform:

```sh
tools/desktop/package-smoke.sh release
python3 tools/desktop/release-metadata.py release --target linux-x86_64
python3 tools/desktop/verify-release-metadata.py release --target linux-x86_64
```

Use `macos-x86_64`, `macos-arm64`, or `windows-x86_64` for the matching native
package. The verifier rejects package formats, architectures, source commits,
checksums, SBOMs, Qt SDK identities, or provenance that do not match the exact
target and checkout.

The release tools also have platform-independent regression checks:

```sh
tools/desktop/release-metadata-smoke.sh
python3 tools/desktop/canary-release-assets-test.py
python3 tools/desktop/export-website-release-manifest-test.py
python3 tools/desktop/generate-unsigned-canary-receipt-test.py
python3 tools/desktop/linux-appimage-contract-test.py
python3 tools/desktop/macos-dmg-smoke-test.py
python3 tools/desktop/macos-unsigned-canary-smoke-test.py
tools/desktop/platform-verification-receipt-smoke.sh
python3 tools/desktop/release-version-test.py
python3 tools/desktop/stage-website-release-assets-test.py
```

These checks validate release contracts; they do not publish a release. See the
[release process](release-process.md) for the distinction between temporary CI
artifacts and public downloads.

## Before opening a pull request

Confirm that:

- formatting and relevant tests pass;
- security, storage, wire-format, migration, and UI-thread implications are
  called out;
- new behavior has a regression test at the affected boundary;
- generated runtime data, keys, recovery files, databases, logs, and build
  output are not staged;
- desktop UI changes include an appropriate smoke result or screenshot; and
- the pull request lists the exact validation commands run.

Read [Contributing](https://github.com/Jurshsmith/chaft/blob/main/CONTRIBUTING.md)
for repository boundaries and the
[Security Policy](https://github.com/Jurshsmith/chaft/blob/main/SECURITY.md)
before reporting a vulnerability.
