# Chaft website

The Chaft website is a fully static Astro application. It presents the public
product story, security posture, and cross-platform desktop release metadata
without requiring a runtime server.

## Local development

The website requires Node.js 22.12 or newer and the pnpm version pinned in
`package.json`. Activate that version with Corepack before using the root Make
targets:

```sh
corepack enable
corepack install
pnpm install --frozen-lockfile
pnpm dev
```

Set `SITE_URL` to the complete public production URL when building canonical
URLs, the sitemap, and `robots.txt`. Root-domain and path-prefixed deployments
are both supported:

```sh
SITE_URL=https://example.com pnpm build
SITE_URL=https://example.com/chaft pnpm build
```

`SITE_URL` must use HTTPS and cannot include credentials, a query, or a
fragment. Its optional path becomes Astro's `base`. Internal links and public
asset references must therefore be generated from `import.meta.env.BASE_URL`
instead of hard-coding root-relative `/...` URLs. A provider serving the second
example must mount the contents of `dist/` at `/chaft`.

## Validation

```sh
pnpm validate
```

This runs Astro's static/type checks, release-manifest tests, and a production
build against a reserved validation origin. A standalone `pnpm build` fails
unless `SITE_URL` is set, preventing deployable output with localhost metadata.

## Release manifest contract

`src/data/release-manifest.json` is the website's build-time input. It must be
generated with `tools/desktop/export-website-release-manifest.py` only after the
desktop release workflow has produced final packages, platform-qualified
checksums, SBOMs, provenance, and native verification receipts. Run the exporter
with `--help` for the complete three-platform contract. Its required
`--source-root` must be a Git checkout containing the release tag; the exporter
resolves that tag locally and requires every platform's provenance commit and
source-material hashes to match it.

The validator requires Windows, macOS, and Linux entries. An asset marked
`available` must include its final filename, positive byte size, and 64-character
SHA-256 digest. It also requires direct GitHub Release links for the platform's
checksum file, CycloneDX SBOM, provenance, and—when signed or notarized—the
native verification receipt. Signed Linux artifacts additionally require a
detached signature. Package extensions, release tag, version, commit, evidence
filenames, and evidence URLs are cross-checked during the static build.

The exporter invokes the portable metadata verifier, independently rehashes
every package, and binds architecture either to native verification evidence or
to direct ELF inspection for checksummed Linux packages. Provenance architecture
must agree but is not accepted as the sole source. Promoting a new release
atomically moves an existing published current manifest into
`src/data/release-history/<version>.json`; it refuses to change an
already-published version in place. Native receipts must be created on the
matching OS with
`tools/desktop/generate-platform-verification-receipt.py` so the exporter never
treats a self-declared signing label as proof.

The `Promote desktop release to website` workflow implements the production
handoff. On a published GitHub Release (or a manual run for an existing
published tag), it downloads every uploaded asset, authenticates the immutable
release and each asset, and reconstructs the three platform package directories
with `tools/desktop/stage-website-release-assets.py`. Read-only native jobs then
rerun Authenticode verification on Windows, Apple signing/notarization checks on
macOS, and OpenPGP/ELF verification for a signed Linux release. The exporter
requires the fresh trusted results to match every security-relevant claim in the
public receipts before it generates the manifest and runs `pnpm validate`.

The final publication job does not install dependencies or execute repository
Python. It can copy only the checksummed manifest/history payload, pushes a
descriptive `release/v<version>-website-manifest` branch, and opens a pull
request rather than mutating `main` directly. Actions used by this release
workflow are pinned to full commits. A repeated run is idempotent when the
default branch or an existing open promotion branch already contains the same
immutable manifest.

## Release repository configuration

Before the first production promotion:

1. Enable GitHub immutable releases for the repository.
2. Enable **Allow GitHub Actions to create and approve pull requests** in the
   repository's Actions settings.
3. Add these Actions secrets:

   - `CHAFT_WINDOWS_SIGNER_THUMBPRINT`: the exact 40-hex SHA-1 or 64-hex SHA-256
     signer-certificate thumbprint required on every Windows payload.
   - `CHAFT_APPLE_TEAM_ID`: the exact 10-character Developer Team ID required on
     the DMG and every mounted app.
   - `CHAFT_LINUX_SIGNING_FINGERPRINT`: the exact 40- or 64-hex primary OpenPGP
     fingerprint, required only when Linux is published as signed.
   - `CHAFT_LINUX_SIGNING_KEYRING_BASE64`: base64 of the exact public keyring
     used for Linux verification, also required only for signed Linux. Never
     place a private signing key in this value.

The Linux public receipt must be generated with the same keyring bytes stored
locally as `chaft-desktop-linux-signing-keyring.gpg`; the promotion runner uses
that canonical filename so its policy claim can match exactly. Publisher
identities are public information, but Actions secrets make the policy inputs
available only to the trusted release workflow and restrict who can change
them.

Create the GitHub Release as a draft, upload the complete package, metadata,
signature, and public-receipt set, and only then publish it. Publication seals
the immutable asset namespace and triggers promotion. Do not publish first and
attempt to append receipts afterward: an immutable release cannot be repaired
in place and the fail-closed promotion rejects an incomplete set.

All uploaded assets are inspected. Windows and macOS must include their native
verification receipts. Linux may remain `checksummed`; it becomes `signed`
only when the release also includes a coherent Linux verification receipt and
detached signature for every package. A checksummed-only Linux release must not
include detached signatures. Unexpected assets stop the promotion so the
published namespace cannot silently diverge from the website evidence.

The preview manifest intentionally points to GitHub Releases without claiming
that current development packages are signed production downloads.

## Deployment

The output in `dist/` is provider-neutral. `public/_headers` and
`public/_redirects` include Cloudflare Pages-compatible defaults; other static
hosts may translate them into their native configuration. The production build
rewrites their route patterns for the configured `SITE_URL` base path. The
deploy artifact also carries the OFL license for its subsetted Space Grotesk
fonts under `licenses/`.

The `Website` GitHub Actions workflow validates pull requests against the
reserved `https://website-validation.invalid` origin. On every push to `main`,
it builds again with the `WEBSITE_SITE_URL` repository variable and uploads
`dist/` as a `chaft-website-<commit>` artifact. A manual workflow run can
override that variable with its `site_url` input. Configure the repository
variable before the first `main` build; the artifact job fails rather than
publishing output with a placeholder URL when neither value is present.

The workflow deliberately stops at the artifact boundary: download the
artifact in the chosen static-host provider's deployment job, publish its
contents at the configured origin/base path, and translate `_headers` and
`_redirects` when the provider does not support those files. No provider
credentials or production domain are assumed by this repository.

Desktop installers do not belong in the website artifact. Publish them as
immutable GitHub Release assets or through dedicated object storage.
