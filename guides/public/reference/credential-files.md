---
title: Credential files and decryption key kits
description: Identify Chaft invite, request, access, and decryption files and handle each one safely.
section: reference
order: 15
audience: users
status: canary
draft: false
---

# Credential files and decryption key kits

Chaft uses explicit files and links to exchange workspace access between
devices. Each file has a different purpose. Treat every one as sensitive and
share it only through a trusted channel.

## Invites

Extension: `.chaftinvite`

An invite authorizes one or more devices to request workspace access before it
expires or is revoked. A group invite has a bounded number of joins.

Current secure invites do not contain the workspace content key, but they are
still bearer credentials: anyone holding an unused invite may consume one of
its remaining joins.

## Workspace request cards

Extension: `.chaftworkspace`

A workspace request card lets another device prepare an access request. It does
not grant membership or provide decryption keys.

## Access requests

Extension: `.chaftrequest`

An access request identifies the requesting device and the display name its
user selected. An owner or administrator must approve it before the device can
join.

## Workspace access files

Extension: `.chaftaccess`

Workspace access files are a legacy handoff format that grants access to the
intended device or teammate. Do not publish or forward them.

## Decryption key kits

Extension: `.chaftrecovery`

A decryption key kit contains the manual workspace and private-room key rings
available on the exporting device. It is protected by a passphrase.

A key kit is **not** an account backup. It does not:

- authorize a new device;
- transfer workspace ownership;
- contain the device signing identity;
- include every kind of private state; or
- guarantee that matching encrypted history is available.

A fresh device still needs a normal invite or access approval. Keep the key kit
separate from its passphrase, and save a new kit after key rotation or after
gaining access to another private room.

The `.chaftrecovery` extension and recovery-bundle schema/API names remain for
compatibility. Current import flows also accept supported older JSON exports.

## Default filenames

Chaft uses descriptive filenames for saved handoff material:

- `Chaft - <workspace> - Invite - <label> - <date>.chaftinvite`
- `Chaft - <workspace> - Access Request - <person> - <date>.chaftrequest`
- `Chaft - <workspace> - Request Card - <date>.chaftworkspace`
- `Chaft - <workspace> - Access File - <date>.chaftaccess`
- `Chaft - <workspace> - Decryption Key Kit.chaftrecovery`

## Handle files safely

- Use a trusted private channel for invites, requests, and access responses.
- Never send a decryption key kit as an invitation.
- Store key kits privately and keep their passphrases somewhere else.
- Delete expired or revoked handoff files when they are no longer needed.
- Never attach credentials, keys, passphrases, or message content to public
  issues.

See [Create, join, and administer a workspace](../getting-started/workspace-lifecycle.md)
for the user flows that create and consume these files.
