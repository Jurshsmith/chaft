---
title: Chaft guides
description: Build the Chaft canary, create a test workspace, understand its security model, or contribute to the project.
section: getting-started
order: 0
audience: users
status: canary
draft: false
---

# Chaft guides

Use these guides to build and test the current Chaft canary, create a workspace,
or inspect how its local-first security and replication model works.

> Chaft is unaudited canary software. Use non-sensitive test data, keep separate
> backups, and expect interfaces and credential formats to change.

## Start here

- **Evaluate Chaft:** [build the desktop app from source](getting-started/build-desktop.md),
  then [create or join a test workspace](getting-started/workspace-lifecycle.md).
- **Review the trust model:** read the [security model](concepts/security-model.md)
  and [networking model](concepts/networking-and-replication.md).
- **Contribute:** start with
  [CONTRIBUTING.md](https://github.com/Jurshsmith/chaft/blob/main/CONTRIBUTING.md)
  and the [testing guide](development/testing.md).

Public Windows, macOS, and Linux packages are not available yet. CI artifacts
are temporary development inputs, not supported downloads.

## Use Chaft

- [Build Chaft Desktop](getting-started/build-desktop.md) covers native
  prerequisites, launch commands, and local package checks.
- [Workspace lifecycle](getting-started/workspace-lifecycle.md) covers workspace
  creation, invitations, access requests, roles, and removal.
- [Credential files and decryption key kits](reference/credential-files.md)
  explains every handoff file and what it does—and does not—authorize.

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

## Canary limits

- Devices must be reachable directly or through an explicitly configured
  replica to exchange new history.
- Chaft does not provide hosted account recovery, global workspace discovery,
  guaranteed delivery, or production support.
- Removing a member and rotating keys protects future access; it cannot retract
  content already received by another device.

See the [security model](concepts/security-model.md) for the complete current
boundary.

## Report a problem

Include the operating system, source revision, failed action, and minimal
reproduction. Remove credentials, passphrases, local paths, and message content
from logs and screenshots. Report security findings through the
[private security process](https://github.com/Jurshsmith/chaft/security/advisories/new),
not a public issue.
