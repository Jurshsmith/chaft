# Chaft website

The Chaft website is a fully static Astro application. It presents the public
product story, security posture, and cross-platform desktop release metadata
without requiring a runtime server.

## Local development

The website requires Node.js 22.13 or newer and the pnpm version pinned in
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
instead of hard-coding root-relative `/...` URLs. For the second example, the
build physically places the public site beneath `dist/chaft/` so Cloudflare can
serve `/chaft/*` directly from the asset tree. Deploy the complete `dist/`
directory; `_headers` and `_redirects` intentionally remain at its root.

## Public documentation

`guides/public/**/*.md` is the single source for the website's `/docs/` pages.
Astro validates each guide's front matter, derives its route from its file
path, excludes drafts, and renders the remaining guides into the same static
`dist/` artifact as the landing page.

Authors should use ordinary relative Markdown links so the guides continue to
work on GitHub. The website's tested remark transform converts links ending in
`.md` to base-aware `/docs/` routes, preserves heading fragments, and rejects
missing targets or paths that escape `guides/public/`.

Do not import private deployment documentation into this public collection.
Public guides must not contain infrastructure-repository references or
passphrases in command arguments.

## Validation

```sh
pnpm validate
```

This validates the documentation sources, Astro components with the supported
TypeScript 6 compatibility package, plain TypeScript sources with the native
TypeScript 7 compiler, release-manifest tests, root-domain output, and
path-prefixed output against reserved validation URLs.
It verifies published and draft routes, metadata, headings, navigation, links,
canonical URLs, sitemap coverage, and the physical Cloudflare asset layout
before running the exact-pinned Wrangler version in strict dry-run mode against
the reviewed Worker configuration. It also validates the checked-in Chaft
Previews cycle manifest. A standalone `pnpm build` fails unless
`SITE_URL` is set, preventing deployable output with localhost metadata.

Browser, accessibility, visual-layout, and performance QA is a separate gate
because it requires installed browser binaries:

```sh
pnpm exec playwright install --with-deps chromium firefox webkit
CHAFT_DEPLOYMENT_MODE=preview \
CHAFT_PREVIEW_BRANCH=preview/landing-hero-1 \
SITE_URL=https://hero-1.chaft.ai \
pnpm qa
```

See [`previews/landing-hero/README.md`](previews/landing-hero/README.md) for the
four fixed Preview slots, invariant product contract, review process, and
focused QA commands.

## Release manifest contract

`src/data/release-manifest.json` is the website's build-time input. It must be
generated with `tools/desktop/export-website-release-manifest.py` only after the
desktop release workflow has produced final packages, target-qualified
checksums, SBOMs, provenance, and native verification receipts. Run the exporter
with `--help` for the complete four-target contract: Windows x86-64, macOS
x86-64, macOS arm64, and Linux x86-64. Its required
`--source-root` must be a Git checkout containing the release tag; the exporter
resolves that tag locally and requires every target's provenance commit and
source-material hashes to match it.

The validator requires exactly those four target entries. The only exception is
the exact immutable three-target manifests for `v0.1.0-canary.1` and
`v0.1.0-canary.2`; their tags, source commits, and legacy OS-qualified evidence
names are pinned, so a partial current release cannot be reclassified as
legacy. An asset marked `available` must include its final filename, positive
byte size, and 64-character SHA-256 digest. It also requires direct GitHub
Release links for the target's checksum file, CycloneDX SBOM, provenance,
and—when signed or notarized—the native verification receipt. Signed Linux
artifacts additionally require a detached signature. Package extensions,
release tag, version, commit, evidence filenames, and evidence URLs are
cross-checked during the static build.

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
release and each asset, and reconstructs the four target-specific package
directories (Windows x86-64, macOS Intel, macOS Apple Silicon, and Linux x86-64)
with `tools/desktop/stage-website-release-assets.py`. Read-only native jobs then
rerun Authenticode verification on Windows, Apple signing/notarization checks on
macOS, and OpenPGP/ELF verification for a signed Linux release. The exporter
requires the fresh trusted results to match every security-relevant claim in the
public receipts before it generates the manifest and runs `pnpm validate`.

The final publication job does not install dependencies or execute repository
Python. It can copy only the checksummed manifest/history payload, pushes a
descriptive `release/v<version>-website-manifest` branch, verifies its exact
remote head, and records a review handoff rather than mutating `main` directly.
It intentionally does not create a pull request with `GITHUB_TOKEN`: an
authenticated maintainer or approved GitHub integration must recheck the
reported branch and commit, then open the pull request. Actions used by this
release workflow are pinned to full commits. A repeated run is idempotent when
the default branch or an existing promotion branch already contains the same
immutable manifest.

## Release repository configuration

Before the first production promotion:

1. Enable GitHub immutable releases for the repository.
2. Add these Actions secrets:

   - `CHAFT_WINDOWS_SIGNER_THUMBPRINT`: the exact 40-hex SHA-1 or 64-hex SHA-256
     signer-certificate thumbprint required on every Windows payload.
   - `CHAFT_APPLE_TEAM_ID`: the exact 10-character Developer Team ID required on
     the DMG and every mounted app.
   - `CHAFT_LINUX_SIGNING_FINGERPRINT`: the exact 40- or 64-hex primary OpenPGP
     fingerprint, required only when Linux is published as signed.
   - `CHAFT_LINUX_SIGNING_KEYRING_BASE64`: base64 of the exact public keyring
     used for Linux verification, also required only for signed Linux. Never
     place a private signing key in this value.

Keep the repository's default workflow permission read-only. The promotion
jobs receive only the narrow `contents: write` permission required to publish
their verified manifest branches; they do not require or request permission to
create or approve pull requests.

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

The canary manifest intentionally points to GitHub Releases without claiming
that current development packages are signed production downloads.

## Deployment foundation

`wrangler.jsonc` defines an asset-only `chaft-website` Worker for Cloudflare
Workers Static Assets. It has no Worker script or runtime binding and owns the
exact Custom Domains `chaft.ai` and `www.chaft.ai`. `workers_dev` and
Cloudflare-generated preview URLs are disabled. Wrangler is an exact
development dependency. Validation performs a strict dry run, while protected
deployment installs an already verified immutable artifact rather than
rebuilding it. The production build rewrites `_headers` and `_redirects` for
the configured `SITE_URL` base path and physically nests every public asset
beneath that path. Headings, navigation, controls, and labels keep the bundled
Space Grotesk UI font and its OFL license. Body copy uses Chillax from
Fontshare's official API under the ITF Free Font License; those font files are
not redistributed with the site. Cloudflare control files stay at the asset
root, as required by Workers Static Assets.

The main `CI` workflow owns pull-request validation. The `Website` workflow
validates pushes against `https://website-validation.invalid`; a push to `main` builds a
production candidate only when the `WEBSITE_SITE_URL` repository variable is
set. While it is unset, validation passes and candidate construction is
intentionally skipped; no placeholder origin is built. Manual candidate
generation requires an explicit `site_url` and is never a production trigger.

Each candidate is uploaded as one atomic `chaft-website-<commit>` bundle:

```text
artifact-manifest.json
site/
  _headers
  _redirects
  {optional SITE_URL base}/
    .well-known/chaft-deployment.json
    404.html
    ...the complete public static site...
```

The manifest records the byte size and SHA-256 digest of every path below
`site/`. The optional base directory is omitted for a root deployment; for
`SITE_URL=https://example.com/chaft`, it is `chaft/`. The public marker binds
the bundle to the source repository, full commit, normalized site URL, and
physical mount. Creation and verification reject symlinks, non-portable or
duplicate paths, extra or missing files, digest mismatches, oversized assets,
and `website-validation.invalid`. Deployment installs the verified `site/`
bytes into `dist/`; it does not rebuild them.

The checked-in deploy workflow consumes only a successful immutable candidate
from the current `main` commit. The production GitHub environment, exact
private-governance revision, scoped Cloudflare credentials, pre-deployment
inventory checks, post-deployment verification, and non-cancelling concurrency
group protect the mutation. The rollback workflow restores only an explicitly
named retained Worker version with matching source and deployment evidence; it
never infers “previous.”

Chaft Previews uses a separate exact four-slot allowlist, Preview-only
configuration, credentials, environments, governance component, and
concurrency. Candidate branches have no credentials. A trusted workflow from
protected `main` verifies each immutable static artifact before deploying it to
the branch's fixed Preview slot. The production Worker and its two domains must
remain unchanged before and after every Preview deployment.

Desktop installers do not belong in the website artifact. Publish them as
immutable GitHub Release assets or through dedicated object storage.
