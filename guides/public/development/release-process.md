---
title: Release Chaft desktop builds
description: Understand Chaft's implemented package, verification, GitHub Release, and website-manifest workflow.
section: development
order: 20
audience: contributors
status: canary
draft: false
---

# Release Chaft desktop builds

Chaft publishes desktop software through two deliberately separate channels:
an explicitly unsigned `canary` prerelease for evaluation, and a future
`stable` release that keeps the full native signing and notarization policy.
The current website manifest is the authority for whether a particular version
is actually available.

GitHub Actions artifacts produced by CI are temporary development inputs. They
are not public releases, are not a supported user download channel, and must
not be linked from the website as finished software.

> **Warning**
>
> Unsigned canary. Do not use Chaft canary builds for sensitive or production
> communication.

## What exists today

The repository implements these release boundaries:

1. Pull-request and `main` CI build four desktop targets on native runners:
   Windows x86-64, macOS Intel, macOS Apple Silicon, and Linux x86-64.
2. The `Publish desktop canary` workflow rebuilds the exact reviewed `main`
   commit on clean native runners before it creates any tag or GitHub Release.
3. Package metadata records checksums, a CycloneDX SBOM, provenance, source
   identity, and platform-specific evidence. Canary receipts state that signing
   and notarization were not performed.
4. The publisher creates one draft, verifies its downloaded package bytes on
   all four native targets, finalizes an exact 24-file namespace, and only
   then publishes it as an immutable prerelease that is never `latest`.
5. A dedicated canary promotion workflow reverifies the published release and
   publishes an exact website-manifest branch. An authenticated maintainer or
   approved GitHub integration opens that branch as a pull request before any
   download is advertised.
6. Stable publication remains a separate workflow with Authenticode, Apple
   signing/notarization, and the configured Linux signing policy.

The current package formats are:

| Target | Canary package | Canary evidence |
| --- | --- | --- |
| Windows x86-64 | `.zip` | Native draft-download smoke receipt; Authenticode marked not performed |
| macOS Intel | `.dmg` | Native x86-64 receipt; signing and notarization marked not performed |
| macOS Apple Silicon | `.dmg` | Native arm64 receipt; signing and notarization marked not performed |
| Linux x86-64 | `.AppImage` | Native draft-download smoke receipt; detached signing marked not performed |

The website and GitHub Releases page remain the source for whether a particular
canary has completed this automation.

## Source builds from immutable revisions

Technical macOS testers may build from a reviewed tag and its full commit using
`tools/macos/build-local.sh`. Announcements and documentation must name an
immutable tag and full 40-character commit; they must not tell users to build a
moving `main` branch. The guided script's `--expected-commit` option requires
the checkout to resolve to that exact clean commit.

When a release publishes a signed tag, users should verify it with an
independently trusted maintainer key. When a source archive is published, the
release must also publish its SHA-256 and users should compare the digest
before extracting. A Git commit comparison, a tag-signature check, and an
archive checksum answer different questions; do not describe one as a
substitute for all three.

The resulting `~/Applications/Chaft.app` is native to the build Mac and has
only a local ad-hoc signature. It is not Developer ID signed, notarized, or
Apple verified, and it must not be redistributed as a trusted binary.

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
python3 tools/desktop/release-metadata.py release --target linux-x86_64
python3 tools/desktop/verify-release-metadata.py release --target linux-x86_64
```

Run the platform-independent release contract tests too:

```sh
tools/desktop/release-metadata-smoke.sh
python3 tools/desktop/export-website-release-manifest-test.py
python3 tools/desktop/canary-release-assets-test.py
python3 tools/desktop/generate-unsigned-canary-receipt-test.py
python3 tools/desktop/linux-appimage-contract-test.py
python3 tools/desktop/macos-dmg-smoke-test.py
python3 tools/desktop/macos-unsigned-canary-smoke-test.py
tools/desktop/platform-verification-receipt-smoke.sh
python3 tools/desktop/release-version-test.py
python3 tools/desktop/stage-website-release-assets-test.py
```

The package contract requires each AppImage, DMG, and ZIP to contain Chaft's
AGPL license plus the Qt notice, GPL/LGPL license copies, and
`QT-CORRESPONDING-SOURCE.json`. Before publishing, confirm that the manifest's
platform-specific module list, patch order, URLs, and SHA-256 digests match the
Qt SDK used for the package.

See the [testing guide](testing.md) for the complete gate selection.

## Build non-publishing stable release inputs

The `Build desktop release inputs` workflow is manually dispatched with an
existing tag formatted as `v<semantic-version>`. It:

- checks out the current protected default branch first, then uses that trusted
  policy code to resolve and validate the requested tag;
- requires the tag commit to be an ancestor of the current default-branch
  commit, rejecting tags on unreviewed or detached history;
- treats the validated tag checkout as data until those checks finish, then
  checks that the tag and source versions agree;
- checks out that immutable commit on all four native target runners;
- runs the platform desktop gates;
- creates package metadata and verifies it against the exact source commit;
- creates and verifies one deterministic Qt 6.8.4 corresponding-source bundle
  on Linux rather than duplicating it in the platform matrix;
- smoke-tests the Windows ZIP, both macOS DMGs, and Linux AppImage on clean
  runners;
- verifies the corresponding-source bundle again on a clean runner;
- includes it in the release-input audit; and
- uploads non-publishing workflow artifacts with seven-day retention.

The combined Qt and package job allows 180 minutes only for an exceptional
cold-cache build. After the exact Qt SDK verifies, it deletes the transient Qt
source/build tree and retains only the verified SDK before Rust, CMake, and
packaging continue. Cache-hit runs remain expected to complete on their normal,
shorter path.

The workflow has read-only repository permissions. Its output is intentionally
insufficient to become a public release by itself: Windows and macOS inputs
still require their native signing processes, and every final artifact must be
verified again after signing.

## Publish an unsigned canary

Run `Publish desktop canary` only from the protected default branch. Supply:

- the next unused tag matching `v<major>.<minor>.<patch>-canary.<positive
  integer>`;
- the exact current, reviewed `main` commit;
- confirmation that the packages are intentionally unsigned canaries; and
- confirmation that immutable GitHub Releases are enabled in repository
  settings.

The workflow then:

1. requires a successful `Required` check from the trusted `CI` workflow's
   exact `main` push run;
2. confirms that neither the tag nor a release with that tag already exists;
3. builds the Qt corresponding-source bundle and all four native packages
   before it receives write permission;
4. smoke-tests the packages once after the GitHub Actions artifact boundary;
5. audits the exact 18 base assets: four packages, four target checksum files,
   four SBOMs, four provenance files, and two Qt source files;
6. creates one draft prerelease through the GitHub API and records its release
   ID;
7. downloads each package from that draft by asset ID, checks its API byte size
   and SHA-256 digest, and reruns the packaged-app smoke on its native runner;
8. emits four target-qualified receipts that bind the package bytes, asset ID,
   release ID, runner, smoke command, version, tag, architecture, and commit;
   both macOS receipts record a natively inspected ad-hoc app-bundle signature
   while the workflow proves there is no Apple team identity, DMG signature,
   or notarization ticket;
9. adds the receipts, a 22-file inventory, and an aggregate checksum file to
   form the exact 24-file public namespace;
10. redownloads and verifies the entire draft before publication;
11. publishes it as a prerelease with `make_latest=false`; and
12. proves the release is immutable, the tag resolves to the reviewed commit,
    all assets authenticate, and no canary became `latest`.

The final job explicitly dispatches `Promote desktop canary to website`.
GitHub suppresses ordinary release-triggered workflows for releases created by
`GITHUB_TOKEN`, so this explicit dispatch is part of the reviewed contract.

If a run fails after creating a draft, do not clobber assets or silently reuse a
published tag. Inspect the release ID and exact namespace. Abandon the failed
draft and increment the canary number unless a reviewed, identity-bound
recovery proves that the existing draft, commit, and asset bytes are the
intended ones. Never delete or reuse an immutable published release.

## Prepare a stable GitHub Release

A release candidate is ready for publication only when all final package bytes
and evidence files agree on the same version, tag, and source commit.

For every platform, require:

- the final package;
- platform-qualified SHA-256 checksums;
- a CycloneDX SBOM;
- build and source provenance; and
- the native verification receipt required by that platform.

Every release also requires these two exact GitHub Release assets:

- `Chaft-Qt-6.8.4-corresponding-source.zip`; and
- `Chaft-Qt-6.8.4-corresponding-source.zip.sha256`.

They are mandatory compliance assets, not temporary build inputs. Retain the
bundle and checksum alongside the Windows, both native macOS, and Linux
binaries for the full lifetime of the release.

Stable Windows packages require trusted Authenticode verification. Each stable
macOS architecture requires Developer ID signing, notarization, stapling,
native policy assessment, and its own target-qualified verification receipt.
Linux publication follows the configured policy:
it is either checksummed-only with no detached signatures, or signed with a
trusted fingerprint, keyring, detached signatures, and verification receipt.

Upload the final, immutable assets to the matching GitHub Release. Do not
replace assets under an existing tag. A correction requires a new version and
tag so users can identify the exact bytes they received.

## Promote a published canary

`Promote desktop canary to website` is dispatch-only and accepts one exact
canary tag. Before it can write a manifest branch, its read-only preparation job:

- requires a published, immutable prerelease containing exactly 24 safe,
  uniquely named assets;
- downloads every asset by immutable asset ID and compares local byte sizes and
  SHA-256 digests with GitHub's API;
- runs GitHub's release and per-asset verification;
- checks that every smoke receipt's release and package asset IDs match the
  remote release;
- verifies the complete inventory and aggregate checksum set offline;
- stages the namespace explicitly as channel `canary`;
- generates a schema-v2 manifest whose final URLs include the immutable tag and
  filename; and
- runs the complete static website validation.

Only a bounded, checksummed JSON payload crosses into the write-enabled job.
That job creates one descriptive `release/<tag>-website-manifest` branch,
rechecks its exact remote head, and writes the branch and commit to the workflow
summary. It intentionally does not use `GITHUB_TOKEN` to create a pull request.
An authenticated maintainer or approved GitHub integration must recheck the
reported head and open the review. The promotion workflow does not merge that
pull request or deploy Cloudflare.

Use the values from the successful promotion summary for the handoff:

```sh
tag=v0.1.0-canary.1
branch="release/${tag}-website-manifest"
expected_head="<verified head commit from the workflow summary>"

test "$(git ls-remote --heads origin "refs/heads/${branch}" | awk 'NR == 1 { print $1 }')" = "${expected_head}"
gh pr create \
  --repo Jurshsmith/chaft \
  --base main \
  --head "${branch}" \
  --title "Publish Chaft ${tag} canary downloads"
```

After creation, confirm that the pull request's head SHA is still
`expected_head` before approving or merging it. Do not recreate, force-push, or
silently update the verified branch during review.

## Promote a stable release

The `Promote desktop release to website` workflow runs when a GitHub Release is
published with `prerelease=false` and can also be dispatched for an existing
stable tag. Prereleases are ignored by this workflow and must use the canary
path above. The stable workflow fails closed unless:

- the tag, release, commit, and reviewed source history agree, including an
  independent check that the tag commit is an ancestor of the current protected
  default branch;
- all expected platform packages and evidence files are present;
- filenames, sizes, and SHA-256 values match;
- the mandatory Qt corresponding-source bundle and checksum are present and
  verify with the policy code from the current default branch;
- Windows and both macOS architecture-specific native verifications succeed on
  their matching runners;
- the Linux evidence matches its declared signing state; and
- unrelated, duplicate, or stale assets are absent.

After stable verification, the workflow stages immutable website release assets,
generates the release manifest, validates the static website, and publishes an
exact manifest branch with a maintainer/integration review handoff. The
workflow does not create the pull request with `GITHUB_TOKEN`. The website must
not show a platform as available until an authenticated reviewer opens and
merges that branch and the reviewed manifest contains the final direct GitHub
Release URL, byte size, digest, signing status, and evidence links.

## Public download policy

Users should obtain published Chaft binaries from the versioned
[GitHub Releases](https://github.com/Jurshsmith/chaft/releases) assets or from
the website links that point to those exact assets.

Do not direct users to:

- pull-request or `main` workflow artifacts;
- manually copied packages;
- mutable “latest” files without a versioned release;
- an unsigned artifact that is not explicitly identified as `unsigned-canary`
  with the required native smoke receipt and warning; or
- a website card whose manifest still says `coming-soon`.

Until a release completes the full gate, contributors should
[build from source](../getting-started/build-desktop.md).

## Release completion checklist

Before describing a version as public:

- all native-platform CI and clean-package smokes are green;
- the canary or stable tag resolves to the reviewed source commit;
- any source-build instructions name that fixed tag and full commit, and any
  published source archive has a reviewed SHA-256;
- final package bytes are immutable;
- checksums, SBOMs, provenance, and native evidence verify;
- the Qt corresponding-source bundle and checksum are retained alongside the
  binaries;
- the GitHub Release contains only the expected coherent asset set;
- the generated release manifest validates;
- its website pull request is reviewed and merged; and
- the public download surface shows no pending or mismatched platform.

Chaft remains canary software after publication unless the project separately
changes its maturity, signing, support, and security policy.
