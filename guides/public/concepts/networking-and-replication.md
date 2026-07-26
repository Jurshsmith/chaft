---
title: Networking and replication
description: How Chaft devices discover explicit peers, synchronize signed events, and use untrusted encrypted replicas.
section: concepts
order: 3
audience: contributors
status: canary
draft: false
---

# Networking and replication

Chaft synchronizes local workspace history with explicit peers. A peer can be
another desktop device or a headless replica node. The network transports bytes
and improves availability; signed history and deterministic authorization
remain the source of truth.

The implementation is usable for development and bounded peer-to-peer testing,
but it is still early-stage. It does not yet provide guaranteed delivery,
global discovery, or a production public relay network.

## Current peer routes

The current transport abstraction supports inventory lookup, event fetch, and
event publication. Two concrete paths implement the shared bounded wire
protocol:

- **Direct TCP** for explicit local or LAN-style `host:port` endpoints.
- **Native Iroh QUIC streams** for explicit Iroh endpoint IDs and addresses.

Workspace members can publish signed peer-endpoint announcements through normal
workspace history. An announcement may identify a direct peer, a native Iroh
peer, or a backup replica and may include a storage-class or retention hint.
These are routing hints, not an authorization grant.

Public relay and public discovery routes are default-deny. Enabling a policy
flag does not turn them into completed services: the general public relay and
discovery backends are not linked as production paths today. Unknown schemes,
malformed addresses, and zero ports are rejected before connection work.

## Event synchronization

A current workspace sync follows this sequence:

1. Ask the peer for a bounded, workspace-scoped inventory of event IDs.
2. Validate that the IDs are canonical and not duplicated.
3. Compare the remote inventory with locally servable history.
4. Publish locally valid missing events and request remote missing events in
   bounded batches.
5. Verify that fetched events were requested, match the expected workspace,
   have valid self-contained signatures, and satisfy storage limits.
6. Append events in materializable order, then derive the workspace view and
   report causal or authorization gaps.

Bidirectional sync reuses one point-in-time inventory comparison for its
publish and pull halves. That reduces redundant requests, but it does not imply
global consensus. Two offline devices can advance independently; later sync
merges the signed causal history and exposes unresolved gaps or conflicts to
the deterministic domain rules.

A device never accepts a peer's materialized UI state as truth. It accepts
bounded signed events, verifies them, and materializes its own view locally.
For the full local data flow, read [Architecture](architecture.md).

## Attachment replication

Attachments are stored separately from events as BLAKE3-addressed ciphertext
blobs. Small blobs can transfer as whole objects. Larger blobs use bounded
descriptors, chunks, availability checks, and reassembly validation.

The signed message event carries the attachment reference and encryption
metadata. A device can therefore have valid message history while the
corresponding ciphertext blob is temporarily unavailable. The runtime exposes
that state, retries eligible peers, and enables export only after the complete
blob is present and validated.

Blob availability is not proof of authorization or plaintext validity. The
event history determines whether a device may see the attachment, and local key
material is required to decrypt it.

## Replica nodes

The headless `chaft-node` can serve an encrypted event/blob store over direct
TCP or native Iroh, and it can mirror a workspace from configured peers.
Signed endpoint hints can help a mirror select additional known workspace
peers, with a bounded candidate set.

Replicas are intentionally non-authoritative:

- publication requires signed events plus sufficient authorization history or
  a signed trust snapshot;
- replica privacy policy rejects plaintext message and attachment publication;
- partial replicas may keep proof slices instead of complete workspace history;
- storage-class and retention values are hints, not durability guarantees;
- clients still verify pulled events and materialize authorization locally.

A replica can see the metadata necessary to store and serve ciphertext, and it
can delete, delay, omit, or refuse data. Use multiple independent copies when
availability matters, but do not treat replica count as a substitute for
verified history or backups.

## Access-request delivery

Join requests and approval, decline, or close responses use bounded access
envelopes distinct from normal workspace events. The runtime has durable local
inboxes and outboxes, duplicate-safe IDs, retry timing, and direct submission
or pull against known endpoints.

While Chaft is open, desktop workers can retry known direct routes. If a route
is unavailable, users retain copy/save handoff as the transparent fallback.
Current behavior does not guarantee that an offline admin or requester will
eventually receive an envelope, and received approvals still require the normal
user-confirmed join flow.

The detailed current boundary and remaining work are tracked in
[Request-access transport hardening](https://github.com/Jurshsmith/chaft/blob/main/guides/request-access-transport-hardening.md).

## Current behavior versus planned direction

**Current behavior:** explicit peer endpoints, bounded direct TCP and native
Iroh exchanges, signed endpoint hints, inventory-based event sync, encrypted
blob transfer, headless encrypted replicas, and durable direct access-envelope
queues.

**Not currently promised:** automatic arbitrary multi-hop relay, public
workspace search, DHT or name-service discovery, metadata anonymity,
always-online delivery, NAT traversal in every network, or consensus-based
global ordering.

**Planned direction:** improve relay-backed availability, conservative peer
selection, delayed envelope delivery, connection resilience, replica
operations, and observability while preserving local verification and explicit
workspace privacy. Any public discovery system requires a separate threat
model, abuse controls, revocation model, and metadata policy; it is not an
implicit extension of sync.

See [Security model](security-model.md) for what peers and replicas can still
observe and why availability must be separated from trust.
