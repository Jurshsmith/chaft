# Portable workspace export

This document defines the first shippable Chaft data-export slice: a portable,
plaintext ZIP for one workspace. It is a readable interoperability copy for a
person who wants to retain their data or prepare it for a later Slack,
Microsoft Teams, Discord, or similar migration adapter. It is not a Chaft
backup, restore point, or platform-specific import package.

The behavioral requirements in this guide are part of the format contract.
The JSON Schema embedded in each archive is the machine-readable structural
reference. In this document, **must**, **should**, and **may** are normative.

## User experience contract

The action belongs in **Settings → Data & portability** and is named
**Export workspace**. Its primary action is **Export ZIP…**.

Before opening the save dialog, the UI must explain that the copy:

- contains the selected workspace only;
- is a local-device snapshot whose remote sync completeness is not assessed;
- contains the current readable state, including private rooms and direct
  messages the current device is authorized to read;
- includes locally available decrypted attachments by default;
- is plaintext and should be stored and shared like the original conversations;
- is not a Chaft backup and cannot restore identity, authorization, or keys.

The suggested filename should combine a filesystem-safe workspace name and a
capture timestamp, with a `.zip` suffix, so repeated exports do not immediately
collide. The user chooses the destination. Export runs asynchronously so normal
workspace use can continue; the UI must not show a fabricated percentage. While
one export is running, a second export request must be rejected or disabled.

Version 2 does not overwrite an existing file. A destination collision must
leave the existing file untouched and ask the user to choose a new filename.

The terminal states are:

- **Success:** show the final path and a way to reveal it in the operating
  system's file manager.
- **Success with warnings:** create the archive, explain that some content was
  unavailable, and direct the user to `completeness.json`.
- **Failure:** show a useful error and do not leave a partial archive at the
  requested path.

Cancellation and background-job recovery after a forced process exit are
outside this first slice. Closing the settings panel must not itself cancel the
export. If the user requests a normal application close while export is active,
the UI must defer that close, keep the native runtime loaded, and automatically
close after the worker has finalized the archive or cleaned up its temporary
file.

## Archive identity and layout

Version 2 archives use this identity:

| Property | Value |
| --- | --- |
| `manifest.json.kind` | `chaft.portable-workspace.v2` |
| `manifest.json.schemaVersion` | `2` |
| Embedded schema | `schemas/chaft-portable-workspace-v2.schema.json` |
| Checksum algorithm | SHA-256 |

The ZIP has this logical layout:

```text
/
├── README.txt
├── index.html
├── manifest.json
├── completeness.json
├── SHA256SUMS
├── data/
│   ├── workspace.json
│   ├── channels.jsonl
│   ├── members.jsonl
│   ├── messages.jsonl
│   └── attachments.jsonl
├── html/channels/<safe-name>-<stable-suffix>.html
├── files/<channel>/<message>/<safe-filename>
└── schemas/chaft-portable-workspace-v2.schema.json
```

`index.html` and the channel pages are the human-readable view. They must work
offline, require no JavaScript, escape all user-controlled content, and apply a
restrictive Content Security Policy. Message markdown is displayed as inert
text in version 2; it is not interpreted as trusted HTML.

The files under `data/` are the adapter-facing representation. Each `.jsonl`
file contains one complete JSON object per line, encoded as UTF-8. Empty
collections are represented by empty JSONL files, not by omitted files. IDs are
opaque strings and consumers must not derive semantics from their spelling.
Consumers validate `manifest.json` against the schema root, `completeness.json`
against `$defs/completeness`, and each JSONL row against its correspondingly
named definition (`channel`, `member`, `message`, or `attachment`).

`SHA256SUMS` covers every archive entry except `SHA256SUMS` itself. Each line
uses the `sha256sum`-compatible grammar `<64 lowercase hex><two spaces><archive-relative path>\n`;
it contains no byte-count suffix or comment. A SHA-256 for the completed ZIP
may also be returned to the caller, but cannot be embedded without making the
archive self-referential. The `eventInventoryBlake3` value in the manifest
identifies the captured source event inventory; it is not a substitute for the
archive-entry checksums.

## Selection and authorization

The export source must be a freshly materialized, reader-authorized runtime
projection. It must not use the desktop's bounded `WorkspaceSnapshot`, cached
visible rows, search results, or UI filters.

The current device must still be a member of the selected workspace when the
capture is authorized. The archive contains:

- every currently readable public room;
- every currently readable private room;
- every currently readable direct-message conversation;
- an actor directory containing every current workspace member and every device
  referenced by an exported message author or reaction;
- current messages and reactions in those conversations;
- an attachment metadata row for each selected message attachment;
- attachment plaintext only when it is locally present and safely decryptable.

Channel IDs form one immutable namespace across standard rooms and direct
messages. A later event must not redefine an existing channel ID or change its
privacy classification. Exact replay of an identical signed event ID and
content is idempotent; a distinct event reusing that ID is rejected. When one
batch contains colliding channel definitions, every colliding definition and
its dependent descendants are rejected regardless of input order. Unrelated
workspace events and other channels remain eligible for authorization.

Unreadable channels and all of their messages, metadata, and attachments must
be absent. The exporter must not leak their names, IDs, counts, membership, or
failure details through `manifest.json`, `completeness.json`, HTML, paths, or
timing-oriented progress text.

An event envelope and the scope implied by its body must agree. A mismatch, an
unexpected channel envelope on a workspace event, or a missing envelope on a
message/reaction event is unresolved and excluded from authorized inventory,
gap, signature, and cutoff diagnostics. Scope ambiguity must fail closed rather
than trusting whichever representation would reveal more data.

## Cutoff and current-state semantics

Version 2 is a **current-state export**, not a raw event-history export:

- message edits are collapsed into the currently materialized body, while
  edit metadata identifies the latest applied edit when available;
- deleted messages remain as tombstones with `deleted: true`,
  `bodyState: "deleted"`, and an empty `markdown` value;
- attachments referenced by a deleted message retain metadata rows with
  `availability: "excluded_deleted"`, a null `archivePath`, a null
  `plaintextSha256`, and no bytes under `files/`;
- current reaction aggregates, channel metadata, profiles, and membership are
  exported rather than every transition that produced them; historical actor
  rows retain only the last known role needed to interpret exported activity;
- messages retain creation time, logical clock, and stable IDs so adapters can
  preserve deterministic order and replies where their destination permits it.

The manifest records `selection.coverageScope: "local_device_snapshot"`,
`selection.syncCompleteness: "not_assessed"`, `capturedAt`,
accepted/parseable/applied event counts, an event-inventory fingerprint, and
the applied causal frontier. Counts, the fingerprint, gap details, and frontier
IDs cover only workspace-level events and events mapped to currently readable
channels; they must not reveal the inventory of unreadable channels. The
fingerprint covers stable event identity and serialized content, not only row
counts. These fields make the authorized local source boundary auditable
without claiming that every remote replica has synchronized.

If local history changes during capture, the exporter should retry a stable
read. If it still cannot obtain one, it may finish the exact inventory it did
capture, but must set `sourceChangedDuringCapture: true` and emit the matching
completeness warning. It must never merge two silently inconsistent source
inventories.

## Structured record semantics

`data/workspace.json` contains one workspace record. The JSONL files contain
channel, member, message, and attachment records respectively. Every record
has `schemaVersion: 2`.

Member rows and message authors expose a neutral `actorId` and `actorKind`.
When a device is linked to a person, the person ID is the actor ID; otherwise
the device ID is the fallback actor. Reaction aggregates include sorted
`actorDeviceIds`, which adapters can join through member rows before mapping to
destination accounts. `members.jsonl` is therefore an actor directory, not
only a current membership list. Each row has `active: true` for a current
member and `active: false` for a former actor retained because exported
activity references that device. `role` is the current or last known role and
may be `null` when no trustworthy role is available. Every exported author and
reaction device ID must resolve to a member row.

Important message states are:

| `bodyState` | `markdown` | Meaning |
| --- | --- | --- |
| `available` | Current plaintext | Body was readable at the cutoff. |
| `deleted` | Empty string | The message is a retained tombstone. |
| `unavailable_encrypted` | Empty string | Metadata was authorized, but local key material was unavailable or authenticated decryption failed. See the item-level completeness reason. |

Important final attachment states are:

| `availability` | Bytes included | Meaning |
| --- | --- | --- |
| `included` | Yes | Plaintext was read, authenticated, and written to `archivePath`. |
| `excluded_deleted` | No | Its message is deleted; metadata is retained intentionally. |
| `missing_local_blob` | No | The referenced ciphertext is not present locally. |
| `invalid_local_blob` | No | Local blob validation failed. |
| `decryption_key_unavailable` | No | The device lacks usable local key material. |
| `decryption_failed` | No | Authenticated decryption failed. |
| `unsupported_encryption_metadata` | No | The reference cannot be exported by this safe plaintext path. |

`pending` is an internal construction state and must never appear in a
completed archive. `archivePath` and `plaintextSha256` are non-null only for an
`included` attachment. `sourceBlobHash` identifies Chaft's encrypted source
blob and must not be presented as the plaintext checksum.

Every attachment has a globally unique, deterministic export-local
`attachmentId` derived from its channel, message, and attachment position.
Adapters must use that value for archive relationships. The optional
`sourceAttachmentId` preserves the original Chaft/source value for reference;
it is not required to be unique.

Timestamps intended for interchange are RFC 3339 strings. Message records also
carry the source physical Unix-millisecond timestamp and logical counter for
deterministic ordering. Consumers should order by physical time, logical time,
then stable event/message identity instead of relying on ZIP entry order.

## Completeness and warnings

An export is not allowed to imply silent completeness. `completeness.json`
must have one of these statuses:

- `complete`: no known omission or capture warning;
- `complete_with_warnings`: the readable projection was exported, but at least
  one known item or source condition needs attention.

Known warning codes are:

| Code | What it accounts for |
| --- | --- |
| `missing_attachments` | Files unavailable, invalid, unsupported, or not decryptable. Intentional `excluded_deleted` files are not missing. |
| `unavailable_message_bodies` | Authorized message metadata whose body could not be decrypted. |
| `history_gaps` | Events not materialized because causal or authorization context was missing. |
| `invalid_signatures` | Parseable events excluded after self-contained signature validation. |
| `corrupt_events` | Parse failures only when they can be attributed safely to the readable export scope. |
| `source_changed_during_capture` | The source inventory changed through the bounded stable-read attempts. |

The report contains item-level details where doing so is safe, plus counts for
conditions that cannot safely expose content. `manifest.json.completeness`
summarizes the same result for fast inspection. Warning counts are counts of
affected items or conditions, not merely the number of warning categories.

Because an unparseable row cannot always be assigned safely to a readable or
unreadable channel, `corruptEventCount` may be `null` and
`storageIntegrityAssessment` will state `not_assessed_for_privacy`. A
workspace-global corrupt-row count must not be substituted.

Missing or corrupt **message/event structure** must never be guessed. A corrupt
attachment may be omitted with a warning because its independently verifiable
metadata remains useful; decryption/authentication failures must never result
in unauthenticated bytes being exported.

## Security and privacy requirements

The resulting archive contains plaintext and may expose workspace names,
messages, identity metadata, device/person IDs, and attachments. It must be
treated as sensitive user data.

The archive must exclude:

- device private keys and credentials;
- workspace/private-channel content keys and key rings;
- OpenMLS private group state, key packages, welcomes, or commits;
- invite/access secrets and recovery material;
- peer addresses, discovery information, and replica configuration;
- raw signed events, public-key envelopes, and signatures;
- local database files, search indexes, logs, and runtime configuration.

Generated archive paths must be relative, traversal-safe, bounded in length,
and independent of user-supplied directory separators or absolute paths. The
runtime must reject unsafe destinations such as symlinks, directories, or its
own data/identity paths. After creating the destination directory, construction
must use its resolved canonical parent rather than returning to an unchecked
alias. On Unix, construction opens that parent directory without following a
symlink, creates the unique sibling temporary file relative to the held
directory handle with exclusive/no-follow flags, and hashes the completed ZIP
through the same open temporary-file handle. Before publication it verifies
both the path-to-parent identity and the temporary name-to-file identity, then
uses a handle-relative no-replace publish operation. Parent replacement,
temporary-file substitution, and a destination created during export must all
fail closed without clobbering the destination. Platforms without those Unix
primitives must close the completed handle before a safe no-clobber fallback.
A failure must clean up only the exporter-owned temporary file and preserve any
previously completed destination.

Attachment bytes should be processed one at a time so export memory is bounded
by the existing attachment size limit instead of total workspace size. Large
archive bytes must not cross the desktop FFI boundary; only a compact result or
error object should.

## Compatibility and adapter rules

Consumers must select behavior from `kind` and `schemaVersion`, ignore unknown
object properties, and accept additive fields within version 2. Fields marked
required by the embedded schema may not disappear or change type in version 2.
A semantic or structural breaking change requires a new kind, schema version,
schema filename, and compatibility fixture.

Version 1 is frozen as a legacy dispatch fixture at
`crates/chaft-runtime/tests/fixtures/portable-workspace-v1.json`. It preserves
the original numeric `corruptEventCount`, source attachment IDs used directly
as `attachmentId`, and the original member shape. Version 2 intentionally
changed those semantics: corrupt counts may be null for authorization privacy,
attachment relationships use deterministic export-local IDs with optional
`sourceAttachmentId`, and actor rows add `active` with a nullable role. The
representative current fixture is
`crates/chaft-runtime/tests/fixtures/portable-workspace-v2.json`; tests verify
version dispatch and validate v2 records against its embedded Draft 2020-12
schema without rewriting the immutable v1 fixture.

Platform adapters should consume `data/*.jsonl` plus `manifest.json`, not
scrape the HTML pages and not depend on Chaft's raw event model. Destination
identifiers created by an adapter should live in an adapter-owned mapping file;
they do not belong in this neutral archive contract.

The following are deliberately outside version 2:

- direct Slack, Microsoft Teams, Discord, or other API delivery;
- vendor-specific ZIP/CSV layouts, mentions, emoji, or thread transforms;
- date-range, channel, member, or attachment selection controls;
- incremental exports and resume/cancel checkpoints;
- importing this archive back into Chaft;
- key/identity recovery, legal-hold attestations, or administrator-wide export;
- server-side export jobs, cloud upload, scheduling, or automatic sharing.

## Release acceptance checklist

Before shipping version 2, automated or manual verification must establish:

- a complete archive opens offline and every required entry exists;
- JSON and every JSONL line parse as UTF-8 JSON and satisfy the embedded schema;
- every `SHA256SUMS` value matches its entry and no archive path is unsafe;
- edits, replies, reactions, tombstones, and deterministic ordering are correct;
- private/direct content appears only when the exporting device can read it;
- duplicate or cross-kind channel creation cannot redefine channel privacy;
- exact event replay is idempotent, distinct same-ID content is rejected, and
  colliding channel definitions plus descendants fail order-independently;
- malformed envelope/body scope and required-envelope omissions fail closed;
- unreadable channel metadata cannot be inferred from any archive entry;
- HTML/script/attribute/path-injection payloads remain inert;
- missing blobs, missing keys, gaps, bad signatures, and a changing source
  produce the documented warning without silent data loss;
- corrupt-row diagnostics are either safely scoped or explicitly unassessed,
  never substituted with a workspace-global private-channel side channel;
- included attachments authenticate and their `plaintextSha256` values match;
- duplicate source attachment IDs produce distinct export IDs and archive paths;
- corrupt encrypted bodies become item-level warnings instead of aborting the
  export;
- deleted attachments emit `excluded_deleted` metadata and no plaintext bytes;
- every exported author/reaction device resolves through `members.jsonl`, and a
  removed actor retains its last known role with `active: false`;
- destination, runtime, identity, key, invite, recovery, endpoint, and raw-event
  secrets are absent from the ZIP;
- concurrent chat use remains responsive and a duplicate export cannot start;
- a normal close is deferred during export and completes automatically after
  the worker finishes;
- success publishes one complete ZIP, while output races, parent replacement,
  temporary substitution, and failure leave no clobbered or partial output.
