# Security Policy

Chaft is early-stage software for a local-first, peer-to-peer chat workspace.
The codebase includes cryptography, signed event history, OpenMLS bootstrap
paths, local secret storage, and untrusted peer/replica handling. Do not use it
for sensitive production communication until production release criteria are
explicitly met.

## Reporting Vulnerabilities

Please report security issues privately through GitHub Security Advisories for
this repository. Do not open a public issue for vulnerabilities involving:

- plaintext leakage
- local key or passphrase exposure
- signature or authorization bypass
- acceptance of malformed peer data
- replica node plaintext access
- denial-of-service through unbounded local, wire, or UI work

If private advisories are unavailable, contact the repository owner out of band
and include only minimal reproduction details until a private channel is agreed.

## Current Security Model

Chaft is designed around these invariants:

- No central server is authoritative.
- Local devices sign events with persistent device identities.
- Peers and replica nodes are untrusted.
- Replica nodes store encrypted, partial event/blob data.
- Materialization must reject missing authorization context and report missing
  history as gaps.
- Peer-supplied payloads, paths, strings, frame sizes, item counts, and report
  samples must remain bounded.

## Known Bootstrap Limits

- Production desktop builds still need OS keychain or user-unlocked vault
  integration for identity and local secret handling.
- Public relay/discovery policy is intentionally default-deny while the native
  Iroh path matures.
- Cross-platform packaging, release metadata, SBOM, provenance, and verification
  tooling exists, but there is no published production release.
- Production signing credentials, macOS notarization, immutable release
  promotion, and an application update channel have not been activated.
- CI packages are development artifacts and must not be treated as supported
  public downloads.

## Passphrase Handling

- Prefer the CLI's hidden interactive prompt. Recovery export and import prompt
  by default; use `--identity-passphrase-prompt` when unlocking an identity.
- For controlled automation, use the CLI's standard-input passphrase mode and
  keep the producer, consumer, and logs isolated.
- On Unix, a passphrase file must be owner-only: create it under `umask 077`,
  keep mode `0600`, and pass its path with `--passphrase-file`.
- Passphrases in command-line arguments or environment variables are not
  recommended because process listings, shell history, logs, or child processes
  can expose them.
- The environment-variable path remains a development startup fallback only,
  not a production secret-delivery mechanism.

## Supported Versions

Security fixes target the `main` branch until tagged releases exist.
