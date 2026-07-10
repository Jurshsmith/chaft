# Request-Access Transport Hardening

Current request access is intentionally conservative. The app can prepare a
signed request handoff, attempt direct delivery to a reachable peer, and fall
back to copy/save when the peer is not reachable. The next transport milestone
is to make requests, approvals, and invites move naturally across reachable
peers without requiring both sides to be online at the same time.

## Current Boundary

This phase supports:

- explicit workspace cards and approval-first invites,
- direct join-request submission to a configured peer endpoint,
- optional requester response-route metadata on join requests, currently populated
  from the requester device's hosted direct endpoint when one is available,
- a durable runtime outbox for prepared join-request envelopes,
- local inbox import/acknowledgement for received requests,
- desktop queuing of copied, saved, and directly submitted request artifacts
  into the runtime outbox,
- background desktop draining of queued request artifacts to known direct peer
  endpoints while the app is open,
- bounded retry metadata for queued request artifacts, including attempt counts
  and next-attempt timestamps after failed direct delivery,
- runtime due-list APIs for queued request artifacts so desktop workers and
  future relay workers share the same retry eligibility rules,
- duplicate-safe direct request reception keyed by stable request ID,
- a distinct direct join-response transport for approval/invite packages and
  decline/close response envelopes,
- durable runtime inbox/outbox storage for join response envelopes,
- bounded, workspace-scoped direct pull of pending request and response
  envelopes from a known peer into the local inbox,
- desktop known-peer pulls after an explicit workspace pull/sync, plus manual
  and conservative background requester-side checks for pending requests with a
  saved admin endpoint,
- desktop queuing and background draining of approval invite packages back to a
  requester-advertised response endpoint,
- desktop polling that surfaces received approval packages in the normal
  workspace handoff buffer for user-confirmed import,
- desktop queuing and background draining of direct decline/close response
  envelopes back to a requester-advertised response endpoint,
- bounded retry metadata for queued approval, decline, and close responses,
  including attempt counts and next-attempt timestamps after failed direct
  delivery,
- runtime due-list APIs for queued approval, decline, and close responses,
  excluding delivered, acknowledged, and backoff-delayed entries,
- requester-side pending-request updates for received approval, decline, and
  close responses,
- desktop cleanup for received decline/close response envelopes after they
  update pending request state, and for received approval invite envelopes after
  the requester successfully imports the invite,
- copy/save fallback for requests and invites,
- admin approval/decline events once the request reaches the workspace.

This phase does not promise:

- global workspace discovery,
- public workspace search,
- DHT or name-service lookup,
- guaranteed offline delivery,
- automatic multi-hop propagation of request/invite envelopes,
- automatic requester-side import of approval packages,
- or polished QR/share flows for request and invite packages.

## Target Behavior

1. A requester prepares one access request.
2. The request is stored durably in the requester outbox. The first runtime
   primitive for this exists now; it queues, lists, marks, acknowledges, and
   directly submits prepared request envelopes.
3. Any reachable peer that accepts request relay can receive the envelope. A
   bounded direct pull primitive now exists for known peers. Desktop uses it
   after explicit known-peer pull/sync, from pending request cards, and from a
   quiet requester-side timer for saved admin endpoints; it does not yet
   automatically choose relay peers.
4. Admin devices eventually see the request in their inbox.
5. An approval creates a signed invite or membership handoff.
6. The response is stored durably in an admin outbox. Direct approval response
   queues exist now for generated approval invite packages.
7. If the original request advertised a response route, the admin device can use
   it as the first direct return path. The desktop now queues and drains that
   direct return path while the app is open.
8. The requester eventually receives the response through direct or relayed sync.
9. Duplicate, delayed, expired, or revoked envelopes remain idempotent.

## Protocol Work

- Define a bounded request/response envelope format separate from UI artifacts.
- Include stable IDs, workspace ID, target device IDs, creation timestamp,
  expiry, envelope kind, and signature metadata.
- Treat request, approval, decline, invite, revoke, and acknowledgement as
  idempotent envelope types.
- Preserve optional requester response routes separately from admin delivery
  routes; this lets approvals return to the requester without overloading the
  endpoint that received the original request.
- Keep envelopes small enough for direct peer exchange and local queue storage.
- Avoid leaking more workspace metadata than the user already chose to share in
  the workspace card or invite.

## Runtime Work

- Extend the durable local request outbox and inbox storage into a protocol
  envelope store shared by all request/response envelope types.
- Track delivery attempts, last error, last attempted peer, and acknowledgement.
  Durable request and response outboxes now track attempt counts, last error,
  last attempt time, delivery time, and the next direct retry time.
- Deduplicate by envelope ID and workspace ID.
- Garbage-collect expired or acknowledged envelopes.
- Expose paged FFI APIs for pending inbox/outbox state.
- Add retry APIs that can be called by desktop background workers. Direct
  request and response outboxes now expose due-list APIs that centralize
  backoff eligibility in the runtime boundary.
- Keep direct delivery retries idempotent at the receiver so repeated attempts
  do not create duplicate inbox work.

## Sync Work

- Let direct peers advertise whether they accept request-envelope exchange.
- Pull pending envelopes from reachable peers using bounded pages. Direct peers
  now support bounded, workspace-scoped request/response pulls from known peer
  endpoints, and FFI can import those pulls into the local inbox.
- Push local pending envelopes to reachable peers with duplicate-safe ack.
- Decide whether ordinary workspace peers can relay request envelopes or only
  admin/owner devices can.
- Keep relay policy explicit; do not turn request access into broad discovery.
- Wire the runtime outbox into background peer sync so queued requests are
  drained without a manual copy/save step when any permitted route is reachable.
  The desktop now drains requests, approval responses, and decline/close
  responses to known direct endpoints using the runtime due-list APIs. Known-peer
  request/response pulls are now wired into explicit desktop pull/sync, a manual
  requester Check action, and a conservative background check for saved admin
  endpoints. Multi-hop relay selection, general scheduled peer-selection policy,
  and requester-side auto-import are still pending.

## Desktop Work

- Show pending outbound requests on the empty-workspace screen.
- Show delivery state: saved locally, sending, delivered, waiting for admin,
  approved, declined, expired, revoked.
- Let users retry direct delivery or copy/save the same request package. The
  current desktop path records those requests in both the UI reminder store and
  the runtime outbox, and pending request cards can manually or quietly
  background-check a known admin endpoint for approval responses. Background
  relay selection is still pending.
- Notify admins when new inbox requests arrive.
- Notify requesters when an approval invite is received. The desktop now opens
  the join dialog with received approval invites preloaded, keeps the normal
  user-confirmed import step, and updates pending request cards for approval,
  decline, and close responses. It acknowledges received response envelopes
  after the state update or successful invite import.
- Surface whether a request has a known response route so admins understand when
  approval can be returned directly versus saved/copied manually.

## Tests

- Requester offline after creating request; admin later receives it.
- Admin offline when request is sent; request is delivered later.
- Duplicate request delivery is harmless.
- Decline followed by stale approval does not grant access.
- Approval followed by revoke updates state predictably.
- Expired request cannot be approved without creating a fresh invite.
- Relay peer cannot alter request or invite payloads.
- Envelope queues survive restart.
- Approval invite generated from a request with a response route is queued,
  delivered directly, received by the requester, and remains importable through
  the normal workspace join flow.
- Decline or close generated from a request with a response route is queued,
  delivered directly, received by the requester, and reflected in the pending
  request card.

## Definition of Done

A user can request access once, close the app, and later receive an approval or
decline when any authorized route becomes reachable, without manually copying
the same request again. Manual copy/save remains as the transparent fallback.
For this phase, requester-side background checks are limited to admin endpoints
saved on the pending request; automatic relay-peer selection remains out of
scope.
