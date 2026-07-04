# Chaft

Chaft is a native, local-first, peer-to-peer desktop chat workspace. The source
repo intentionally lives in this nested `chaft/` folder so the parent directory
can hold private context, local notes, and secrets without becoming part of Git.

## Architecture

- Native desktop shell: Qt 6/QML.
- Core runtime: Rust workspace.
- Data model: signed append-only events, materialized locally for fast UI.
- Causal/auth safety: materialization refuses events with missing parents or
  missing authorization context and reports gaps instead of silently rendering
  incomplete or unauthorized history.
- Channel privacy: private channel writes and read markers are blocked unless
  signed history authorizes the device through channel creation or a
  `ChannelMemberAdded` grant. Reader-aware runtime snapshots and search hide
  private-channel content from devices without that signed grant.
- App view model: `chaft-app` converts signed event history into channel,
  member, profile, peer-endpoint, timeline, encrypted-message, history-gap, and
  failed-signature rows for the desktop shell.
- Local runtime: `chaft-runtime` owns app data directories, stable device
  identity, optional passphrase-encrypted identity-file unlock,
  workspace/private-channel content keys, OpenMLS device key-package bootstrap
  material, local private OpenMLS workspace group state, encrypted event
  creation, and local snapshots.
- Storage: SQLite WAL for local cache and FTS5 for local search.
- Wire format: protobuf-compatible envelopes via `prost`, with sync payload
  decode capped at 16 MiB before protobuf parsing.
- Sync loop: reusable pull sync compares peer inventories, fetches missing
  events, verifies signatures, stores locally, and returns materialization gaps.
- Network target: no central server. Optional replica nodes store encrypted,
  partial event/blob data and never become authority nodes. Signed peer endpoint
  announcements let members advertise direct TCP, native Iroh, or backup-peer
  hints through normal workspace replication without granting trust.
- Message privacy: encrypted message event variants carry sealed markdown
  payloads. Public channels use the workspace content key, private channels use
  per-channel content keys. When local OpenMLS workspace group state exists,
  public-channel messages and attachments use an AES-256-GCM-SIV key derived
  from the current OpenMLS workspace epoch instead of the manual workspace key.
  Private channels also use channel-scoped OpenMLS exporter-derived keys when
  local channel group state exists. Generated OpenMLS device key packages,
  workspace/channel group state, member-add welcome commits, and MLS-derived
  payload keys are now in place as the production-key bootstrap path. OpenMLS
  parser inputs, generated artifacts, and peer-supplied event bodies are capped
  before decode, event publication, authorization, or materialization: 64 KiB
  public key packages, 512 KiB private key-package bundles, 1 MiB
  welcomes/commits, and 4 MiB private group state/ratchet trees.
  Prior local OpenMLS exporter-derived content keys are retained in private
  group state so older ciphertext remains readable after add/update/remove
  epochs. Workspace/channel self-update commits, member-removal commits, and
  local commit catch-up are wired. Local invites, private-channel grants, and
  pull/sync can now provision OpenMLS member-add welcomes when an unused
  OpenMLS key package is available. Explicit removal-triggered and
  suspected-compromise key rotation exist for the manual fallback bridge,
  local OpenMLS self-update groups, and a combined operator-triggered policy
  that rotates both when mixed key state exists.
  Workspace recovery bundles can wrap manual workspace/private-channel key
  rings with a passphrase for explicit device transfer. A conservative
  compromise-signal report can now flag invalid self-contained signatures and
  tell operators when local secret rotation is the recommended response. The
  explicit response policy rotates local secret state once for unhandled
  local-device signals while leaving remote-only signals as review-only.
- Blob storage: BLAKE3 content-addressed files with whole-blob files capped at
  128 MiB + 1 KiB, chunk manifests capped at 1 MiB before parse, chunk files
  capped at 16 MiB, chunk descriptors capped at 16,384 chunks, and chunk
  availability reporting under each node's data directory. Whole blobs, chunk
  files, and manifests are persisted through unique synced temp files before
  replacement so concurrent replica writes do not share staging paths.
- Attachment privacy: attachment refs can carry encrypted blob metadata while
  replicas store only ciphertext-addressed blob bytes; plaintext attachment
  exports land through unique synced temp files before replacement.
- Attachment availability: runtime snapshots mark when a signed attachment's
  local ciphertext is missing, treating whole blobs and complete chunked blobs
  as available, and the desktop UI disables Save until peer sync or retry
  restores the blob.
- Bootstrap transport: direct TCP peer sync for executable local/LAN tests plus
  native Iroh QUIC streams for explicit `iroh://<endpoint-id>?addr=<host:port>`
  peers. Runtime peer entrypoints reject central-server, public-relay,
  public-discovery, unknown-scheme, malformed, or zero-port dial targets before
  network work while carrying scoped event/blob sync over the same protobuf
  protocol with typed event/proof batches.
- Replica publish policy: direct replica sync rejects plaintext message bodies,
  development-plaintext sealed payload markers, and attachment refs without
  AES-256-GCM-SIV encryption metadata in both proof slices and stored events.

## Current Bootstrap

The initial Rust crates establish the contracts for event identity, device
signing, storage, search, wire framing, sync inventory, and future P2P adapters.
The Qt app is scaffolded but requires Qt 6 and CMake to build. Its first paint
uses a built-in `WorkspaceSnapshot` with the production JSON shape, then the
controller replaces it with runtime or raw-store data on background workers.

Recommended public checks:

```sh
tools/ci/rust-gates.sh --offline
tools/smoke/local-p2p.sh --offline
tools/smoke/visual-workspace.sh --offline
tools/desktop/preflight.sh
tools/desktop/build.sh debug
tools/desktop/smoke.sh debug
tools/desktop/screenshot-smoke.sh debug
tools/desktop/package.sh release
tools/desktop/package-smoke.sh release
python3 tools/desktop/release-metadata.py release
python3 tools/desktop/verify-release-metadata.py release
cargo bench -p chaft-benchmarks --bench hot_paths --no-run
```

Desktop build prerequisites by OS:

- Linux: Rust 1.92, CMake 3.28+, Ninja, Qt 6.7+ desktop libraries, and the
  usual C/C++ build toolchain. CI installs `cmake` and `ninja-build`, then
  uses Qt `linux_gcc_64`.
- macOS: Rust 1.92, CMake 3.28+, Ninja, Xcode command line tools, and Qt 6.7+
  desktop libraries. CI installs CMake/Ninja through Homebrew and uses Qt
  `clang_64`.
- Windows: Rust 1.92, CMake 3.28+, Ninja, MSVC x64 build tools, Python 3, and
  Qt 6.7+ desktop libraries. CI runs inside the MSVC developer environment and
  uses Qt `win64_msvc2019_64`.

CI builds release desktop packages on Linux, macOS, and Windows after the debug
desktop smoke passes. The uploaded artifact bundle for each OS includes the
native package (`.tgz`, `.dmg`, or `.zip`), `SHA256SUMS`,
`chaft-desktop-sbom.cdx.json`, and `chaft-desktop-provenance.json`. Generate the
same metadata locally after packaging with:

```sh
python3 tools/desktop/release-metadata.py release
python3 tools/desktop/verify-release-metadata.py release
```

Linux CI also runs `tools/desktop/screenshot-smoke.sh debug`, verifies the PNG is
non-blank, and compares broad image metrics against
`tools/desktop/screenshot-baseline.json` before uploading the smoke screenshot.

Phase 2 performance work starts with `chaft-benchmarks`, a public Criterion
benchmark crate for append, decrypted snapshot hydration, local search, direct
sync pull, direct blob transfer, and FFI JSON payload generation. Compile the
benchmark target without running samples with:

```sh
cargo bench -p chaft-benchmarks --bench hot_paths --no-run
```

Run samples locally with Criterion options when investigating regressions:

```sh
cargo bench -p chaft-benchmarks --bench hot_paths -- --sample-size 10
```

```sh
cargo test --workspace
cargo build -p chaft-ffi
cargo run -p chaft-cli -- --data-dir ./scratch/app paths
cargo run -p chaft-cli -- --data-dir ./scratch/app list-workspaces
cargo run -p chaft-cli -- --data-dir ./scratch/app init-workspace --name "Chaft Local" --channel general
cargo run -p chaft-cli -- --data-dir ./scratch/app update-device-profile --workspace-id <workspace-id> --display-name "Mira"
cargo run -p chaft-cli -- --data-dir ./scratch/app publish-device-key-package --workspace-id <workspace-id> --key-package-file ./scratch/openmls-key-package.bin
cargo run -p chaft-cli -- --data-dir ./scratch/app publish-peer-endpoint --workspace-id <workspace-id> --endpoint-id desktop --endpoint direct+tcp://127.0.0.1:7777 --backup-peer
cargo run -p chaft-cli -- --data-dir ./scratch/app publish-open-mls-device-key-package --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app create-open-mls-workspace-group --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app add-open-mls-workspace-group-member --workspace-id <workspace-id> --key-package-id <device-key-package-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-open-mls-workspace-group-member --workspace-id <workspace-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/second-app join-open-mls-workspace-group --workspace-id <workspace-id> --source-event-id <member-add-event-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app update-open-mls-workspace-group --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app update-workspace-open-mls-groups --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app detect-compromise --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app respond-compromise --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app rotate-workspace-for-suspected-compromise --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/second-app apply-open-mls-workspace-group-commits --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app create-open-mls-channel-group --workspace-id <workspace-id> --channel-id <private-channel-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app add-open-mls-channel-group-member --workspace-id <workspace-id> --channel-id <private-channel-id> --key-package-id <device-key-package-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-open-mls-channel-group-member --workspace-id <workspace-id> --channel-id <private-channel-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/second-app join-open-mls-channel-group --workspace-id <workspace-id> --channel-id <private-channel-id> --source-event-id <channel-member-add-event-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app update-open-mls-channel-group --workspace-id <workspace-id> --channel-id <private-channel-id>
cargo run -p chaft-cli -- --data-dir ./scratch/second-app apply-open-mls-channel-group-commits --workspace-id <workspace-id> --channel-id <private-channel-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app send-message --workspace-id <workspace-id> --channel-id <channel-id> --text "encrypted hello"
cargo run -p chaft-cli -- --data-dir ./scratch/app send-message --workspace-id <workspace-id> --channel-id <channel-id> --reply-to <message-id> --text "reply with context"
cargo run -p chaft-cli -- --data-dir ./scratch/app send-attachment --workspace-id <workspace-id> --channel-id <channel-id> --text "encrypted file" --file ./README.md
cargo run -p chaft-cli -- --data-dir ./scratch/app send-attachment --workspace-id <workspace-id> --channel-id <channel-id> --reply-to <message-id> --text "file reply" --file ./README.md
cargo run -p chaft-cli -- --data-dir ./scratch/app save-attachment --workspace-id <workspace-id> --message-id <message-id> --attachment-id <attachment-id> --output ./scratch/downloaded-file
cargo run -p chaft-cli -- --data-dir ./scratch/app prune-blobs
cargo run -p chaft-cli -- --data-dir ./scratch/app edit-message --workspace-id <workspace-id> --message-id <message-id> --text "encrypted hello, edited"
cargo run -p chaft-cli -- --data-dir ./scratch/app add-reaction --workspace-id <workspace-id> --message-id <message-id> --reaction "+1"
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-reaction --workspace-id <workspace-id> --message-id <message-id> --reaction "+1"
cargo run -p chaft-cli -- --data-dir ./scratch/app mark-channel-read --workspace-id <workspace-id> --channel-id <channel-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app snapshot --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app snapshot --workspace-id <workspace-id> --decrypt
cargo run -p chaft-cli -- --data-dir ./scratch/app search-workspace --workspace-id <workspace-id> --query "encrypted"
cargo run -p chaft-cli -- --data-dir ./scratch/app reindex-workspace-search --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app delete-message --workspace-id <workspace-id> --message-id <message-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app publish-queue --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app export-workspace-key --workspace-id <workspace-id> > ./scratch/workspace-key.json
cargo run -p chaft-cli -- --data-dir ./scratch/app export-recovery-bundle --workspace-id <workspace-id> --passphrase "<private passphrase>" > ./scratch/recovery-bundle.json
cargo run -p chaft-cli -- --data-dir ./scratch/second-app device-id
cargo run -p chaft-cli -- --data-dir ./scratch/app invite-member --workspace-id <workspace-id> --device-id <second-device-id> --role member
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-member-with-open-mls --workspace-id <workspace-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-member-with-key-rotation --workspace-id <workspace-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app rotate-workspace-manual-keys --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-member --workspace-id <workspace-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app create-channel --workspace-id <workspace-id> --name strategy --private
cargo run -p chaft-cli -- --data-dir ./scratch/app add-channel-member --workspace-id <workspace-id> --channel-id <private-channel-id> --device-id <second-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-channel-member-with-open-mls --workspace-id <workspace-id> --channel-id <private-channel-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-channel-member-with-key-rotation --workspace-id <workspace-id> --channel-id <private-channel-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app remove-channel-member --workspace-id <workspace-id> --channel-id <private-channel-id> --device-id <removed-device-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app export-channel-key --workspace-id <workspace-id> --channel-id <private-channel-id> > ./scratch/private-channel-key.json
cargo run -p chaft-cli -- --identity-file ../secrets/dev-device.json device-id
cargo run -p chaft-cli -- --identity-file ../secrets/dev-device.json --identity-passphrase "<private passphrase>" device-id
cargo run -p chaft-cli -- --identity-file ../secrets/dev-device.json sample-event
cargo run -p chaft-node -- --data-dir ./scratch/node
cargo run -p chaft-node -- --data-dir ./scratch/node serve --listen 127.0.0.1:7777
cargo run -p chaft-node -- --data-dir ./scratch/node serve-iroh
cargo run -p chaft-cli -- --data-dir ./scratch/app export-trust-snapshot --workspace-id <workspace-id> > ./scratch/trust-snapshot.json
cargo run -p chaft-cli -- --data-dir ./scratch/app publish-workspace --workspace-id <workspace-id> --peer 127.0.0.1:7777
cargo run -p chaft-cli -- --data-dir ./scratch/app backup-workspace --workspace-id <workspace-id> --peer 127.0.0.1:7777
cargo run -p chaft-cli -- --data-dir ./scratch/app retry-blob-transfers --workspace-id <workspace-id> --peer 127.0.0.1:7777 --peer 127.0.0.1:7778
cargo run -p chaft-cli -- --data-dir ./scratch/app publish-event-with-trust-snapshot --workspace-id <workspace-id> --event-id <event-id> --peer 127.0.0.1:7777
cargo run -p chaft-cli -- --data-dir ./scratch/app storage-health --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/app repair-storage-metadata --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/second-app pull-workspace --workspace-id <workspace-id> --peer 127.0.0.1:7777
cargo run -p chaft-node -- --data-dir ./scratch/backup mirror-workspace --workspace-id <workspace-id> --peer 127.0.0.1:7777 --listen 127.0.0.1:7778 --once
cargo run -p chaft-node -- --data-dir ./scratch/backup mirror-workspace --workspace-id <workspace-id> --peer 127.0.0.1:7777 --listen-iroh --once
cargo run -p chaft-node -- --data-dir ./scratch/backup status
cargo run -p chaft-node -- --data-dir ./scratch/backup status --json --require-healthy --max-age-seconds 120
cargo run -p chaft-node -- --data-dir ./scratch/backup repair-storage-metadata --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/second-app import-workspace-key --key-file ./scratch/workspace-key.json
cargo run -p chaft-cli -- --data-dir ./scratch/second-app import-channel-key --key-file ./scratch/private-channel-key.json
cargo run -p chaft-cli -- --data-dir ./scratch/second-app import-recovery-bundle --bundle-file ./scratch/recovery-bundle.json --passphrase "<private passphrase>"
cargo run -p chaft-cli -- --data-dir ./scratch/second-app snapshot --workspace-id <workspace-id>
cargo run -p chaft-cli -- --data-dir ./scratch/second-app snapshot --workspace-id <workspace-id> --decrypt
cargo run -p chaft-cli -- --identity-file ../secrets/dev-device.json publish-sample --peer 127.0.0.1:7777
cargo run -p chaft-cli -- inventory --peer 127.0.0.1:7777
```

For desktop runs, either set `CHAFT_FFI_LIBRARY` to the built dynamic library or
launch the app from a location where `target/debug/libchaft_ffi.*` is nearby.
`tools/smoke/visual-workspace.sh` creates a deterministic public UI smoke
runtime through normal CLI commands. Set `CHAFT_KEEP_SMOKE=1` to keep its
temporary runtime and use the printed `runtimeDir`, `workspaceId`, and
`desktopExpectedText` values for manual desktop hydration or screenshot work.
`tools/desktop/screenshot-smoke.sh debug` runs the same desktop smoke and writes
a verified PNG under `build/desktop-debug/smoke/` for local visual regression
review.
To hydrate the shell from a real local runtime with decrypted local message
bodies, set `CHAFT_RUNTIME_DIR` and `CHAFT_WORKSPACE_ID`; optionally set
`CHAFT_IDENTITY_FILE` when using a non-default identity path, and set
`CHAFT_IDENTITY_PASSPHRASE` to pre-unlock passphrase-encrypted identity or
runtime secret files. Whitespace-only or over-16 KiB passphrase values are
ignored. If a runtime open needs a passphrase and no usable passphrase is
present, the desktop shows an unlock prompt and retries the runtime load in the
current process through the FFI library's
runtime-directory-scoped unlock cache; typed prompt text is cleared when the
prompt is dismissed or closed. To hydrate from a raw event store without local
keys, set `CHAFT_EVENT_STORE` to the SQLite `events.db` path shown by
`chaft-cli paths` and set `CHAFT_WORKSPACE_ID` without setting
`CHAFT_RUNTIME_DIR`.
When the runtime is unlocked from the desktop prompt, the Setup panel can lock
the runtime again by clearing that process-local cache and immediately blanking
decrypted timeline/search/key-transfer state, unsent drafts, and pending
attachment text from the visible shell, and clearing app-owned clipboard buffers
when Qt can prove Chaft still owns the copied data. Locking also advances the
runtime worker generation barrier so in-flight runtime refresh/write results
cannot repaint decrypted state after the shell has been locked. While manually
locked, automatic search, read-marker, sync, backup, metadata-repair, endpoint
announcement, and timeline/page refresh paths are paused until the user presses
`Unlock runtime` and supplies the passphrase again; sessions launched with
usable `CHAFT_IDENTITY_PASSPHRASE` keep the lock control disabled because the
process can still read the environment fallback.
Raw event-store mode is view-only: runtime write, search, sync, and hosting
controls remain disabled because there is no local runtime identity/key context.
The shell first paints the built-in bootstrap snapshot, then runtime and
raw-store hydration prefer the latest-window FFI snapshot exports when
available, loading the latest 500 timeline rows plus `timelineWindow` metadata
by default; `CHAFT_TIMELINE_LIMIT` can lower that for local stress tests and is
ignored if its raw text is over 16 bytes before being clamped to the 500-row
app-view-model window budget. Runtime startup discovery,
raw-store loading,
default-workspace selection, and initial latest-window snapshot hydration run on
a background Qt worker after QML is loaded, so cold launch can show the shell
without blocking on large encrypted local histories. When `timelineWindow`
reports older rows, the desktop can page backward with the runtime window export
or raw event-store window export instead of reloading full history; those
older-page loads run on a background Qt worker so large local histories do not
block the UI thread. Desktop attachment
sends and saves also run on background Qt workers so file encryption,
decryption, blob storage, post-send snapshot refresh, and large file writes do
not freeze composer input. Local blob-cache pruning also runs off the UI thread.
Runtime-backed desktop read models skip corrupt local event JSON while still
rendering parseable invalid self-contained signatures as security rows, so a
poisoned cache row does not blank snapshots, channel pages, member pages, or
local search. Raw event-store inspection remains strict and can still report
storage corruption. `chaft-cli storage-health` and the FFI
`chaft_runtime_workspace_storage_health_result_json` export provide compact
workspace cache counters for total, parseable, corrupt, signature-valid
metadata, verified servable, poisoned-metadata, promotable-metadata, and
parseable non-servable rows without contacting peers or requiring content keys.
The health scan verifies parseable self-contained signatures instead of only
trusting the fast metadata bit. The Qt shell binds the same report into compact
cache and row counters near the sync/backup controls and exposes a Repair action
when metadata drift is likely repairable.
`chaft-cli repair-storage-metadata` and
`chaft_runtime_repair_workspace_storage_metadata_result_json` recompute the
signature-valid metadata bit for one workspace without deleting event rows, so
poisoned inventory metadata can be corrected and valid demoted rows can be
promoted while corrupt bytes remain available for strict inspection or later
peer repair. The desktop repair action uses the same worker-backed path,
refreshes cache health immediately, then rehydrates the current runtime snapshot
so promoted legacy rows and cleared poisoned rows are reflected in the UI.
Runtime write contexts use that same parseable-row policy for local sends,
edits/deletes, reactions, read markers, setup/admin actions, key rotation, and
search reindexing: malformed cached event bytes are skipped, invalid
self-contained signatures are filtered, and authorization still comes from the
remaining signed materialized history. A poisoned cache row therefore does not
leave the app readable but unable to send.
Read-marker writes and their snapshot refreshes run on background workers so
channel navigation remains responsive. Plain message sends, message edits,
message deletes, reaction adds/removes, channel creation, profile updates, workspace
creation, invites/removals, private-channel grants/revocations, device
key-package publishing, and workspace/private-channel OpenMLS group create,
join, catch-up, self-update, all-local self-update rotation, and member-add
actions also append and refresh snapshots on background workers with stale
snapshot guards, so composer input, quick reactions, message actions, first-run
setup, and common setup actions are not blocked by local encryption, indexing,
authorization checks, OpenMLS fallback work, MLS state updates, or snapshot
hydration.
Manual workspace/private-channel key import/export, recovery bundle
import/export, trust-snapshot export, and suspected-compromise manual key
rotation also run on background workers, so large key-ring serialization,
passphrase wrapping, search reindexing, rotation, and post-import snapshot
hydration do not stall the shell. Manual key import JSON is capped at 256 KiB,
and recovery-bundle import JSON is capped at 4 MiB before desktop dispatch,
FFI parse, or CLI file reads. Device identity, local secret, OpenMLS
private-state, blob-transfer ledger, and compromise-response ledger writes use
unique synced temp files before replacement so concurrent writes do not share
one staging path.
When an older successful worker result is skipped after a newer snapshot has
already applied, the desktop queues a fresh background snapshot refresh so the UI
still converges to the full local event log. Workspace rail switching applies a
lightweight selected-workspace placeholder immediately, then hydrates the real
latest-window snapshot on a background worker; workspace summary refreshes also
run off the UI thread after snapshot applies, so large local histories do not
stall workspace navigation or message action completion. The desktop reads
workspace summaries through the paged FFI result when available, falling back to
the legacy symbol for older local libraries with a desktop-side 128-row cap, and
hydrates only the first 128-row rail window on each refresh. Desktop summary
reads scan at most 512 KiB of returned FFI JSON before parsing, so older
libraries cannot force a huge rail parse before the row cap is applied. If the
selected workspace is outside that window, the QML rail keeps it visible from
the active snapshot instead of forcing a full summary scan. Runtime pages
preserve total-count metadata while asking SQLite for only the requested
first-seen workspace-ID window. Other desktop FFI result reads scan at most
16 MiB before parsing so a large or incompatible local library response cannot
force an unbounded JSON copy. The runtime clamps oversized page limits to 128
summary rows, counts workspace rows from metadata, and materializes only servable
local rows for those workspace summaries so corrupt cached rows do not break the
rail.
The desktop stores its
selected workspace ID, default direct peer endpoint, saved
backup peer endpoints, backup peer status metadata, and the Auto backup setting
in `desktop.json` under the runtime directory. The desktop ignores that config
file if it grows beyond 64 KiB and writes it with an atomic bounded save so a
failed persistence attempt keeps the previous routing metadata intact; set
`CHAFT_PEER_ENDPOINT` to override the saved peer endpoint for a run,
`CHAFT_BACKUP_PEERS` to add comma- or semicolon-separated backup endpoints, and
`CHAFT_AUTO_BACKUP=1` to enable Auto backup for a run. `CHAFT_BACKUP_PEERS` is
ignored if its raw list text exceeds the 32-peer saved-backup budget before
splitting. Selected workspace IDs
loaded from `desktop.json` or `CHAFT_WORKSPACE_ID` are trimmed, and blank or
oversized selected IDs are rejected before runtime hydration or worker dispatch.
Desktop startup also ignores empty or over-64 KiB runtime, raw-store, config,
or library paths before probing `desktop.json`, saving config, choosing
raw-store mode, or loading an FFI library. Whitespace-only path environment
values are treated as absent for startup mode selection.
Direct peer/listen endpoints are capped at 2 KiB before they are saved, passed
to FFI, used for network I/O, or written into blob-transfer retry metadata;
desktop peer
endpoints must also be direct TCP or native Iroh direct routes before they enter
the saved default, backup-peer list, or retry set. Blob retry accepts at most 33
endpoints per call, matching one explicit endpoint plus the 32 saved backup-peer
slots. Workspace, channel, message, and key-package IDs accepted at
local boundaries are capped at 128 UTF-8 bytes; event IDs are capped at their
canonical 68-byte `evt_` plus hash form, and device ID references remain capped
at 512 bytes. CLI and FFI workspace, channel, message, device, key-package, and
source-event ID arguments are trimmed; blank or oversized values are rejected
before local runtime action dispatch, and source-event IDs must be canonical.
Direct CLI/FFI publish, backup, pull, sync, proof-publish, and blob-retry calls
reject blank workspace IDs before opening the runtime, and proof-publish
rejects non-canonical event IDs before local store access.
The inspector and sidebar snapshot caps channel metadata to an exact 128-row
bootstrap page with full `channelCount` preserved, and the Qt shell can load
additional channel pages while preserving the selected channel during refresh.
Search hits include materialized channel labels/privacy and can resolve the
sorted channel page containing an unloaded hit channel without walking every
earlier sidebar page; runtime channel pages and containing-channel lookups clamp
oversized limits to 128 rows. Message search hits seed resolved channel metadata
immediately, and exact channel pages hydrate asynchronously without changing the
loaded sidebar prefix. The `Search or jump` field also runs bounded runtime
channel search capped to 128 result rows and merges those results with loaded
local channel matches, so quick-jump is not limited to the bootstrapped sidebar
page. Empty or
punctuation-only search states clear desktop search without dispatching runtime
message or channel workers, and the QML shell treats them as non-search mode so
loaded timeline/sidebar rows stay visible.
It caps replicated profile metadata to 256 sorted rows while preserving the
current device's profile in reader-aware snapshots; snapshot `profileCount`
still preserves the full materialized total. It caps workspace members to an
exact 128-row bootstrap page with full `memberCount` preserved, and runtime
member pages clamp oversized limits to 128 rows while the Qt shell can load
additional member pages for management controls. It also caps
replicated key-package metadata to the newest 4 rows per device/protocol so
repeated key-package publication does not make desktop JSON or the key-package
list unbounded; snapshot `keyPackageCount` still preserves the full materialized
total. Discovered peer endpoint hints are also displayed from capped snapshot
rows while `peerEndpointCount` keeps the full total.
Top-level snapshot diagnostics also keep full `gapCount` and
`invalidSignatureCount` totals while capping serialized `gaps` and
`invalidSignatures` arrays to the newest 64 rows each.
Direct publish, backup, pull, sync, and blob-retry worker results also preserve
full count fields while capping returned ID/hash/gap/transfer/detail arrays as
samples, so desktop status and backup health remain accurate without letting one
large sync produce unbounded FFI JSON. Chunked blob-transfer attempts preserve
full chunk counts while returning bounded chunk-hash samples.
Publish queue, direct push, proof-backed backup, and single-event proof publish
materialize only parseable and self-contained-signature-valid local rows, while
still reporting causally incomplete parseable rows as skipped gaps, so one
corrupt cached event cannot disable P2P controls or leak unverifiable data to a
peer.
Shared pull sync and headless mirror-node materialization use the same
parseable-row policy for local cache reads: malformed stored bytes are skipped,
parseable invalid self-contained signatures are filtered, and only
causally/authorization-complete rows drive mirror blob hydration or signed peer
discovery. This keeps partial backup nodes repairable without making a poisoned
cache row stop mirror refreshes.
Recovery-bundle import, explicit OpenMLS catch-up/self-update, manual key
rotation, member-removal rekey, compromise detect/respond, and
suspected-compromise rotation FFI results use the same count-first shape,
preserving full totals while returning bounded status samples instead of
unbounded event/channel lists.
Blob-cache prune results follow the same count-first pattern, returning bounded
workspace/hash/temp-path samples while preserving full reference/removal totals;
prune also clears stale blob-store temp files from older process IDs without
touching current-process staging files.
The desktop Host control starts a local direct TCP peer over the same runtime
`events.db` and encrypted `blobs/` cache; the adjacent Iroh host action serves
the same local store as a native Iroh QUIC peer. Other profiles or replica nodes
can use the displayed endpoint as their peer endpoint for pull, push, or sync.
After a host starts, the desktop publishes that endpoint as a signed workspace
hint with a short expiry, refreshes it while hosting, and publishes an immediate
expiry update on toolbar Stop or best-effort normal desktop shutdown. Saving a
backup peer in the desktop also publishes an `isBackupPeer` hint. The runtime
and CLI expose the same signed peer-endpoint path, so newly synced profiles can
discover operator-approved endpoints from the replicated log instead of relying
only on out-of-band notes.
Direct TCP and Iroh host start/stop bind and close peer threads on background Qt
workers, so local store opens, port binding, QUIC endpoint setup, and shutdown
joins do not freeze the shell.
Peer fields accept bare `host:port`, `direct+tcp://host:port`, and native
`iroh://<endpoint-id>?addr=<host:port>` forms through the policy-aware Iroh
adapter. CLI, desktop FFI, and headless node transport paths keep public
relay/discovery off by default; set `CHAFT_IROH_ALLOW_PUBLIC_RELAYS=1` or
`CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY=1` only for an explicitly approved relay or
discovery deployment. `CHAFT_IROH_DISABLE_DIRECT_TCP_BRIDGE=1` disables the
direct TCP bridge for policy tests. Those Iroh policy flags ignore raw values
above 16 bytes before matching `1`, `true`, `yes`, or `on`. Signed peer endpoint
hints use the same 2 KiB endpoint cap, a 2304-byte endpoint-ID cap, and a
64-byte transport-label cap before append, and the CLI/FFI publish endpoints
reject unsupported routes or endpoint/transport mismatches before opening the
runtime, so replicated discovery metadata stays bounded.
The `Live` toggle periodically syncs the selected peer endpoint and suppresses
overlapping sync workers so the desktop can keep a workspace fresh without
manual button presses. If the endpoint field is empty, Live sync falls back to
the newest non-expired signed peer endpoint hint in the workspace snapshot,
preferring member-hosted peers before backup peers. Snapshot JSON caps replicated
endpoint hints to the newest 32 member-hosted rows plus newest 32 backup rows so
large logs do not make QML peer selection or the P2P panel unbounded. QML
rechecks those hints before automatic selection and only accepts direct TCP hints
labelled `direct-tcp` or explicit-address native Iroh hints labelled
`iroh-direct`, so older fallback JSON cannot silently route autosync or
auto-backup through relay, discovery, central-server, custom, or mislabelled
endpoints, and malformed direct TCP or native Iroh direct hints are ignored
before automatic selection. The Backup
peer list can also run periodic proof-backed backup slices while sharing the
same worker guard. Auto backup uses saved backup peers first, then falls back to
non-expired signed backup-peer hints when no saved peer is available. Auto
backup also debounces workspace snapshot changes, so local writes, pulls, and
manual rekeys are offered to backup targets shortly after the UI applies them
instead of waiting only for the periodic timer. Saved backup peers keep persisted
last-attempt/success/failure/partial metadata in `desktop.json`; the desktop
caps that saved list and its status map at 32 peers so persisted backup state,
auto-backup rotation, retry ranking, and QML lists stay bounded. Automatic backup
skips peers inside their retry cooldown, while manual Backup remains immediate.
Protocol-classified backup and retry failures mark a saved peer as Suspect with
a bounded `suspectScore`; later successful backups decrement that score and
clear the marker when it reaches zero. Saved retry ranking places clean peers
before suspect peers and lower suspect scores before higher scores.
A successful backup with missing
encrypted blobs is shown as Partial instead of Backed up; so is a backup that
skipped stored local events because their causal parents are still missing. The
Retry control uses the current peer endpoint plus saved backup peers to resume
pending or failed encrypted attachment blob uploads from the local transfer
ledger; it ranks saved backup peers with the same status metadata, preferring
peers outside cooldown, then clean peers before suspect peers, lower suspect
scores before higher scores, then Partial peers with more missing blobs, then
peers with fewer failures or more recent success. Successful retry attempts
decrement the saved partial missing-blob
count and clear the blob part of Partial when the count reaches zero, while
unresolved skipped gaps keep the peer Partial until history is repaired and
backed up again.
Direct push/sync materializes local history first after dropping failed
self-contained signature rows, uses workspace-scoped peer inventory to publish
only missing authorized events, pulls workspace inventory through the core
transport contract, requests new-peer full and workspace inventory in 1024-ID
pages with total-count metadata while keeping legacy one-shot fallback, rejects
advertised paged-inventory totals above 1,048,576 IDs per pull, rejects blank,
padded, or over-128-byte local workspace scopes before network streams and
peer-supplied workspace scopes before hosted store/decode work, caps
peer-supplied sync error strings at 2 KiB before turning them into protocol
errors, avoids echoing unvalidated duplicate request values from hosted
diagnostics, and sends that event delta as bounded protobuf `PublishEvents`
frames before checking peer blob availability and uploading only missing or
incomplete encrypted attachment blobs from materialized events.
Publish and backup results also report `skippedGaps` when stored local events
are held back because their causal parents are still missing. `publish-queue`
and the matching FFI export report full local queue counts/completeness plus
bounded samples of publishable event IDs, backup-slice event IDs, local blob
availability, and skipped gaps without contacting a peer, which gives the
desktop an explicit offline/local-only queued state in its P2P toolbar and
backup inspector before the next network sync without unbounded queue JSON.
Pull/sync also joins newly pulled OpenMLS workspace/private-channel welcomes
when this device has the matching local private key-package bundle, then
applies later OpenMLS commits, and owner/admin devices can provision
member-add welcomes for invited/granted peers that have published unused
OpenMLS key packages. Any locally provisioned welcomes are pushed in a
follow-up publish during combined sync before snapshots and search refresh; the
desktop status includes the number of MLS events handled by that sync, reports
pull/sync history gap counts, and reports missing encrypted attachment blob
counts separately from sent/fetched blob counts for publish, proof-publish,
backup, pull, and sync operations.
Desktop pull/sync performs the post-network latest snapshot hydration in the
same worker before applying the parsed view model on the UI thread, so large
remote updates do not freeze the shell after network I/O completes.
Larger attachment blobs use chunk manifests and chunk frames instead of one
whole-blob frame. Direct pull/sync fetches missing events in bounded batches
with split retries for likely oversized responses, then fetches whole attachment
blobs and availability in bounded batches only after their referencing events
materialize locally, with chunked fallback when a peer stores only a manifest
and chunks, so stored gaps remain repairable without pulling untrusted blob
data. Shared wire sync decoders reject payloads above 16 MiB before protobuf
parsing, while direct TCP and native Iroh reject over-limit length prefixes
before allocating the frame body, and clients reject returned event/blob vectors
above the requested batch size before deeper decode, hashing, or descriptor
conversion. Direct publish/backup rejects malformed remote inventory before
using it to skip local event uploads. Direct TCP and native Iroh publish also
resume partial chunked uploads by checking peer chunk availability after
publishing the manifest and sending only missing chunks. The runtime persists
per-blob transfer
attempts in `blob-transfer-ledger.json`, including in-progress chunk plans, so
interrupted or failed uploads remain visible after restart and can be reconciled
when the original peer or another supplied retry peer later advertises the blob
as complete. The ledger file is capped at 16 MiB before parse.
`retry-blob-transfers` consumes pending or failed ledger entries and retries
them for the supplied peer set.
Attachment sends infer common media types from the file extension when
`--media-type` is omitted; callers can still pass an explicit value when needed.
Local runtime send/edit APIs reject message markdown above 64 KiB before sealing
or appending, and attachment sends reject selected plaintext files above 128 MiB
before local reads, keeping event-log, snapshot, search, and sync payloads
bounded. Core authorization/materialization also rejects peer-supplied plaintext
message markdown above 64 KiB, oversized sealed-message ciphertext, nonce, or
associated-data buffers, message events with more than 16 attachments, and
oversized attachment reference metadata before those events enter read models.
The local event store rejects blank or over-64 KiB event-store paths before
SQLite open, rejects serialized event JSON above 16 MiB, and treats preexisting
oversized rows as corrupt for repair. Peer publish decode enforces the same 16
MiB serialized event JSON ceiling before authorization or store work. Device
key-package publishing rejects opaque package bytes above 64 KiB;
path-based CLI/FFI/desktop callers preflight the selected file before reading
it. Signed metadata writes also cap workspace/channel/display names, member
device ID references, key-package protocol labels, peer endpoint hints, reaction
text, and caller-supplied workspace/channel/message/event/key-package IDs before
append, store reads, FFI action handoff, or network work so local UI and sync
payloads do not inherit unbounded strings. FFI JSON imports for legacy snapshots,
manual key transfer, and recovery bundles are also capped at the C-string reader
before parse, while desktop FFI result strings are scanned only to the
workspace-summary 512 KiB cap or the general 16 MiB result cap. `LocalRuntime`
normalizes blank identity passphrases to absent, rejects passphrases above 16
KiB, and rejects blank data directories plus over-64 KiB data, identity, and
derived runtime paths before directory creation, identity open, KDF work, or
SQLite open. Direct runtime attachment sends and saves also reject blank or
over-64 KiB source/output paths before file stat/read work, workspace scans, or
attachment export. CLI attachment, key-transfer, recovery-bundle, and output
paths are also capped at 64 KiB before file stat/read work or attachment
export. FFI filesystem paths,
passphrases, role strings, and bounded metadata fields scan only their limit plus
one byte before rejecting oversized input; unknown future FFI string fields fall
back to a 16 MiB cap rather than an unbounded C-string scan. Core
authorization/materialization applies the same peer metadata budget to signed
event bodies before accepting remote workspace
names, channel names, device profiles, key-package protocol labels, endpoint
hints, reactions, content-key epoch labels, OpenMLS metadata labels, or sealed
message key IDs into read models. Core also validates signed event envelope
IDs, parent IDs, body reference IDs, and trust-snapshot proof IDs before those
values enter authorization indexes or materialized state, and rejects event
author public keys above 32 bytes or signatures above 64 bytes before
materialization or self-contained signature verification.

`search-workspace` reads the private local FTS index in `search.db` with
512-byte-capped, sanitized prefix terms, so message search remains responsive
while a user is typing partial words. `SearchIndex::open` rejects blank or
over-64 KiB index paths before SQLite open. Valid searches query the local FTS
index before materializing workspace history, so no-hit searches and searches
with only stale cached rows stay cheap even in large local workspaces by checking
only the bounded raw candidate event IDs against local servable-event metadata.
The low-level FTS reader also clamps direct result requests to 512 rows before
SQLite execution, and the event-store candidate filter chunks direct oversized
candidate sets into 1024-ID SQLite statements. Search
JSON reports capped-row counts, bounded hit/candidate counts, and raw-window
overflow separately so shells can show that a result list is a preview without
loading more history. Search hit bodies are bounded FTS snippets with original
length/truncation metadata rather than unbounded message payloads.
`send-message`, workspace-key import, and peer pull rebuild enough local search
state for normal use when the required workspace and private-channel keys are
available.
`reindex-workspace-search` forces a rebuild from encrypted local events plus the
local workspace key and any imported private-channel keys. The index is
plaintext local cache and is never published to peers or replica nodes. Search
schema upgrades preserve legacy cache rows with a neutral timestamp, while
reindex restores exact event-time ordering. Search rebuilds and query-time
filtering ignore events with embedded author public keys
that fail self-contained signature verification, matching the snapshot
failed-signature quarantine.

In the desktop shell, the search field still filters channels locally. When a
runtime workspace is open, message matches come from the same FFI-backed FTS
index on a background Qt worker after the controller's 512-byte query preflight
and render as workspace-wide timeline rows with channel labels, including author
identity and signed event time for matches outside the loaded timeline window,
ordered newest-first after authorization filtering; stale cache rows are dropped
before the visible hit cap, and stale worker results are ignored after newer
queries or workspace switches. Selecting a workspace-wide message result switches
the active channel context to that row while keeping the query active, so the
inspector, composer, and sidebar stay anchored to the result's channel.
Demo/keyless startup paths fall back to filtering the
currently loaded snapshot.
Keyboard control is first-class: `Ctrl/Cmd+K` focuses search/jump,
`Ctrl/Cmd+M` focuses the composer, `Alt+Up/Down` steps channels,
`Alt+Left/Right` steps workspaces, `Ctrl/Cmd+O` opens file attachment,
`Alt+Home/End` jumps the timeline, `Ctrl/Cmd+Shift+C` copies selected message
text, and `Esc` clears search or exits message editing; when the setup panel is
open, `Esc` closes it and returns focus to the composer. The desktop composer is a
bounded multi-line input where Enter submits and Shift+Enter inserts a newline.
It keeps unsent drafts scoped by workspace and channel, restores them when the
user returns, shows compact draft previews in the channel list without writing
draft text to the runtime event log, and keeps an active Reply target as local
view state until the next message or attachment send writes a signed same-channel
reply event. Runtime lock clears those in-memory drafts and reply targets.
The left sidebar keeps channels directly under search and channel creation.
Channel rows are sorted by latest materialized message activity before name and
show compact previews from the snapshot, so navigation does not scan timeline
rows or call FFI.
Timeline rows also render compact local event-time labels from the snapshot's
`physicalMs` field, quoted reply previews from `replyPreview`, parent-row thread
chips from `threadReplyCount`/`threadLatestReply`, and bounded multi-line message
bodies, so scrolling does not fetch or decode raw events. Selecting a thread chip
opens the row in the right inspector, which renders bounded `threadReplyPreviews`
from the loaded snapshot.
Normal channel timelines open at the first visible unread message when the
selected channel has unread incoming rows;
otherwise they open at the newest message and keep following new rows while the
user is already near the bottom. Runtime channel snapshots scope
`timelineWindow` to the selected channel and set `timelineChannelId`, so opening
or paging an old channel does not require the desktop to load workspace-wide
history until that channel appears. When the user scrolls away from the live
edge, the QML timeline shows a viewport-pinned jump control that returns to the
newest messages without touching runtime state. The unread divider is derived
from snapshot `unreadCount` plus local device identity in QML, without extra
runtime calls. `Load older` preserves the current viewport when older history is
prepended, history-gap rows expose a Repair action that pulls from the current
peer endpoint, and search mode starts at the top of result sets instead of
auto-following chronological chat. Timeline rows expose a Reply action, passive
thread-count chips, capped attachment preview chips with overflow counts,
bounded reaction chips with overflow counts, and QML-only quick reaction choices;
replies write signed encrypted reply-message events while reactions use signed
add/remove runtime reaction event paths with 64-byte reaction text caps. Reaction
counts are derived per author device, so duplicate local add/remove events are
idempotent under replay; the UI gets a `myReactions` row list so only reactions
owned by the local device are styled and removed as local reactions.
Device/profile, invite, backup, key-package, OpenMLS, key-transfer, and manual
rekey controls live behind the scrollable Setup panel so daily channel
navigation stays visible on normal desktop window heights.
On wide desktop windows, the right inspector derives selected-message details,
thread reply context, selected attachment save/copy controls with overflow
counts, channel counts, recent media, backup peer health, members, and
key-package actions from the already-loaded snapshot. Selecting a timeline row
only updates local QML state; it does not dispatch FFI or network work.
The main header keeps channel identity and sync status fixed while direct/Iroh
host, sync, push, backup, retry, pull, and prune controls live in a horizontal
action strip, so the inspector can be open without squeezing or overlapping P2P
controls. A compact route chip in that strip distinguishes local-only/offline
queue, direct TCP, explicit-address Iroh, relay-style Iroh, discovery-style
Iroh, and replica-backed peer choices; relay/discovery labels describe
operator-approved endpoint strings and do not make a relay an authority node.

The desktop shell also exposes local workspace switching in the rail, the local
device ID, signed device display-name updates, workspace member roster,
workspace member invite/removal, private-channel creation, private-channel
member grants/revocation, local device key-package publishing, generated
OpenMLS key packages, workspace/private-channel OpenMLS group create, join,
catch-up, self-update, all-local self-update rotation, and member-add actions,
automatic member-add welcome provisioning when key packages are already
available, manual workspace/private-channel key import/export controls,
private-channel key rotation backed by local key rings that retain prior
content-key epochs, and combined
suspected-compromise review and rotation for local OpenMLS and manual key state.
It can also rebuild the local message search index, export a root-signed trust
snapshot, and publish a selected message event plus that proof to a partial
backup peer after rejecting non-canonical selected event IDs before starting a
worker. Its Backup control publishes materialized
profile/key-package/OpenMLS add-remove-update/content/key-epoch/activity slices
with trust-snapshot proof, while Push publishes the full materialized workspace
history; saved backup peers can run this backup periodically and after coalesced
snapshot changes with the Auto toggle. For a second profile/device, start Host on
the first profile, use its Copy control to place the endpoint in the second
profile's peer field, copy the second device ID into the owner profile,
invite it, grant private-channel access when needed, export the workspace key
and any granted private-channel key bundles over a private channel, import those
keys on the second profile, then pull workspace history from the hosted profile
or a replica.

`chaft-node mirror-workspace` lets a headless replica periodically pull one
workspace from one or more peers into its own encrypted event/blob store. With
`--listen`, the same process also serves that mirrored store as a direct peer for
other profiles or backup nodes; with `--listen-iroh`, it serves the mirrored
store as a native Iroh QUIC peer and prints the `iroh://...` endpoint. A node can
also serve its current encrypted store directly with `serve-iroh`. Its outbound
peers accept bare `host:port`, `tcp://host:port`, `direct+tcp://host:port`, and
native `iroh://<endpoint-id>?addr=<host:port>` through the policy-aware bridge.
The `--workspace-id` argument is trimmed, rejected when blank, and capped at
128 bytes before the node opens its event/blob stores or starts a hosted mirror.
The node also rejects blank or over-64 KiB `--data-dir` paths and rejects derived
`events.db`, `blobs`, default mirror-status, or explicit `--status-file` paths
above the same 64 KiB path budget before store or status-file work begins. The
same checks are enforced inside the node store, hosted-mirror, and mirror-status
helpers so direct internal callers cannot bypass the CLI path budget.
The same `CHAFT_IROH_ALLOW_PUBLIC_RELAYS`,
`CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY`, and
`CHAFT_IROH_DISABLE_DIRECT_TCP_BRIDGE` environment flags control node outbound
mirroring and native Iroh hosting policy.
Repeat `--peer` to configure multiple upstreams; each mirror cycle tries every
reachable peer in order and merges what they can serve, so one peer can provide
events while another fills missing blobs or parents. After each successful pull,
the mirror also learns non-expired signed `PeerEndpointPublished` hints from the
materialized workspace log and can use those discovered peers in the same
one-shot run or later periodic cycles. Discovered peers are priority-sorted,
deduplicated, blank/oversized/unsupported/malformed/mislabelled endpoint hints
are prefiltered before discovery materialization, and the discovered set is
capped at 32 after sorting; configured `--peer` endpoints stay
operator-controlled, are tried first, and are rejected if blank, over 2 KiB,
unsupported, malformed, or more than 33 are supplied. Pass
`--no-discover-peers` to disable that behavior.
Periodic mirroring logs transient upstream failures and retries
on the next interval; `--once` returns an error only if all active peers fail.
Ctrl+C stops periodic mirroring and closes any hosted mirror peer.
It writes machine-readable replica health to `<data-dir>/mirror-status.json` by
default, or to `--status-file <path>` when set. The status file contains the
workspace ID, configured peers, discovered peers, the active peer set, any
hosted direct/Iroh endpoint, last result, last successful peer, per-peer failure
messages, successful peer count, report counters, local `storageHealth`
counters, and a `health` value of `healthy`, `partial`, or `unreachable`.
Mirror status writes use unique synced temp files before replacement so
concurrent mirror cycles never share a deterministic staging path, and
`chaft-node status` rejects local status files above 1 MiB before JSON parse.
Successful mirror cycles are marked `partial` when encrypted blobs,
materialized parents, corrupt local rows, poisoned servable metadata,
promotable metadata, or parseable non-servable rows still need attention.
`chaft-node status` prints a one-line operator summary from that file;
`--json` prints the raw status document and `--require-healthy` exits with an
error unless the mirror is healthy. `--max-age-seconds <n>` also fails if
`checkedAtUnixMs` is too old, and the text summary includes `ageMs` for quick
freshness checks. It treats complete local chunk manifests as available
encrypted blobs even without a whole-blob file, does not need workspace keys, and
prints peer/requested/fetched/blob/ignored/gap plus storage-health counts after
each pull. When a mirror is partial, `lastReport.missingBlobHashes` samples the
encrypted attachment blobs still unavailable locally and `lastReport.gaps`
samples unresolved event IDs with their missing causal parent event IDs.
`storageHealth` keeps exact compact row counters for the local mirror store.
`chaft-node repair-storage-metadata` repairs that local mirror store's fast
servable metadata and prints repair counters plus post-repair `storageHealth`
JSON without needing workspace keys or deleting raw event rows.
Blob/gap samples are capped at 64 rows; `missingBlobCount` and `gapCount`
remain the full authoritative totals.

`save-attachment` decrypts a locally available encrypted attachment to a chosen
output path. New message attachments carry a stable `attachmentId`; the legacy
`--blob-hash` CLI argument remains available for older scripts and legacy events
that do not carry an attachment ID. New local attachment sends currently cap the
selected plaintext file at 128 MiB before encryption and reject blank or
over-64 KiB selected source paths before local file metadata reads. Encrypted
blobs below that budget still use chunked P2P transfer when they exceed the
whole-blob frame size. Attachment saves reject blank or over-64 KiB output paths
before materializing workspace state or creating export temp files.
The runtime requires the right local content key (workspace key for public
channels, channel key for private channels) and the ciphertext blob, so a newly
hydrated profile usually imports keys and pulls/syncs blobs before saving
attachments. `prune-blobs` removes local ciphertext blob-cache objects that are
not referenced by any materialized local workspace event while preserving
referenced whole blobs, manifests, and chunks. `BlobStore::open` rejects blank
or over-64 KiB blob-store roots before directory creation, even for direct crate
callers below the runtime and node path checks.

`export-workspace-key` emits a plaintext secret key bundle for explicit manual
device bootstrap only. The bundle includes the current workspace content key and
any previous local epochs required for older ciphertext. `export-channel-key`
does the same for one private channel. These bundles must be moved over a
private channel and should not be published to peers or stored in replica nodes.
Signed key-epoch events are replica-safe metadata; the exported key bytes are
not.

`export-recovery-bundle` emits one passphrase-encrypted JSON bundle containing
the workspace key ring plus every local private-channel key ring for that
workspace. It is the preferred bootstrap transfer format over raw key exports,
but it is still not the final production secret store: new exports use Argon2id
with fixed memory/time parameters plus AES-256-GCM-SIV, and imports retain
legacy BLAKE3-wrapped bundle support. When the runtime is opened with
`--identity-passphrase`, `CHAFT_IDENTITY_PASSPHRASE`, or the FFI
runtime-directory-scoped unlock cache, newly written local workspace/private
channel key rings and OpenMLS private state files are also sealed with Argon2id
plus AES-256-GCM-SIV. `LocalRuntime` treats blank identity passphrases as absent
and rejects passphrases above 16 KiB before identity open or Argon2id work; the
lower `DeviceIdentity` API rejects the same oversized passphrases before
identity generation, file read/write work, or Argon2id, and rejects blank or
over-64 KiB identity paths before existence checks, reads, directory creation,
or writes. Identity file reads are capped at 64 KiB, and local
secret-file reads under `keys/` are capped at 16 MiB before parse/decrypt. The
OpenMLS parser layer also rejects oversized public key packages, private
key-package bundles, welcomes, commits, private group state, and ratchet trees
before JSON/TLS/OpenMLS decode, event publication, authorization, or
materialization. The desktop unlock prompt uses the FFI
runtime-directory-scoped process cache and no longer writes typed passphrases to
`CHAFT_IDENTITY_PASSPHRASE`; cached copies are zeroized on clear, replacement,
or process shutdown. The environment variable remains a development startup
fallback only. Chaft still needs an OS keychain or user-unlocked encrypted vault
for production unlock UX. Passphrases must move privately and should not be
passed through shared shell history in real use.

Production multi-device and member key distribution now has the first real
OpenMLS workspace member-add/welcome path, and public workspace payloads can use
OpenMLS exporter-derived keys once local group state exists. Private-channel
payloads can also use OpenMLS exporter-derived keys once local channel group
state exists. A device also needs signed `invite-member` and, for private
channels, `add-channel-member` events before replicas will accept events it
authors; key import alone is not authorization.

`publish-sample` sends a valid bootstrap sequence: workspace root, channel, and
an encrypted message event. The sample content key is throwaway developer state;
it proves replica-visible ciphertext storage, not durable user recovery. Passing
`--identity-file` gives the sample a restart-stable signing identity. A lone
message from an uninvited device is rejected by replica storage, and plaintext
message bodies are refused even from authorized devices. Partial replicas can
also authorize a later event from proof events supplied in the publish request
without storing those proof events. For larger workspaces, a root-owner-signed
trust snapshot can replace long invite/channel proof chains while remaining
proof-only replica input. Sync sends these proof snapshots as protobuf frames.
`export-trust-snapshot` emits that signed snapshot, and
`publish-event-with-trust-snapshot` publishes one materialized event plus the
snapshot so a partial backup node can store a later encrypted slice without
storing the full proof chain. Single-event proof publish scopes the snapshot to
the selected event's needed roles, channels, message targets, and read-marker
event targets. Multi-event backup similarly scopes proof to the events the
replica is missing in each backup publish chunk, while manual export keeps a
full current snapshot.

## License

Chaft is licensed as free software under `AGPL-3.0-or-later`; see `LICENSE`.
