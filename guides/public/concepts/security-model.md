---
title: Security model
description: What Chaft protects today, what remains visible, and which early-stage limitations users must understand.
section: concepts
order: 2
audience: users
status: preview
draft: false
---

# Security model

Chaft is designed so that no central server or replica is the authority for a
workspace. Devices authenticate changes with signatures, derive workspace state
from signed history, and encrypt message and attachment content before
replication.

Chaft is also early-stage and has not reached a production security bar. Do not
use the current builds for sensitive production communication. Cryptographic
building blocks are present, but secure operation also depends on local secret
storage, metadata privacy, transport hardening, packaging, updates, and
independent review.

## What the current implementation protects

### Device identity and event integrity

Each runtime has a persistent Ed25519 device signing identity. Signed events
carry the author's public key, and their event IDs are bound to their content
and signature. Peers cannot legitimately rewrite an event without verification
failing.

Authorization is derived from signed workspace history. Membership, roles,
private-channel grants, removals, and other administrative changes are not
trusted merely because a peer sent them. Materialization checks their causal
and authorization context, and reports missing context as a history gap.

A device identity represents one device today, not a person-wide account.
Copying another runtime's identity file would clone that device's authority and
must not be used as an onboarding method.

### Message and attachment confidentiality

Current encrypted message events use AES-256-GCM-SIV payloads. Public channels
use workspace-scoped key material and private channels use channel-scoped key
material. Where local OpenMLS group state exists, the runtime derives payload
keys from the current workspace or channel MLS epoch. Manual workspace and
channel key rings remain as a compatibility path.

Attachments carry encrypted metadata and replicas store ciphertext-addressed
blob bytes. The direct replica protocol rejects plaintext message variants,
development-plaintext payload markers, and attachments without the required
encrypted metadata.

This is content encryption, not complete metadata hiding. A peer or replica can
observe information needed to route and store signed history, such as event and
workspace identifiers, authorship, timing, causal relationships, sizes, and
blob hashes. Traffic analysis and endpoint visibility are not solved by
encrypting message bodies.

### Untrusted peers and replicas

Peers, relay candidates, and replica nodes are treated as untrusted. Received
frames, identifiers, lists, strings, events, OpenMLS artifacts, and blobs have
explicit size or count bounds. Pulled events must be requested, belong to the
expected workspace, and pass self-contained signature verification before
normal use.

An untrusted node can still withhold, delay, replay, or selectively serve data,
and it can attempt denial of service within accepted limits. Replication
improves availability; it does not make a node authoritative or guarantee that
history is complete.

## Local-device trust boundary

Chaft currently relies on the operating-system account and filesystem
permissions to protect its runtime directory.

When a user configures a runtime unlock passphrase, the identity and supported
local secret files are wrapped with Argon2id-derived keys and
AES-256-GCM-SIV. Passphrase copies and several intermediate secret buffers are
zeroized in the Rust runtime. Without an unlock passphrase, the identity and
local key files can be stored without that application-level encryption.

The event database is not a full-disk encrypted vault. Local search indexes
decrypted terms so that on-device search is fast. A process or user that can
read the runtime directory, inspect the running process, or control the device
may therefore recover sensitive local data. Use an encrypted, access-controlled
device and do not share runtime directories between users.

Recovery bundles are passphrase-wrapped decryption key kits. They do not
contain device authorization, membership, root ownership, or OpenMLS private
group state. Anyone who obtains both a bundle and its passphrase may recover
the manual keys contained in that export, so store them separately and
privately.

## Important current limitations

- The project has not completed an independent security audit.
- Production desktop integration with an OS keychain or user-unlocked vault is
  not complete.
- Release signing, notarization, trusted updates, and production packaging are
  still being hardened.
- Public relay and public discovery are default-deny and are not a general
  availability service today.
- Encryption does not hide all event, endpoint, membership, timing, or size
  metadata.
- Removing a member and rotating keys protects future access; it cannot erase
  plaintext or keys that an authorized device already obtained.
- Recovery bundles cover manual key rings only and are not complete account or
  workspace recovery.
- Mixed OpenMLS and manual-key compatibility paths increase the surface that
  still needs review.
- Availability depends on reachable devices or replicas and is not guaranteed.

## Safe secret handling

Use the application's hidden interactive prompt for passphrases. Do not place
passphrases, recovery material, signing keys, or other secrets in command-line
arguments, environment variables, shell history, logs, issue reports, or chat
messages. Keep decryption key kits separate from their passphrases, and share
invites only through channels appropriate for bearer credentials.

If a device may be compromised, stop using it, review workspace compromise
signals from a trusted device, remove the affected membership where possible,
and rotate future key material. Rotation cannot retract information already
available to the compromised device.

## Planned security direction

Planned work includes OS-backed secret storage, a clearer lock/unlock lifecycle,
stronger production release provenance and updates, expanded adversarial and
cross-platform testing, metadata minimization, and independent review. These
are planned improvements, not protections users should assume today.

See [Architecture](architecture.md) for component boundaries and
[Networking and replication](networking-and-replication.md) for peer and
replica behavior. Report vulnerabilities through the private process in the
[security policy](https://github.com/Jurshsmith/chaft/blob/main/SECURITY.md),
never through a public issue.
