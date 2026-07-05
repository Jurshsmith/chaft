import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Chaft

ApplicationWindow {
    id: root
    width: 1440
    height: 820
    minimumWidth: 1040
    minimumHeight: 640
    visible: true
    title: "Chaft"
    color: Tokens.surfaceBase
    readonly property var workspaceSnapshot: chaftController.workspaceSnapshot || initialWorkspaceSnapshot
    readonly property var channels: workspaceSnapshot.channels || []
    readonly property var resolvedChannels: workspaceSnapshot.resolvedChannels || ({})
    readonly property var profiles: workspaceSnapshot.profiles || []
    readonly property var members: workspaceSnapshot.members || []
    readonly property var keyPackages: workspaceSnapshot.keyPackages || []
    readonly property int channelCount: root.countOrLength(workspaceSnapshot.channelCount, channels)
    readonly property int memberCount: root.countOrLength(workspaceSnapshot.memberCount, members)
    readonly property int keyPackageCount: root.countOrLength(workspaceSnapshot.keyPackageCount, keyPackages)
    readonly property int peerEndpointCount: root.countOrLength(workspaceSnapshot.peerEndpointCount, workspaceSnapshot.peerEndpoints || [])
    readonly property var peerEndpointHints: root.normalizedPeerEndpointHints()
    readonly property var backupPeerEndpointHints: root.filteredPeerEndpointHints(true)
    readonly property bool hasAutoBackupTargets: (chaftController.backupPeerEndpoints || []).length > 0
        || backupPeerEndpointHints.length > 0
    readonly property bool runtimeAccessReady: !chaftController.runtimeLocked
        && !chaftController.runtimeUnlockRequired
        && !chaftController.rawEventStoreMode
    readonly property bool runtimeWorkReady: chaftController.hasRuntimeWorkspace
        && root.runtimeAccessReady
    readonly property bool hasWorkspaceContent: chaftController.hasRuntimeWorkspace
        || chaftController.rawEventStoreMode
    readonly property var timeline: workspaceSnapshot.timeline || []
    readonly property var timelineWindow: workspaceSnapshot.timelineWindow || ({ startIndex: 0, itemCount: timeline.length, totalCount: timeline.length, hasMoreBefore: false, hasMoreAfter: false })
    readonly property string timelineChannelId: String(workspaceSnapshot.timelineChannelId || "")
    readonly property var workspaceRailItems: root.workspaceRailItemsForSummaries(
        chaftController.workspaceSummaries || [])
    property string selectedChannelId: ""
    property string searchQuery: ""
    readonly property string trimmedSearchQuery: searchQuery.trim()
    readonly property bool searchHasTerms: chaftController.searchQueryHasTerms(trimmedSearchQuery)
    readonly property string normalizedSearchQuery: searchHasTerms ? trimmedSearchQuery.toLowerCase() : ""
    readonly property var selectedChannel: root.channelById(root.selectedChannelId)
    readonly property string selectedChannelKey: String(selectedChannel.channelId || "")
    readonly property string selectedChannelName: root.hasWorkspaceContent
        ? String(selectedChannel.name || "general")
        : ""
    readonly property bool selectedChannelPrivate: Boolean(selectedChannel.isPrivate)
    readonly property bool selectedChannelTimelineReady: timelineChannelId.length > 0
        && timelineChannelId === selectedChannelKey
    readonly property bool channelSearchReady: root.runtimeWorkReady
        && normalizedSearchQuery.length > 0
        && chaftController.channelSearchQuery === root.trimmedSearchQuery
    readonly property var filteredChannels: root.filteredChannelRows()
    readonly property bool runtimeSearchReady: root.runtimeWorkReady
        && normalizedSearchQuery.length > 0
        && chaftController.messageSearchQuery === root.trimmedSearchQuery
    readonly property var channelTimeline: timeline.filter(function(item) {
        return root.timelineItemInSelectedChannel(item)
    })
    readonly property var runtimeSearchTimeline: (chaftController.messageSearchHits || []).map(function(item) {
        return root.searchTimelineItemWithChannelName(item)
    })
    readonly property var localSearchTimeline: channelTimeline.filter(function(item) {
        return root.timelineItemMatchesSearch(item)
    })
    readonly property var decoratedLocalSearchTimeline: root.timelineWithUnreadDivider(localSearchTimeline)
    readonly property var selectedTimeline: runtimeSearchReady ? runtimeSearchTimeline : decoratedLocalSearchTimeline
    property string inspectorItemKey: ""
    readonly property var inspectorItem: root.currentInspectorItem()
    readonly property bool inspectorItemIsSelected: inspectorItemKey.length > 0
        && root.timelineItemKey(inspectorItem) === inspectorItemKey
    readonly property int inspectorThreadReplyCount: root.threadReplyCountForItem(inspectorItem)
    readonly property var inspectorThreadReplyPreviews: root.threadReplyPreviewsForItem(inspectorItem)
    readonly property int inspectorAttachmentPreviewLimit: 4
    readonly property int inspectorAttachmentCount: root.attachmentTotalCountForItem(inspectorItem)
    readonly property var inspectorAttachmentPreviews: root.attachmentPreviewsForItem(inspectorItem)
    readonly property string inspectorAttachmentOverflowLabel: root.attachmentOverflowLabelForItem(inspectorItem)
    readonly property int selectedChannelGapCount: root.countMissingHistoryGaps(channelTimeline)
    readonly property int selectedChannelInvalidSignatureCount: root.countInvalidSignatures(channelTimeline)
    readonly property int selectedChannelIssueCount: selectedChannelGapCount + selectedChannelInvalidSignatureCount
    readonly property int recentChannelAttachmentLimit: 6
    readonly property var recentChannelAttachments: root.attachmentsFromTimeline(
        channelTimeline,
        recentChannelAttachmentLimit
    )
    readonly property var channelAttachments: recentChannelAttachments || []
    property string editingMessageId: ""
    property var replyTarget: ({})
    readonly property string replyTargetMessageId: String(replyTarget.messageId || "")
    property var composerDrafts: ({})
    property string pendingDraftRestoreWorkspaceId: ""
    property bool setupPanelOpen: false
    property bool autoSyncEnabled: false
    property bool autoBackupEnabled: chaftController.autoBackupEnabled
    property bool runtimeUnlockDismissed: false
    property string workspaceEntryMode: "join"
    property bool pendingPostCreateExport: false

    readonly property bool systemThemeMode: chaftController.themeMode === "system"
    readonly property bool systemPrefersDark: Application.styleHints.colorScheme !== Qt.ColorScheme.Light
    readonly property string resolvedDarkThemeId: chaftController.darkThemeId.length > 0
        ? chaftController.darkThemeId
        : Themes.defaultThemeId
    readonly property string resolvedLightThemeId: chaftController.lightThemeId.length > 0
        ? chaftController.lightThemeId
        : Themes.defaultLightThemeId

    Binding {
        target: Tokens
        property: "activeThemeId"
        value: root.systemThemeMode
            ? (root.systemPrefersDark ? root.resolvedDarkThemeId : root.resolvedLightThemeId)
            : (chaftController.themeId.length > 0 ? chaftController.themeId : Themes.defaultThemeId)
    }

    Binding {
        target: Tokens
        property: "motionEnabled"
        value: !chaftController.reducedMotionEnabled
    }

    property var pendingDeletes: []
    readonly property var pendingDeleteIds: root.pendingDeleteIdsForRows()
    property bool syncDrawerOpen: false
    property bool inspectorDetailsOpen: false

    readonly property int channelCryptoExceptionCount: {
        var rows = root.selectedTimeline || []
        var count = 0
        for (var i = 0; i < rows.length; ++i) {
            var kind = String(rows[i].kind || "")
            if (kind === "encrypted_message"
                    || (kind === "message" && rows[i].encrypted !== true && rows[i].deleted !== true)) {
                count += 1
            }
        }
        return count
    }

    readonly property string syncPillLabel: {
        if (chaftController.rawEventStoreMode) {
            return "View-only store"
        }
        if (!chaftController.hasRuntimeWorkspace) {
            return "No workspace"
        }
        if (chaftController.runtimeLocked) {
            return "Locked"
        }
        if (chaftController.syncInFlight) {
            return "Syncing"
        }
        if (chaftController.peerHosting) {
            return "Hosting"
        }
        if (root.autoSyncEnabled) {
            return "Live sync"
        }
        if (root.queuedPublishableEventCount > 0) {
            return "Local-only, " + root.queuedPublishableEventCount + " queued"
        }
        return "Local-only"
    }

    readonly property color syncPillTone: {
        if (chaftController.runtimeLocked) {
            return Tokens.warning
        }
        if (chaftController.syncInFlight) {
            return Tokens.textMuted
        }
        if (chaftController.peerHosting || root.autoSyncEnabled) {
            return Tokens.success
        }
        if (root.queuedPublishableEventCount > 0) {
            return Tokens.secure
        }
        return Tokens.textMuted
    }

    readonly property bool macosShortcuts: Qt.platform.os === "osx"
    readonly property string primaryKeyLabel: macosShortcuts ? "⌘" : "Ctrl+"
    readonly property string altKeyLabel: macosShortcuts ? "⌥" : "Alt+"
    readonly property string shiftKeyLabel: macosShortcuts ? "⇧" : "Shift+"

    // Single source for the command palette entries. Each entry:
    // { id, label, shortcut, enabled(), run() }. The palette evaluates
    // enabled() while building rows, so state stays live while it is open.
    readonly property var paletteActions: [
        {
            id: "toggle-setup",
            label: "Toggle setup panel",
            shortcut: "",
            enabled: function () {
                return chaftController.deviceId.length > 0
                    || chaftController.hasRuntimeWorkspace
            },
            run: function () { root.setupPanelOpen = !root.setupPanelOpen }
        },
        {
            id: "toggle-sync-drawer",
            label: "Toggle sync drawer",
            shortcut: "",
            enabled: function () { return chaftController.hasRuntimeWorkspace },
            run: function () { root.syncDrawerOpen = !root.syncDrawerOpen }
        },
        {
            id: "join-workspace",
            label: "Join workspace",
            shortcut: "",
            enabled: function () { return root.runtimeAccessReady },
            run: function () { root.openWorkspaceEntry("join") }
        },
        {
            id: "create-workspace",
            label: "Create workspace",
            shortcut: "",
            enabled: function () { return root.runtimeAccessReady },
            run: function () { root.openWorkspaceEntry("create") }
        },
        {
            id: "new-channel",
            label: "New channel",
            shortcut: "",
            enabled: function () { return root.runtimeWorkReady },
            run: function () { newChannelPopup.open() }
        },
        {
            id: "focus-composer",
            label: "Focus composer",
            shortcut: root.primaryKeyLabel + "M",
            enabled: function () { return root.runtimeWorkReady },
            run: function () { root.focusComposer() }
        },
        {
            id: "focus-search",
            label: "Focus search",
            shortcut: root.primaryKeyLabel + "F",
            enabled: function () { return root.hasWorkspaceContent },
            run: function () { root.focusSearch() }
        },
        {
            id: "jump-latest",
            label: "Jump to latest messages",
            shortcut: root.altKeyLabel + "End",
            enabled: function () { return true },
            run: function () { timelineView.scrollToLatest() }
        },
        {
            id: "toggle-live-sync",
            label: "Toggle live sync",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.autoSyncEnabled = !root.autoSyncEnabled }
        },
        {
            id: "sync-now",
            label: "Sync with peer now",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !chaftController.syncInFlight
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.syncWorkspaceFromPreferredPeer() }
        },
        {
            id: "backup-now",
            label: "Back up to peer now",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !chaftController.syncInFlight
                    && root.preferredManualBackupPeerEndpoint().length > 0
            },
            run: function () { root.backupWorkspaceToPreferredPeer() }
        },
        {
            id: "pull-peer",
            label: "Pull from peer",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !chaftController.syncInFlight
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.pullWorkspaceFromPreferredPeer() }
        },
        {
            id: "push-peer",
            label: "Push to peer",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !chaftController.syncInFlight
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.publishWorkspaceToPreferredPeer() }
        },
        {
            id: "lock-runtime",
            label: "Lock runtime",
            shortcut: "",
            enabled: function () {
                return chaftController.hasRuntimeWorkspace
                    && chaftController.runtimeUnlocked
                    && chaftController.runtimeUnlockClearable
                    && !chaftController.keyTransferInFlight
                    && !chaftController.syncInFlight
            },
            run: function () { chaftController.clearRuntimeUnlock() }
        },
        {
            id: "reindex-search",
            label: "Reindex search",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady && !chaftController.keyTransferInFlight
            },
            run: function () { chaftController.reindexWorkspaceSearch() }
        },
        {
            id: "shortcut-overlay",
            label: "Keyboard shortcuts",
            shortcut: root.primaryKeyLabel + "/",
            enabled: function () { return true },
            run: function () { shortcutOverlay.open() }
        }
    ]

    // Single registry behind the Ctrl/Cmd+/ overlay. Keep in sync with the
    // Shortcut elements below and the paletteActions shortcut labels.
    readonly property var shortcutRows: [
        { label: "Open command palette", keys: root.primaryKeyLabel + "K" },
        { label: "Focus search", keys: root.primaryKeyLabel + "F" },
        { label: "Focus composer", keys: root.primaryKeyLabel + "M" },
        { label: "Attach file to draft", keys: root.primaryKeyLabel + "O" },
        {
            label: "Copy selected message text",
            keys: root.primaryKeyLabel + root.shiftKeyLabel + "C"
        },
        {
            label: "Previous or next channel",
            keys: root.altKeyLabel + "↑ " + root.altKeyLabel + "↓"
        },
        {
            label: "Previous or next workspace",
            keys: root.altKeyLabel + "← " + root.altKeyLabel + "→"
        },
        { label: "Jump to oldest messages", keys: root.altKeyLabel + "Home" },
        { label: "Jump to latest messages", keys: root.altKeyLabel + "End" },
        { label: "Keyboard shortcuts overlay", keys: root.primaryKeyLabel + "/" },
        { label: "Close, cancel edit or reply, clear search", keys: "Esc" }
    ]

    Timer {
        id: pendingDeleteTimer
        repeat: false
        onTriggered: root.flushExpiredPendingDeletes()
    }

    CommandPalette {
        id: commandPalette
        parent: Overlay.overlay
        app: root
        actions: root.paletteActions
    }

    ShortcutOverlay {
        id: shortcutOverlay
        parent: Overlay.overlay
        rows: root.shortcutRows
    }

    ToastHost {
        id: toastHost
        parent: Overlay.overlay
        anchors.right: parent ? parent.right : undefined
        anchors.bottom: parent ? parent.bottom : undefined
        anchors.margins: Tokens.space4
        width: 340
        z: 1000
        onActionTriggered: function (actionId) {
            var id = String(actionId || "")
            if (id.indexOf("undo-delete:") === 0) {
                root.undoMessageDelete(id.slice("undo-delete:".length))
            }
        }
    }

    onClosing: root.flushPendingDeletes()

    ConfirmDialog {
        id: confirmDialog
        parent: Overlay.overlay
        onConfirmed: function (contextId) {
            var id = String(contextId || "")
            if (id.indexOf("remove-member:") === 0) {
                chaftController.removeMember(id.slice("remove-member:".length))
            } else if (id.indexOf("revoke-channel-member:") === 0) {
                if (chaftController.removeChannelMember(
                        root.selectedChannelKey,
                        id.slice("revoke-channel-member:".length))) {
                    setupPanel.clearChannelMemberField()
                }
            } else if (id === "rotate-keys") {
                chaftController.rotateWorkspaceManualKeys()
            } else if (id.indexOf("rotate-channel-key:") === 0) {
                chaftController.rotateChannelKey(id.slice("rotate-channel-key:".length))
            }
        }
    }

    // Danger-zone confirmation path shared with SetupPanel. Everything routed
    // here renders with destructive styling; actionId dispatch stays in
    // confirmDialog.onConfirmed above.
    function confirmSetupAction(title, message, confirmLabel, actionId) {
        confirmDialog.ask(title, message, confirmLabel, actionId, true)
    }
    readonly property var publishQueue: chaftController.publishQueue || ({})
    readonly property var publishQueueSummary: publishQueue.summary || ({})
    readonly property int queuedPublishableEventCount: publishQueueSummary.publishableEventCount === undefined
        ? (publishQueue.publishableEventIds || []).length
        : Number(publishQueueSummary.publishableEventCount || 0)
    readonly property int queuedBackupEventCount: publishQueueSummary.backupEventCount === undefined
        ? (publishQueue.backupEventIds || []).length
        : Number(publishQueueSummary.backupEventCount || 0)
    readonly property int queuedAvailableBlobCount: publishQueueSummary.availableBlobCount === undefined
        ? (publishQueue.availableBlobHashes || []).length
        : Number(publishQueueSummary.availableBlobCount || 0)
    readonly property int queuedMissingBlobCount: publishQueueSummary.missingBlobCount === undefined
        ? (publishQueue.missingBlobHashes || []).length
        : Number(publishQueueSummary.missingBlobCount || 0)
    readonly property int queuedSkippedGapCount: publishQueueSummary.skippedGapCount === undefined
        ? (publishQueue.skippedGaps || []).length
        : Number(publishQueueSummary.skippedGapCount || 0)
    readonly property int publishQueueIssueCount: queuedMissingBlobCount + queuedSkippedGapCount
    readonly property string publishQueueError: String(publishQueue.error || "")
    readonly property var storageHealth: chaftController.workspaceStorageHealth || ({})
    readonly property bool storageHealthKnown: storageHealth.workspaceId !== undefined
        || storageHealth.error !== undefined
    readonly property string storageHealthError: String(storageHealth.error || "")
    readonly property int storageTotalEventCount: Number(storageHealth.totalEventCount || 0)
    readonly property int storageServableEventCount: Number(storageHealth.servableEventCount || 0)
    readonly property int storageCorruptEventCount: Number(storageHealth.corruptEventCount || 0)
    readonly property int storagePoisonedMetadataCount: Number(storageHealth.poisonedServableMetadataCount || 0)
    readonly property int storagePromotableMetadataCount: Number(storageHealth.promotableServableMetadataCount || 0)
    readonly property int storageNonServableParseableEventCount: Number(storageHealth.nonServableParseableEventCount || 0)
    readonly property int storageHealthAttentionCount: storageCorruptEventCount
        + storageNonServableParseableEventCount
        + storagePoisonedMetadataCount
        + storagePromotableMetadataCount
    readonly property bool storageMetadataRepairSuggested: storagePoisonedMetadataCount > 0
        || storagePromotableMetadataCount > 0
    readonly property bool storageHealthHasIssue: storageHealthError.length > 0
        || storageHealthAttentionCount > 0

    function countOrLength(value, rows) {
        var parsed = Number(value)
        return isNaN(parsed) ? (rows || []).length : parsed
    }

    function backupPeerTimeLabel(timestamp) {
        var value = String(timestamp || "")
        if (value.length === 0) {
            return ""
        }
        var date = new Date(value)
        if (isNaN(date.getTime())) {
            return ""
        }
        return Qt.formatDateTime(date, "MMM d HH:mm")
    }

    function backupPeerSuspectScore(status) {
        var score = Number((status || {}).suspectScore || 0)
        if (!isNaN(score) && score > 0) {
            return score
        }
        return Boolean((status || {}).lastSuspectPeer) ? 1 : 0
    }

    function backupPeerSuspectSuffix(status) {
        var score = root.backupPeerSuspectScore(status)
        return score > 0 ? " | suspect " + score : ""
    }

    function backupPeerStatusText(peerEndpoint) {
        var statuses = chaftController.backupPeerStatuses || {}
        var status = statuses[peerEndpoint] || {}
        var suspectSuffix = root.backupPeerSuspectSuffix(status)
        var failureCount = Number(status.failureCount || 0)
        if (failureCount > 0) {
            var message = String(status.lastMessage || "backup failed")
            var retryAt = root.backupPeerTimeLabel(status.nextAttemptAfter || "")
            var failureSuffix = failureCount > 1 ? " (" + failureCount + " failures)" : ""
            return retryAt.length > 0
                ? message + failureSuffix + suspectSuffix + " | retry " + retryAt
                : message + failureSuffix + suspectSuffix
        }

        var partial = Boolean(status.lastPartial || false)
        var successAt = root.backupPeerTimeLabel(status.lastSuccessAt || "")
        if (partial) {
            var partialMessage = String(status.lastMessage || "backup partial")
            return successAt.length > 0
                ? partialMessage + " | last backup " + successAt + suspectSuffix
                : partialMessage + suspectSuffix
        }

        if (successAt.length > 0) {
            return "last backup " + successAt + suspectSuffix
        }

        var attemptAt = root.backupPeerTimeLabel(status.lastAttemptAt || "")
        return attemptAt.length > 0
            ? "last attempted " + attemptAt + suspectSuffix
            : "ready" + suspectSuffix
    }

    function backupPeerStateLabel(peerEndpoint) {
        var statuses = chaftController.backupPeerStatuses || {}
        var status = statuses[peerEndpoint] || {}
        if (Number(status.failureCount || 0) > 0) {
            return "Failed"
        }
        if (root.backupPeerSuspectScore(status) > 0) {
            return "Suspect"
        }
        if (Boolean(status.lastPartial || false)) {
            return "Partial"
        }
        if (String(status.lastSuccessAt || "").length > 0) {
            return "Backed up"
        }
        if (String(status.lastAttemptAt || "").length > 0) {
            return "Attempted"
        }
        return "Ready"
    }

    function backupPeerStateColor(peerEndpoint) {
        var statuses = chaftController.backupPeerStatuses || {}
        var status = statuses[peerEndpoint] || {}
        return (Number(status.failureCount || 0) > 0
                || Boolean(status.lastPartial || false)
                || root.backupPeerSuspectScore(status) > 0)
            ? Tokens.warningText
            : Tokens.success
    }

    function normalizedPeerEndpointHints() {
        var hints = []
        var seen = {}
        var endpoints = root.workspaceSnapshot.peerEndpoints || []
        for (var i = 0; i < endpoints.length; i++) {
            var hint = endpoints[i] || {}
            var endpoint = String(hint.endpoint || "").trim()
            if (endpoint.length === 0
                    || seen[endpoint]
                    || !root.peerEndpointHintIsSupported(hint)) {
                continue
            }
            seen[endpoint] = true
            hints.push({
                deviceId: String(hint.deviceId || ""),
                displayName: hint.displayName,
                endpointId: String(hint.endpointId || ""),
                endpoint: endpoint,
                transport: String(hint.transport || "").trim(),
                isBackupPeer: Boolean(hint.isBackupPeer),
                expiresAtMs: hint.expiresAtMs,
                publishedEventId: String(hint.publishedEventId || ""),
                physicalMs: hint.physicalMs
            })
        }
        return hints
    }

    function isPeerEndpointExpired(peer) {
        var expiresAtMs = (peer || {}).expiresAtMs
        if (expiresAtMs === undefined || expiresAtMs === null) {
            return false
        }
        var date = new Date(Number(expiresAtMs))
        return !isNaN(date.getTime()) && date.getTime() <= new Date().getTime()
    }

    function filteredPeerEndpointHints(backupOnly) {
        var filtered = []
        for (var i = 0; i < root.peerEndpointHints.length; i++) {
            var peer = root.peerEndpointHints[i] || {}
            if (root.isPeerEndpointExpired(peer)) {
                continue
            }
            if (backupOnly && !Boolean(peer.isBackupPeer)) {
                continue
            }
            filtered.push(peer)
        }
        return filtered
    }

    function peerEndpointHintIsSupported(peer) {
        var endpoint = String((peer || {}).endpoint || "").trim()
        if (endpoint.length === 0) {
            return false
        }
        var transport = String((peer || {}).transport || "").trim().toLowerCase()
        var route = root.supportedPeerEndpointRouteKind(endpoint)
        if (route === "direct-tcp") {
            return transport === "direct-tcp"
        }
        if (route === "iroh-direct") {
            return transport === "iroh" || transport === "iroh-direct"
        }
        return false
    }

    function containsAsciiWhitespace(value) {
        return /[\t\n\r\f\v ]/.test(String(value || ""))
    }

    function splitHostPort(address) {
        var normalized = String(address || "").trim()
        if (normalized.length === 0 || root.containsAsciiWhitespace(normalized)) {
            return null
        }
        if (normalized.charAt(0) === "[") {
            var separator = normalized.indexOf("]:")
            if (separator <= 1 || separator + 2 >= normalized.length) {
                return null
            }
            return {
                host: normalized.slice(1, separator),
                port: normalized.slice(separator + 2),
                bracketedIpv6: true
            }
        }

        var colon = normalized.lastIndexOf(":")
        if (colon <= 0 || colon === normalized.length - 1) {
            return null
        }
        var host = normalized.slice(0, colon)
        if (host.indexOf(":") >= 0) {
            return null
        }
        return {
            host: host,
            port: normalized.slice(colon + 1),
            bracketedIpv6: false
        }
    }

    function validTcpPort(port) {
        var text = String(port || "")
        if (!/^[0-9]+$/.test(text)) {
            return false
        }
        var value = Number(text)
        return value >= 1 && value <= 65535
    }

    function validDirectTcpHost(host, bracketedIpv6) {
        var value = String(host || "")
        if (value.length === 0) {
            return false
        }
        if (bracketedIpv6) {
            return /^[0-9A-Fa-f:.]+$/.test(value) && value.indexOf(":") >= 0
        }
        return /^[A-Za-z0-9._-]+$/.test(value)
    }

    function validNativeIrohAddrHost(host, bracketedIpv6) {
        var value = String(host || "")
        if (bracketedIpv6) {
            return /^[0-9A-Fa-f:.]+$/.test(value) && value.indexOf(":") >= 0
        }
        if (!/^([0-9]{1,3}\.){3}[0-9]{1,3}$/.test(value)) {
            return false
        }
        var parts = value.split(".")
        for (var i = 0; i < parts.length; i++) {
            var segmentValue = Number(parts[i])
            if (segmentValue < 0 || segmentValue > 255) {
                return false
            }
        }
        return true
    }

    function directTcpAddressIsValid(address) {
        var parsed = root.splitHostPort(address)
        return parsed !== null
            && root.validTcpPort(parsed.port)
            && root.validDirectTcpHost(parsed.host, parsed.bracketedIpv6)
    }

    function nativeIrohDirectAddrIsValid(address) {
        var parsed = root.splitHostPort(address)
        return parsed !== null
            && root.validTcpPort(parsed.port)
            && root.validNativeIrohAddrHost(parsed.host, parsed.bracketedIpv6)
    }

    function nativeIrohEndpointIdSyntaxIsValid(endpointId) {
        var value = String(endpointId || "")
        return /^[0-9a-f]{64}$/.test(value) || /^[A-Za-z2-7]{52}$/.test(value)
    }

    function supportedPeerEndpointRouteKind(endpoint) {
        var normalized = String(endpoint || "").trim()
        if (normalized.length === 0) {
            return "unsupported"
        }
        if (normalized.indexOf("direct+tcp://") === 0) {
            return root.directTcpAddressIsValid(normalized.slice(13))
                ? "direct-tcp"
                : "unsupported"
        }
        if (normalized.indexOf("tcp://") === 0) {
            return root.directTcpAddressIsValid(normalized.slice(6))
                ? "direct-tcp"
                : "unsupported"
        }
        if (normalized.indexOf("iroh://") === 0) {
            var rest = normalized.slice(7)
            var querySeparator = rest.indexOf("?")
            if (querySeparator <= 0) {
                return "unsupported"
            }
            var endpointId = rest.slice(0, querySeparator)
            if (!root.nativeIrohEndpointIdSyntaxIsValid(endpointId)) {
                return "unsupported"
            }
            var query = rest.slice(querySeparator + 1)
            var fragmentSeparator = query.indexOf("#")
            if (fragmentSeparator >= 0) {
                query = query.slice(0, fragmentSeparator)
            }
            var parameters = query.split("&")
            var hasDirectAddr = false
            for (var i = 0; i < parameters.length; i++) {
                var parameter = parameters[i]
                var equals = parameter.indexOf("=")
                var key = equals >= 0 ? parameter.slice(0, equals).trim() : parameter.trim()
                var value = equals >= 0 ? parameter.slice(equals + 1).trim() : ""
                if (key === "relay") {
                    return "unsupported"
                }
                if (key !== "addr" || !root.nativeIrohDirectAddrIsValid(value)) {
                    return "unsupported"
                }
                hasDirectAddr = true
            }
            return hasDirectAddr ? "iroh-direct" : "unsupported"
        }
        if (normalized.indexOf("://") >= 0) {
            return "unsupported"
        }
        return root.directTcpAddressIsValid(normalized) ? "direct-tcp" : "unsupported"
    }

    function preferredSyncPeerEndpoint() {
        var manualEndpoint = String(peerEndpointField.text || "").trim()
        if (manualEndpoint.length > 0) {
            return manualEndpoint
        }

        var fallback = ""
        var localDeviceId = String(chaftController.deviceId || "")
        for (var i = 0; i < root.peerEndpointHints.length; i++) {
            var peer = root.peerEndpointHints[i] || {}
            var endpoint = String(peer.endpoint || "").trim()
            if (endpoint.length === 0 || root.isPeerEndpointExpired(peer)) {
                continue
            }
            if (String(peer.deviceId || "") === localDeviceId) {
                continue
            }
            if (!Boolean(peer.isBackupPeer)) {
                return endpoint
            }
            if (fallback.length === 0) {
                fallback = endpoint
            }
        }
        return fallback
    }

    function preferredManualBackupPeerEndpoint() {
        var manualEndpoint = String(peerEndpointField.text || "").trim()
        if (manualEndpoint.length > 0) {
            return manualEndpoint
        }

        var savedPeers = chaftController.backupPeerEndpoints || []
        for (var i = 0; i < savedPeers.length; i++) {
            var savedEndpoint = String(savedPeers[i] || "").trim()
            if (savedEndpoint.length > 0) {
                return savedEndpoint
            }
        }

        var discoveredBackup = root.preferredBackupPeerEndpoint()
        if (discoveredBackup.length > 0) {
            return discoveredBackup
        }

        return root.preferredSyncPeerEndpoint()
    }

    function preferredRetryPeerEndpoint() {
        var manualEndpoint = String(peerEndpointField.text || "").trim()
        if (manualEndpoint.length > 0) {
            return manualEndpoint
        }

        var discoveredBackup = root.preferredBackupPeerEndpoint()
        return discoveredBackup.length > 0 ? discoveredBackup : ""
    }

    function preferredBackupPeerEndpoint() {
        for (var i = 0; i < root.backupPeerEndpointHints.length; i++) {
            var endpoint = String((root.backupPeerEndpointHints[i] || {}).endpoint || "").trim()
            if (endpoint.length > 0) {
                return endpoint
            }
        }
        return ""
    }

    function endpointRouteKind(endpoint) {
        var value = String(endpoint || "").trim().toLowerCase()
        if (value.length === 0) {
            return "none"
        }
        if (value.indexOf("iroh+relay://") === 0
                || value.indexOf("relay://") === 0
                || value.indexOf("relay=") >= 0) {
            return "iroh-relay"
        }
        if (value.indexOf("iroh+discovery://") === 0
                || value.indexOf("discovery://") === 0) {
            return "iroh-discovery"
        }
        if (value.indexOf("iroh://") === 0) {
            return value.indexOf("addr=") >= 0 ? "iroh-direct" : "iroh-discovery"
        }
        if (value.indexOf("direct+tcp://") === 0 || value.indexOf("tcp://") === 0) {
            return "direct-tcp"
        }
        return value.indexOf("://") >= 0 ? "custom" : "direct-tcp"
    }

    function endpointRouteLabel(endpoint) {
        switch (root.endpointRouteKind(endpoint)) {
        case "direct-tcp":
            return "Direct TCP"
        case "iroh-direct":
            return "Iroh direct"
        case "iroh-relay":
            return "Iroh relay"
        case "iroh-discovery":
            return "Iroh discovery"
        case "custom":
            return "Custom peer"
        default:
            return "Local only"
        }
    }

    function endpointIsBackupPeer(endpoint) {
        var normalized = String(endpoint || "").trim()
        if (normalized.length === 0) {
            return false
        }
        var savedPeers = chaftController.backupPeerEndpoints || []
        for (var i = 0; i < savedPeers.length; i++) {
            if (String(savedPeers[i] || "").trim() === normalized) {
                return true
            }
        }
        for (var j = 0; j < root.backupPeerEndpointHints.length; j++) {
            if (String((root.backupPeerEndpointHints[j] || {}).endpoint || "").trim() === normalized) {
                return true
            }
        }
        return false
    }

    function activePeerRouteLabel() {
        if (chaftController.runtimeLocked) {
            return "Locked"
        }
        if (chaftController.runtimeUnlockRequired) {
            return "Unlock needed"
        }
        if (chaftController.rawEventStoreMode) {
            return "Store view"
        }
        if (!chaftController.hasRuntimeWorkspace) {
            return "No workspace"
        }
        if (chaftController.syncInFlight) {
            return "Syncing"
        }

        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length === 0) {
            return (root.queuedPublishableEventCount > 0 || root.queuedBackupEventCount > 0)
                ? "Offline queue"
                : "Local only"
        }
        return root.endpointIsBackupPeer(endpoint)
            ? "Replica | " + root.endpointRouteLabel(endpoint)
            : root.endpointRouteLabel(endpoint)
    }

    function activePeerRouteDetail() {
        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length === 0) {
            return root.publishQueueStatusText()
        }
        return endpoint
    }

    function activePeerRouteIsWarning() {
        return chaftController.runtimeLocked
            || chaftController.runtimeUnlockRequired
            || root.activePeerRouteLabel() === "Offline queue"
            || root.endpointRouteKind(root.preferredSyncPeerEndpoint()) === "iroh-discovery"
    }

    function shortDeviceId(deviceId) {
        var value = String(deviceId || "")
        return value.length > 14 ? value.slice(0, 7) + "..." + value.slice(value.length - 4) : value
    }

    function peerEndpointKindLabel(peer) {
        var transport = String((peer || {}).transport || root.endpointRouteLabel((peer || {}).endpoint))
        return (Boolean((peer || {}).isBackupPeer) ? "Backup" : "Peer") + " | " + transport
    }

    function peerEndpointDetailLabel(peer) {
        var device = root.shortDeviceId((peer || {}).deviceId)
        var endpointId = String((peer || {}).endpointId || "")
        var expiry = root.peerEndpointExpiryLabel((peer || {}).expiresAtMs)
        var parts = []
        if (device.length > 0) {
            parts.push(device)
        }
        if (endpointId.length > 0) {
            parts.push(endpointId)
        }
        if (expiry.length > 0) {
            parts.push(expiry)
        }
        return parts.join(" | ")
    }

    function peerEndpointExpiryLabel(expiresAtMs) {
        if (expiresAtMs === undefined || expiresAtMs === null) {
            return ""
        }
        var date = new Date(Number(expiresAtMs))
        if (isNaN(date.getTime())) {
            return ""
        }
        return date.getTime() <= new Date().getTime()
            ? "expired"
            : "until " + Qt.formatDateTime(date, "MMM d HH:mm")
    }

    function isBackupPeerSaved(peerEndpoint) {
        var endpoint = String(peerEndpoint || "").trim()
        var peers = chaftController.backupPeerEndpoints || []
        for (var i = 0; i < peers.length; i++) {
            if (String(peers[i] || "").trim() === endpoint) {
                return true
            }
        }
        return false
    }

    function usePeerEndpoint(peerEndpoint) {
        var endpoint = String(peerEndpoint || "").trim()
        if (endpoint.length === 0) {
            return false
        }
        peerEndpointField.text = endpoint
        chaftController.defaultPeerEndpoint = endpoint
        return true
    }

    function currentWorkspaceId() {
        var selectedWorkspaceId = String(chaftController.selectedWorkspaceId || "")
        if (selectedWorkspaceId.length > 0) {
            return selectedWorkspaceId
        }
        return String(root.workspaceSnapshot.workspaceId || "")
    }

    function composerDraftKey(workspaceId, channelId) {
        var normalizedWorkspaceId = String(workspaceId || "")
        var normalizedChannelId = String(channelId || "")
        return normalizedWorkspaceId.length > 0 && normalizedChannelId.length > 0
            ? normalizedWorkspaceId + "::" + normalizedChannelId
            : ""
    }

    function selectedComposerDraftKey() {
        return root.composerDraftKey(root.currentWorkspaceId(), root.selectedChannel.channelId)
    }

    function draftTextForChannel(channelId) {
        var key = root.composerDraftKey(root.currentWorkspaceId(), channelId)
        return key.length > 0 ? String(root.composerDrafts[key] || "") : ""
    }

    function draftPreviewForChannel(channelId) {
        var draft = root.draftTextForChannel(channelId).replace(/\s+/g, " ").trim()
        if (draft.length === 0) {
            return ""
        }
        return "Draft: " + (draft.length > 46 ? draft.slice(0, 43) + "..." : draft)
    }

    function channelSidebarLabel(channel) {
        var draft = root.draftPreviewForChannel(channel.channelId)
        return draft.length > 0 ? draft : root.channelActivityLabel(channel)
    }

    function syncStatusColor(status) {
        var normalized = String(status || "").toLowerCase()
        if (normalized.indexOf("failed") !== -1 || normalized.indexOf("error") !== -1
                || normalized.indexOf("rejected") !== -1 || normalized.indexOf("suspect") !== -1
                || normalized.indexOf("invalid") !== -1) {
            return Tokens.warningText
        }
        if (normalized.indexOf("synced") !== -1 || normalized.indexOf("published") !== -1
                || normalized.indexOf("backed up") !== -1 || normalized.indexOf("complete") !== -1
                || normalized.indexOf("verified") !== -1 || normalized.indexOf("hosting") !== -1) {
            return Tokens.success
        }
        return Tokens.textMuted
    }

    function applySmokeUiState() {
        var state = String(chaftController.smokeUiState || "")
        if (state === "setup") {
            root.setupPanelOpen = true
        } else if (state === "drawer") {
            root.syncDrawerOpen = true
        } else if (state === "palette") {
            commandPalette.open()
        }
    }

    function queueMessageDelete(messageId) {
        var id = String(messageId || "")
        if (id.length === 0 || root.pendingDeleteIds.indexOf(id) !== -1) {
            return
        }
        var next = root.pendingDeletes.slice()
        next.push({
            messageId: id,
            deadlineMs: Date.now() + 5000
        })
        while (next.length > 8) {
            root.dispatchMessageDelete(next.shift().messageId)
        }
        root.pendingDeletes = next
        root.schedulePendingDeleteTimer()
        toastHost.show("info", "Message deleted", "Undo", "undo-delete:" + id, 5000)
        if (root.editingMessageId === id) {
            root.cancelEditMessage()
        }
    }

    function pendingDeleteIdsForRows() {
        var ids = []
        for (var i = 0; i < root.pendingDeletes.length; ++i) {
            ids.push(String(root.pendingDeletes[i].messageId || ""))
        }
        return ids
    }

    function schedulePendingDeleteTimer() {
        if (root.pendingDeletes.length === 0) {
            pendingDeleteTimer.stop()
            return
        }
        var now = Date.now()
        var nextDeadline = Number(root.pendingDeletes[0].deadlineMs || now)
        for (var i = 1; i < root.pendingDeletes.length; ++i) {
            nextDeadline = Math.min(nextDeadline, Number(root.pendingDeletes[i].deadlineMs || now))
        }
        pendingDeleteTimer.interval = Math.max(1, nextDeadline - now)
        pendingDeleteTimer.restart()
    }

    function undoMessageDelete(messageId) {
        var id = String(messageId || "")
        var next = []
        for (var i = 0; i < root.pendingDeletes.length; ++i) {
            if (String(root.pendingDeletes[i].messageId || "") !== id) {
                next.push(root.pendingDeletes[i])
            }
        }
        root.pendingDeletes = next
        root.schedulePendingDeleteTimer()
    }

    function dispatchMessageDelete(messageId) {
        chaftController.deleteMessage(messageId)
    }

    function flushExpiredPendingDeletes() {
        var now = Date.now()
        var expired = []
        var remaining = []
        for (var i = 0; i < root.pendingDeletes.length; ++i) {
            var pending = root.pendingDeletes[i]
            if (Number(pending.deadlineMs || 0) <= now) {
                expired.push(pending.messageId)
            } else {
                remaining.push(pending)
            }
        }
        root.pendingDeletes = remaining
        for (var j = 0; j < expired.length; ++j) {
            root.dispatchMessageDelete(expired[j])
        }
        root.schedulePendingDeleteTimer()
    }

    function flushPendingDeletes() {
        var pending = root.pendingDeletes
        root.pendingDeletes = []
        pendingDeleteTimer.stop()
        for (var i = 0; i < pending.length; ++i) {
            root.dispatchMessageDelete(pending[i].messageId)
        }
    }

    function isIncomingTimelineMessage(item) {
        if (!item || !item.messageId) {
            return false
        }
        var kind = String(item.kind || "")
        if (kind !== "message" && kind !== "encrypted_message") {
            return false
        }
        return String(item.authorDeviceId || "") !== String(chaftController.deviceId || "")
    }

    function copyTimelineItem(item) {
        var row = {}
        var source = item || {}
        for (var key in source) {
            if (Object.prototype.hasOwnProperty.call(source, key)) {
                row[key] = source[key]
            }
        }
        return row
    }

    function timelineWithUnreadDivider(items) {
        if (root.normalizedSearchQuery.length > 0) {
            return items
        }

        var unreadRemaining = Number(root.selectedChannel.unreadCount || 0)
        if (unreadRemaining <= 0) {
            return items
        }

        var dividerIndex = -1
        for (var i = items.length - 1; i >= 0 && unreadRemaining > 0; i -= 1) {
            if (root.isIncomingTimelineMessage(items[i])) {
                unreadRemaining -= 1
                dividerIndex = i
            }
        }
        if (dividerIndex < 0) {
            return items
        }

        var decorated = []
        for (var j = 0; j < items.length; j += 1) {
            var row = root.copyTimelineItem(items[j])
            row.unreadDividerBefore = j === dividerIndex
            decorated.push(row)
        }
        return decorated
    }

    function saveDraftForKey(key, text) {
        if (key.length === 0) {
            return
        }
        var draft = String(text || "")
        var drafts = {}
        var existingDrafts = root.composerDrafts || {}
        for (var draftKey in existingDrafts) {
            if (Object.prototype.hasOwnProperty.call(existingDrafts, draftKey)) {
                drafts[draftKey] = existingDrafts[draftKey]
            }
        }
        if (draft.trim().length > 0) {
            drafts[key] = draft
        } else {
            delete drafts[key]
        }
        root.composerDrafts = drafts
    }

    function saveSelectedDraftText(text) {
        if (root.editingMessageId.length > 0) {
            return
        }
        root.saveDraftForKey(root.selectedComposerDraftKey(), text)
    }

    function saveCurrentDraft() {
        if (root.editingMessageId.length > 0) {
            return
        }
        root.saveSelectedDraftText(composer.draftText())
    }

    function restoreSelectedDraft(focusDraft) {
        if (root.editingMessageId.length > 0) {
            return
        }
        composer.restoreDraft(root.draftTextForChannel(root.selectedChannel.channelId))
        if (focusDraft) {
            root.focusComposer()
        }
    }

    function clearDraftForChannel(channelId) {
        root.saveDraftForKey(root.composerDraftKey(root.currentWorkspaceId(), channelId), "")
    }

    function clearDraftForWorkspaceChannel(workspaceId, channelId) {
        root.saveDraftForKey(root.composerDraftKey(workspaceId, channelId), "")
    }

    function queueCountLabel(value, singular, plural) {
        return String(value) + " " + (value === 1 ? singular : plural)
    }

    function publishQueueStatusText() {
        if (root.publishQueueError.length > 0) {
            return root.publishQueueError
        }

        var parts = []
        if (root.queuedPublishableEventCount > 0) {
            parts.push(root.queueCountLabel(root.queuedPublishableEventCount, "event", "events"))
        }
        if (root.queuedBackupEventCount > 0) {
            parts.push(root.queueCountLabel(root.queuedBackupEventCount, "backup event", "backup events"))
        }
        if (root.queuedMissingBlobCount > 0) {
            parts.push(root.queueCountLabel(root.queuedMissingBlobCount, "blob missing", "blobs missing"))
        }
        if (root.queuedSkippedGapCount > 0) {
            parts.push(root.queueCountLabel(root.queuedSkippedGapCount, "gap", "gaps"))
        }
        return parts.length > 0 ? parts.join(" | ") : "Queue empty"
    }

    function storageHealthStatusText() {
        if (root.storageHealthError.length > 0) {
            return root.storageHealthError
        }
        if (!root.storageHealthKnown) {
            return "Cache pending"
        }

        var parts = []
        if (root.storageCorruptEventCount > 0) {
            parts.push(root.queueCountLabel(root.storageCorruptEventCount, "corrupt row", "corrupt rows"))
        }
        if (root.storageNonServableParseableEventCount > 0) {
            parts.push(root.queueCountLabel(root.storageNonServableParseableEventCount, "untrusted row", "untrusted rows"))
        }
        if (root.storagePoisonedMetadataCount > 0) {
            parts.push(root.queueCountLabel(root.storagePoisonedMetadataCount, "metadata mismatch", "metadata mismatches"))
        }
        if (root.storagePromotableMetadataCount > 0) {
            parts.push(root.queueCountLabel(root.storagePromotableMetadataCount, "metadata gap", "metadata gaps"))
        }
        if (parts.length > 0) {
            return parts.join(" | ")
        }

        return "Cache healthy | " + root.queueCountLabel(root.storageServableEventCount, "servable row", "servable rows")
    }

    function channelActivityLabel(channel) {
        var activity = channel.latestActivity || {}
        var preview = String(activity.preview || "")
        if (preview.length === 0) {
            return ""
        }

        var author = String(activity.authorDisplayName || "")
        return author.length > 0 ? author + ": " + preview : preview
    }

    function compactInlineText(value, fallback) {
        var text = String(value || "").replace(/\s+/g, " ").trim()
        if (text.length === 0) {
            return fallback || ""
        }
        return text.length > 96 ? text.slice(0, 93) + "..." : text
    }

    function replyTargetLabel(item) {
        var author = root.itemAuthorLabel(item)
        var body = root.compactInlineText(item.body, "message")
        return author.length > 0 ? author + ": " + body : body
    }

    function threadReplyCountForItem(item) {
        return Number((item && item.threadReplyCount) || 0)
    }

    function threadReplyPreviewsForItem(item) {
        return (item && item.threadReplyPreviews) || []
    }

    function byteSizeLabel(byteLen) {
        var value = Number(byteLen || 0)
        if (value >= 1024 * 1024) {
            return (value / (1024 * 1024)).toFixed(value >= 10 * 1024 * 1024 ? 0 : 1) + " MB"
        }
        if (value >= 1024) {
            return (value / 1024).toFixed(value >= 10 * 1024 ? 0 : 1) + " KB"
        }
        return String(value) + " B"
    }

    function attachmentDetailLabel(attachment) {
        return String((attachment && attachment.mediaType) || "application/octet-stream")
            + " | "
            + root.byteSizeLabel(Number((attachment && attachment.byteLen) || 0))
            + ((attachment && attachment.localBlobAvailable === false) ? " | missing locally" : "")
    }

    function timelineItemKey(item) {
        if (!item) {
            return ""
        }
        var messageId = String(item.messageId || "")
        return messageId.length > 0 ? messageId : String(item.eventId || "")
    }

    function currentInspectorItem() {
        var key = String(root.inspectorItemKey || "")
        if (key.length > 0) {
            for (var i = 0; i < root.selectedTimeline.length; i += 1) {
                if (root.timelineItemKey(root.selectedTimeline[i]) === key) {
                    return root.selectedTimeline[i]
                }
            }
        }
        return root.selectedTimeline.length > 0 ? root.selectedTimeline[root.selectedTimeline.length - 1] : ({})
    }

    function selectInspectorItem(item) {
        var key = root.timelineItemKey(item)
        if (root.runtimeSearchReady) {
            var itemChannelId = String((item && item.channelId) || "")
            if (root.hasChannelId(itemChannelId) && itemChannelId !== String(root.selectedChannel.channelId || "")) {
                root.selectChannelId(itemChannelId, false)
            }
        }
        root.inspectorItemKey = key
    }

    function countMissingHistoryGaps(items) {
        var count = 0
        for (var i = 0; i < items.length; i += 1) {
            if (items[i].kind === "missing_history_gap") {
                count += 1
            }
        }
        return count
    }

    function countInvalidSignatures(items) {
        var count = 0
        for (var i = 0; i < items.length; i += 1) {
            if (items[i].kind === "invalid_signature") {
                count += 1
            }
        }
        return count
    }

    function attachmentsFromTimeline(items, limit) {
        items = items || []
        var result = []
        var maxItems = Number(limit === undefined ? -1 : limit)
        if (!isFinite(maxItems)) {
            maxItems = -1
        }
        maxItems = Math.floor(maxItems)
        if (maxItems === 0) {
            return result
        }
        for (var i = items.length - 1; i >= 0; i -= 1) {
            var item = items[i]
            var attachments = item.attachments || []
            for (var j = 0; j < attachments.length; j += 1) {
                var attachment = attachments[j]
                result.push({
                    blobHash: String(attachment.blobHash || ""),
                    attachmentId: String(attachment.attachmentId || ""),
                    mediaType: String(attachment.mediaType || "application/octet-stream"),
                    byteLen: Number(attachment.byteLen || 0),
                    displayName: String(attachment.displayName || "attachment"),
                    encrypted: Boolean(attachment.encrypted),
                    localBlobAvailable: attachment.localBlobAvailable,
                    messageId: String(item.messageId || ""),
                    eventId: String(item.eventId || ""),
                    authorDisplayName: String(item.authorDisplayName || ""),
                    authorDeviceId: String(item.authorDeviceId || "")
                })
                if (maxItems > 0 && result.length >= maxItems) {
                    return result
                }
            }
        }
        return result
    }

    function reactionTotal(reactions) {
        var total = 0
        var source = reactions || {}
        for (var key in source) {
            if (Object.prototype.hasOwnProperty.call(source, key)) {
                total += Number(source[key] || 0)
            }
        }
        return total
    }

    function reactionKeyCount(reactions) {
        var count = 0
        var source = reactions || {}
        for (var key in source) {
            if (Object.prototype.hasOwnProperty.call(source, key)) {
                count += 1
            }
        }
        return count
    }

    function reactionDistinctCountForItem(item) {
        var visible = root.reactionKeyCount((item && item.reactions) || {})
        var count = Number((item && item.reactionCount) === undefined
            ? visible
            : item.reactionCount)
        if (!isFinite(count)) {
            count = visible
        }
        return Math.max(visible, Math.max(0, count))
    }

    function itemAuthorLabel(item) {
        var displayName = String(item.authorDisplayName || "").trim()
        if (displayName.length > 0) {
            return displayName
        }
        return String(item.authorDeviceId || "")
    }

    function timelineKindLabel(item) {
        if (!item || !item.kind) {
            return ""
        }
        if (item.kind === "missing_history_gap") {
            return "History gap"
        }
        if (item.kind === "invalid_signature") {
            return "Failed signature"
        }
        if (Boolean(item.deleted)) {
            return "Deleted"
        }
        if (Boolean(item.bodyTruncated)) {
            return Boolean(item.encrypted) ? "Encrypted preview" : "Preview"
        }
        return Boolean(item.encrypted) ? "Encrypted" : "Plain"
    }

    function timelineItemInSelectedChannel(item) {
        if (!item) {
            return false
        }
        if (item.kind === "missing_history_gap") {
            return true
        }
        if (item.kind === "invalid_signature") {
            var channelId = String(item.channelId || "")
            return channelId.length === 0 || channelId === root.selectedChannel.channelId
        }
        return item.channelId === root.selectedChannel.channelId
    }

    function timelineItemMatchesSearch(item) {
        if (root.normalizedSearchQuery.length === 0) {
            return true
        }

        if (!root.timelineItemInSelectedChannel(item)) {
            return false
        }

        var haystack = [
            item.body || "",
            item.authorDisplayName || "",
            item.authorDeviceId || "",
            item.eventId || "",
            item.channelId || "",
            (item.missingParentIds || []).join(" ")
        ].join(" ").toLowerCase()
        return haystack.indexOf(root.normalizedSearchQuery) !== -1
    }

    function channelMatchesSearch(channel) {
        return String(channel.name || "").toLowerCase().indexOf(root.normalizedSearchQuery) !== -1
            || String(channel.channelId || "").toLowerCase().indexOf(root.normalizedSearchQuery) !== -1
    }

    function filteredChannelRows() {
        if (root.normalizedSearchQuery.length === 0) {
            return channels
        }

        var rows = []
        var seen = ({})
        for (var i = 0; i < channels.length; i += 1) {
            var localChannel = channels[i]
            var localChannelId = String(localChannel.channelId || "")
            if (localChannelId.length > 0 && root.channelMatchesSearch(localChannel)) {
                seen[localChannelId] = true
                rows.push(localChannel)
            }
        }

        if (root.channelSearchReady) {
            var results = chaftController.channelSearchResults || []
            for (var j = 0; j < results.length; j += 1) {
                var result = results[j]
                var resultChannelId = String(result.channelId || "")
                if (resultChannelId.length > 0 && !seen[resultChannelId]) {
                    seen[resultChannelId] = true
                    rows.push(result)
                }
            }
        }
        return rows
    }

    function loadedChannelById(channelId) {
        var normalizedChannelId = String(channelId || "")
        for (var i = 0; i < channels.length; i += 1) {
            if (String(channels[i].channelId || "") === normalizedChannelId) {
                return channels[i]
            }
        }
        return ({})
    }

    function resolvedChannelById(channelId) {
        var normalizedChannelId = String(channelId || "")
        if (normalizedChannelId.length === 0) {
            return ({})
        }
        var resolved = root.resolvedChannels[normalizedChannelId] || ({})
        return String(resolved.channelId || "") === normalizedChannelId ? resolved : ({})
    }

    function searchChannelById(channelId) {
        var normalizedChannelId = String(channelId || "")
        var hits = chaftController.messageSearchHits || []
        for (var i = 0; i < hits.length; i += 1) {
            var hit = hits[i]
            if (String(hit.channelId || "") === normalizedChannelId) {
                var name = String(hit.channelName || "").trim()
                return ({
                    channelId: normalizedChannelId,
                    name: name.length > 0 ? name : "Loading",
                    isPrivate: Boolean(hit.channelIsPrivate),
                    unreadCount: 0
                })
            }
        }
        return ({})
    }

    function channelSearchResultById(channelId) {
        var normalizedChannelId = String(channelId || "")
        if (!root.channelSearchReady) {
            return ({})
        }
        var results = chaftController.channelSearchResults || []
        for (var i = 0; i < results.length; i += 1) {
            if (String(results[i].channelId || "") === normalizedChannelId) {
                return results[i]
            }
        }
        return ({})
    }

    function channelById(channelId) {
        var normalizedChannelId = String(channelId || "")
        var loaded = root.loadedChannelById(normalizedChannelId)
        if (String(loaded.channelId || "") === normalizedChannelId) {
            return loaded
        }
        var resolved = root.resolvedChannelById(normalizedChannelId)
        if (String(resolved.channelId || "") === normalizedChannelId) {
            return resolved
        }
        var searchChannel = root.searchChannelById(normalizedChannelId)
        if (String(searchChannel.channelId || "") === normalizedChannelId) {
            return searchChannel
        }
        var channelSearchResult = root.channelSearchResultById(normalizedChannelId)
        if (String(channelSearchResult.channelId || "") === normalizedChannelId) {
            return channelSearchResult
        }
        if (normalizedChannelId.length > 0 && root.channelCount > channels.length) {
            return ({ channelId: normalizedChannelId, name: "Loading", isPrivate: false, unreadCount: 0 })
        }
        return channels.length > 0
            ? channels[0]
            : ({ channelId: "", name: "general", isPrivate: false, unreadCount: 0 })
    }

    function loadedChannelId(channelId) {
        var normalizedChannelId = String(channelId || "")
        return String(root.loadedChannelById(normalizedChannelId).channelId || "") === normalizedChannelId
    }

    function hasChannelId(channelId) {
        var normalizedChannelId = String(channelId || "")
        return normalizedChannelId.length > 0
            && (root.loadedChannelId(normalizedChannelId)
                || String(root.resolvedChannelById(normalizedChannelId).channelId || "") === normalizedChannelId
                || String(root.searchChannelById(normalizedChannelId).channelId || "") === normalizedChannelId
                || String(root.channelSearchResultById(normalizedChannelId).channelId || "") === normalizedChannelId)
    }

    function requestChannelPageForId(channelId) {
        var normalizedChannelId = String(channelId || "")
        if (normalizedChannelId.length === 0
                || root.loadedChannelId(normalizedChannelId)
                || !root.runtimeWorkReady) {
            return false
        }
        if (chaftController.loadChannelPageContaining(normalizedChannelId)) {
            return true
        }
        return chaftController.loadMoreChannels()
    }

    function requestChannelTimelineForId(channelId) {
        var normalizedChannelId = String(channelId || "")
        if (normalizedChannelId.length === 0
                || !root.runtimeWorkReady
                || root.normalizedSearchQuery.length > 0
                || root.timelineChannelId === normalizedChannelId) {
            return false
        }
        return chaftController.loadChannelTimelineLatest(normalizedChannelId)
    }

    function requestSelectedChannelTimelineIfNeeded() {
        var channelId = String(root.selectedChannel.channelId || root.selectedChannelId || "")
        return root.requestChannelTimelineForId(channelId)
    }

    function ensureSelectedChannelInSnapshot() {
        if (root.channels.length === 0 || root.hasChannelId(root.selectedChannelId)) {
            return false
        }
        if (String(root.selectedChannelId || "").length > 0
                && root.channelCount > root.channels.length
                && root.runtimeWorkReady) {
            root.requestChannelPageForId(root.selectedChannelId)
            return false
        }
        root.selectedChannelId = String(root.channels[0].channelId || "")
        return root.selectedChannelId.length > 0
    }

    function channelNameForId(channelId) {
        var normalizedChannelId = String(channelId || "")
        var loaded = root.loadedChannelById(normalizedChannelId)
        if (String(loaded.channelId || "") === normalizedChannelId) {
            return String(loaded.name || "")
        }
        var resolved = root.resolvedChannelById(normalizedChannelId)
        if (String(resolved.channelId || "") === normalizedChannelId) {
            return String(resolved.name || "")
        }
        var searchChannel = root.searchChannelById(normalizedChannelId)
        if (String(searchChannel.channelId || "") === normalizedChannelId) {
            return String(searchChannel.name || "")
        }
        var channelSearchResult = root.channelSearchResultById(normalizedChannelId)
        if (String(channelSearchResult.channelId || "") === normalizedChannelId) {
            return String(channelSearchResult.name || "")
        }
        return ""
    }

    function searchTimelineItemWithChannelName(item) {
        var row = {}
        var source = item || {}
        for (var key in source) {
            if (Object.prototype.hasOwnProperty.call(source, key)) {
                row[key] = source[key]
            }
        }

        var channelName = root.channelNameForId(row.channelId)
        if (channelName.length > 0) {
            row.channelName = channelName
        }
        return row
    }

    function messageSearchHitCountLabel() {
        var count = Number(chaftController.messageSearchHitCount || 0)
        if (count <= 0) {
            count = root.runtimeSearchTimeline.length
        }
        var suffix = count > 0 && Boolean(chaftController.messageSearchHasMoreHits) ? "+" : ""
        return String(count) + suffix
    }

    function localDeviceDisplayName() {
        for (var i = 0; i < root.profiles.length; i += 1) {
            if (root.profiles[i].deviceId === chaftController.deviceId) {
                return String(root.profiles[i].displayName || "")
            }
        }
        return ""
    }

    function memberLabel(member) {
        var displayName = String(member.displayName || "").trim()
        if (displayName.length > 0) {
            return displayName
        }
        return String(member.deviceId || "")
    }

    function memberInitial(member) {
        var label = root.memberLabel(member)
        return label.length > 0 ? label.slice(0, 1).toUpperCase() : "?"
    }

    function roleLabel(role) {
        var value = String(role || "")
        return value.length > 0 ? value.slice(0, 1).toUpperCase() + value.slice(1) : "Member"
    }

    function isOpenMlsKeyPackage(keyPackage) {
        return String(keyPackage.protocol || "").indexOf("openmls/key-package") === 0
    }

    function workspaceInitial(workspace) {
        var name = String(workspace.name || workspace.workspaceId || "C").trim()
        return name.length > 0 ? name.slice(0, 1).toUpperCase() : "C"
    }

    function workspaceRailItemsForSummaries(summaries) {
        var items = []
        for (var i = 0; i < summaries.length; i += 1) {
            items.push(summaries[i])
        }

        var selectedId = String(chaftController.selectedWorkspaceId
            || root.workspaceSnapshot.workspaceId || "")
        if (selectedId.length === 0) {
            return items
        }

        for (var index = 0; index < items.length; index += 1) {
            if (String(items[index].workspaceId || "") === selectedId) {
                return items
            }
        }

        items.push({
            workspaceId: selectedId,
            name: root.workspaceSnapshot.name || selectedId
        })
        return items
    }

    function openWorkspaceEntry(mode) {
        root.workspaceEntryMode = mode === "create" ? "create" : "join"
        workspaceEntryDialog.open()
    }

    function resetWorkspaceEntryForm() {
        workspaceCreateNameField.text = ""
        workspaceCreateChannelField.text = "general"
        workspaceCredentialsField.text = ""
        workspaceRecoveryPassphraseField.text = ""
        workspacePeerEndpointField.text = chaftController.defaultPeerEndpoint
    }

    function parsedCredentialObject(credentials) {
        try {
            var parsed = JSON.parse(String(credentials || ""))
            return parsed && typeof parsed === "object" && !Array.isArray(parsed)
                ? parsed
                : null
        } catch (error) {
            return null
        }
    }

    function credentialJsonForImport(credentials, passphrase) {
        var parsed = root.parsedCredentialObject(credentials)
        if (parsed === null) {
            return credentials
        }
        if (String(parsed.kind || "") === "chaft.workspace-invite.v1"
                && parsed.workspaceKey !== undefined) {
            return JSON.stringify(parsed.workspaceKey)
        }
        if (parsed.recoveryBundle !== undefined
                && String(passphrase || "").trim().length > 0) {
            return JSON.stringify(parsed.recoveryBundle)
        }
        if (parsed.workspaceKey !== undefined
                && String(passphrase || "").trim().length === 0) {
            return JSON.stringify(parsed.workspaceKey)
        }
        return credentials
    }

    function credentialUsesWorkspaceKey(credentials) {
        var parsed = root.parsedCredentialObject(credentials)
        return parsed !== null && parsed.workspaceKey !== undefined
    }

    function credentialPeerEndpoint(credentials) {
        var parsed = root.parsedCredentialObject(credentials)
        if (parsed === null) {
            return ""
        }
        return String(parsed.peerEndpoint || "").trim()
    }

    function submitWorkspaceCreate() {
        if (!root.runtimeAccessReady) {
            return false
        }
        if (chaftController.createWorkspace(
                    workspaceCreateNameField.text,
                    workspaceCreateChannelField.text)) {
            chaftController.clearKeyTransferJson()
            root.pendingPostCreateExport = true
            workspaceEntryDialog.close()
            return true
        }
        return false
    }

    function submitWorkspaceJoin() {
        if (!root.runtimeAccessReady) {
            return false
        }
        var credentials = workspaceCredentialsField.text.trim()
        if (credentials.length === 0) {
            return false
        }
        var packagePeerEndpoint = root.credentialPeerEndpoint(credentials)
        var peerEndpoint = workspacePeerEndpointField.text.trim()
        if (peerEndpoint.length === 0 && packagePeerEndpoint.length > 0) {
            peerEndpoint = packagePeerEndpoint
            workspacePeerEndpointField.text = peerEndpoint
        }
        if (peerEndpoint.length > 0) {
            chaftController.defaultPeerEndpoint = peerEndpoint
        }
        var passphrase = workspaceRecoveryPassphraseField.text.trim()
        var credentialJson = root.credentialJsonForImport(credentials, passphrase)
        var accepted = passphrase.length > 0
                && !root.credentialUsesWorkspaceKey(credentials)
            ? chaftController.importRecoveryBundle(credentialJson, passphrase)
            : chaftController.importWorkspaceKey(credentialJson)
        if (accepted) {
            workspaceEntryDialog.close()
        }
        return accepted
    }

    function selectedChannelIndex() {
        var selectedId = String(root.selectedChannel.channelId || "")
        for (var i = 0; i < root.channels.length; i += 1) {
            if (root.channels[i].channelId === selectedId) {
                return i
            }
        }
        return root.channels.length > 0 ? 0 : -1
    }

    function selectChannelAtIndex(channelIndex, focusDraft) {
        if (root.channels.length === 0) {
            return false
        }
        var nextIndex = Math.max(0, Math.min(root.channels.length - 1, channelIndex))
        return root.selectChannelId(root.channels[nextIndex].channelId, focusDraft)
    }

    function selectChannelId(channelId, focusDraft) {
        var normalizedChannelId = String(channelId || "")
        if (!root.hasChannelId(normalizedChannelId)
                && !(normalizedChannelId.length > 0
                    && root.channelCount > root.channels.length
                    && chaftController.hasRuntimeWorkspace)) {
            return false
        }
        root.saveCurrentDraft()
        if (root.selectedChannelId !== normalizedChannelId) {
            root.selectedChannelId = normalizedChannelId
            root.cancelReplyMessage()
        } else if (focusDraft) {
            root.focusComposer()
        }
        root.requestChannelPageForId(normalizedChannelId)
        root.requestChannelTimelineForId(normalizedChannelId)
        return true
    }

    function selectChannelByOffset(offset) {
        var currentIndex = root.selectedChannelIndex()
        if (currentIndex < 0) {
            return false
        }
        return root.selectChannelAtIndex(currentIndex + offset, true)
    }

    function selectedWorkspaceIndex() {
        var selectedId = String(chaftController.selectedWorkspaceId || "")
        var items = root.workspaceRailItems || []
        for (var i = 0; i < items.length; i += 1) {
            if (String(items[i].workspaceId || "") === selectedId) {
                return i
            }
        }
        return items.length > 0 ? 0 : -1
    }

    function selectWorkspaceAtIndex(workspaceIndex) {
        var items = root.workspaceRailItems || []
        if (items.length === 0) {
            return false
        }
        var nextIndex = Math.max(0, Math.min(items.length - 1, workspaceIndex))
        return root.selectWorkspaceId(items[nextIndex].workspaceId)
    }

    function selectWorkspaceId(workspaceId) {
        var normalizedWorkspaceId = String(workspaceId || "")
        if (normalizedWorkspaceId.length === 0) {
            return false
        }
        root.saveCurrentDraft()
        if (root.editingMessageId.length > 0) {
            root.editingMessageId = ""
        }
        root.cancelReplyMessage()
        root.pendingDraftRestoreWorkspaceId = normalizedWorkspaceId
        if (!chaftController.selectWorkspace(normalizedWorkspaceId)) {
            root.pendingDraftRestoreWorkspaceId = ""
            return false
        }
        return true
    }

    function selectWorkspaceByOffset(offset) {
        var currentIndex = root.selectedWorkspaceIndex()
        if (currentIndex < 0) {
            return false
        }
        return root.selectWorkspaceAtIndex(currentIndex + offset)
    }

    function focusSearch() {
        searchField.forceActiveFocus()
        searchField.selectAll()
    }

    function focusComposer() {
        composer.focusDraft()
    }

    function resetTimelineForChannelContext() {
        if (root.normalizedSearchQuery.length === 0) {
            if (Number(root.selectedChannel.unreadCount || 0) > 0) {
                timelineView.resetToUnreadOnNextModel()
            } else {
                timelineView.resetToLatestOnNextModel()
            }
        }
    }

    function activateSearchResult() {
        if (root.normalizedSearchQuery.length > 0 && root.runtimeSearchReady && root.runtimeSearchTimeline.length > 0) {
            root.selectInspectorItem(root.runtimeSearchTimeline[0])
            root.focusComposer()
            return true
        }
        if (root.normalizedSearchQuery.length > 0 && root.filteredChannels.length > 0) {
            root.selectChannelId(root.filteredChannels[0].channelId, false)
            searchField.text = ""
            root.searchQuery = ""
            root.focusComposer()
            return true
        }
        root.focusComposer()
        return false
    }

    function dismissFocusedState() {
        if (composer.cancelEdit()) {
            return true
        }
        if (root.cancelReplyMessage()) {
            root.focusComposer()
            return true
        }
        if (root.searchQuery.length > 0) {
            searchField.text = ""
            root.searchQuery = ""
            root.focusComposer()
            return true
        }
        if (root.setupPanelOpen) {
            root.setupPanelOpen = false
            root.focusComposer()
            return true
        }
        return false
    }

    function scheduleMarkSelectedChannelRead() {
        if (!root.runtimeWorkReady) {
            return
        }
        if (String(root.selectedChannel.channelId || "").length === 0) {
            return
        }
        markReadDebounce.restart()
    }

    function markSelectedChannelRead() {
        var channelId = String(root.selectedChannel.channelId || "")
        if (root.runtimeWorkReady && channelId.length > 0) {
            chaftController.markChannelRead(channelId)
        }
    }

    function beginEditMessage(messageId, body) {
        var normalizedMessageId = String(messageId || "")
        if (normalizedMessageId.length === 0) {
            return
        }
        root.saveCurrentDraft()
        root.cancelReplyMessage()
        root.editingMessageId = normalizedMessageId
        composer.setDraft(String(body || ""))
    }

    function cancelEditMessage() {
        root.editingMessageId = ""
        root.restoreSelectedDraft(false)
    }

    function beginReplyMessage(item) {
        var messageId = String((item && item.messageId) || "")
        var channelId = String((item && item.channelId) || "")
        if (messageId.length === 0 || channelId !== String(root.selectedChannel.channelId || "")) {
            return false
        }
        if (root.editingMessageId.length > 0) {
            root.cancelEditMessage()
        }
        root.replyTarget = item
        root.focusComposer()
        return true
    }

    function cancelReplyMessage() {
        if (root.replyTargetMessageId.length === 0) {
            return false
        }
        root.replyTarget = ({})
        return true
    }

    function localPathFromUrl(fileUrl) {
        var value = String(fileUrl || "")
        if (value.indexOf("file://") === 0) {
            value = decodeURIComponent(value.slice(7))
        }
        if (Qt.platform.os === "windows" && value.charAt(0) === "/") {
            value = value.slice(1)
        }
        return value
    }

    function copyTextToClipboard(text, label) {
        return chaftController.copyText(String(text || ""), String(label || "text"))
    }

    function openSaveKeyTransferDialog() {
        if (chaftController.keyTransferJson.length === 0) {
            return false
        }
        saveKeyTransferDialog.open()
        return true
    }

    function inspectorBodyCopyText() {
        return String(root.inspectorItem.body || "").trim()
    }

    function copyInspectorBody() {
        return root.copyTextToClipboard(
            root.inspectorBodyCopyText(),
            Boolean(root.inspectorItem.bodyTruncated) ? "message preview" : "message text"
        )
    }

    function copyInspectorEventId() {
        return root.copyTextToClipboard(root.inspectorItem.eventId || "", "event ID")
    }

    function copyInspectorMessageId() {
        return root.copyTextToClipboard(root.inspectorItem.messageId || "", "message ID")
    }

    function attachmentSelectorFor(attachment) {
        var attachmentId = String((attachment && attachment.attachmentId) || "")
        if (attachmentId.length > 0) {
            return attachmentId
        }
        return String((attachment && attachment.blobHash) || "")
    }

    function copyAttachmentSelector(attachment) {
        var selector = root.attachmentSelectorFor(attachment)
        if (selector.length === 0) {
            return false
        }
        var label = String((attachment && attachment.attachmentId) || "").length > 0
            ? "attachment ID"
            : "attachment blob hash"
        return root.copyTextToClipboard(selector, label)
    }

    function attachmentPreviewsForItem(item) {
        var attachments = (item && item.attachments) || []
        return attachments.slice(0, root.inspectorAttachmentPreviewLimit)
    }

    function attachmentTotalCountForItem(item) {
        var attachments = (item && item.attachments) || []
        var count = Number((item && item.attachmentCount) === undefined
            ? attachments.length
            : item.attachmentCount)
        if (!isFinite(count)) {
            count = attachments.length
        }
        return Math.max(attachments.length, Math.max(0, count))
    }

    function attachmentOverflowLabelForItem(item) {
        var attachments = (item && item.attachments) || []
        var hidden = root.attachmentTotalCountForItem(item)
            - Math.min(attachments.length, root.inspectorAttachmentPreviewLimit)
        if (hidden <= 0) {
            return ""
        }
        return String(hidden) + (hidden === 1 ? " more file on message" : " more files on message")
    }

    function openSaveAttachmentDialog(messageId, attachment) {
        var selector = root.attachmentSelectorFor(attachment)
        if (selector.length === 0) {
            return false
        }
        saveAttachmentDialog.messageId = String(messageId || "")
        saveAttachmentDialog.attachmentSelector = selector
        saveAttachmentDialog.displayName = String((attachment && attachment.displayName) || "attachment")
        saveAttachmentDialog.open()
        return true
    }

    function syncSelectedPeerIfReady() {
        if (!root.autoSyncEnabled || !root.runtimeWorkReady || chaftController.syncInFlight) {
            return
        }
        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length > 0) {
            chaftController.syncWorkspaceIfIdle(endpoint)
        }
    }

    function repairHistoryFromPeer() {
        if (!root.runtimeWorkReady || chaftController.syncInFlight) {
            return false
        }
        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length === 0) {
            return false
        }
        return chaftController.pullWorkspace(endpoint)
    }

    function syncWorkspaceFromPreferredPeer() {
        var endpoint = root.preferredSyncPeerEndpoint()
        return root.runtimeWorkReady && endpoint.length > 0 && chaftController.syncWorkspace(endpoint)
    }

    function publishWorkspaceToPreferredPeer() {
        var endpoint = root.preferredSyncPeerEndpoint()
        return root.runtimeWorkReady && endpoint.length > 0 && chaftController.publishWorkspace(endpoint)
    }

    function backupWorkspaceToPreferredPeer() {
        var endpoint = root.preferredManualBackupPeerEndpoint()
        return root.runtimeWorkReady && endpoint.length > 0 && chaftController.backupWorkspace(endpoint)
    }

    function pullWorkspaceFromPreferredPeer() {
        var endpoint = root.preferredSyncPeerEndpoint()
        return root.runtimeWorkReady && endpoint.length > 0 && chaftController.pullWorkspace(endpoint)
    }

    function retryBlobTransfersWithPreferredPeers() {
        var endpoint = root.preferredRetryPeerEndpoint()
        return root.runtimeWorkReady && chaftController.retryBlobTransfers(endpoint)
    }

    function repairStorageMetadata() {
        return root.runtimeWorkReady
            && root.storageMetadataRepairSuggested
            && chaftController.repairWorkspaceStorageMetadata()
    }

    function publishEventWithTrustSnapshotToPreferredPeer(eventId) {
        var endpoint = root.preferredSyncPeerEndpoint()
        return root.runtimeWorkReady
            && endpoint.length > 0
            && chaftController.publishEventWithTrustSnapshot(eventId, endpoint)
    }

    function backupConfiguredPeerIfReady() {
        var backupPeers = chaftController.backupPeerEndpoints || []
        if (!root.autoBackupEnabled || !root.runtimeWorkReady
                || chaftController.syncInFlight || !root.hasAutoBackupTargets) {
            return
        }
        if (backupPeers.length > 0 && chaftController.backupConfiguredPeersIfIdle()) {
            return
        }

        var discoveredBackup = root.preferredBackupPeerEndpoint()
        if (discoveredBackup.length > 0) {
            chaftController.backupWorkspaceIfIdle(discoveredBackup)
        }
    }

    onSelectedChannelIdChanged: {
        if (root.editingMessageId.length > 0) {
            root.editingMessageId = ""
        }
        root.inspectorItemKey = ""
        root.restoreSelectedDraft(false)
        root.resetTimelineForChannelContext()
        root.requestSelectedChannelTimelineIfNeeded()
        root.scheduleMarkSelectedChannelRead()
    }
    onNormalizedSearchQueryChanged: {
        if (root.normalizedSearchQuery.length > 0) {
            timelineView.resetToBeginningOnNextModel()
        } else {
            root.resetTimelineForChannelContext()
            root.requestSelectedChannelTimelineIfNeeded()
        }
    }
    Component.onCompleted: {
        root.ensureSelectedChannelInSnapshot()
        root.restoreSelectedDraft(false)
        root.resetTimelineForChannelContext()
        root.requestSelectedChannelTimelineIfNeeded()
        root.scheduleMarkSelectedChannelRead()
        root.applySmokeUiState()
    }
    onAutoSyncEnabledChanged: {
        if (autoSyncEnabled) {
            root.syncSelectedPeerIfReady()
        }
    }
    onAutoBackupEnabledChanged: {
        chaftController.autoBackupEnabled = autoBackupEnabled
        if (autoBackupEnabled) {
            root.backupConfiguredPeerIfReady()
        }
    }
    onHasAutoBackupTargetsChanged: {
        if (!root.hasAutoBackupTargets) {
            root.autoBackupEnabled = false
        } else if (root.autoBackupEnabled) {
            root.backupConfiguredPeerIfReady()
        }
    }
    onRuntimeWorkReadyChanged: {
        if (!runtimeWorkReady) {
            searchDebounce.stop()
            markReadDebounce.stop()
            return
        }
        root.requestSelectedChannelTimelineIfNeeded()
        root.scheduleMarkSelectedChannelRead()
        if (root.searchHasTerms) {
            searchDebounce.restart()
        }
        if (root.autoSyncEnabled) {
            root.syncSelectedPeerIfReady()
        }
        if (root.autoBackupEnabled) {
            root.backupConfiguredPeerIfReady()
        }
    }

    Timer {
        id: searchDebounce
        interval: 120
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady) {
                return
            }
            if (root.searchHasTerms) {
                chaftController.searchWorkspaceMessages(root.searchQuery)
                chaftController.searchWorkspaceChannels(root.searchQuery)
            } else {
                chaftController.searchWorkspaceMessages("")
                chaftController.searchWorkspaceChannels("")
            }
        }
    }

    Timer {
        id: markReadDebounce
        interval: 900
        repeat: false
        onTriggered: root.markSelectedChannelRead()
    }

    Timer {
        id: autoSyncTimer
        interval: 3000
        repeat: true
        running: root.autoSyncEnabled && root.runtimeWorkReady
        onTriggered: root.syncSelectedPeerIfReady()
    }

    Timer {
        id: autoBackupTimer
        interval: 15000
        repeat: true
        running: root.autoBackupEnabled && root.runtimeWorkReady
            && root.hasAutoBackupTargets
        onTriggered: root.backupConfiguredPeerIfReady()
    }

    Timer {
        id: autoBackupDebounce
        interval: 1200
        repeat: false
        onTriggered: root.backupConfiguredPeerIfReady()
    }

    Timer {
        id: hostedPeerHintRefreshTimer
        interval: 300000
        repeat: true
        running: chaftController.peerHosting && root.runtimeWorkReady
        onTriggered: chaftController.refreshHostedPeerEndpointHint()
    }

    Shortcut {
        sequences: ["Ctrl+K", "Meta+K"]
        context: Qt.ApplicationShortcut
        onActivated: commandPalette.open()
    }

    Shortcut {
        sequences: ["Ctrl+F", "Meta+F"]
        context: Qt.ApplicationShortcut
        enabled: root.hasWorkspaceContent
        onActivated: root.focusSearch()
    }

    Shortcut {
        sequences: ["Ctrl+/", "Meta+/"]
        context: Qt.ApplicationShortcut
        onActivated: shortcutOverlay.open()
    }

    Shortcut {
        sequences: ["Ctrl+M", "Meta+M"]
        context: Qt.ApplicationShortcut
        enabled: root.runtimeWorkReady
        onActivated: root.focusComposer()
    }

    Shortcut {
        sequence: "Alt+Up"
        context: Qt.ApplicationShortcut
        enabled: root.hasWorkspaceContent
        onActivated: root.selectChannelByOffset(-1)
    }

    Shortcut {
        sequence: "Alt+Down"
        context: Qt.ApplicationShortcut
        enabled: root.hasWorkspaceContent
        onActivated: root.selectChannelByOffset(1)
    }

    Shortcut {
        sequence: "Alt+Left"
        context: Qt.ApplicationShortcut
        onActivated: root.selectWorkspaceByOffset(-1)
    }

    Shortcut {
        sequence: "Alt+Right"
        context: Qt.ApplicationShortcut
        onActivated: root.selectWorkspaceByOffset(1)
    }

    Shortcut {
        sequences: ["Ctrl+O", "Meta+O"]
        context: Qt.ApplicationShortcut
        enabled: root.runtimeWorkReady && root.editingMessageId.length === 0
        onActivated: composer.attachDraft()
    }

    Shortcut {
        sequence: "Alt+Home"
        context: Qt.ApplicationShortcut
        enabled: root.hasWorkspaceContent
        onActivated: timelineView.scrollToOldest()
    }

    Shortcut {
        sequence: "Alt+End"
        context: Qt.ApplicationShortcut
        enabled: root.hasWorkspaceContent
        onActivated: timelineView.scrollToLatest()
    }

    Shortcut {
        sequences: ["Ctrl+Shift+C", "Meta+Shift+C"]
        context: Qt.ApplicationShortcut
        enabled: root.inspectorBodyCopyText().length > 0
        onActivated: root.copyInspectorBody()
    }

    Shortcut {
        sequence: "Esc"
        context: Qt.ApplicationShortcut
        onActivated: root.dismissFocusedState()
    }

    Connections {
        target: chaftController
        function onWorkspaceSnapshotChanged() {
            var channelChanged = root.ensureSelectedChannelInSnapshot()
            if (root.pendingDraftRestoreWorkspaceId.length > 0
                    && root.channels.length > 0
                    && root.currentWorkspaceId() === root.pendingDraftRestoreWorkspaceId) {
                if (!channelChanged) {
                    root.restoreSelectedDraft(false)
                    root.resetTimelineForChannelContext()
                }
                root.pendingDraftRestoreWorkspaceId = ""
            }
            root.requestSelectedChannelTimelineIfNeeded()
            root.scheduleMarkSelectedChannelRead()
            if (root.autoBackupEnabled) {
                autoBackupDebounce.restart()
            }
            if (root.pendingPostCreateExport
                    && root.runtimeWorkReady
                    && root.currentWorkspaceId().length > 0) {
                root.pendingPostCreateExport = false
                postCreateExportDialog.open()
            }
        }
        function onSelectedWorkspaceChanged() {
            root.inspectorItemKey = ""
            root.searchQuery = ""
            searchField.text = ""
            timelineView.resetToLatestOnNextModel()
        }
        function onBackupPeerEndpointsChanged() {
            if (!root.hasAutoBackupTargets) {
                root.autoBackupEnabled = false
            } else if (root.autoBackupEnabled) {
                root.backupConfiguredPeerIfReady()
            }
        }
        function onAutoBackupEnabledChanged() {
            root.autoBackupEnabled = chaftController.autoBackupEnabled
        }
        function onSyncInFlightChanged() {
            if (!chaftController.syncInFlight) {
                root.requestSelectedChannelTimelineIfNeeded()
            }
        }
        function onRuntimeUnlockChanged() {
            if (chaftController.runtimeUnlockRequired) {
                root.runtimeUnlockDismissed = false
            }
            if (!chaftController.runtimeUnlocked) {
                root.inspectorItemKey = ""
                root.replyTarget = ({})
                root.editingMessageId = ""
                root.searchQuery = ""
                root.composerDrafts = ({})
                composer.clearDraft()
                attachmentDialog.pendingText = ""
                attachmentDialog.pendingWorkspaceId = ""
                attachmentDialog.pendingChannelId = ""
                attachmentDialog.pendingReplyToMessageId = ""
            }
        }
    }

    Dialog {
        id: workspaceEntryDialog
        modal: true
        width: Math.min(root.width - 48, 560)
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        title: root.workspaceEntryMode === "create" ? "Create workspace" : "Join workspace"

        ColumnLayout {
            anchors.fill: parent
            spacing: Tokens.space3

            TabBar {
                id: workspaceEntryTabs
                Layout.fillWidth: true
                currentIndex: root.workspaceEntryMode === "create" ? 1 : 0
                onCurrentIndexChanged: {
                    root.workspaceEntryMode = currentIndex === 1 ? "create" : "join"
                }

                TabButton {
                    text: "Join"
                    Accessible.name: "Join workspace"
                }

                TabButton {
                    text: "Create"
                    Accessible.name: "Create workspace"
                }
            }

            StackLayout {
                Layout.fillWidth: true
                currentIndex: workspaceEntryTabs.currentIndex

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: "Credentials JSON"
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        font.weight: Font.DemiBold
                    }

                    TextArea {
                        id: workspaceCredentialsField
                        Layout.fillWidth: true
                        Layout.preferredHeight: 156
                        placeholderText: "Paste invite, workspace key, or recovery bundle JSON"
                        Accessible.name: "Workspace credentials JSON"
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
                        wrapMode: TextEdit.WrapAnywhere
                        background: Rectangle {
                            radius: Tokens.radiusMd
                            color: Tokens.sidebarInput
                        }
                    }

                    TextField {
                        id: workspaceRecoveryPassphraseField
                        Layout.fillWidth: true
                        placeholderText: "Recovery passphrase"
                        Accessible.name: "Recovery passphrase"
                        echoMode: TextInput.Password
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
                        background: Rectangle {
                            radius: Tokens.radiusMd
                            color: Tokens.sidebarInput
                        }
                        onAccepted: root.submitWorkspaceJoin()
                    }

                    TextField {
                        id: workspacePeerEndpointField
                        Layout.fillWidth: true
                        placeholderText: "Peer endpoint"
                        Accessible.name: "Peer endpoint"
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
                        background: Rectangle {
                            radius: Tokens.radiusMd
                            color: Tokens.sidebarInput
                        }
                        onAccepted: root.submitWorkspaceJoin()
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Tokens.space2

                        Button {
                            Layout.fillWidth: true
                            text: "Cancel"
                            onClicked: workspaceEntryDialog.close()
                        }

                        Button {
                            Layout.fillWidth: true
                            text: "Join workspace"
                            enabled: root.runtimeAccessReady
                                && workspaceCredentialsField.text.trim().length > 0
                            onClicked: root.submitWorkspaceJoin()
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    TextField {
                        id: workspaceCreateNameField
                        Layout.fillWidth: true
                        placeholderText: "Workspace name"
                        Accessible.name: "Workspace name"
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
                        background: Rectangle {
                            radius: Tokens.radiusMd
                            color: Tokens.sidebarInput
                        }
                        onAccepted: root.submitWorkspaceCreate()
                    }

                    TextField {
                        id: workspaceCreateChannelField
                        Layout.fillWidth: true
                        text: "general"
                        placeholderText: "First channel"
                        Accessible.name: "First channel"
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
                        background: Rectangle {
                            radius: Tokens.radiusMd
                            color: Tokens.sidebarInput
                        }
                        onAccepted: root.submitWorkspaceCreate()
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Tokens.space2

                        Button {
                            Layout.fillWidth: true
                            text: "Cancel"
                            onClicked: workspaceEntryDialog.close()
                        }

                        Button {
                            Layout.fillWidth: true
                            text: "Create workspace"
                            enabled: root.runtimeAccessReady
                                && workspaceCreateNameField.text.trim().length > 0
                            onClicked: root.submitWorkspaceCreate()
                        }
                    }
                }
            }
        }

        onOpened: {
            workspacePeerEndpointField.text = chaftController.defaultPeerEndpoint
            if (root.workspaceEntryMode === "create") {
                workspaceCreateNameField.forceActiveFocus()
                workspaceCreateNameField.selectAll()
            } else {
                workspaceCredentialsField.forceActiveFocus()
            }
        }

        onClosed: root.resetWorkspaceEntryForm()
    }

    Dialog {
        id: postCreateExportDialog
        modal: true
        width: Math.min(root.width - 48, 560)
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        closePolicy: Popup.CloseOnEscape
        title: "Save workspace credentials"

        ColumnLayout {
            anchors.fill: parent
            spacing: Tokens.space3

            Text {
                Layout.fillWidth: true
                text: "Export recovery credentials before inviting other devices."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeSm
                wrapMode: Text.WordWrap
            }

            TextField {
                id: postCreateRecoveryPassphraseField
                Layout.fillWidth: true
                placeholderText: "Recovery passphrase"
                Accessible.name: "Recovery passphrase"
                echoMode: TextInput.Password
                color: Tokens.textStrong
                placeholderTextColor: Tokens.textMuted
                background: Rectangle {
                    radius: Tokens.radiusMd
                    color: Tokens.sidebarInput
                }
                onAccepted: {
                    if (text.trim().length > 0 && root.runtimeWorkReady) {
                        chaftController.exportRecoveryBundle(text)
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                Button {
                    Layout.fillWidth: true
                    text: "Export recovery"
                    enabled: root.runtimeWorkReady
                        && postCreateRecoveryPassphraseField.text.trim().length > 0
                        && !chaftController.keyTransferInFlight
                    onClicked: chaftController.exportRecoveryBundle(
                        postCreateRecoveryPassphraseField.text)
                }

                Button {
                    Layout.fillWidth: true
                    text: "Export workspace"
                    enabled: root.runtimeWorkReady
                        && !chaftController.keyTransferInFlight
                    onClicked: chaftController.exportWorkspaceKey()
                }
            }

            TextArea {
                Layout.fillWidth: true
                Layout.preferredHeight: 132
                visible: chaftController.keyTransferJson.length > 0
                readOnly: true
                text: chaftController.keyTransferJson
                Accessible.name: "Exported credentials JSON"
                color: Tokens.textStrong
                wrapMode: TextEdit.WrapAnywhere
                background: Rectangle {
                    radius: Tokens.radiusMd
                    color: Tokens.sidebarInput
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                Button {
                    Layout.fillWidth: true
                    text: "Copy JSON"
                    enabled: chaftController.keyTransferJson.length > 0
                    onClicked: root.copyTextToClipboard(
                        chaftController.keyTransferJson,
                        "credentials JSON")
                }

                Button {
                    Layout.fillWidth: true
                    text: "Save JSON"
                    enabled: chaftController.keyTransferJson.length > 0
                    onClicked: root.openSaveKeyTransferDialog()
                }

                Button {
                    Layout.fillWidth: true
                    text: "Done"
                    onClicked: postCreateExportDialog.close()
                }
            }
        }

        onOpened: postCreateRecoveryPassphraseField.forceActiveFocus()
        onClosed: postCreateRecoveryPassphraseField.text = ""
    }

    Dialog {
        id: runtimeUnlockDialog
        modal: true
        width: Math.min(root.width - 48, 420)
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        visible: chaftController.runtimeUnlockRequired && !root.runtimeUnlockDismissed
        closePolicy: Popup.NoAutoClose
        title: "Unlock runtime"

        ColumnLayout {
            anchors.fill: parent
            spacing: 10

            TextField {
                id: runtimePassphraseField
                Layout.fillWidth: true
                placeholderText: "Passphrase"
                echoMode: TextInput.Password
                color: Tokens.textStrong
                placeholderTextColor: Tokens.textMuted
                background: Rectangle {
                    radius: Tokens.radiusMd
                    color: Tokens.sidebarInput
                }
                onAccepted: {
                    if (chaftController.unlockRuntime(text)) {
                        text = ""
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Button {
                    Layout.fillWidth: true
                    text: "Later"
                    onClicked: {
                        runtimePassphraseField.text = ""
                        root.runtimeUnlockDismissed = true
                    }
                }

                Button {
                    Layout.fillWidth: true
                    text: "Unlock"
                    enabled: runtimePassphraseField.text.trim().length > 0
                    onClicked: {
                        if (chaftController.unlockRuntime(runtimePassphraseField.text)) {
                            runtimePassphraseField.text = ""
                        }
                    }
                }
            }
        }

        onVisibleChanged: {
            if (visible) {
                runtimePassphraseField.forceActiveFocus()
                runtimePassphraseField.selectAll()
            } else {
                runtimePassphraseField.text = ""
            }
        }
    }

    FileDialog {
        id: attachmentDialog
        property string pendingText: ""
        property string pendingWorkspaceId: ""
        property string pendingChannelId: ""
        property string pendingReplyToMessageId: ""
        title: "Attach file"
        fileMode: FileDialog.OpenFile
        onAccepted: {
            var filePath = root.localPathFromUrl(selectedFile)
            var sent = false
            if (pendingWorkspaceId === root.currentWorkspaceId()) {
                sent = pendingReplyToMessageId.length > 0
                    ? chaftController.sendAttachmentReply(
                        pendingChannelId,
                        pendingReplyToMessageId,
                        pendingText,
                        filePath,
                        ""
                    )
                    : chaftController.sendAttachment(pendingChannelId, pendingText, filePath, "")
            }
            if (sent) {
                root.clearDraftForWorkspaceChannel(pendingWorkspaceId, pendingChannelId)
                if (pendingWorkspaceId === root.currentWorkspaceId()
                        && pendingChannelId === root.selectedChannel.channelId) {
                    composer.clearDraft()
                    if (pendingReplyToMessageId === root.replyTargetMessageId) {
                        root.cancelReplyMessage()
                    }
                }
            }
            pendingText = ""
            pendingWorkspaceId = ""
            pendingChannelId = ""
            pendingReplyToMessageId = ""
        }
        onRejected: {
            pendingText = ""
            pendingWorkspaceId = ""
            pendingChannelId = ""
            pendingReplyToMessageId = ""
        }
    }

    FileDialog {
        id: saveAttachmentDialog
        property string messageId: ""
        property string attachmentSelector: ""
        property string displayName: ""
        title: displayName.length > 0 ? "Save " + displayName : "Save attachment"
        fileMode: FileDialog.SaveFile
        onAccepted: {
            var outputPath = root.localPathFromUrl(selectedFile)
            chaftController.saveAttachment(messageId, attachmentSelector, outputPath)
            messageId = ""
            attachmentSelector = ""
            displayName = ""
        }
        onRejected: {
            messageId = ""
            attachmentSelector = ""
            displayName = ""
        }
    }

    FileDialog {
        id: saveKeyTransferDialog
        title: "Save credentials JSON"
        fileMode: FileDialog.SaveFile
        onAccepted: chaftController.saveKeyTransferJson(root.localPathFromUrl(selectedFile))
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 72
            color: Tokens.rail

            Flickable {
                id: workspaceRailFlick
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: addWorkspaceButton.top
                anchors.topMargin: 14
                anchors.bottomMargin: 10
                clip: true
                contentWidth: width
                contentHeight: workspaceRailColumn.implicitHeight
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                }

                Column {
                    id: workspaceRailColumn
                    x: Math.round((workspaceRailFlick.width - width) / 2)
                    width: 40
                    spacing: 10

                    Repeater {
                        model: root.workspaceRailItems
                        delegate: WorkspaceRailItem {
                            id: workspaceRailDelegate
                            required property int index
                            required property var modelData

                            workspaceId: String(workspaceRailDelegate.modelData.workspaceId || "")
                            workspaceName: String(workspaceRailDelegate.modelData.name
                                || workspaceRailDelegate.modelData.workspaceId
                                || "Workspace")
                            initial: root.workspaceInitial(workspaceRailDelegate.modelData)
                            selected: String(workspaceRailDelegate.modelData.workspaceId || "") === chaftController.selectedWorkspaceId
                            actionable: root.runtimeAccessReady
                            // Workspace summaries may not carry unreadCount yet;
                            // degrade to no badge silently.
                            unreadCount: Number(workspaceRailDelegate.modelData.unreadCount || 0)
                            onActivated: function(workspaceId) {
                                root.selectWorkspaceId(workspaceId)
                            }
                        }
                    }
                }
            }

            Button {
                id: addWorkspaceButton
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 14
                width: 40
                height: 40
                text: "+"
                Accessible.name: "Add workspace"
                Accessible.description: "Join or create a workspace"
                enabled: root.runtimeAccessReady
                onClicked: root.openWorkspaceEntry("join")
                ToolTip.visible: hovered
                ToolTip.text: "Add workspace"
                background: Rectangle {
                    radius: Tokens.radiusMd
                    color: addWorkspaceButton.hovered ? Tokens.railElevated : Tokens.rail
                    border.width: 1
                    border.color: Tokens.borderSubtle
                }
                contentItem: Text {
                    text: addWorkspaceButton.text
                    color: Tokens.railText
                    font.pixelSize: Tokens.fontSizeXl
                    font.weight: Font.DemiBold
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
            }
        }

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 268
            color: Tokens.sidebar

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 12

                Text {
                    text: root.workspaceSnapshot.name || "Chaft"
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeXl
                    font.weight: Font.DemiBold
                }

                TextField {
                    id: searchField
                    Layout.fillWidth: true
                    visible: root.hasWorkspaceContent
                    placeholderText: "Search or jump"
                    Accessible.name: "Search or jump"
                    Accessible.description: root.searchHasTerms
                        ? "Search messages and channels"
                        : "Find messages, channels, or jump to a channel"
                    color: Tokens.textStrong
                    placeholderTextColor: Tokens.textMuted
                    onTextChanged: {
                        root.searchQuery = text
                        if (root.runtimeWorkReady) {
                            if (!chaftController.searchQueryHasTerms(text)) {
                                searchDebounce.stop()
                                chaftController.searchWorkspaceMessages("")
                                chaftController.searchWorkspaceChannels("")
                            } else {
                                searchDebounce.restart()
                            }
                        }
                    }
                    onAccepted: root.activateSearchResult()
                    background: Rectangle {
                        radius: Tokens.radiusMd
                        color: Tokens.sidebarInput
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    spacing: 6

                    Text {
                        Layout.fillWidth: true
                        text: "Channels"
                        color: Tokens.sidebarTextMuted
                        font.pixelSize: Tokens.fontSizeXs
                        font.weight: Font.DemiBold
                    }

                    Button {
                        id: newChannelButton
                        text: "+"
                        Accessible.name: "New channel"
                        Accessible.description: "Open the channel creation form"
                        implicitWidth: 30
                        enabled: root.runtimeWorkReady
                        onClicked: newChannelPopup.open()
                        ToolTip.visible: hovered
                        ToolTip.text: "New channel"
                    }

                    Popup {
                        id: newChannelPopup
                        parent: newChannelButton
                        y: newChannelButton.height + 4
                        x: newChannelButton.width - width
                        width: 232
                        padding: Tokens.space3
                        modal: true
                        focus: true
                        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
                        onOpened: channelNameField.forceActiveFocus()
                        onClosed: {
                            channelNameField.text = ""
                            privateChannelCheck.checked = false
                        }

                        function createFromForm() {
                            if (root.runtimeWorkReady
                                    && chaftController.createChannel(channelNameField.text, privateChannelCheck.checked)) {
                                newChannelPopup.close()
                            }
                        }

                        background: Rectangle {
                            radius: Tokens.radiusMd
                            color: Tokens.surfaceRaised
                            border.width: 1
                            border.color: Tokens.borderSubtle
                        }

                        contentItem: ColumnLayout {
                            spacing: Tokens.space2

                            Text {
                                text: "Channel name"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                            }

                            TextField {
                                id: channelNameField
                                Layout.fillWidth: true
                                placeholderText: "e.g. launch-plan"
                                Accessible.name: "Channel name"
                                onAccepted: newChannelPopup.createFromForm()
                            }

                            CheckBox {
                                id: privateChannelCheck
                                text: "Private channel"
                                Accessible.name: "Private channel"
                                Accessible.description: checked
                                    ? "New channel will require explicit member grants"
                                    : "New channel will be visible to workspace members"
                            }

                            Button {
                                Layout.fillWidth: true
                                text: privateChannelCheck.checked ? "Create private channel" : "Create channel"
                                enabled: root.runtimeWorkReady
                                    && channelNameField.text.trim().length > 0
                                onClicked: newChannelPopup.createFromForm()
                            }
                        }
                    }
                }

                ScrollView {
                    id: channelListScroll
                    Layout.fillWidth: true
                    Layout.fillHeight: !root.setupPanelOpen
                    visible: root.hasWorkspaceContent
                    Layout.preferredHeight: root.setupPanelOpen
                        ? Math.min(180, channelListColumn.implicitHeight + 8)
                        : 1
                    clip: true
                    contentWidth: availableWidth
                    contentHeight: channelListColumn.implicitHeight
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                    ScrollBar.vertical.policy: ScrollBar.AsNeeded

                    Item {
                        width: channelListScroll.availableWidth
                        height: channelListColumn.implicitHeight

                        ColumnLayout {
                            id: channelListColumn
                            width: parent.width
                            spacing: 2

                            Text {
                                visible: root.filteredChannels.length === 0
                                Layout.fillWidth: true
                                text: root.channels.length === 0
                                        && root.runtimeWorkReady
                                        && root.normalizedSearchQuery.length === 0
                                    ? "No channels yet — press + to create one"
                                    : "No matching channels"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeSm
                                wrapMode: Text.WordWrap
                            }

                            Repeater {
                                model: root.filteredChannels
                                delegate: SidebarItem {
                                    id: channelSidebarDelegate
                                    required property var modelData

                                    label: channelSidebarDelegate.modelData.name
                                    secondaryLabel: root.channelSidebarLabel(channelSidebarDelegate.modelData)
                                    selected: channelSidebarDelegate.modelData.channelId === root.selectedChannel.channelId
                                    unreadCount: channelSidebarDelegate.modelData.unreadCount
                                    privateChannel: channelSidebarDelegate.modelData.isPrivate
                                    hasDraft: root.draftTextForChannel(channelSidebarDelegate.modelData.channelId).trim().length > 0
                                    onActivated: root.selectChannelId(channelSidebarDelegate.modelData.channelId, true)
                                }
                            }

                            Button {
                                Layout.fillWidth: true
                                visible: root.channelCount > root.channels.length
                                text: "Load more channels"
                                enabled: root.runtimeWorkReady
                                onClicked: chaftController.loadMoreChannels()
                                ToolTip.visible: hovered
                                ToolTip.text: String(root.channels.length) + " of "
                                    + String(root.channelCount)
                            }
                        }
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: chaftController.deviceId.length > 0 || chaftController.hasRuntimeWorkspace
                    text: root.setupPanelOpen ? "Hide setup" : "Setup"
                    onClicked: root.setupPanelOpen = !root.setupPanelOpen
                }

                SetupPanel {
                    id: setupPanel
                    app: root
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: root.setupPanelOpen
                        && (chaftController.deviceId.length > 0 || chaftController.hasRuntimeWorkspace)
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: Tokens.surfaceBase

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: !root.hasWorkspaceContent

                    ColumnLayout {
                        anchors.centerIn: parent
                        width: Math.min(520, Math.max(280, parent.width - 64))
                        spacing: Tokens.space4

                        Text {
                            Layout.fillWidth: true
                            text: "No workspaces yet"
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeXl
                            font.weight: Font.Bold
                            horizontalAlignment: Text.AlignHCenter
                        }

                        Text {
                            Layout.fillWidth: true
                            text: "Join with workspace credentials, or create a local-first workspace."
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeMd
                            wrapMode: Text.WordWrap
                            horizontalAlignment: Text.AlignHCenter
                        }

                        RowLayout {
                            Layout.alignment: Qt.AlignHCenter
                            spacing: Tokens.space2

                            Button {
                                text: "Join workspace"
                                enabled: root.runtimeAccessReady
                                onClicked: root.openWorkspaceEntry("join")
                            }

                            Button {
                                text: "Create workspace"
                                enabled: root.runtimeAccessReady
                                onClicked: root.openWorkspaceEntry("create")
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: chaftController.syncStatus.length > 0
                            text: chaftController.syncStatus
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeSm
                            horizontalAlignment: Text.AlignHCenter
                            elide: Text.ElideRight
                        }
                    }
                }

                Rectangle {
                    visible: root.hasWorkspaceContent
                    Layout.fillWidth: true
                    Layout.preferredHeight: visible
                        ? (chaftController.hasRuntimeWorkspace && root.syncDrawerOpen ? 104 : 58)
                        : 0
                    color: Tokens.surfaceBase

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        RowLayout {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 58
                            Layout.leftMargin: 18
                            Layout.rightMargin: 18
                            spacing: 10

                            Text {
                                Layout.fillWidth: true
                                text: "# " + root.selectedChannelName
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeXl
                                font.weight: Font.Bold
                                elide: Text.ElideRight
                            }

                            Rectangle {
                                visible: root.channelCryptoExceptionCount > 0
                                implicitHeight: 24
                                implicitWidth: cryptoExceptionLabel.implicitWidth + 16
                                radius: Tokens.radiusSm
                                color: Tokens.warningSurface
                                border.width: 1
                                border.color: Tokens.warning

                                Accessible.role: Accessible.StaticText
                                Accessible.name: cryptoExceptionLabel.text

                                Text {
                                    id: cryptoExceptionLabel
                                    anchors.centerIn: parent
                                    text: String(root.channelCryptoExceptionCount)
                                        + " unprotected row" + (root.channelCryptoExceptionCount === 1 ? "" : "s")
                                    color: Tokens.warningText
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                }
                            }

                            SyncStatusPill {
                                visible: chaftController.hasRuntimeWorkspace || chaftController.rawEventStoreMode
                                label: root.syncPillLabel
                                tone: root.syncPillTone
                                detail: chaftController.syncStatus || root.workspaceSnapshot.syncStatus || ""
                                expanded: root.syncDrawerOpen
                                onToggled: root.syncDrawerOpen = !root.syncDrawerOpen
                            }
                        }

                        Flickable {
                            id: syncControlsFlick
                            Layout.fillWidth: true
                            Layout.preferredHeight: 46
                            visible: chaftController.hasRuntimeWorkspace && root.syncDrawerOpen
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds
                            contentWidth: Math.max(width, syncControlsRow.implicitWidth + 36)
                            contentHeight: height
                            ScrollBar.horizontal: ScrollBar {
                                policy: ScrollBar.AsNeeded
                            }

                            RowLayout {
                                id: syncControlsRow
                                x: 18
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 8

                                TextField {
                                    id: localPeerListenField
                                    Layout.preferredWidth: 132
                                    enabled: !chaftController.peerHosting
                                        && !chaftController.peerHostingInFlight
                                    text: "127.0.0.1:0"
                                    placeholderText: "Listen"
                                    color: Tokens.textStrong
                                    placeholderTextColor: Tokens.textMuted
                                    background: Rectangle {
                                        radius: Tokens.radiusMd
                                        color: Tokens.surfaceRaised
                                        border.color: Tokens.borderSubtle
                                    }
                                }

                                Button {
                                    enabled: !chaftController.peerHostingInFlight
                                        && (chaftController.peerHosting || root.runtimeWorkReady)
                                    text: chaftController.peerHosting ? "Stop" : "Host"
                                    onClicked: {
                                        if (chaftController.peerHosting) {
                                            chaftController.stopLocalPeer()
                                        } else {
                                            chaftController.startLocalPeer(localPeerListenField.text)
                                        }
                                    }
                                }

                                Button {
                                    visible: !chaftController.peerHosting
                                    text: "Iroh"
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.peerHostingInFlight
                                    onClicked: chaftController.startLocalIrohPeer()
                                }

                                Text {
                                    Layout.preferredWidth: 132
                                    visible: chaftController.peerHosting
                                    text: chaftController.hostedPeerEndpoint
                                    color: Tokens.textMuted
                                    font.family: Tokens.fontMono
                                    font.pixelSize: Tokens.fontSizeSm
                                    elide: Text.ElideMiddle
                                }

                                Button {
                                    visible: chaftController.peerHosting
                                        && chaftController.hostedPeerEndpoint.length > 0
                                    text: "Copy"
                                    Layout.preferredWidth: 58
                                    onClicked: root.copyTextToClipboard(
                                        chaftController.hostedPeerEndpoint,
                                        "peer endpoint"
                                    )
                                }

                                StatusChip {
                                    text: root.publishQueueStatusText()
                                    warning: root.publishQueueIssueCount > 0 || root.publishQueueError.length > 0
                                    secure: !(root.publishQueueIssueCount > 0 || root.publishQueueError.length > 0)
                                    maxWidth: 260
                                }

                                StatusChip {
                                    visible: root.storageHealthKnown
                                    text: root.storageHealthStatusText()
                                    warning: root.storageHealthHasIssue
                                }

                                Button {
                                    visible: root.storageMetadataRepairSuggested
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.syncInFlight
                                    text: "Repair"
                                    onClicked: root.repairStorageMetadata()
                                }

                                TextField {
                                    id: peerEndpointField
                                    Layout.preferredWidth: 180
                                    placeholderText: "Peer endpoint"
                                    color: Tokens.textStrong
                                    placeholderTextColor: Tokens.textMuted
                                    Component.onCompleted: text = chaftController.defaultPeerEndpoint
                                    onEditingFinished: chaftController.defaultPeerEndpoint = text.trim()
                                    background: Rectangle {
                                        radius: Tokens.radiusMd
                                        color: Tokens.surfaceRaised
                                        border.color: Tokens.borderSubtle
                                    }

                                    Connections {
                                        target: chaftController
                                        function onDefaultPeerEndpointChanged() {
                                            if (!peerEndpointField.activeFocus) {
                                                peerEndpointField.text = chaftController.defaultPeerEndpoint
                                            }
                                        }
                                    }
                                }

                                PeerRouteChip {
                                    label: root.activePeerRouteLabel()
                                    detail: root.activePeerRouteDetail()
                                    warning: root.activePeerRouteIsWarning()
                                }

                                CheckBox {
                                    enabled: root.runtimeWorkReady
                                        && root.preferredSyncPeerEndpoint().length > 0
                                    text: "Live"
                                    checked: root.autoSyncEnabled
                                    onToggled: root.autoSyncEnabled = checked
                                }

                                Button {
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.syncInFlight
                                        && root.preferredSyncPeerEndpoint().length > 0
                                    text: "Sync"
                                    onClicked: root.syncWorkspaceFromPreferredPeer()
                                }

                                Button {
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.syncInFlight
                                        && root.preferredSyncPeerEndpoint().length > 0
                                    text: "Push"
                                    onClicked: root.publishWorkspaceToPreferredPeer()
                                }

                                Button {
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.syncInFlight
                                        && root.preferredManualBackupPeerEndpoint().length > 0
                                    text: "Backup"
                                    onClicked: root.backupWorkspaceToPreferredPeer()
                                }

                                Button {
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.syncInFlight
                                        && (root.preferredRetryPeerEndpoint().length > 0
                                            || (chaftController.backupPeerEndpoints || []).length > 0)
                                    text: "Retry"
                                    onClicked: root.retryBlobTransfersWithPreferredPeers()
                                }

                                Button {
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.syncInFlight
                                        && root.preferredSyncPeerEndpoint().length > 0
                                    text: "Pull"
                                    onClicked: root.pullWorkspaceFromPreferredPeer()
                                }

                                Button {
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.syncInFlight
                                    text: "Prune"
                                    onClicked: chaftController.pruneBlobs()
                                }
                            }
                        }
                    }
                }

                Button {
                    Layout.alignment: Qt.AlignHCenter
                    visible: root.hasWorkspaceContent
                        && root.runtimeWorkReady
                        && !root.runtimeSearchReady
                        && Boolean(root.timelineWindow.hasMoreBefore)
                    enabled: !chaftController.syncInFlight
                    text: "Load older"
                    onClicked: {
                        timelineView.prepareForPrepend()
                        var loaded = root.selectedChannelTimelineReady
                            ? chaftController.loadOlderChannelTimeline(root.selectedChannel.channelId)
                            : chaftController.loadOlderTimeline()
                        if (!loaded) {
                            timelineView.cancelPrepend()
                        }
                    }
                }

                TimelineView {
                    id: timelineView
                    visible: root.hasWorkspaceContent
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    timelineModel: root.selectedTimeline
                    emptyText: root.normalizedSearchQuery.length > 0 ? "No matching messages" : "No messages yet"
                    actionsEnabled: root.runtimeWorkReady
                    historyRepairEnabled: root.runtimeWorkReady
                        && !chaftController.syncInFlight
                        && root.preferredSyncPeerEndpoint().length > 0
                    autoFollowLatest: root.normalizedSearchQuery.length === 0
                    showChannelLabels: root.runtimeSearchReady
                    selectedItemKey: root.inspectorItemKey
                    pendingDeleteMessageIds: root.pendingDeleteIds
                    onItemSelected: function(item) {
                        root.selectInspectorItem(item)
                    }
                    onReactionRequested: function(messageId, reaction) {
                        chaftController.addReaction(messageId, reaction)
                    }
                    onReactionRemoveRequested: function(messageId, reaction) {
                        chaftController.removeReaction(messageId, reaction)
                    }
                    onReplyRequested: function(item) {
                        root.beginReplyMessage(item)
                    }
                    onThreadRequested: function(item) {
                        root.selectInspectorItem(item)
                    }
                    onEditRequested: function(messageId, body) {
                        root.beginEditMessage(messageId, body)
                    }
                    onDeleteRequested: function(messageId) {
                        root.queueMessageDelete(messageId)
                    }
                    onAttachmentSaveRequested: function(messageId, attachmentSelector, displayName) {
                        saveAttachmentDialog.messageId = messageId
                        saveAttachmentDialog.attachmentSelector = attachmentSelector
                        saveAttachmentDialog.displayName = displayName
                        saveAttachmentDialog.open()
                    }
                    onProofPublishRequested: function(eventId) {
                        root.publishEventWithTrustSnapshotToPreferredPeer(eventId)
                    }
                    onHistoryRepairRequested: {
                        root.repairHistoryFromPeer()
                    }
                }

                ComposerBar {
                    id: composer
                    visible: root.hasWorkspaceContent
                    Layout.fillWidth: true
                    channelName: root.selectedChannelName
                    editMode: root.editingMessageId.length > 0
                    replyMode: root.replyTargetMessageId.length > 0
                    replyLabel: root.replyTargetMessageId.length > 0
                        ? root.replyTargetLabel(root.replyTarget)
                        : ""
                    enabled: root.runtimeWorkReady && root.selectedChannelKey.length > 0
                    onDraftChanged: function(text) {
                        root.saveSelectedDraftText(text)
                    }
                    onSendRequested: function(text) {
                        var sent = root.replyTargetMessageId.length > 0
                            ? chaftController.sendMessageReply(
                                root.selectedChannelKey,
                                root.replyTargetMessageId,
                                text
                            )
                            : chaftController.sendMessage(root.selectedChannelKey, text)
                        if (sent) {
                            root.clearDraftForChannel(root.selectedChannelKey)
                            composer.clearDraft()
                            root.cancelReplyMessage()
                        }
                    }
                    onAttachRequested: function(text) {
                        attachmentDialog.pendingText = text
                        attachmentDialog.pendingWorkspaceId = root.currentWorkspaceId()
                        attachmentDialog.pendingChannelId = root.selectedChannelKey
                        attachmentDialog.pendingReplyToMessageId = root.replyTargetMessageId
                        attachmentDialog.open()
                    }
                    onSaveEditRequested: function(text) {
                        if (chaftController.editMessage(root.editingMessageId, text)) {
                            root.cancelEditMessage()
                        }
                    }
                    onCancelEditRequested: root.cancelEditMessage()
                    onCancelReplyRequested: root.cancelReplyMessage()
                }
            }
        }

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 300
            visible: root.width >= 1400
                && root.hasWorkspaceContent
                && (chaftController.inspectorPinned || root.inspectorItemKey.length > 0)
            color: Tokens.surfaceRaised
            border.color: Tokens.borderSubtle

            ScrollView {
                id: inspectorScroll
                anchors.fill: parent
                clip: true
                contentWidth: availableWidth
                contentHeight: inspectorColumn.implicitHeight + 28
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: ScrollBar.AsNeeded

                Item {
                    width: inspectorScroll.availableWidth
                    height: inspectorColumn.implicitHeight + 28

                    ColumnLayout {
                        id: inspectorColumn
                        x: 14
                        y: 14
                        width: Math.max(0, parent.width - 28)
                        spacing: 14

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: root.selectedChannelName.length > 0
                                        ? "# " + root.selectedChannelName
                                        : "Details"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeXl
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: root.workspaceSnapshot.name || "Chaft"
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }
                            }

                            Rectangle {
                                Layout.preferredWidth: Math.max(64, channelKindText.implicitWidth + 18)
                                Layout.preferredHeight: 24
                                radius: Tokens.radiusSm
                                color: root.selectedChannelPrivate ? Tokens.secureSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                Text {
                                    id: channelKindText
                                    anchors.centerIn: parent
                                    text: root.selectedChannelPrivate ? "Private" : "Open"
                                    color: root.selectedChannelPrivate ? Tokens.secure : Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                }
                            }

                            Button {
                                text: chaftController.inspectorPinned ? "Unpin" : "Pin"
                                Layout.preferredWidth: 56
                                Accessible.name: chaftController.inspectorPinned
                                    ? "Unpin inspector"
                                    : "Pin inspector"
                                onClicked: chaftController.inspectorPinned = !chaftController.inspectorPinned
                                ToolTip.visible: hovered
                                ToolTip.text: chaftController.inspectorPinned
                                    ? "Inspector stays open; unpin to open only on selection"
                                    : "Keep the inspector open even with nothing selected"
                            }
                        }

                        GridLayout {
                            Layout.fillWidth: true
                            columns: 2
                            columnSpacing: 8
                            rowSpacing: 8

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 58
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 2

                                    Text {
                                        text: "Messages"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String(root.channelTimeline.length)
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXl
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 58
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 2

                                    Text {
                                        text: root.runtimeSearchReady ? "Hits" : "Loaded"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: root.runtimeSearchReady ? root.messageSearchHitCountLabel() : String(root.selectedTimeline.length)
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXl
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 58
                                radius: Tokens.radiusSm
                                color: root.selectedChannelIssueCount > 0 ? Tokens.warningSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 2

                                    Text {
                                        text: "Issues"
                                        color: root.selectedChannelIssueCount > 0 ? Tokens.warningText : Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String(root.selectedChannelIssueCount)
                                        color: root.selectedChannelIssueCount > 0 ? Tokens.warningText : Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXl
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 58
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 2

                                    Text {
                                        text: "Files"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String(root.channelAttachments.length)
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXl
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: Tokens.borderSubtle
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                text: "Message"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeLg
                                font.weight: Font.DemiBold
                            }

                            Rectangle {
                                Layout.preferredWidth: Math.max(58, messageModeText.implicitWidth + 16)
                                Layout.preferredHeight: 22
                                radius: Tokens.radiusSm
                                color: root.inspectorItemIsSelected ? Tokens.secureSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                Text {
                                    id: messageModeText
                                    anchors.centerIn: parent
                                    text: root.inspectorItemIsSelected ? "Selected" : "Latest"
                                    color: root.inspectorItemIsSelected ? Tokens.secure : Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                }
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: root.timelineItemKey(root.inspectorItem).length === 0
                            text: "No messages yet"
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeSm
                            elide: Text.ElideRight
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: inspectorCardContent.implicitHeight + 20
                            visible: root.timelineItemKey(root.inspectorItem).length > 0
                            radius: Tokens.radiusSm
                            color: Tokens.surfaceBase
                            border.color: Tokens.borderSubtle

                            ColumnLayout {
                                id: inspectorCardContent
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 8

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8

                                    Rectangle {
                                        Layout.preferredWidth: 32
                                        Layout.preferredHeight: 32
                                        radius: Tokens.radiusMd
                                        color: (root.inspectorItem.kind === "missing_history_gap" || root.inspectorItem.kind === "invalid_signature")
                                            ? Tokens.warning
                                            : (root.inspectorItem.encrypted ? Tokens.secure : Tokens.accent)

                                        Text {
                                            anchors.centerIn: parent
                                            text: (root.inspectorItem.kind === "missing_history_gap" || root.inspectorItem.kind === "invalid_signature")
                                                ? "!"
                                                : (root.itemAuthorLabel(root.inspectorItem).length > 0
                                                    ? root.itemAuthorLabel(root.inspectorItem).slice(0, 1).toUpperCase()
                                                    : "?")
                                            color: Tokens.onAccent
                                            font.pixelSize: Tokens.fontSizeSm
                                            font.weight: Font.DemiBold
                                        }
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 2

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.inspectorItem.kind === "missing_history_gap"
                                                ? "History gap"
                                                : root.inspectorItem.kind === "invalid_signature"
                                                    ? "Invalid signature"
                                                : root.itemAuthorLabel(root.inspectorItem)
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeSm
                                            font.weight: Font.DemiBold
                                            elide: Text.ElideRight
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.timelineKindLabel(root.inspectorItem)
                                            color: (root.inspectorItem.kind === "missing_history_gap" || root.inspectorItem.kind === "invalid_signature")
                                                ? Tokens.warningText
                                                : Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            elide: Text.ElideRight
                                        }
                                    }
                                }

                                Text {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 58
                                    text: root.inspectorItem.body || ""
                                    color: (root.inspectorItem.kind === "missing_history_gap" || root.inspectorItem.kind === "invalid_signature")
                                        ? Tokens.warningText
                                        : Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 3
                                    elide: Text.ElideRight
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Button {
                                        text: root.inspectorItem.bodyTruncated ? "Copy preview" : "Copy text"
                                        Layout.preferredWidth: root.inspectorItem.bodyTruncated ? 112 : 86
                                        enabled: root.inspectorBodyCopyText().length > 0
                                        onClicked: root.copyInspectorBody()
                                    }

                                    Button {
                                        text: "Copy event ID"
                                        Layout.preferredWidth: 108
                                        enabled: String(root.inspectorItem.eventId || "").length > 0
                                        onClicked: root.copyInspectorEventId()
                                    }

                                    Button {
                                        text: "Copy message ID"
                                        Layout.preferredWidth: 126
                                        visible: String(root.inspectorItem.messageId || "").length > 0
                                        enabled: String(root.inspectorItem.messageId || "").length > 0
                                        onClicked: root.copyInspectorMessageId()
                                    }

                                    Item {
                                        Layout.fillWidth: true
                                    }
                                }

                                Button {
                                    text: root.inspectorDetailsOpen ? "Hide details" : "Details"
                                    Layout.preferredWidth: 104
                                    onClicked: root.inspectorDetailsOpen = !root.inspectorDetailsOpen
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Event and message identifiers, counts"
                                }

                                GridLayout {
                                    Layout.fillWidth: true
                                    visible: root.inspectorDetailsOpen
                                    columns: 2
                                    columnSpacing: 10
                                    rowSpacing: 4

                                    Text {
                                        text: "Event"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: String(root.inspectorItem.eventId || "")
                                        color: Tokens.textMuted
                                        font.family: Tokens.fontMono
                                        font.pixelSize: Tokens.fontSizeXs
                                        elide: Text.ElideMiddle
                                    }

                                    Text {
                                        visible: String(root.inspectorItem.messageId || "").length > 0
                                        text: "Message"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: String(root.inspectorItem.messageId || "").length > 0
                                        text: String(root.inspectorItem.messageId || "")
                                        color: Tokens.textMuted
                                        font.family: Tokens.fontMono
                                        font.pixelSize: Tokens.fontSizeXs
                                        elide: Text.ElideMiddle
                                    }

                                    Text {
                                        text: "Files"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: String((root.inspectorItem.attachments || []).length)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: "Reactions"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: String(root.reactionDistinctCountForItem(root.inspectorItem))
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.inspectorAttachmentCount > 0
                                    spacing: 6

                                    Repeater {
                                        model: root.inspectorAttachmentPreviews

                                        delegate: AttachmentRow {
                                            Layout.fillWidth: true
                                            attachment: modelData
                                            detailText: root.attachmentDetailLabel(modelData)
                                            messageId: String(root.inspectorItem.messageId || "")
                                            selector: root.attachmentSelectorFor(modelData)
                                            runtimeReady: root.runtimeWorkReady
                                            onSaveRequested: function(messageId, attachment) {
                                                root.openSaveAttachmentDialog(messageId, attachment)
                                            }
                                            onCopyRequested: function(attachment) {
                                                root.copyAttachmentSelector(attachment)
                                            }
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: root.inspectorAttachmentOverflowLabel.length > 0
                                        text: root.inspectorAttachmentOverflowLabel
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        elide: Text.ElideRight
                                    }
                                }
                            }
                        }

                        InspectorThreadPanel {
                            Layout.fillWidth: true
                            replyCount: root.inspectorThreadReplyCount
                            replyPreviews: root.inspectorThreadReplyPreviews
                            runtimeReady: root.runtimeWorkReady
                            messageId: String(root.inspectorItem.messageId || "")
                            messageDeleted: Boolean(root.inspectorItem.deleted)
                            onReplyRequested: root.beginReplyMessage(root.inspectorItem)
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: Tokens.borderSubtle
                        }

                        ChannelMediaPanel {
                            Layout.fillWidth: true
                            attachments: root.channelAttachments
                            recentAttachments: root.recentChannelAttachments
                            runtimeReady: root.runtimeWorkReady
                            onSaveRequested: function(messageId, attachment) {
                                root.openSaveAttachmentDialog(messageId, attachment)
                            }
                            onCopyRequested: function(attachment) {
                                root.copyAttachmentSelector(attachment)
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: Tokens.borderSubtle
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                text: "Backup"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeLg
                                font.weight: Font.DemiBold
                            }

                            Rectangle {
                                Layout.preferredWidth: Math.max(50, backupAutoText.implicitWidth + 16)
                                Layout.preferredHeight: 22
                                radius: Tokens.radiusSm
                                color: root.autoBackupEnabled ? Tokens.secureSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                Text {
                                    id: backupAutoText
                                    anchors.centerIn: parent
                                    text: root.autoBackupEnabled ? "Auto" : "Manual"
                                    color: root.autoBackupEnabled ? Tokens.secure : Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                }
                            }
                        }

                        GridLayout {
                            Layout.fillWidth: true
                            columns: 2
                            columnSpacing: 8
                            rowSpacing: 8

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 1

                                    Text {
                                        text: "Peers"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String((chaftController.backupPeerEndpoints || []).length)
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeLg
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 1

                                    Text {
                                        text: "Host"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: chaftController.peerHosting
                                            ? "Serving"
                                            : (chaftController.peerHostingInFlight ? "Busy" : "Local")
                                        color: chaftController.peerHosting ? Tokens.success : Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeLg
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Tokens.radiusSm
                                color: root.queuedPublishableEventCount > 0 ? Tokens.secureSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 1

                                    Text {
                                        text: "Queue"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String(root.queuedPublishableEventCount)
                                        color: root.queuedPublishableEventCount > 0 ? Tokens.secure : Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeLg
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Tokens.radiusSm
                                color: root.publishQueueIssueCount > 0 ? Tokens.warningSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 1

                                    Text {
                                        text: "Issues"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String(root.publishQueueIssueCount)
                                        color: root.publishQueueIssueCount > 0 ? Tokens.warningText : Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeLg
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Tokens.radiusSm
                                color: root.storageHealthHasIssue ? Tokens.warningSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 1

                                    Text {
                                        text: "Cache"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: root.storageHealthKnown
                                            ? String(root.storageHealthAttentionCount)
                                            : "-"
                                        color: root.storageHealthHasIssue ? Tokens.warningText : Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeLg
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 1

                                    Text {
                                        text: "Rows"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: root.storageHealthKnown
                                            ? String(root.storageTotalEventCount)
                                            : "-"
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeLg
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: chaftController.hasRuntimeWorkspace
                            text: root.publishQueueStatusText()
                            color: root.publishQueueIssueCount > 0 || root.publishQueueError.length > 0
                                ? Tokens.warningText
                                : Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeSm
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: chaftController.hasRuntimeWorkspace
                                && root.storageHealthKnown
                            text: root.storageHealthStatusText()
                            color: root.storageHealthHasIssue
                                ? Tokens.warningText
                                : Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeSm
                            elide: Text.ElideRight
                        }

                        Button {
                            Layout.fillWidth: true
                            visible: chaftController.hasRuntimeWorkspace
                                && root.storageMetadataRepairSuggested
                            enabled: root.runtimeWorkReady
                                && !chaftController.syncInFlight
                            text: "Repair cache metadata"
                            onClicked: root.repairStorageMetadata()
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            visible: root.peerEndpointHints.length > 0
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                text: "Discovered"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                            }

                            Text {
                                text: String(root.peerEndpointCount)
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                            }
                        }

                        ListView {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.min(174, contentHeight)
                            visible: root.peerEndpointHints.length > 0
                            clip: true
                            interactive: contentHeight > height
                            spacing: 6
                            model: root.peerEndpointHints

                            delegate: PeerEndpointHintRow {
                                id: peerEndpointHintDelegate
                                required property var modelData

                                width: ListView.view.width
                                endpoint: String(peerEndpointHintDelegate.modelData.endpoint || "")
                                kindLabel: root.peerEndpointKindLabel(peerEndpointHintDelegate.modelData)
                                detailLabel: root.peerEndpointDetailLabel(peerEndpointHintDelegate.modelData)
                                backupPeer: Boolean(peerEndpointHintDelegate.modelData.isBackupPeer)
                                expired: root.isPeerEndpointExpired(peerEndpointHintDelegate.modelData)
                                runtimeReady: root.runtimeWorkReady
                                syncInFlight: chaftController.syncInFlight
                                savedAsBackup: root.isBackupPeerSaved(peerEndpointHintDelegate.endpoint)
                                onUseRequested: function (endpoint) {
                                    root.usePeerEndpoint(endpoint)
                                }
                                onSyncRequested: function (endpoint) {
                                    chaftController.syncWorkspace(endpoint)
                                }
                                onSaveRequested: function (endpoint) {
                                    chaftController.addBackupPeerEndpoint(endpoint)
                                }
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: (chaftController.backupPeerEndpoints || []).length === 0
                            text: "No backup peers"
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeSm
                            elide: Text.ElideRight
                        }

                        ListView {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.min(150, contentHeight)
                            visible: (chaftController.backupPeerEndpoints || []).length > 0
                            clip: true
                            interactive: contentHeight > height
                            spacing: 6
                            model: chaftController.backupPeerEndpoints

                            delegate: BackupPeerStatusRow {
                                id: backupPeerStatusDelegate
                                required property string modelData

                                width: ListView.view.width
                                endpoint: backupPeerStatusDelegate.modelData
                                statusText: root.backupPeerStatusText(backupPeerStatusDelegate.modelData)
                                stateLabel: root.backupPeerStateLabel(backupPeerStatusDelegate.modelData)
                                stateColor: root.backupPeerStateColor(backupPeerStatusDelegate.modelData)
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: Tokens.borderSubtle
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                text: "Members"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeLg
                                font.weight: Font.DemiBold
                            }

                            Text {
                                text: String(root.memberCount)
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                            }
                        }

                        ListView {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.min(210, contentHeight)
                            clip: true
                            interactive: contentHeight > height
                            spacing: 6
                            model: root.members

                            delegate: MemberRow {
                                id: memberRowDelegate
                                required property var modelData

                                width: ListView.view.width
                                deviceId: String(memberRowDelegate.modelData.deviceId || "")
                                displayLabel: root.memberLabel(memberRowDelegate.modelData)
                                initial: root.memberInitial(memberRowDelegate.modelData)
                                roleLabel: root.roleLabel(memberRowDelegate.modelData.role)
                                owner: memberRowDelegate.modelData.role === "owner"
                                localDevice: memberRowDelegate.deviceId === chaftController.deviceId
                                canRemove: chaftController.hasRuntimeWorkspace
                                    && memberRowDelegate.deviceId !== chaftController.deviceId
                                onRemoveRequested: function (deviceId) {
                                    confirmDialog.ask(
                                        "Remove member",
                                        "Remove device " + deviceId + " from the workspace? "
                                            + "This writes a signed removal event and triggers key rotation "
                                            + "so the device cannot read new messages.",
                                        "Remove member",
                                        "remove-member:" + deviceId,
                                        true)
                                }
                            }
                        }

                        Button {
                            Layout.fillWidth: true
                            visible: root.memberCount > root.members.length
                            text: "Load more"
                            enabled: root.runtimeWorkReady
                            onClicked: chaftController.loadMoreMembers()
                            ToolTip.visible: hovered
                            ToolTip.text: String(root.members.length) + " of "
                                + String(root.memberCount)
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: Tokens.borderSubtle
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                text: "Keys"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeLg
                                font.weight: Font.DemiBold
                            }

                            Text {
                                text: String(root.keyPackageCount)
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: root.keyPackages.length === 0
                            text: "No key packages"
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeSm
                            elide: Text.ElideRight
                        }

                        ListView {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.min(240, contentHeight)
                            visible: root.keyPackages.length > 0
                            clip: true
                            interactive: contentHeight > height
                            spacing: 6
                            model: root.keyPackages

                            delegate: KeyPackageRow {
                                id: keyPackageRowDelegate
                                required property var modelData

                                width: ListView.view.width
                                deviceId: String(keyPackageRowDelegate.modelData.deviceId || "")
                                keyPackageId: String(keyPackageRowDelegate.modelData.keyPackageId || "")
                                openMls: root.isOpenMlsKeyPackage(keyPackageRowDelegate.modelData)
                                runtimeReady: root.runtimeWorkReady
                                privateChannelSelected: root.selectedChannelPrivate
                                selectedChannelId: root.selectedChannelKey
                                onWorkspaceMlsRequested: function (keyPackageId) {
                                    chaftController.addOpenMlsWorkspaceGroupMember(keyPackageId)
                                }
                                onChannelMlsRequested: function (channelId, keyPackageId) {
                                    chaftController.addOpenMlsChannelGroupMember(channelId, keyPackageId)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
