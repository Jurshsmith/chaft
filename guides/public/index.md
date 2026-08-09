---
title: Documentation
description: Try the early Chaft desktop build, set up a test workspace, understand its security model, or contribute to the project.
section: getting-started
order: 0
audience: users
status: canary
draft: false
---

# Documentation

Learn how to try the early desktop build, work in rooms, authorize devices, and
understand how sync works without a required central chat server.

> **Early build.** Use non-sensitive test data, keep separate backups, and expect
> interfaces and credential formats to change.

## Start here

- **Try the early build:** [Download Chaft](https://chaft.ai/download/), then
  [create or join a non-sensitive test workspace](getting-started/workspace-lifecycle.md).
- **Understand the risks:** Review the [security model](concepts/security-model.md)
  and learn how
  [networking and replication](concepts/networking-and-replication.md) work before
  sharing test data.
- **Contribute:** Start with
  [CONTRIBUTING.md](https://github.com/Jurshsmith/chaft/blob/main/CONTRIBUTING.md)
  and the [testing guide](development/testing.md).

## Use Chaft

1. **Install:** [Download the early build](https://chaft.ai/download/) for your
   operating system, or [build the desktop app from source](getting-started/build-desktop.md).
2. **Create or join:**
   [Create a test workspace and its first room][create-workspace],
   or
   [join with an invite or access handoff](getting-started/workspace-lifecycle.md#join-a-workspace).
3. **Invite and manage access:**
   [Authorize teammates' devices](getting-started/workspace-lifecycle.md#invite-teammates),
   then
   [review roles and remove access](getting-started/workspace-lifecycle.md#manage-roles)
   when membership changes.
4. **Keep credentials safe:** Learn what each
   [credential file and decryption key kit](reference/credential-files.md) can do
   before storing or sharing it.
5. **Back up and export:** Keep key kits and passphrases separately, then
   [create a portable workspace export](reference/portable-workspace-export.md)
   when you need a readable copy. A portable export is not a backup.

## Understand Chaft

- [Architecture](concepts/architecture.md): desktop, Rust runtime, local stores,
  signed history, and networking boundaries.
- [Security model](concepts/security-model.md): implemented protections,
  current limitations, and safe secret handling.
- [Networking and replication](concepts/networking-and-replication.md):
  explicit peer routes, synchronization, and untrusted replicas.

## Develop and release

- [Testing Chaft](development/testing.md) maps changes to their required checks.
- [Release process](development/release-process.md) explains the difference
  between temporary CI artifacts and public GitHub Release downloads.

## Reference

- [CLI reference](reference/cli.md)
- [Credential files and decryption key kits](reference/credential-files.md)
- [Portable workspace export](reference/portable-workspace-export.md)

## Current preview limits

- **Sync depends on reachability:** New room history moves between authorized
  devices only when they can connect directly or through an explicit replica.
  Delivery is not guaranteed.
- **Recovery and discovery are not hosted:** Chaft does not provide hosted
  account recovery, global workspace discovery, or production support. Keep
  separate backups of the material you need.
- **Removal protects future access only:** Removing a member and rotating keys
  cannot retract content another device already received.

[Read the full security model](concepts/security-model.md) before expanding a
test workspace.

## Report a problem

- **Public bugs and documentation:**
  [Open a public issue](https://github.com/Jurshsmith/chaft/issues/new) with the
  operating system, build or source revision, failed action, expected result,
  and a minimal reproduction.
- **Security findings:**
  [Report suspected vulnerabilities privately][private-security-report].
  Do not open a public issue for a security finding.

Before sharing logs or screenshots, remove credentials, passphrases, local
paths, workspace identifiers, and message content.

[create-workspace]: getting-started/workspace-lifecycle.md#create-a-workspace
[private-security-report]: https://github.com/Jurshsmith/chaft/security/advisories/new
