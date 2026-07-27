---
title: Workspace lifecycle
description: Use Chaft's current desktop flows to create, join, recover, and administer a workspace.
section: getting-started
order: 20
audience: users
status: canary
draft: false
---

# Workspace lifecycle

Use this guide to create, join, and administer a non-sensitive test workspace.
Joining requires an invite or request handoff from someone who already has
access.

> Chaft is unaudited canary software. Use non-sensitive test workspaces, keep
> separate backups, and expect credential formats and interface wording to
> change. Offline delivery still relies on explicit file, link, or request
> handoff between people.

If the desktop app is not running, start with
[Build Chaft Desktop](build-desktop.md).

## Before you begin

- Give every person or test device its own runtime. Two peers that reuse the
  same device identity are not a valid multi-device test.
- Agree on a trusted channel for exchanging invites, requests, and access
  responses.
- Keep decryption key kits private and store their passphrases separately.
- Plan for at least one reachable peer or an explicit out-of-band handoff.

Chaft does not currently provide automatic global workspace discovery, hosted
account recovery, or root ownership transfer.

## First run

A fresh Chaft runtime starts with no joined workspace. This is expected. The
first screen lets you:

- join a workspace from an access file, invite, request card, access request,
  or older JSON export,
- create a new workspace,
- import a decryption key kit to unlock matching history,
- or return later if you do not have credentials yet.

## Create a workspace

1. Launch the desktop app with a fresh runtime:

   ```sh
   tools/desktop/launch.sh debug --fresh
   ```

2. Choose `Create workspace` from the first-run screen.
3. Enter the workspace name, default channel name, and access policy.
4. Save a decryption key kit in durable private storage when prompted. Copying
   it to the clipboard alone does not complete the safety checkpoint.
5. Use a long, unique passphrase and keep it separate from the exported file.

A decryption key kit can unlock matching history, but it does not authorize a
new device or transfer ownership. Store it separately from its passphrase. See
[Credential files and decryption key kits](../reference/credential-files.md)
for the complete boundary.

## Invite teammates

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

Treat an invite as a bearer credential until it expires, is revoked, or reaches
its maximum. Anyone who receives it may consume one of the remaining joins.

Capacities above 20 require every workspace device to run a build that supports
100-join invites. Older builds enforce the previous 20-join protocol bound and
can reject higher-capacity invite events. Keep the maximum at 20 or fewer until
all participating devices have been updated.

## Join a workspace

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

If you open a workspace request card, Chaft prepares an access request instead
of joining immediately.

A secure invite does not contain the workspace content key. The joining device
signs its claim and provides a response-encryption key. The inviter validates
the invite's expiry, revocation state, remaining capacity, and device/request
uniqueness before encrypting access for that device. Another device cannot
import the response.

## Import a decryption key kit

1. Choose `Key kit` from the first-run or workspace-entry screen.
2. Open or paste the passphrase-protected `.chaftrecovery` file.
3. Enter the exact passphrase used when the kit was saved.
4. Provide a teammate address when matching encrypted history is not already
   available locally.
5. Choose `Import keys`.

Import installs only the key rings contained in the kit. Matching encrypted
history must already exist locally or arrive from a reachable peer. A fresh
device must still be authorized through the normal invite flow.

## Request access

Use request access when a workspace allows requests but does not hand over join
credentials immediately.

1. Open the workspace card or approval-first invite.
2. Confirm the display name teammates will see and optionally add a note.
3. Submit the request if a peer is reachable.
4. If direct delivery fails, copy or save the request package and send it to an
   admin out of band.
5. Wait for an admin to approve or decline the request.

Chaft checks for a response while the app is open and the admin is reachable.
If direct delivery fails, exchange the request and response through the same
trusted channel. Chaft does not provide automatic global discovery.
Pending requests with a saved admin endpoint can check for responses in the
background while Chaft is open; the manual `Check` action remains available.

## Approve or decline requests

Owners and admins can process join requests.

1. Open `Setup`.
2. Review pending access requests.
3. Review the requester-provided name together with the device support code,
   then approve or decline access.
4. If the requester is not reachable, save or copy the generated invite package
   and send it out of band.

Chaft attempts direct delivery while both apps are reachable. Otherwise, send
the generated response through the same trusted channel used for the request.
Chaft records approvals and declines as signed workspace events. Direct decline
or closure notices are currently unsigned, so confirm them with a workspace
admin. Stable request and invite identifiers make repeated delivery safe, and
an approved response returns to the normal join flow with the original display
name preserved.

## Manage roles

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

## Remove members and rotate access

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
