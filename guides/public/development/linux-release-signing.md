---
title: Configure Linux release signing
description: Configure protected OpenPGP signing for Linux AppImage releases in GitHub Actions.
section: development
order: 21
audience: contributors
status: canary
draft: false
---

# Configure Linux release signing

`Publish desktop canary` signs the Linux AppImage by default with its
`sign_linux` input. Windows and macOS remain unsigned canaries. Signing does
not change the release's canary maturity or production suitability.

## Public release key

The primary key fingerprint for Linux releases is:

```text
8653E481B50B4334631E7D7DFF8D501CF1090BC1
```

The current signing subkey expires on 2027-10-18. Verify the primary fingerprint
against this reviewed project record before trusting a downloaded release key.

## GitHub configuration

Create a GitHub Actions environment named `linux-release-signing`. Restrict
its deployment branches to the repository's protected default branch (`main`).
You can additionally require a maintainer review before the signing job starts.
Environment secrets are released only after the environment's configured
protection rules pass. See [GitHub environment configuration](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).

Add these **environment secrets** to `linux-release-signing`:

| Secret | Value |
| --- | --- |
| `CHAFT_LINUX_SIGNING_PRIVATE_KEY_BASE64` | Base64 of the binary, passphrase-protected OpenPGP signing subkey export. Export with `gpg --export-secret-subkeys`, keeping the certification primary key offline. |
| `CHAFT_LINUX_SIGNING_PASSPHRASE` | The signing subkey's passphrase, non-empty and on one line. |

Add these **repository Actions secrets**, so the draft verification and website
promotion jobs can independently verify signatures:

| Secret | Value |
| --- | --- |
| `CHAFT_LINUX_SIGNING_KEYRING_BASE64` | Base64 of the binary **public-only** keyring produced by `gpg --export`. This is published with each signed canary. |
| `CHAFT_LINUX_SIGNING_FINGERPRINT` | The full primary OpenPGP key fingerprint, without spaces. |

The public keyring and fingerprint are public verification inputs; using Actions
secrets preserves compatibility with the existing stable-release verifier.
Keep the private key and passphrase scoped to the signing environment.

## Prepare a signing subkey

Run these commands on a trusted machine in a private directory outside the
repository. GnuPG prompts for the passphrase. Use an existing dedicated project
key if one is already established, rather than replacing its identity.

```sh
umask 077
gpg --quick-generate-key 'Chaft release signing' ed25519 cert 2y
gpg --fingerprint 'Chaft release signing'
```

Copy the full **primary** fingerprint into the following variable, then create
a signing subkey:

```sh
CHAFT_SIGNING_FINGERPRINT='REPLACE_WITH_FULL_PRIMARY_FINGERPRINT'
gpg --quick-add-key "$CHAFT_SIGNING_FINGERPRINT" ed25519 sign 1y
gpg --export "$CHAFT_SIGNING_FINGERPRINT" > linux-signing-public.gpg
gpg --export-secret-subkeys "$CHAFT_SIGNING_FINGERPRINT" > linux-signing-subkeys.gpg
```

Retain the primary key and its revocation certificate in an offline backup.
The CI export needs the signing subkey, not the certification primary key.
Keep the export files private and delete temporary copies after configuration.
GnuPG documents [key and subkey management](https://www.gnupg.org/documentation/manuals/gnupg/OpenPGP-Key-Management.html).

You can configure the secrets with GitHub CLI from that private directory:

```sh
base64 < linux-signing-subkeys.gpg | gh secret set \
  CHAFT_LINUX_SIGNING_PRIVATE_KEY_BASE64 \
  --repo Jurshsmith/chaft --env linux-release-signing

# Prompts for the passphrase; do not put it in a shell command or history.
gh secret set CHAFT_LINUX_SIGNING_PASSPHRASE \
  --repo Jurshsmith/chaft --env linux-release-signing

base64 < linux-signing-public.gpg | gh secret set \
  CHAFT_LINUX_SIGNING_KEYRING_BASE64 --repo Jurshsmith/chaft

printf '%s' "$CHAFT_SIGNING_FINGERPRINT" | gh secret set \
  CHAFT_LINUX_SIGNING_FINGERPRINT --repo Jurshsmith/chaft
```

Publish the actual public key fingerprint through a separately reviewed project
channel before announcing signed downloads, so users can establish trust in it.
The signing pipeline cannot establish that initial trust for them.

## Release behavior

Dispatch `Publish desktop canary` from `main` with `sign_linux=true` and the
normal exact-commit, unused-tag, canary, and immutability confirmations.
Missing or mismatched signing secrets stop the release; they never silently
produce an unsigned Linux download. Explicit `sign_linux=false` retains the
existing unsigned-canary path for evaluation.

The dedicated signing job has read-only repository access, restores no build
cache, and never executes the AppImage. It verifies the original build metadata,
imports the encrypted signing subkey into an ephemeral GnuPG home, signs the
final package, and verifies the result against the independently supplied
public keyring and fingerprint. It adds the signature to the package checksums,
SBOM artifact properties, and provenance artifact list while retaining the
original build/source/dependency records. The GnuPG agent and temporary key
home are removed at the end of the signing operation.

The normal native smoke tests still run after the Actions artifact boundary and
after downloading the draft package. The Linux draft-download job additionally
downloads the signature and public key by GitHub asset ID, checks their API
digests, and performs native OpenPGP verification. Its receipt binds both the
signature evidence and the smoked download's package bytes, release ID, asset
ID, runner, and source identity.

A signed Linux canary has exactly **26 assets**: the original 24-file release
namespace plus an AppImage `.asc` signature and
`chaft-linux-release-signing-key.gpg`. The inventory and aggregate checksums bind
the complete set. Older unsigned releases retain their exact 24-file namespace.
Website promotion independently reruns OpenPGP verification using repository
trust inputs before advertising the Linux artifact as `signed`.

## Verify a downloaded release

Download the AppImage, its matching `.asc`, and
`chaft-linux-release-signing-key.gpg` from the same versioned release. Run:

```sh
gpg --show-keys --fingerprint chaft-linux-release-signing-key.gpg
```

Compare the full primary fingerprint with the independently published project
fingerprint. Then use exact filenames for the chosen version:

```sh
gpg --no-default-keyring --keyring ./chaft-linux-release-signing-key.gpg \
  --verify Chaft-VERSION-Linux-x86_64.AppImage.asc \
  Chaft-VERSION-Linux-x86_64.AppImage
```

SHA-256 checksums detect byte changes; the OpenPGP signature additionally binds
those bytes to the trusted release key. Neither changes canary support or
security guarantees. Renew the signing subkey before expiry and review any
primary-key rotation before changing the pinned fingerprint and public keyring.
