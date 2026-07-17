#!/usr/bin/env python3
"""Static guardrails for the desktop's local-first reactivity contract."""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
MAIN = (ROOT / "src/main.cpp").read_text(encoding="utf-8")
APP = (ROOT / "qml/Chaft/App.qml").read_text(encoding="utf-8")
COMPOSER = (ROOT / "qml/Chaft/features/composer/ComposerBar.qml").read_text(
    encoding="utf-8"
)
TIMELINE = (ROOT / "qml/Chaft/features/timeline/TimelineView.qml").read_text(
    encoding="utf-8"
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


require("QFileSystemWatcher" in MAIN, "runtime event store changes must be watched")
require('QStringLiteral("events.db-wal")' not in MAIN, "WAL path must derive from events.db")
require('storePath + QStringLiteral("-wal")' in MAIN, "SQLite WAL changes must be watched")
require(
    "emit localWorkspaceMutationCommitted();" in MAIN,
    "confirmed local mutations must wake outbound sync",
)
watcher_start = MAIN.index("void handleRuntimeStorePathChanged(")
watcher_end = MAIN.index("void acknowledgeRuntimeStoreSnapshot(", watcher_start)
watcher = MAIN[watcher_start:watcher_end]
require(
    "emit localWorkspaceMutationCommitted();" not in watcher,
    "ambiguous filesystem notifications must not echo inbound writes to a peer",
)
require(
    "periodic inventory sync" in watcher,
    "the watcher boundary must document the bounded relay fallback",
)
require(
    "kOpenMlsAccessOwnWriteQuietPeriodMs" in watcher,
    "late watcher signals from access maintenance must not defeat backoff",
)

friendly_start = MAIN.index("QString friendlyRuntimeStatusText(")
friendly_end = MAIN.index("QString desktopConfigPath(", friendly_start)
friendly_status = MAIN[friendly_start:friendly_end]
friendly_rendered = re.sub(r'"\s*"', "", friendly_status)
for runtime_marker in (
    "openmlschannelrevocationpending",
    "open_mls_channel_revocation_pending",
    "openmlsworkspacerevocationpending",
    "open_mls_workspace_revocation_pending",
):
    require(
        runtime_marker in friendly_status,
        f"desktop must map revocation guard marker {runtime_marker}",
    )
for user_copy in (
    "Secure room access could not be revoked on this device",
    "room membership was not changed",
    "admin device that can open this room",
    "Secure workspace access could not be revoked on this device",
    "membership was not changed",
    "admin device that can open this workspace",
):
    require(
        user_copy in friendly_rendered,
        f"revocation guard needs safe actionable copy: {user_copy}",
    )

auto_sync = re.search(
    r"id:\s*autoSyncTimer(?P<body>.*?)(?:\n\s*}\n)", APP, re.DOTALL
)
require(auto_sync is not None, "auto-sync recovery timer is required")
interval = re.search(r"interval:\s*(\d+)", auto_sync.group("body"))
require(interval is not None, "auto-sync interval is required")
require(
    int(interval.group(1)) <= 5000,
    "the missed-wakeup fallback must remain responsive (at most five seconds)",
)
require("function scheduleImmediatePeerSync()" in APP, "local writes must sync immediately")
require(
    "function onLocalWorkspaceMutationCommitted()" in APP,
    "the controller mutation signal must reach the immediate-sync scheduler",
)

reconcile_start = MAIN.index("void runRuntimeSnapshotReconcile(")
reconcile_end = MAIN.index("void runDeviceKeyPackagePublish(", reconcile_start)
reconcile = MAIN[reconcile_start:reconcile_end]
require(
    "m_reconcileOpenMlsAccessJson" in reconcile,
    "hosted changes must reconcile private-room access",
)
require(
    reconcile.index("reconcileAccessFn(")
    < reconcile.index("latestRuntimeSnapshotValuePreservingTimeline("),
    "private-room access must reconcile before the UI snapshot",
)
require(
    re.search(
        r"finishHostedStoreRefreshAttempt\(\s*operationId,\s*snapshotAccepted\s*\)",
        reconcile,
    )
    is not None,
    "a valid UI snapshot must clear hosted watcher retries even if access maintenance fails",
)
require(
    "snapshotAccepted && reconcileAccessError.isEmpty()" not in reconcile,
    "access maintenance errors must not force the 500ms hosted refresh retry loop",
)
require(
    "private-room access is " in reconcile and "still catching up" in reconcile,
    "access maintenance failures need generic user-facing status copy",
)
require(
    "shouldAttemptOpenMlsAccessReconcile" in reconcile
    and "recordOpenMlsAccessReconcileFailure" in reconcile,
    "access maintenance retries must use bounded backoff",
)
require(
    "retry in about %1" in reconcile,
    "access maintenance status must state the actual retry timing",
)

peer_state_start = MAIN.index("void setPeerUpdateState(")
peer_state_end = MAIN.index(
    "void resetOpenMlsAccessReconcileBackoff(", peer_state_start
)
peer_state = MAIN[peer_state_start:peer_state_end]
require(
    "kPeerUpdateFinishedNotifyIntervalMs" in peer_state,
    "unchanged finished timestamps must be coalesced before notifying QML",
)

for field in ("accessState", "localContentReady"):
    require(field in APP, f"App.qml must consume ChannelSnapshot.{field}")
for field in ("provisioningState", "provisioningError"):
    require(field in MAIN, f"desktop actions must consume result DTO field {field}")

require("property string blockedReason" in COMPOSER, "composer needs an explicit block reason")
require("signal blockedActionRequested()" in COMPOSER, "blocked composer needs recovery action")
require("root.blocked || root.operationPending" in COMPOSER, "blocked sends must be rejected")
require("pendingLocal" in TIMELINE, "pending local messages must be rendered")
require("pendingDeliveryStatus" in TIMELINE, "pending messages need visible delivery state")
require(
    "timelineWithPendingComposerFeedback" in APP,
    "composer operations must be represented optimistically in the timeline",
)

print("desktop reactivity contract: ok")
