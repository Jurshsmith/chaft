# Workspace Admin Policy

This phase uses a conservative owner/admin/member model. The goal is to make
day-to-day access management possible without making workspace ownership easy to
lose by accident.

## Roles

- Root owner: the device that created the workspace. This device is always an
  owner and cannot be removed or demoted in this phase.
- Owner: can manage privileged roles, admins, normal members, guests, invites,
  access requests, workspace access policy, and future access rotation.
- Admin: can invite normal members or guests, approve or decline access
  requests, update workspace access policy, remove normal members or guests, and
  manage normal member/guest roles.
- Member: can participate in workspace conversations and normal room workflows.
- Guest: can participate only where access has been granted.

## Privileged Role Rules

- Only owners can grant admin access.
- Only owners can grant owner access, and the desktop flow expects this to
  happen after the person has joined as a member/admin rather than through a
  first invite.
- Admins cannot promote someone to admin or owner.
- Admins cannot demote or remove admins or owners.
- Owners can demote or remove non-root admins and non-root owners.
- The root owner cannot be removed, demoted, or transferred in this phase.
- The desktop UI blocks changing or removing your own role from the member list;
  privileged handoff should be an explicit future flow.

## Ownership Transfer

Root ownership transfer is intentionally deferred. Supporting it safely requires
a product flow for confirming the receiving device, rotating recovery material,
and preventing accidental ownerless workspaces.

Until that exists, use this operational model:

- protect and back up the root owner's actual device identity/runtime,
- add another trusted owner when a second long-lived admin device is needed,
- demote or remove non-root privileged devices through People & Access,
- keep decryption key kits private, but do not treat one as root-owner recovery,
  an invite, or an ownership-transfer package. The current kit contains content
  keys, not the root signing identity or authorization.

## Backend Enforcement

The core authorization layer enforces the same policy:

- root owner removal returns `WorkspaceRootCannotBeRemoved`,
- root owner role changes return `WorkspaceRootRoleCannotBeChanged`,
- admin attempts to create, demote, or remove privileged roles return
  `InsufficientRole { action: "manage_privileged_roles" }`,
- member attempts to manage access or roles return `InsufficientRole`.

The desktop UI should hide or disable actions before they reach those backend
errors, but the backend remains the source of truth.
