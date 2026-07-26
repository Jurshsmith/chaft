---
title: Chaft public guides
description: Build, evaluate, understand, and contribute to the Chaft desktop application.
section: getting-started
order: 0
audience: users
status: preview
draft: false
---

# Chaft public guides

Chaft is an early-stage, local-first, peer-to-peer desktop chat workspace. The
codebase and development builds are available for evaluation, but the product
is still in preview: interfaces can change, peer connectivity is intentionally
explicit, and you should not rely on Chaft as the only copy of important data.

These guides describe behavior that exists in the repository today. They do not
promise a stable release schedule, unattended internet-wide discovery, or
production support.

## Choose a starting point

| Goal | Guide |
| --- | --- |
| Build and launch the native desktop app from source | [Build Chaft Desktop](getting-started/build-desktop.md) |
| Create, join, and administer a workspace | [Workspace lifecycle](getting-started/workspace-lifecycle.md) |
| Contribute changes to the project | [Contributing](https://github.com/Jurshsmith/chaft/blob/main/CONTRIBUTING.md) |

If you are evaluating the app for the first time, build it on a supported
native development host, launch it with a fresh local runtime, and then follow
the workspace lifecycle guide.

## Understand the system

- [Architecture](concepts/architecture.md) explains the desktop, Rust runtime,
  local stores, signed event model, and networking boundary.
- [Security model](concepts/security-model.md) separates current protections
  from preview-stage limitations and operational responsibilities.
- [Networking and replication](concepts/networking-and-replication.md) explains
  explicit peer routes, synchronization, and untrusted replica behavior.

## Develop and release

- [Testing Chaft](development/testing.md) maps Rust, replication, desktop,
  website, and release changes to the checks they require.
- [Release process](development/release-process.md) distinguishes temporary CI
  packages from verified public GitHub Release downloads.

## Reference

- [CLI reference](reference/cli.md) covers common developer commands and safe
  prompt, standard-input, and owner-only file handling for passphrases.
- [Portable workspace export](reference/portable-workspace-export.md) defines
  the readable plaintext export and its privacy and completeness contract.

## Preview boundaries

- Chaft stores runtime state locally and synchronizes with explicitly supplied
  peers. A central service does not silently recover a device or discover every
  workspace.
- Invites, access files, request packages, and decryption key kits are sensitive
  handoff material. Share them through a trusted channel and keep decryption key
  kits separate from their passphrases.
- A decryption key kit is not an account backup. It does not authorize a new
  device, transfer ownership, or include every kind of private state.
- Removal and key rotation protect future access. They do not retract content
  that another device already received.
- Locally built packages are development artifacts. Do not present them as
  official signed releases.

## Before reporting a problem

Record the operating system, CPU architecture, Qt version, Rust version, the
command or UI action that failed, and a minimal reproduction. Remove workspace
credentials, passphrases, local paths, and message content before sharing logs
or screenshots.

For build failures, start with the preflight and troubleshooting sections in
[Build Chaft Desktop](getting-started/build-desktop.md). For access or recovery
questions, start with [Workspace lifecycle](getting-started/workspace-lifecycle.md).
