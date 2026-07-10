# Workspace Lifecycle Manual QA Report

Copy this template for each real-device QA run. Keep the completed report with
the release evidence for the build under test.

## Run Metadata

- Date:
- Tester:
- Chaft commit:
- Build/profile:
- Device A OS/version:
- Device B OS/version:
- Network shape:
- Runtime A path:
- Runtime B path:
- Notes/log bundle path:

## Summary

- Overall result: `pass` / `fail` / `blocked`
- Blocking issue IDs:
- Follow-up issue IDs:
- Screenshots or recordings:

## Automated Baseline

| Gate | Commit | Result | Evidence / notes |
| --- | --- | --- | --- |
| `make workspace-qa-baseline ARGS=--offline PROFILE=debug` |  |  |  |
| `make smoke-lifecycle ARGS=--offline` |  |  |  |
| `make smoke-access ARGS=--offline` |  |  |  |
| `make desktop-empty-smoke` |  |  |  |
| `make screenshot-smoke` |  |  |  |

## Create and Recover

| Check | Result | Evidence / notes |
| --- | --- | --- |
| Fresh launch shows empty workspace state |  |  |
| Create workspace with invite-only access |  |  |
| Save `.chaftrecovery` recovery kit |  |  |
| Copy recovery kit text |  |  |
| Relaunch without `--fresh` restores workspace |  |  |
| Restore into fresh runtime from recovery kit |  |  |
| Wrong passphrase fails without partial workspace |  |  |

## Invite and Join

| Check | Result | Evidence / notes |
| --- | --- | --- |
| Device A exposes or enters reachable peer endpoint |  |  |
| Device A creates `.chaftinvite` for Device B |  |  |
| Device B imports invite |  |  |
| Device A message syncs to Device B |  |  |
| Device B reply syncs to Device A |  |  |
| Restart keeps decrypted history readable on both devices |  |  |

## Request Access

| Check | Result | Evidence / notes |
| --- | --- | --- |
| Device A sets access policy to request access |  |  |
| Device A exports workspace request card |  |  |
| Device B opens request card |  |  |
| Direct request delivery works while Device A is reachable |  |  |
| Fallback saved request uses `.chaftrequest` |  |  |
| Device A receives or imports request |  |  |
| Approval opens invite preloaded on Device B |  |  |
| Saved admin endpoint auto-check works without pressing `Check` |  |  |
| Device B joins from received or saved invite |  |  |
| Decline updates Device B pending request card |  |  |
| Close updates duplicate/stale pending request card |  |  |
| Request card state persists after restart |  |  |
| Imported approval is not restaged after successful join |  |  |

## Roles and Admins

| Check | Result | Evidence / notes |
| --- | --- | --- |
| Owner invites normal member |  |  |
| Owner promotes member to admin |  |  |
| Admin invites normal member |  |  |
| Admin changes guest/member role |  |  |
| Admin cannot create, demote, or remove another admin |  |  |
| Owner demotes or removes admin |  |  |
| Root owner cannot be demoted or removed |  |  |

## Removal and Rotation

| Check | Result | Evidence / notes |
| --- | --- | --- |
| Remove normal workspace member |  |  |
| Rotate workspace access |  |  |
| Removed device cannot send future workspace messages |  |  |
| Remove private-room member |  |  |
| Rotate private-room access |  |  |
| Removed device cannot read or send future private-room messages |  |  |

## Failure Cases

| Check | Result | Evidence / notes |
| --- | --- | --- |
| Malformed JSON import is explicit and recoverable |  |  |
| Wrong-workspace credential import is rejected |  |  |
| Expired or revoked invite is rejected |  |  |
| Join without reachable peer and without key material fails clearly |  |  |
| Save-over-existing credential behavior is understandable |  |  |
| Credential text survives copy/paste through a plain text editor |  |  |

## Final Decision

- Release decision:
- Required fixes before release:
- Optional follow-ups:
