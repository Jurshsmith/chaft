# Workspace Lifecycle Manual QA

Use this checklist before treating the workspace lifecycle as product-ready.
Automated smoke covers deterministic UI states; this checklist covers real
runtime behavior, files, clipboard, peer reachability, restart persistence, and
human handoff mistakes.

## Test Setup

- Use two physical devices when possible.
- If physical devices are not available, use two isolated runtime directories
  and two identity files.
- Keep logs, exported credential files, and exact runtime paths for each run.
- Start with fresh runtimes for baseline runs, then repeat without `--fresh` to
  verify persistence.

## Create and Recover

1. Launch device A with no workspace.
2. Create a workspace with `Invite only`.
3. Save the recovery kit with the suggested `.chaftrecovery` filename.
4. Copy the recovery kit text and verify it is not empty.
5. Quit and relaunch device A without `--fresh`; verify the workspace returns.
6. Launch a fresh runtime and restore from the recovery kit.
7. Verify wrong passphrase fails without creating a partial workspace.

Pass criteria: the recovery kit can restore access, wrong passphrases are clear,
and first-run state only appears when no workspace exists.

## Invite and Join

1. On device A, publish or enter a reachable peer endpoint.
2. Create an invite package for device B.
3. Save the invite package with the suggested `.chaftinvite` filename.
4. Import the invite on device B.
5. Send a message from device A and sync to device B.
6. Send a reply from device B and sync to device A.
7. Restart both devices and verify the timeline remains readable.

Pass criteria: both devices can join, sync, restart, and keep decrypted history.

## Request Access

1. On device A, set access policy to `People can request access`.
2. Export a workspace request card.
3. Open the request card on device B.
4. Submit directly while device A is reachable.
5. Repeat with device A unreachable and save/copy the request package; verify
   the saved fallback uses a `.chaftrequest` filename.
6. Import or receive the request on device A.
7. Approve it; if device B is reachable, verify the received approval opens the
   join dialog with the invite preloaded, otherwise save/copy the generated
   invite.
8. Repeat approval with device B open but without pressing `Check`; verify the
   pending request card receives the response from the saved admin endpoint.
9. Join from device B using the preloaded approval invite or the saved/copied
   invite.
10. Repeat with a second request and decline it while device B is reachable.
11. Repeat with a duplicate request and close it while device B is reachable.
12. Restart device B and verify approved/declined/closed request card state
    persists until the user hides it or completes the join.
13. After a successful join from a received approval, restart device B and verify
    the same received approval is not staged again.

Pass criteria: direct delivery works when reachable, fallback files are usable
when not reachable, saved admin endpoints are checked without requiring a manual
button press, and the UI shows clear waiting, approved, declined, closed, and
failed states without replaying already-imported approvals.

## Roles and Admins

1. Owner invites a normal member.
2. Owner promotes that member to admin.
3. Admin invites a normal member.
4. Admin changes a guest/member role.
5. Admin attempts to create, demote, or remove another admin.
6. Owner demotes or removes an admin.
7. Owner attempts to demote or remove the root owner.

Pass criteria: owner-only privileged role changes are enforced, while admins can
perform day-to-day member management.

## Removal and Rotation

1. Remove a normal member from the workspace.
2. Rotate workspace access.
3. Verify the removed device cannot send future workspace messages.
4. Create a private room, grant a member access, then remove them.
5. Rotate private-room access.
6. Verify the removed device cannot read or send future private-room messages.

Pass criteria: future access is revoked after removal/rotation, and the UI makes
the future-only nature of removal clear.

## Failure Cases

- Import malformed JSON.
- Import a credential for the wrong workspace.
- Import an expired or revoked invite.
- Try to join without a reachable peer when the credential has no key material.
- Save over an existing credential file.
- Copy and paste credential text through a plain text editor.

Pass criteria: failures are explicit, recoverable, and do not corrupt the local
runtime.
