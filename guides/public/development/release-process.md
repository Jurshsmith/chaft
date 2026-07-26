---
title: Release Chaft desktop builds
description: Understand Chaft's implemented package, verification, GitHub Release, and website-manifest workflow.
section: development
order: 20
audience: contributors
status: preview
draft: false
---

# Release process

Chaft can build and verify desktop packages for Windows, macOS, and Linux, but
public downloads are not available yet. The checked-in website manifest is
`coming-soon`, has no release tag, and marks every platform artifact
unavailable.

GitHub Actions artifacts produced by CI are temporary development inputs. They
are not public releases, are not a supported user download channel, and must
not be linked from the website as finished software.

## What exists today

The repository implements these release boundaries:

1. Pull-request and `main` CI build desktop packages on native platform
   runners.
2. A manual workflow rebuilds non-publishing release inputs from an existing
   stable tag and immutable commit.
3. Package metadata records checksums, a CycloneDX SBOM, provenance, source
   identity, and platform-specific evidence.
4. A published GitHub Release can trigger strict asset and signing
   verification.
5. Successful verification prepares a reviewed website-manifest change before
   any download is advertised.

The current x86-64 package formats are:

| Platform | Package | Current release-input state |
| --- | --- | --- |
| Windows | `.zip` | Built and smoke-tested; public delivery requires trusted signing evidence |
| macOS | `.dmg` | Built in CI; public delivery requires signing, notarization, stapling, and verification |
| Linux | `.AppImage` | Built and smoke-tested; publication requires checksums and the configured Linux signing policy |

This describes implemented automation, not an announcement that a release has
passed it.

## Validate a release change locally

Run the normal repository and desktop gates first:

```sh
tools/ci/rust-gates.sh --locked
tools/desktop/ci-gates.sh Linux
```

Use the platform name matching the current machine. To build only the package
and its local verification data:

```sh
tools/desktop/package-smoke.sh release
python3 tools/desktop/release-metadata.py release
python3 tools/desktop/verify-release-metadata.py release --platform Linux
```

Run the platform-independent release contract tests too:

```sh
tools/desktop/release-metadata-smoke.sh
python3 tools/desktop/export-website-release-manifest-test.py
python3 tools/desktop/linux-appimage-contract-test.py
tools/desktop/platform-verification-receipt-smoke.sh
python3 tools/desktop/release-version-test.py
python3 tools/desktop/stage-website-release-assets-test.py
```

See the [testing guide](testing.md) for the complete gate selection.

## Build immutable release inputs

The `Build desktop release inputs` workflow is manually dispatched with an
existing tag formatted as `v<semantic-version>`. It:

- resolves the tag to an exact commit and checks that the version agrees;
- checks out that immutable commit on Windows, macOS, and Linux runners;
- runs the platform desktop gates;
- creates package metadata and verifies it against the exact source commit;
- smoke-tests the Windows ZIP and Linux AppImage on clean runners; and
- uploads non-publishing workflow artifacts with seven-day retention.

The workflow has read-only repository permissions. Its output is intentionally
insufficient to become a public release by itself: Windows and macOS inputs
still require their native signing processes, and every final artifact must be
verified again after signing.

## Prepare a public GitHub Release

A release candidate is ready for publication only when all final package bytes
and evidence files agree on the same version, tag, and source commit.

For every platform, require:

- the final package;
- platform-qualified SHA-256 checksums;
- a CycloneDX SBOM;
- build and source provenance; and
- the native verification receipt required by that platform.

Windows packages require trusted Authenticode verification. macOS packages
require Developer ID signing, notarization, stapling, Gatekeeper assessment,
and a verification receipt. Linux publication follows the configured policy:
it is either checksummed-only with no detached signatures, or signed with a
trusted fingerprint, keyring, detached signatures, and verification receipt.

Upload the final, immutable assets to the matching GitHub Release. Do not
replace assets under an existing tag. A correction requires a new version and
tag so users can identify the exact bytes they received.

## Promote a published release

The `Promote desktop release to website` workflow runs when a GitHub Release is
published and can also be dispatched for an existing published tag. It fails
closed unless:

- the tag, release, commit, and reviewed source history agree;
- all expected platform packages and evidence files are present;
- filenames, sizes, and SHA-256 values match;
- Windows and macOS native verification succeeds on their native runners;
- the Linux evidence matches its declared signing state; and
- unrelated, duplicate, or stale assets are absent.

After verification, the workflow stages immutable website release assets,
generates the release manifest, validates the static website, and prepares a
reviewable manifest pull request. The website must not show a platform as
available until the reviewed manifest contains the final direct GitHub Release
URL, byte size, digest, signing status, and evidence links.

## Public download policy

Users should obtain published Chaft binaries from the versioned
[GitHub Releases](https://github.com/Jurshsmith/chaft/releases) assets or from
the website links that point to those exact assets.

Do not direct users to:

- pull-request or `main` workflow artifacts;
- manually copied packages;
- mutable “latest” files without a versioned release;
- unsigned Windows or macOS release inputs; or
- a website card whose manifest still says `coming-soon`.

Until a release completes the full gate, contributors should
[build from source](../getting-started/build-desktop.md).

## Release completion checklist

Before describing a version as public:

- all native-platform CI and clean-package smokes are green;
- the stable tag resolves to the reviewed source commit;
- final package bytes are immutable;
- checksums, SBOMs, provenance, and native evidence verify;
- the GitHub Release contains only the expected coherent asset set;
- the generated release manifest validates;
- its website pull request is reviewed and merged; and
- the public download surface shows no pending or mismatched platform.

Chaft remains preview software after publication unless the project separately
changes its maturity and support policy.
