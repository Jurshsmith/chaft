---
title: Architecture
description: How Chaft's local-first desktop, signed event history, storage, and peer networking fit together.
section: concepts
order: 1
audience: contributors
status: canary
draft: false
---

# Architecture

Chaft is a native, local-first chat workspace. Each device keeps its own
identity, event history, search index, keys, and attachments. Devices exchange
signed data directly or through optional replica nodes; no central service is
the authority for workspace state.

This page describes the implementation in the repository today. Chaft is
early-stage software, so an implemented component is not the same as a
production-readiness or security guarantee.

## Current system shape

The current desktop path has six main layers:

1. **Qt 6 and QML desktop shell.** The native application owns windows,
   navigation, settings, and user interaction.
2. **Rust FFI boundary.** Bounded C-compatible calls connect the desktop shell
   to the Rust runtime and return structured result envelopes.
3. **Application view model.** `chaft-app` turns materialized workspace state
   into member, channel, timeline, search, attachment, and health rows for the
   UI.
4. **Local runtime.** `chaft-runtime` coordinates device identity, event
   creation, authorization, encryption, key state, storage, search, attachment
   transfer, and peer synchronization.
5. **Domain and security crates.** Domain crates define signed event types and
   deterministic authorization/materialization rules. Security crates provide
   device signatures, payload encryption, and OpenMLS state.
6. **Storage and network crates.** SQLite stores event metadata and local
   search state, the media store holds content-addressed blobs, and transport
   crates carry bounded protobuf-compatible messages over direct TCP or native
   Iroh streams.

The command-line client and headless replica use the same Rust contracts. They
are alternate entry points, not separate sources of workspace truth.

## Events are the durable collaboration record

Chaft models workspace changes as signed, append-only events. An event includes
its workspace, author device, causal parents, body, and signature-derived
identity. Creating a new event currently follows this shape:

1. Load and verify the relevant local history.
2. Materialize it to determine the current members, roles, channels, and causal
   heads.
3. Authorize the proposed action against that history.
4. Encrypt private content when the event carries a message or attachment.
5. Sign the event with the local device identity and append it to the event
   store.
6. Refresh the local view and search state.

Receiving an event does not make it valid. Signatures are checked, event IDs
must match their signed content, and materialization applies authorization and
causal rules. Missing parents or missing authorization context are reported as
gaps rather than silently treated as complete history.

Materialized views are derived state. The signed history remains the portable
collaboration record, while UI snapshots and the search index can be rebuilt
from locally available, valid history and key material.

## Local state and ownership

A runtime directory currently contains:

- a persistent device signing identity;
- a SQLite event store using WAL mode;
- a separate local FTS5 search database;
- workspace and private-channel key material;
- private OpenMLS key packages and group state when OpenMLS is active;
- content-addressed attachment blobs and chunk manifests;
- bounded transfer, recovery, and compromise-response metadata.

That directory belongs to one local device profile. Copying it is not the
supported way to create another member device because it also copies identity
and private state. A new device should create its own identity and join through
the normal authorization flow.

Workspace recovery bundles are deliberately narrower: they transfer available
manual decryption key rings, but not the device identity, membership,
authorization, OpenMLS private state, or ownership. See
[Create, join, and administer a workspace](../getting-started/workspace-lifecycle.md)
for the user-facing distinction between an invite and a decryption key kit.

## Data crosses explicit boundaries

The runtime treats UI input, files, peer frames, event batches, OpenMLS
artifacts, and blob chunks as bounded inputs. Network data is decoded and
validated before it reaches storage or materialization. Peers may provide
history, but they do not decide whether that history is authentic or
authorized.

Message bodies and attachments are encrypted before replica publication.
Workspace metadata and the signed event graph are not fully hidden by content
encryption, and local search needs decrypted content on the device. The
[security model](security-model.md) explains these trust boundaries and the
current at-rest limitations.

Peer synchronization compares event inventories, transfers missing signed
events, and separately transfers required encrypted blobs. Read
[Networking and replication](networking-and-replication.md) for the current
transport and replica behavior.

## Current behavior versus planned direction

**Current behavior:** the repository contains an executable local runtime,
native desktop integration, direct TCP sync, native Iroh streams for explicit
endpoints, encrypted replica storage, signed endpoint announcements, local
search, OpenMLS bootstrap and group-update paths, and manual-key compatibility
paths.

**Not currently promised:** a globally discoverable workspace directory,
guaranteed offline delivery, an always-available public relay service,
person-wide identity across devices, root-owner transfer, or production-grade
account recovery.

**Planned direction:** keep the signed local event model while hardening
transport availability, secret storage, multi-device identity, recovery,
packaging, and release operations. Public discovery, if introduced, requires a
separate metadata, abuse, and privacy design. Planned work is a direction, not
a statement that those capabilities already exist.

For repository setup and supported development entry points, use the
[project README](https://github.com/Jurshsmith/chaft/blob/main/README.md).
