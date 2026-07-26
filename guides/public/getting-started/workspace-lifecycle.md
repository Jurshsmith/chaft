---
title: Create, join, and administer a workspace
description: Use Chaft's current desktop flows to create, join, recover, and administer a workspace.
section: getting-started
order: 20
audience: users
status: preview
draft: false
---

# Create, join, and administer a workspace

This guide covers the current desktop workspace lifecycle. Chaft workspaces are
local-first and peer-to-peer; joining a workspace requires explicit material
from someone with access. Broad distributed discovery is intentionally not part
of this phase.

Chaft is still in preview. Use test or non-critical workspaces, retain separate
backups of important material, and expect credential formats and UI wording to
evolve. The flows below are implemented, but offline delivery still relies on
explicit file, link, or request handoff between people.

Return to the [public guide index](../index.md). If the desktop app is not
running yet, start with [Build Chaft Desktop](build-desktop.md).

## Before You Begin

- Give every person or test device its own runtime. Two peers that reuse the
  same device identity are not a valid multi-device test.
- Agree on a trusted channel for exchanging invites, requests, and access
  responses.
- Keep decryption key kits private and store their passphrases separately.
- Plan for at least one reachable peer or an explicit out-of-band handoff.

Chaft does not currently provide automatic global workspace discovery, hosted
account recovery, or root ownership transfer.

## First Run

A fresh Chaft runtime starts with no joined workspace. This is expected. The
first screen lets you:

- join a workspace from an access file, invite, request card, access request,
  or older JSON export,
- create a new workspace,
- import a decryption key kit to unlock matching history,
- or return later if you do not have credentials yet.

The seeded `Chaft Visual Smoke` workspace is only for deterministic visual
smoke testing. Launch it explicitly with `--smoke-workspace`; it should not be
the default user state.

## Create a Workspace

1. Launch the desktop app with a fresh runtime:

   ```sh
   tools/desktop/launch.sh debug --fresh
   ```

2. Choose `Create workspace` from the first-run screen.
3. Enter the workspace name, default channel name, and access policy.
4. Save a decryption key kit in durable private storage when prompted. Copying
   it to the clipboard alone does not complete the safety checkpoint.
5. Use a long, unique passphrase and keep it separate from the exported file.

The user-facing decryption key kit uses the existing `.chaftrecovery` file
format and recovery-bundle APIs. It contains the manual workspace key ring and
the private-room key rings available on the exporting device. It does not
contain the device signing identity, workspace membership authorization,
OpenMLS private group state, or root ownership.

Treat the kit as sensitive decryption material, not as a complete account or
workspace recovery mechanism. The kit supplies decryption keys, but a fresh
device must also be authorized before Chaft can show, send, or administer
workspace content. Once authorized, matching ciphertext can be decrypted when
history is available locally or from a reachable peer. Save a fresh kit after
key rotation or gaining access to another private room so it contains the latest
available key rings.

## Share Workspace Access

Do not share a decryption key kit with teammates. To authorize another person
or device:

1. Open the workspace, then open `Setup`.
2. In People & Access, choose a role and expiry, then select `Single-use` or
   `Group`. For a group invite, choose a preset or enter a custom
   `Maximum joins` value from 2 through 100. Single-use remains the default. An
   optional invite label is only for your invite list; each person chooses
   their own display name.
3. Save or copy the generated `.chaftinvite` file or invite link once.
4. Send that same artifact privately to each intended joiner. You do not need
   to generate a separate invite for every person.

The invite contains a bounded capability, workspace metadata, and the admin's
signed routing details. It does not contain the workspace key and does not need
recipient device IDs in advance. Each successful device consumes one join from
the shared limit. Treat the artifact as a bearer credential until it expires,
is revoked, or reaches its maximum: anyone who receives it can consume one of
the remaining joins.

Capacities above 20 require every workspace device to run a build that supports
100-join invites. Older builds enforce the previous 20-join protocol bound and
can reject higher-capacity invite events. Keep the maximum at 20 or fewer until
all participating devices have been updated.

The recipient opens the invite in `Join workspace`, confirms the display name
teammates will see, and chooses `Join workspace`. Their device signs the request
and supplies a response-encryption key. The admin's
runtime verifies the capability, expiry, revocation state, remaining capacity,
and device/request uniqueness before adding that device. The returned workspace
key is encrypted for the claiming device and signed by the expected admin.
Another device cannot import the response.

## Join a Workspace

1. Launch the desktop app.
2. Choose `Join workspace`.
3. Paste or open one of the supported credential types:
   - workspace access file,
   - signed invite or invite package,
   - workspace card or request handoff.
4. Confirm the display name teammates will see. Chaft asks once and preserves it
   through approval; existing profiles appear as `Joining as <name>`.
5. Provide a teammate address only when the credential does not include one.
6. Choose `Join workspace`. Chaft completes delivery automatically when the
   inviter is reachable and offers manual transfer only as a fallback.

If the credential is a workspace card, the app prepares an access request
instead of joining immediately. A secure invite prepares a cryptographic claim;
it never imports a key directly from the invite file.

## Import a Decryption Key Kit

1. Choose `Key kit` from the first-run or workspace-entry screen.
2. Open or paste the passphrase-protected `.chaftrecovery` file.
3. Enter the exact passphrase used when the kit was saved.
4. Provide a teammate address when matching encrypted history is not already
   available locally.
5. Choose `Import keys`.

The underlying credential remains a recovery bundle for schema/API
compatibility. Import installs only its contained key rings. Matching history
must already exist locally or come from a reachable peer, and a fresh device
must be authorized before Chaft can show it. Import does not replace the fresh
device identity, authorize that device, or transfer root ownership; use the
normal invite flow when authorization is needed.

## Credential Files

Chaft saves user-facing handoff material with explicit extensions:

- invites: `Chaft - <workspace> - Invite - <label> - <date>.chaftinvite`,
- access requests: `Chaft - <workspace> - Access Request - <person> - <date>.chaftrequest`,
- workspace request cards: `Chaft - <workspace> - Request Card - <date>.chaftworkspace`,
- workspace access files: `Chaft - <workspace> - Access File - <date>.chaftaccess`,
- decryption key kits: `Chaft - <workspace> - Decryption Key Kit.chaftrecovery`.

The `.chaftrecovery` extension and recovery-bundle schema/API names remain
unchanged for compatibility. The open/import flows also accept older JSON
exports. Treat decryption key kits as private key material, not invitations;
store the kit privately, keep its passphrase separate from the file, and never
send the kit as an invite.
Legacy workspace access files still grant workspace access and should only go
to the intended teammate or device. Current secure invite files do not contain
the workspace key, but must still be shared privately because their remaining
claims are bearer-held. Older invites and invites created with the default
limit allow one claim.

## Request Access

Use request access when a workspace allows requests but does not hand over join
credentials immediately.

1. Open the workspace card or approval-first invite.
2. Confirm the display name teammates will see and optionally add a note.
3. Submit the request if a peer is reachable.
4. If direct delivery fails, copy or save the request package and send it to an
   admin out of band.
5. Wait for an admin to approve or decline the request.

The current phase supports explicit request handoff and direct delivery
fallbacks. If your device is already hosting a direct peer endpoint, the request
also advertises that response route so a later approval invite can be queued
back to your device when both apps are reachable. Decline and close responses
can use the same route to update the pending request card. Pending request cards
with a saved admin endpoint can also check for approval responses in the
background while Chaft is open; the manual `Check` action remains available.
Received encrypted access opens the join dialog with the response already
loaded and the original display name preserved; the name is not requested
again. Fully asynchronous discovery,
multi-hop request propagation across offline peers, and automatic approval import
are later transport-hardening work.

## Invite a Member

Admins and owners can invite normal members. Owners control admin-level access.

1. Open the workspace.
2. Open `Setup`.
3. Go to the people/access area.
4. Choose the role and expiry. Select `Single-use`, or select `Group` and
   set `Maximum joins` from 2 through 100 using a preset or custom value.
   Optionally add an internal invite label; this does not name any recipient.
5. Copy the invite or save it as a file for offline transfer.
6. For a group invite, send the same invite artifact to every intended joiner
   through trusted channels. Each joining device consumes one join.

The invitee chooses their own display name in the `Join workspace` flow. Chaft
binds the resulting membership and encrypted access response to that device.

## Approve or Decline Requests

Owners and admins can process join requests.

1. Open `Setup`.
2. Review pending access requests.
3. Review the requester-provided name together with the device support code,
   then approve or decline access.
4. If the requester is not reachable, save or copy the generated invite package
   and send it out of band.

Approvals and declines are signed workspace events. Duplicate deliveries should
be safe because request and invite IDs are stable. When a request includes a
response route, Chaft queues approval invite delivery or decline/close response
delivery to that route while the app is open; otherwise use the same trusted
out-of-band channel the requester used. The requester confirms the received
access through the normal join flow. When delivery returns directly, the join
dialog opens with the response loaded and correlated by its request ID.

## Manage Roles

Current role policy:

- The root owner is immutable and cannot be removed or demoted.
- Owners can create or remove privileged owner/admin roles, while the original
  root owner remains immutable.
- Admins can invite members or guests, process join requests, manage normal
  member/guest roles, and remove normal members or guests.
- Admins cannot create, demote, or remove other admins.
- Root ownership transfer is not implemented in this phase.

Use owner-only changes for privileged role transitions. Use admin actions for
day-to-day member access. See the
[workspace administration policy](https://github.com/Jurshsmith/chaft/blob/main/guides/workspace-admin-policy.md)
for the full owner/admin/member policy.

## Remove Members and Rotate Access

When someone should no longer read future messages:

1. Remove the member from the workspace or private channel.
2. Rotate workspace or private-channel access when prompted.
3. Verify the removed device no longer appears as active in member/access views.
4. Keep old decryption key material private and save a fresh kit after rotation;
   issue new invite material only to current members.

Removal stops future authorization. Rotation protects future content keys.
Already-synced historical content remains a separate product/security concern.

## Troubleshooting

- Wrong kit passphrase: retry with the exact passphrase used when the
  decryption key kit was exported.
- Imported keys but cannot send: the current device is not authorized by the
  workspace history. Ask an owner or admin for an invite.
- Recent content remains locked: import a newer decryption key kit created
  after the relevant key or private-room access change.
- Unreachable peer: copy or save the join request or approval and send it out
  of band, or retry when the admin endpoint is reachable.
- Expired or revoked invite: ask an admin for a fresh invite.
- Unknown credential file: verify that the file is a Chaft access file, invite
  package, workspace card, join request, decryption key kit, or older JSON
  export.
- No workspaces on launch: this is the expected first-run state. Create or join
  a workspace to enter the app.

For build and launch problems, see [Build Chaft Desktop](build-desktop.md).
