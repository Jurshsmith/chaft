# Create, Join, and Administer a Workspace

This guide covers the current desktop workspace lifecycle. Chaft workspaces are
local-first and peer-to-peer; joining a workspace requires explicit material
from someone with access. Broad distributed discovery is intentionally not part
of this phase.

## Create a Workspace

1. Launch the desktop app with a fresh runtime:

   ```sh
   tools/desktop/launch.sh debug --fresh
   ```

2. Choose `Create workspace` from the first-run screen.
3. Enter the workspace name, default channel name, and access policy.
4. Save or copy the recovery export when prompted.
5. Keep the recovery passphrase separate from the exported file.

The recovery export is the owner device's safety path back into the workspace.
Treat it as sensitive access material.

## Join a Workspace

1. Launch the desktop app.
2. Choose `Join workspace`.
3. Paste or open one of the supported credential types:
   - workspace access file,
   - signed invite or invite package,
   - workspace card or request handoff,
   - passphrase-protected recovery kit.
4. Provide a peer endpoint when the credential does not already include one.
5. Confirm the import.

If the credential is a recovery bundle, the app asks for the recovery
passphrase. If it is an approval-first invite or workspace card, the app may
prepare an access request instead of joining immediately.

## Credential Files

Chaft saves user-facing handoff material with explicit extensions:

- invites: `Chaft - <workspace> - Invite - <person> - <date>.chaftinvite`,
- access requests: `Chaft - <workspace> - Access Request - <person> - <date>.chaftrequest`,
- workspace request cards: `Chaft - <workspace> - Request Card - <date>.chaftworkspace`,
- workspace access files: `Chaft - <workspace> - Access File - <date>.chaftaccess`,
- recovery kits: `Chaft - <workspace> - Recovery Kit.chaftrecovery`.

The open/import flows also accept older JSON exports. Treat recovery kits as
private restore material, not invitations; store the kit privately, keep its
passphrase separate from the file, and never send the kit as an invite.
Workspace access files are less sensitive than recovery kits, but still grant
workspace access and should only go to the intended teammate or device.

## Request Access

Use request access when a workspace allows requests but does not hand over join
credentials immediately.

1. Open the workspace card or approval-first invite.
2. Enter a display name and optional note.
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
Received approval invites open the join dialog with the invite already loaded;
you still confirm the import before joining. Fully asynchronous discovery,
multi-hop request propagation across offline peers, and automatic approval import
are later transport-hardening work.

## Invite a Member

Admins and owners can invite normal members. Owners control admin-level access.

1. Open the workspace.
2. Open `Setup`.
3. Go to the people/access area.
4. Enter the invitee device ID, display name, role, and peer endpoint.
5. Save or copy the generated invite package.
6. Send the invite package to the invitee through a trusted channel.

The invitee imports the package from the `Join workspace` flow.

## Approve or Decline Requests

Owners and admins can process join requests.

1. Open `Setup`.
2. Review pending access requests.
3. Approve the request to create an invite handoff for the requester, or decline
   it when access should not be granted.
4. If the requester is not reachable, save or copy the generated invite package
   and send it out of band.

Approvals and declines are signed workspace events. Duplicate deliveries should
be safe because request and invite IDs are stable. When a request includes a
response route, Chaft queues approval invite delivery or decline/close response
delivery to that route while the app is open; otherwise use the same trusted
out-of-band channel the requester used. The requester still confirms the
received invite through the normal join flow; when delivery returns directly,
the join dialog opens with the invite already loaded.

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
day-to-day member access. See `guides/workspace-admin-policy.md` for the full
owner/admin/member policy.

## Remove Members and Rotate Access

When someone should no longer read future messages:

1. Remove the member from the workspace or private channel.
2. Rotate workspace or private-channel access when prompted.
3. Verify the removed device no longer appears as active in member/access views.
4. Keep old recovery material private; issue new invite material only to current
   members.

Removal stops future authorization. Rotation protects future content keys.
Already-synced historical content remains a separate product/security concern.

## Troubleshooting

- Wrong recovery passphrase: retry with the original passphrase used when the
  recovery bundle was exported.
- Unreachable peer: copy or save the request/invite package and send it out of
  band.
- Expired or revoked invite: ask an admin for a fresh invite.
- Unknown credential file: verify that the file is a Chaft access file, invite
  package, workspace card, join request, recovery kit, or older JSON export.
- No workspaces on launch: this is the expected first-run state. Create or join
  a workspace to enter the app.
