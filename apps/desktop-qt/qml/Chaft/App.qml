import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import QtCore
import Chaft

ApplicationWindow {
    id: root
    signal keyTransferFileSaveFinished(bool success, string label,
                                       string artifactKind, string operationToken)
    property int keyTransferFileSaveSequence: 0
    width: root.restoredWindowWidth()
    height: root.restoredWindowHeight()
    minimumWidth: 1040
    minimumHeight: 640
    visible: true
    readonly property string windowBaseTitle: chaftController.instanceLabel.length > 0
        ? "Chaft — " + chaftController.instanceLabel
        : "Chaft"
    title: root.totalUnreadCount > 0
        ? root.windowBaseTitle + " (" + root.unreadCountTitleLabel(root.totalUnreadCount) + ")"
        : root.windowBaseTitle
    color: Tokens.surfaceBase
    readonly property var workspaceSnapshot: root.demoTourActive
        ? initialWorkspaceSnapshot
        : (chaftController.workspaceSnapshot || initialWorkspaceSnapshot)
    readonly property var channels: workspaceSnapshot.channels || []
    readonly property var resolvedChannels: workspaceSnapshot.resolvedChannels || ({})
    readonly property var profiles: workspaceSnapshot.profiles || []
    readonly property var personProfiles: workspaceSnapshot.personProfiles || []
    readonly property var personDeviceLinks: workspaceSnapshot.personDeviceLinks || []
    readonly property var members: workspaceSnapshot.members || []
    readonly property var invites: workspaceSnapshot.invites || []
    readonly property var joinRequests: workspaceSnapshot.joinRequests || []
    readonly property string accessPolicy: root.normalizedWorkspaceAccessPolicy(workspaceSnapshot.accessPolicy)
    readonly property var keyPackages: workspaceSnapshot.keyPackages || []
    readonly property int channelCount: root.countOrLength(workspaceSnapshot.channelCount, channels)
    readonly property int memberCount: root.countOrLength(workspaceSnapshot.memberCount, members)
    readonly property int inviteCount: root.countOrLength(workspaceSnapshot.inviteCount, invites)
    readonly property int joinRequestCount: root.countOrLength(workspaceSnapshot.joinRequestCount, joinRequests)
    readonly property int waitingAccessRequestCount: root.joinRequestStatusCount(
        root.joinRequests, "waiting")
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
    readonly property bool workspaceOperationInFlight:
        chaftController.workspaceOperationInFlight
    readonly property bool hasWorkspaceContent: chaftController.hasRuntimeWorkspace
        || chaftController.rawEventStoreMode
        || root.demoTourActive
    readonly property var timeline: workspaceSnapshot.timeline || []
    readonly property var timelineWindow: workspaceSnapshot.timelineWindow || ({ startIndex: 0, itemCount: timeline.length, totalCount: timeline.length, hasMoreBefore: false, hasMoreAfter: false })
    readonly property string timelineChannelId: String(workspaceSnapshot.timelineChannelId || "")
    readonly property var workspaceRailItems: root.workspaceRailItemsForSummaries(
        chaftController.workspaceSummaries || [])
    readonly property int totalUnreadCount: root.windowUnreadCount()
    readonly property var mutedChannels: chaftController.mutedChannels || ({})
    readonly property var pendingAccessRequests: root.pendingAccessRequestRows(
        chaftController.pendingJoinRequests || ({}))
    property string selectedChannelId: ""
    property string searchQuery: ""
    readonly property string trimmedSearchQuery: searchQuery.trim()
    readonly property bool searchHasTerms: trimmedSearchQuery.length > 0
    readonly property bool indexedSearchHasTerms:
        chaftController.searchQueryHasTerms(trimmedSearchQuery)
    readonly property string normalizedSearchQuery: searchHasTerms ? trimmedSearchQuery.toLowerCase() : ""
    readonly property var selectedChannel: root.channelById(root.selectedChannelId)
    readonly property string selectedChannelKey: String(selectedChannel.channelId || "")
    readonly property bool selectedChannelPrivate: Boolean(selectedChannel.isPrivate)
    readonly property bool selectedChannelTimelineReady: timelineChannelId.length > 0
        && timelineChannelId === selectedChannelKey
    readonly property bool channelSearchReady: root.runtimeWorkReady
        && root.indexedSearchHasTerms
        && chaftController.channelSearchQuery === root.trimmedSearchQuery
    readonly property var filteredChannels: root.filteredChannelRows()
    readonly property var filteredRoomChannels: root.filteredChannels.filter(function(channel) {
        return !root.channelIsDirectMessage(channel) && !root.channelArchived(channel)
    })
    readonly property var filteredArchivedChannels: root.filteredChannels.filter(function(channel) {
        return !root.channelIsDirectMessage(channel) && root.channelArchived(channel)
    })
    readonly property var filteredDirectMessageChannels: root.filteredChannels.filter(function(channel) {
        return root.channelIsDirectMessage(channel)
    })
    readonly property bool runtimeSearchReady: root.runtimeWorkReady
        && root.indexedSearchHasTerms
        && chaftController.messageSearchQuery === root.trimmedSearchQuery
    readonly property bool searchResultsLoading: root.searchHasTerms
        && root.runtimeWorkReady
        && root.indexedSearchHasTerms
        && (!root.runtimeSearchReady || !root.channelSearchReady)
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
    property var inspectorSelectedItemSnapshot: ({})
    readonly property var inspectorItem: root.currentInspectorItem()
    readonly property bool selectedChannelDirectMessage: root.channelIsDirectMessage(root.selectedChannel)
    readonly property string selectedChannelDisplayName: root.channelDisplayName(root.selectedChannel)
    readonly property string selectedChannelTopic: String(root.selectedChannel.topic || "").trim()
    readonly property bool selectedChannelMuted: root.channelMuted(root.selectedChannel)
    readonly property bool selectedChannelArchived: root.channelArchived(root.selectedChannel)
    readonly property int selectedChannelMemberCount: root.channelMemberCount(root.selectedChannel)
    readonly property var selectedChannelMemberDeviceIds: root.channelMemberDeviceIds(root.selectedChannel)
    readonly property var selectedChannelAccessMembers: root.membersWithDeviceIds(root.selectedChannelMemberDeviceIds)
    readonly property var selectedChannelAccessHistory: root.channelAccessHistory(root.selectedChannel)
    property bool channelAccessHistoryExpanded: false
    property bool inspectorAccessHistoryExpanded: false
    readonly property var selectedChannelGrantCandidates: root.privateChannelGrantCandidates()
    readonly property bool selectedChannelCanLeave: root.runtimeWorkReady
        && root.selectedChannelKey.length > 0
        && root.selectedChannelPrivate
        && !root.selectedChannelDirectMessage
        && chaftController.deviceId.length > 0
    readonly property bool selectedChannelCanRefreshKey: root.runtimeWorkReady
        && root.selectedChannelKey.length > 0
        && root.selectedChannelPrivate
        && !root.selectedChannelDirectMessage
        && root.canManageWorkspaceAccess()
        && !chaftController.keyTransferInFlight
    readonly property int channelHeaderHeight: root.selectedChannelTopic.length > 0
        && !root.selectedChannelDirectMessage ? 76 : 58
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
    readonly property int selectedChannelLoadedMessageCount: root.countLoadedMessages(channelTimeline)
    readonly property int selectedChannelLockedMessageCount: root.countLockedMessages(channelTimeline)
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
    property string pendingComposerOperationKind: ""
    property string pendingComposerOperationWorkspaceId: ""
    property string pendingComposerOperationChannelId: ""
    property string pendingComposerOperationReplyToMessageId: ""
    property string pendingComposerOperationMessageId: ""
    property string pendingComposerOperationText: ""
    property string pendingComposerOperationFilePath: ""
    property bool suppressComposerDraftSave: false
    readonly property bool composerOperationPending:
        pendingComposerOperationKind.length > 0
    readonly property bool composerContextBusy:
        composerOperationPending || editingMessageId.length > 0
    property string pendingDraftRestoreWorkspaceId: ""
    property string mainDestination: "conversation"
    property string settingsCategory: "profile"
    readonly property bool compactAppChrome: root.width < 1200
    readonly property bool pinnedInspectorAvailable: root.width >= 1280
    readonly property int workspaceRailWidth: compactAppChrome ? 64 : 72
    readonly property int navigationSidebarWidth: compactAppChrome ? 228 : 268
    readonly property int inspectorPanelWidth: root.width >= 1500
        ? 312
        : (root.width >= 1200 ? 280 : 248)
    readonly property bool conversationDestination: mainDestination === "conversation"
    readonly property bool settingsDestination: mainDestination === "settings"
    readonly property bool peopleAccessDestination: mainDestination === "peopleAccess"
    property bool autoSyncEnabled: true
    property bool autoBackupEnabled: chaftController.autoBackupEnabled
    property bool runtimeUnlockDismissed: false
    property string workspaceEntryMode: "join"
    property string workspaceEntryIntent: "join"
    property bool pendingPostCreateExport: false
    property string pendingPostCreateWorkspaceId: ""
    property string pendingWorkspaceCreateDisplayName: ""
    property string pendingWorkspaceCreateAvatarId: ""
    property string pendingEntryDisplayName: ""
    property string pendingEntryAvatarId: ""
    property string pendingEntryDisplayNameWorkspaceId: ""
    property string pendingEntryDisplayNameRequestId: ""
    property string pendingEntryDisplayNameRequestKey: ""
    property bool pendingEntryDisplayNameUpdateInFlight: false
    property string pendingEntryDisplayNameUpdateName: ""
    property string pendingEntryDisplayNameUpdateAvatarId: ""
    property string pendingEntryDisplayNameUpdateWorkspaceId: ""
    property bool pendingEntryDisplayNameWriteSucceeded: false
    property int pendingEntryDisplayNameRetryAttempt: 0
    property int pendingEntryDisplayNameReconcileAttempt: 0
    readonly property int pendingEntryDisplayNameRetryLimit: 5
    property string pendingJoinPeerEndpoint: ""
    property bool pendingJoinPullCompletion: false
    property bool hostedRuntimeReconcileDue: true
    property bool controllerIdleWorkPending: false
    property bool controllerIdleWorkScheduled: false
    property bool pendingPrivateRoomHistoryRepairCompletion: false
    property string pendingPrivateRoomHistoryRepairChannelId: ""
    property string pendingPrivateRoomHistoryRepairRoomName: ""
    property string lastPrivateRoomHistoryRepairFailedChannelId: ""
    property bool pendingJoinAwaitingReachablePeer: false
    property string pendingJoinAwaitingWorkspaceId: ""
    property string pendingJoinAwaitingSource: ""
    property int pendingJoinRecoveryPrivateRoomCount: -1
    property bool pendingWorkspaceImportActive: false
    property string pendingWorkspaceImportWorkspaceId: ""
    property string pendingWorkspaceImportRequestId: ""
    property string pendingWorkspaceImportPeerEndpoint: ""
    property string pendingWorkspaceImportSource: ""
    property string pendingWorkspaceImportInboxArtifact: ""
    property string pendingExternalLink: ""
    property string pendingAccessRequestSaveText: ""
    property string pendingAccessRequestSaveKey: ""
    property string pendingAccessRequestSaveLabel: "access request"
    property string pendingAccessRequestSendingKey: ""
    property string pendingAccessRequestSentUnpersistedKey: ""
    property string pendingAccessResponseAutoCheckLastKey: ""
    property string peerEndpointFormError: ""
    property int lastNotificationUnreadCount: 0
    property bool unreadNotificationsReady: false
    property string accessRequestNotificationWorkspaceId: ""
    property int accessRequestNotificationBaseline: 0
    property bool accessRequestNotificationsReady: false
    property bool pendingSmokeArchivedChannelSelection: false
    property bool pendingSmokePrivateChannelDetailsSelection: false
    property bool pendingSmokeSetupRoomAccessSelection: false
    property bool pendingSmokePrivateChannelInspectorSelection: false
    property bool pendingSmokePrivateChannelRepairFailedSelection: false
    property bool pendingSmokePrivateChannelRepairFailedSavedAddress: false
    property real inspectorSmokeScrollOffsetY: 0
    readonly property bool smokePrivateRoomRepairFailureActive:
        chaftController.smokeUiState === "private-channel-repair-failed"
        || chaftController.smokeUiState === "private-channel-repair-saved"
    readonly property bool smokePrivateRoomRepairSavedAddressActive:
        chaftController.smokeUiState === "private-channel-repair-saved"
    readonly property bool joinWaitingForPeerBannerVisible: root.hasWorkspaceContent
        && root.runtimeWorkReady
        && root.pendingJoinAwaitingReachablePeer
        && (root.pendingJoinAwaitingWorkspaceId.length === 0
            || root.pendingJoinAwaitingWorkspaceId === root.currentWorkspaceId())

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
    property bool syncAdvancedToolsOpen: false
    property bool customReachableAddressOpen: false
    property bool channelAccessToolsOpen: false
    property bool inspectorDetailsOpen: false
    property bool demoTourActive: false

    onSyncDrawerOpenChanged: {
        if (!root.syncDrawerOpen) {
            root.syncAdvancedToolsOpen = false
            root.customReachableAddressOpen = false
        }
    }

    readonly property int channelCryptoExceptionCount: {
        var rows = root.selectedTimeline || []
        var count = 0
        for (var i = 0; i < rows.length; ++i) {
            var kind = String(rows[i].kind || "")
            var deleted = rows[i].deleted === true
            var locked = kind === "encrypted_message" && rows[i].bodyDecrypted !== true && !deleted
            var plaintext = kind === "message" && rows[i].encrypted !== true && !deleted
            if (locked || plaintext) {
                count += 1
            }
        }
        return count
    }

    readonly property string syncPillLabel: {
        if (chaftController.rawEventStoreMode) {
            return "Read-only history"
        }
        if (!chaftController.hasRuntimeWorkspace) {
            return "No workspace"
        }
        if (chaftController.runtimeLocked) {
            return "Locked"
        }
        if (chaftController.syncInFlight) {
            return "Updating"
        }
        if (chaftController.timelineLoadInFlight) {
            return "Loading messages"
        }
        if (chaftController.peerHosting) {
            return "Listening"
        }
        if (root.preferredSyncPeerEndpoint().length === 0) {
            return "Local only"
        }
        if (root.autoSyncEnabled) {
            return "Updates on"
        }
        return "Updates paused"
    }

    readonly property color syncPillTone: {
        if (chaftController.runtimeLocked) {
            return Tokens.warning
        }
        if (root.workspaceOperationInFlight) {
            return Tokens.textMuted
        }
        if (chaftController.peerHosting) {
            return Tokens.textMuted
        }
        return Tokens.textMuted
    }

    function syncDrawerSummaryText() {
        if (chaftController.syncInFlight) {
            return "Checking for newer messages..."
        }
        if (root.preferredSyncPeerEndpoint().length === 0) {
            return "Messages stay on this device until you add a teammate address."
        }
        if (root.autoSyncEnabled) {
            return "Chaft checks the saved teammate address automatically."
        }
        return "A teammate address is saved. Update whenever you are ready."
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
            label: "Open settings",
            shortcut: "",
            enabled: function () {
                return chaftController.deviceId.length > 0
                    || chaftController.hasRuntimeWorkspace
            },
            run: function () { root.openSettings(root.settingsCategory) }
        },
        {
            id: "toggle-sync-drawer",
            label: "Toggle updates and backup",
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
            label: "New room",
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
            label: "Toggle automatic updates",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.autoSyncEnabled = !root.autoSyncEnabled }
        },
        {
            id: "sync-now",
            label: "Update with teammate",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !root.workspaceOperationInFlight
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.syncWorkspaceFromPreferredPeer() }
        },
        {
            id: "backup-now",
            label: "Back up to saved address",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !root.workspaceOperationInFlight
                    && root.preferredManualBackupPeerEndpoint().length > 0
            },
            run: function () { root.backupWorkspaceToPreferredPeer() }
        },
        {
            id: "pull-peer",
            label: "Fetch from teammate",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !root.workspaceOperationInFlight
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.pullWorkspaceFromPreferredPeer() }
        },
        {
            id: "push-peer",
            label: "Share with teammate",
            shortcut: "",
            enabled: function () {
                return root.runtimeWorkReady
                    && !root.workspaceOperationInFlight
                    && root.preferredSyncPeerEndpoint().length > 0
            },
            run: function () { root.publishWorkspaceToPreferredPeer() }
        },
        {
            id: "lock-runtime",
            label: "Lock workspace",
            shortcut: "",
            enabled: function () {
                return chaftController.hasRuntimeWorkspace
                    && chaftController.runtimeUnlocked
                    && chaftController.runtimeUnlockClearable
                    && !chaftController.keyTransferInFlight
                    && !root.workspaceOperationInFlight
                    && !root.composerContextBusy
            },
            run: function () { root.lockWorkspace() }
        },
        {
            id: "reindex-search",
            label: "Refresh search",
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
            label: "Previous or next conversation",
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
            } else if (id === "copy-private-room-help") {
                root.copyPrivateRoomHelpNote()
            } else if (id === "open-received-approval") {
                root.openReceivedApprovalInvite(true)
            } else if (id === "open-access-requests") {
                root.openAccessRequestsPanel()
            } else if (id === "invite-after-create") {
                root.openPeopleAccess(true)
            }
        }
    }

    Timer {
        id: composerDraftPersistTimer
        interval: 450
        repeat: false
        onTriggered: root.persistComposerDrafts()
    }

    Timer {
        id: windowGeometryPersistTimer
        interval: 800
        repeat: false
        onTriggered: root.persistWindowGeometry()
    }

    Timer {
        id: pendingJoinPullCompletionTimer
        interval: 100
        repeat: false
        onTriggered: root.handlePendingJoinPullStatus()
    }

    Timer {
        id: pendingPrivateRoomHistoryRepairCompletionTimer
        interval: 100
        repeat: false
        onTriggered: root.handlePrivateRoomHistoryRepairCompletion()
    }

    Timer {
        id: smokeInvitePackageTimer
        interval: 450
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady || chaftController.keyTransferInFlight) {
                restart()
                return
            }
            chaftController.prepareClaimableWorkspaceInvite(
                "Sam Rivera",
                "member",
                root.preferredInvitePeerEndpoint(),
                root.inviteExpiresAtIso(7))
        }
    }

    Timer {
        id: smokeApprovalInvitePackageTimer
        interval: 450
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady || chaftController.keyTransferInFlight) {
                restart()
                return
            }
            chaftController.prepareApprovalInvitePackage(
                "dev_visual_smoke_joiner",
                "member",
                root.preferredInvitePeerEndpoint(),
                "Sam Rivera",
                root.inviteExpiresAtIso(7))
        }
    }

    Timer {
        id: smokeApprovedRequestInviteTimer
        interval: 450
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady || chaftController.keyTransferInFlight) {
                restart()
                return
            }
            chaftController.prepareWorkspaceInvitePackage(
                "dev_visual_smoke_joiner",
                "member",
                root.preferredInvitePeerEndpoint(),
                "Sam Rivera",
                root.inviteExpiresAtIso(7),
                "preapproved",
                "req_visual_smoke_joiner")
        }
    }

    Timer {
        id: smokeRequestReviewTimer
        interval: 450
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady
                    || !setupPanel.openFirstWaitingJoinRequestReview()) {
                restart()
            }
        }
    }

    Timer {
        id: smokeEntryRequestSentTimer
        interval: 450
        repeat: false
        onTriggered: {
            if (!root.runtimeAccessReady || chaftController.deviceId.length === 0
                    || chaftController.keyTransferInFlight) {
                restart()
                return
            }
            workspaceEntryDialog.prepareJoinRequestForSmoke()
        }
    }

    Timer {
        id: smokeExternalLinkTimer
        interval: 850
        repeat: false
        onTriggered: root.requestExternalLink("https://example.com/chaft-smoke")
    }

    Timer {
        id: smokeAddWorkspaceTimer
        interval: 850
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady) {
                restart()
                return
            }
            root.openAddWorkspaceChooser()
        }
    }

    Timer {
        id: smokeDirectMessageTimer
        interval: 450
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady || root.currentWorkspaceId().length === 0) {
                restart()
                return
            }
            root.startDirectMessage("dev_visual_smoke_member", "Mina Park")
        }
    }

    Timer {
        id: smokeMemberRolesScrollTimer
        interval: 350
        repeat: false
        onTriggered: root.scrollInspectorToPeople()
    }

    Timer {
        id: smokeFirstSyncWaitingTimer
        interval: 350
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady || root.currentWorkspaceId().length === 0) {
                restart()
                return
            }
            root.rememberJoinWaitingForPeer(root.currentWorkspaceId(), false, "access")
        }
    }

    Timer {
        id: smokeFirstSyncRecoveryTimer
        interval: 350
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady || root.currentWorkspaceId().length === 0) {
                restart()
                return
            }
            root.rememberJoinWaitingForPeer(
                root.currentWorkspaceId(),
                false,
                "recovery",
                2)
        }
    }

    onWidthChanged: root.scheduleWindowGeometryPersist()
    onHeightChanged: root.scheduleWindowGeometryPersist()
    onActiveChanged: {
        if (root.active) {
            root.updateUnreadNotificationBaseline()
            root.scheduleMarkSelectedChannelRead()
        }
    }
    onTotalUnreadCountChanged: root.handleUnreadNotificationCountChanged()
    onClosing: {
        root.persistComposerDrafts()
        root.persistWindowGeometry()
        root.flushPendingDeletes()
    }

    ConfirmDialog {
        id: confirmDialog
        parent: Overlay.overlay
        onConfirmed: function (contextId) {
            var id = String(contextId || "")
            if (id.indexOf("remove-member:") === 0) {
                chaftController.removeMember(id.slice("remove-member:".length))
            } else if (id.indexOf("update-member-role:") === 0) {
                var roleTarget = id.slice("update-member-role:".length)
                var roleSeparator = roleTarget.indexOf("::")
                if (roleSeparator >= 0) {
                    var roleDeviceId = roleTarget.slice(0, roleSeparator)
                    var roleValue = roleTarget.slice(roleSeparator + 2)
                    chaftController.updateMemberRole(roleDeviceId, roleValue)
                }
            } else if (id.indexOf("revoke-workspace-invite:") === 0) {
                chaftController.resolveWorkspaceInvite(
                    id.slice("revoke-workspace-invite:".length),
                    "revoked")
            } else if (id.indexOf("decline-join-request:") === 0) {
                var declineTarget = id.slice("decline-join-request:".length)
                var declineSeparator = declineTarget.indexOf("::")
                var declineRequestId = declineSeparator >= 0
                    ? decodeURIComponent(declineTarget.slice(0, declineSeparator))
                    : declineTarget
                var declineResponseEndpoint = declineSeparator >= 0
                    ? decodeURIComponent(declineTarget.slice(declineSeparator + 2))
                    : ""
                chaftController.resolveWorkspaceJoinRequest(
                    declineRequestId,
                    "declined",
                    declineResponseEndpoint)
            } else if (id.indexOf("revoke-join-request:") === 0) {
                var revokeJoinTarget = id.slice("revoke-join-request:".length)
                var revokeJoinSeparator = revokeJoinTarget.indexOf("::")
                var revokeJoinRequestId = revokeJoinSeparator >= 0
                    ? decodeURIComponent(revokeJoinTarget.slice(0, revokeJoinSeparator))
                    : revokeJoinTarget
                var revokeJoinResponseEndpoint = revokeJoinSeparator >= 0
                    ? decodeURIComponent(revokeJoinTarget.slice(revokeJoinSeparator + 2))
                    : ""
                chaftController.resolveWorkspaceJoinRequest(
                    revokeJoinRequestId,
                    "revoked",
                    revokeJoinResponseEndpoint)
            } else if (id.indexOf("replace-workspace-invite:") === 0) {
                setupPanel.startPendingWorkspaceInviteReplacement(
                    id.slice("replace-workspace-invite:".length))
            } else if (id === "discard-secure-invite-response") {
                setupPanel.discardSecureInviteResponse()
            } else if (id.indexOf("dismiss-pending-access-request:") === 0) {
                root.dismissPendingAccessRequest({
                    key: id.slice("dismiss-pending-access-request:".length)
                })
            } else if (id === "hide-first-sync-waiting") {
                root.clearJoinWaitingForPeer()
            } else if (id.indexOf("revoke-channel-member:") === 0) {
                var revokeTarget = id.slice("revoke-channel-member:".length)
                var revokeChannelId = root.selectedChannelKey
                var revokeDeviceId = revokeTarget
                var revokeSeparator = revokeTarget.indexOf("::")
                if (revokeSeparator >= 0) {
                    revokeChannelId = revokeTarget.slice(0, revokeSeparator)
                    revokeDeviceId = revokeTarget.slice(revokeSeparator + 2)
                }
                chaftController.removeChannelMember(revokeChannelId, revokeDeviceId)
            } else if (id.indexOf("leave-private-room:") === 0) {
                var leaveChannelId = id.slice("leave-private-room:".length)
                if (leaveChannelId.length > 0 && chaftController.deviceId.length > 0) {
                    chaftController.removeChannelMember(
                        leaveChannelId,
                        chaftController.deviceId)
                }
            } else if (id === "rotate-keys") {
                chaftController.rotateWorkspaceManualKeys()
            } else if (id.indexOf("rotate-channel-key:") === 0) {
                chaftController.rotateChannelKey(id.slice("rotate-channel-key:".length))
            } else if (id === "invite-without-post-create-key-kit") {
                postCreateExportDialog.explicitlyDeferred = true
                root.setKeyKitReminder(postCreateExportDialog.workspaceId, false)
                postCreateExportDialog.close()
                root.openPeopleAccess(true)
            }
        }
    }

    Dialog {
        id: infoDialog
        property string message: ""

        modal: true
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(440, parent ? parent.width - Tokens.space4 * 2 : 440)
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        ColumnLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            spacing: Tokens.space3

            Text {
                Layout.fillWidth: true
                text: infoDialog.message
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                wrapMode: Text.WordWrap
            }

            Button {
                Layout.alignment: Qt.AlignRight
                text: "OK"
                onClicked: infoDialog.close()
            }
        }
    }

    function showInfo(title, message) {
        infoDialog.title = String(title || "Details")
        infoDialog.message = String(message || "")
        infoDialog.open()
    }

    Dialog {
        id: externalLinkDialog

        modal: true
        parent: Overlay.overlay
        anchors.centerIn: parent
        title: "Open link?"
        width: Math.min(500, parent ? parent.width - Tokens.space4 * 2 : 500)
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        ColumnLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            spacing: Tokens.space3

            Text {
                Layout.fillWidth: true
                text: "This opens outside Chaft."
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                wrapMode: Text.WordWrap
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: Math.max(44, externalLinkText.implicitHeight + Tokens.space3)
                radius: Tokens.radiusSm
                color: Tokens.surfaceRaised
                border.color: Tokens.borderSubtle

                Text {
                    id: externalLinkText
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: Tokens.space2
                    anchors.rightMargin: Tokens.space2
                    text: root.compactExternalLink(root.pendingExternalLink)
                    color: Tokens.textMuted
                    font.family: Tokens.fontMono
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WrapAnywhere
                }
            }

            CheckBox {
                id: externalLinkSkipConfirmation
                text: "Don't ask again"
                Accessible.name: "Do not ask again before opening external links"
            }

            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: Tokens.space2

                Button {
                    text: "Cancel"
                    onClicked: externalLinkDialog.close()
                }

                Button {
                    text: "Open link"
                    enabled: root.pendingExternalLink.length > 0
                    onClicked: root.openPendingExternalLink()
                }
            }
        }
    }

    function externalLinkScheme(link) {
        var match = String(link || "").trim().match(/^([a-zA-Z][a-zA-Z0-9+.-]*):/)
        return match ? match[1].toLowerCase() : ""
    }

    function externalLinkAllowed(link) {
        var scheme = root.externalLinkScheme(link)
        return scheme === "http" || scheme === "https" || scheme === "mailto"
    }

    function compactExternalLink(link) {
        var value = String(link || "").trim()
        return value.length > 220 ? value.slice(0, 217) + "..." : value
    }

    function requestExternalLink(link) {
        var value = String(link || "").trim()
        if (value.length === 0) {
            return
        }
        if (!root.externalLinkAllowed(value)) {
            root.showInfo(
                "Link blocked",
                "Chaft opens web and email links from messages. This link uses an app type Chaft does not open from chat.")
            return
        }
        if (!chaftController.externalLinkConfirmationEnabled) {
            Qt.openUrlExternally(value)
            return
        }
        root.pendingExternalLink = value
        externalLinkSkipConfirmation.checked = false
        externalLinkDialog.open()
    }

    function openPendingExternalLink() {
        var value = String(root.pendingExternalLink || "").trim()
        if (value.length === 0) {
            externalLinkDialog.close()
            return
        }
        if (externalLinkSkipConfirmation.checked) {
            chaftController.externalLinkConfirmationEnabled = false
        }
        externalLinkDialog.close()
        root.pendingExternalLink = ""
        Qt.openUrlExternally(value)
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

    function joinRequestStatusCount(rows, status) {
        var count = 0
        var normalizedStatus = String(status || "").trim()
        var source = rows || []
        for (var i = 0; i < source.length; i += 1) {
            if (String(source[i].status || "").trim() === normalizedStatus) {
                count += 1
            }
        }
        return count
    }

    function accessRequestCountLabel(count) {
        return Number(count) === 1 ? "1 request" : String(count) + " requests"
    }

    function accessRequestBadgeLabel(count) {
        return String(Number(count || 0)) + " req"
    }

    function normalizedSettingsCategory(categoryId) {
        var value = String(categoryId || "profile")
        if (value === "profile" || value === "preferences") {
            return value
        }
        if (chaftController.hasRuntimeWorkspace
                && (value === "workspace"
                    || value === "devices"
                    || value === "backup"
                    || value === "advanced")) {
            return value
        }
        return "profile"
    }

    function openSettings(categoryId) {
        if (chaftController.deviceId.length === 0
                && !chaftController.hasRuntimeWorkspace) {
            return false
        }
        if (root.conversationDestination
                && root.composerContextChangeBlocked()) {
            return false
        }
        root.saveCurrentDraft()
        root.settingsCategory = root.normalizedSettingsCategory(
            categoryId || root.settingsCategory)
        root.mainDestination = "settings"
        return true
    }

    function openPeopleAccess(focusInvite) {
        if (!chaftController.hasRuntimeWorkspace) {
            return false
        }
        if (root.conversationDestination
                && root.composerContextChangeBlocked()) {
            return false
        }
        root.saveCurrentDraft()
        root.mainDestination = "peopleAccess"
        Qt.callLater(function() {
            if (focusInvite) {
                setupPanel.focusInviteForm()
            } else {
                setupPanel.openPeopleAccessSection()
            }
        })
        return true
    }

    function closeMainDestination(focusComposerAfterClose) {
        if (root.conversationDestination) {
            return false
        }
        root.mainDestination = "conversation"
        if (focusComposerAfterClose && root.hasWorkspaceContent) {
            Qt.callLater(function() {
                composer.focusDraft()
            })
        }
        return true
    }

    function resetAccessRequestNotificationBaseline() {
        root.accessRequestNotificationWorkspaceId = root.currentWorkspaceId()
        root.accessRequestNotificationBaseline = root.waitingAccessRequestCount
        root.accessRequestNotificationsReady = false
    }

    function handleAccessRequestNotification() {
        var workspaceId = root.currentWorkspaceId()
        var count = Number(root.waitingAccessRequestCount || 0)
        if (!root.runtimeWorkReady
                || workspaceId.length === 0
                || !root.canManageWorkspaceAccess()) {
            root.accessRequestNotificationWorkspaceId = workspaceId
            root.accessRequestNotificationBaseline = count
            root.accessRequestNotificationsReady = false
            return false
        }
        if (root.accessRequestNotificationWorkspaceId !== workspaceId
                || !root.accessRequestNotificationsReady) {
            root.accessRequestNotificationWorkspaceId = workspaceId
            root.accessRequestNotificationBaseline = count
            root.accessRequestNotificationsReady = true
            return false
        }
        if (count > root.accessRequestNotificationBaseline) {
            var added = count - root.accessRequestNotificationBaseline
            if (!root.peopleAccessDestination) {
                toastHost.show(
                    "info",
                    added === 1
                        ? "New access request waiting."
                        : String(added) + " new access requests waiting.",
                    "Open",
                    "open-access-requests",
                    7000)
            }
        }
        root.accessRequestNotificationBaseline = count
        return true
    }

    function openAccessRequestsPanel() {
        if (!root.openPeopleAccess(false)) {
            return false
        }
        Qt.callLater(function() {
            setupPanel.openAccessRequestsSection()
        })
        return true
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

    function physicalMsTimeLabel(physicalMs) {
        var value = Number(physicalMs)
        if (isNaN(value) || value <= 0) {
            return ""
        }
        return Qt.formatDateTime(new Date(value), "MMM d HH:mm")
    }

    function pendingAccessRequestReceiptLabel(row) {
        var status = String((row && row.status) || "").trim()
        var sentAt = root.backupPeerTimeLabel((row && row.sentAt) || "")
        var lastAttemptAt = root.backupPeerTimeLabel((row && row.lastAttemptAt) || "")
        var resolvedAt = root.backupPeerTimeLabel((row && row.resolvedAt) || "")
        var createdAt = root.backupPeerTimeLabel((row && row.createdAt) || "")
        if (status === "approved" && resolvedAt.length > 0) {
            return "Approved " + resolvedAt
        }
        if (status === "declined" && resolvedAt.length > 0) {
            return "Declined " + resolvedAt
        }
        if (status === "closed" && resolvedAt.length > 0) {
            return "Closed " + resolvedAt
        }
        if (status === "unverified_response" && resolvedAt.length > 0) {
            return "Notice received " + resolvedAt
        }
        if (status === "sent" && sentAt.length > 0) {
            return "Sent " + sentAt
        }
        if (status === "sent_unpersisted") {
            return "Sent · storage retry needed"
        }
        if (status === "send_failed" && lastAttemptAt.length > 0) {
            return "Last tried " + lastAttemptAt
        }
        if (status === "sending" && lastAttemptAt.length > 0) {
            return "Trying again since " + lastAttemptAt
        }
        if (status === "copied" && createdAt.length > 0) {
            return "Copied " + createdAt
        }
        if (status === "file_ready" && createdAt.length > 0) {
            return "File ready " + createdAt
        }
        if (createdAt.length > 0) {
            return "Created " + createdAt
        }
        return ""
    }

    function pendingAccessRequestStatusMessage(row) {
        var status = String((row && row.status) || "").trim()
        var label = String((row && row.deliveryLabel) || "an owner or admin")
        var secureClaim = String((row && row.sourceType) || "").trim()
            === "invite_claim"
        if (status === "approved") {
            return secureClaim
                ? "Encrypted workspace access is ready. Open it here to finish joining."
                : "An admin approved this request. Open the received invite here to finish joining."
        }
        if (status === "unverified_response") {
            var notice = String((row && row.error) || "").trim()
            return notice.length > 0
                ? notice
                : "An unsigned response was received. Confirm it with a workspace admin before hiding or resending this request."
        }
        if (status === "sent_unpersisted") {
            return "The request was sent, but Chaft could not save that status. Restore disk access; response checks will keep running."
        }
        if (status === "declined") {
            return "An admin declined this request. Ask for a fresh invite or send a new request if this changed."
        }
        if (status === "closed") {
            return "An admin closed this request. Send a new request if you still need access."
        }
        if (status === "sent") {
            if (secureClaim) {
                return "Join request delivered to " + label
                    + ". Keep Chaft open while it checks for encrypted workspace access."
            }
            return row.canSendDirect
                ? "Sent to " + label
                    + ". Check after they approve, open the invite here when it arrives, or resend if they did not receive it."
                : "Sent to " + label + ". Check after they approve, then open the invite here."
        }
        if (status === "sending") {
            return secureClaim
                ? "Sending the join request to " + label + "."
                : "Sending to " + label + "."
        }
        if (status === "send_failed") {
            if (secureClaim) {
                return "Could not reach " + label
                    + ". Copy or save the join request, or try again when their Chaft device is reachable."
            }
            return "Could not reach " + label
                + ". Copy the request link or save the file, then send it to them. Check after they approve, then open the invite here."
        }
        if (status === "copied") {
            return secureClaim
                ? "Send the copied join request to " + label
                    + ", then open the encrypted access response here."
                : "Send the copied request link to " + label
                    + ". Check after they approve, then open the invite here."
        }
        if (status === "file_ready") {
            return secureClaim
                ? "Send the saved join request to " + label
                    + ", then open the encrypted access response here."
                : "Save or send the request file to " + label
                    + ". Check after they approve, then open the invite here."
        }
        if (secureClaim) {
            return row.canSendDirect
                ? "Send the join request now, or copy or save it for " + label
                    + ". Open the encrypted access response here when it arrives."
                : "Copy or save the join request, then send it to " + label
                    + ". Open the encrypted access response here when it arrives."
        }
        return row.canSendDirect
            ? "Send it now, or share the request link or file with "
                + label + ". Open the invite here when it arrives."
            : "Copy the request link or save the file, then send it to "
                + label + ". Open the invite here when it arrives."
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
        return score > 0 ? " | needs review" : ""
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
            return "Needs review"
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
            : (String(status.lastSuccessAt || "").length > 0
                ? Tokens.success
                : Tokens.textMuted)
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
        if (route === "iroh-relay") {
            return transport === "iroh" || transport === "iroh-relay"
        }
        if (route === "iroh-discovery") {
            return transport === "iroh" || transport === "iroh-discovery"
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

    function nativeIrohRelayUrlIsValid(relayUrl) {
        var value = String(relayUrl || "").trim()
        return value.length > 0
            && !root.containsAsciiWhitespace(value)
            && /^https:\/\/[^/]+/.test(value)
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
            if (querySeparator === 0) {
                return "unsupported"
            }
            var endpointId = querySeparator < 0 ? rest : rest.slice(0, querySeparator)
            if (!root.nativeIrohEndpointIdSyntaxIsValid(endpointId)) {
                return "unsupported"
            }
            if (querySeparator < 0) {
                return "iroh-discovery"
            }
            var query = rest.slice(querySeparator + 1)
            var fragmentSeparator = query.indexOf("#")
            if (fragmentSeparator >= 0) {
                query = query.slice(0, fragmentSeparator)
            }
            var parameters = query.split("&")
            var hasDirectAddr = false
            var hasRelay = false
            for (var i = 0; i < parameters.length; i++) {
                var parameter = parameters[i]
                var equals = parameter.indexOf("=")
                var key = equals >= 0 ? parameter.slice(0, equals).trim() : parameter.trim()
                var value = equals >= 0 ? parameter.slice(equals + 1).trim() : ""
                if (key === "relay") {
                    if (!root.nativeIrohRelayUrlIsValid(value)) {
                        return "unsupported"
                    }
                    hasRelay = true
                    continue
                }
                if (key !== "addr" || !root.nativeIrohDirectAddrIsValid(value)) {
                    return "unsupported"
                }
                hasDirectAddr = true
            }
            if (hasRelay) {
                return "iroh-relay"
            }
            return hasDirectAddr ? "iroh-direct" : "unsupported"
        }
        if (normalized.indexOf("://") >= 0) {
            return "unsupported"
        }
        return root.directTcpAddressIsValid(normalized) ? "direct-tcp" : "unsupported"
    }

    function preferredSyncPeerEndpoint() {
        if (root.smokePrivateRoomRepairSavedAddressActive) {
            return "direct+tcp://127.0.0.1:44944"
        }
        var manualEndpoint = String(peerEndpointField.text || "").trim()
        if (manualEndpoint.length > 0
                && root.supportedPeerEndpointRouteKind(manualEndpoint)
                    !== "unsupported") {
            return manualEndpoint
        }

        var savedEndpoint = String(
            chaftController.defaultPeerEndpoint || "").trim()
        if (savedEndpoint.length > 0
                && root.supportedPeerEndpointRouteKind(savedEndpoint)
                    !== "unsupported") {
            return savedEndpoint
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

    function preferredInvitePeerEndpoint() {
        return String(chaftController.hostedPeerEndpoint || "").trim()
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
            return "Direct address"
        case "iroh-direct":
            return "Direct address"
        case "iroh-relay":
            return "Relay address"
        case "iroh-discovery":
            return "Discovery address"
        case "custom":
            return "Custom address"
        default:
            return "Not sharing"
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
            return "Workspace locked"
        }
        if (chaftController.runtimeUnlockRequired) {
            return "Unlock workspace"
        }
        if (chaftController.rawEventStoreMode) {
            return "Read-only history"
        }
        if (!chaftController.hasRuntimeWorkspace) {
            return "No workspace"
        }
        if (chaftController.syncInFlight) {
            return "Updating"
        }
        if (chaftController.timelineLoadInFlight) {
            return "Loading messages"
        }

        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length === 0) {
            return (root.queuedPublishableEventCount > 0 || root.queuedBackupEventCount > 0)
                ? "Local history"
                : "Local only"
        }
        return root.endpointIsBackupPeer(endpoint)
            ? "Backup address"
            : root.endpointRouteLabel(endpoint)
    }

    function activePeerRouteDetail() {
        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length === 0) {
            return root.publishQueueDetailText()
        }
        return endpoint
    }

    function activePeerRouteIsWarning() {
        return chaftController.runtimeLocked
            || chaftController.runtimeUnlockRequired
    }

    function shortDeviceId(deviceId) {
        var value = String(deviceId || "")
        return value.length > 14 ? value.slice(0, 7) + "..." + value.slice(value.length - 4) : value
    }

    function supportDeviceCodeLabel(deviceId) {
        var value = String(deviceId || "").trim()
        if (value.length === 0) {
            return "Unavailable"
        }
        var shortValue = root.shortDeviceId(value)
        return shortValue === value ? value : shortValue + " (full code: " + value + ")"
    }

    function peerEndpointKindLabel(peer) {
        var title = Boolean((peer || {}).isBackupPeer) ? "Backup address" : "Teammate address"
        var route = root.endpointRouteLabel((peer || {}).endpoint)
        return route.length > 0 && route !== "Not sharing" ? title + " - " + route : title
    }

    function peerEndpointDetailLabel(peer) {
        var displayName = String((peer || {}).displayName || "").trim()
        var device = root.shortDeviceId((peer || {}).deviceId)
        var endpointId = String((peer || {}).endpointId || "")
        var expiry = root.peerEndpointExpiryLabel((peer || {}).expiresAtMs)
        var parts = []
        if (displayName.length > 0) {
            parts.push(displayName)
        } else if (device.length > 0) {
            parts.push("Unnamed teammate")
        }
        if (device.length > 0) {
            parts.push("Support code " + device)
        }
        if (endpointId.length > 0) {
            parts.push("Route ID " + root.shortDeviceId(endpointId))
        }
        if (expiry.length > 0) {
            parts.push(expiry)
        }
        return parts.join(" - ")
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

    function peerEndpointFormValue() {
        return String(peerEndpointField.text || "").trim()
    }

    function peerEndpointFormIsValid() {
        return root.supportedPeerEndpointRouteKind(
            root.peerEndpointFormValue()) !== "unsupported"
    }

    function savePeerEndpointFromField() {
        var endpoint = root.peerEndpointFormValue()
        if (!root.peerEndpointFormIsValid()) {
            root.peerEndpointFormError = "Enter a valid teammate address."
            return false
        }
        chaftController.defaultPeerEndpoint = endpoint
        root.peerEndpointFormError = ""
        toastHost.show("success", "Teammate address saved.", "", "", 2500)
        return true
    }

    function connectPeerEndpointFromField() {
        var endpoint = root.peerEndpointFormValue()
        if (!root.peerEndpointFormIsValid()) {
            root.peerEndpointFormError = "Enter a valid teammate address."
            return false
        }
        root.peerEndpointFormError = ""
        if (!root.runtimeWorkReady || root.workspaceOperationInFlight) {
            return false
        }
        chaftController.syncWorkspace(endpoint)
        return true
    }

    function currentWorkspaceId() {
        var selectedWorkspaceId = String(chaftController.selectedWorkspaceId || "")
        if (selectedWorkspaceId.length > 0) {
            return selectedWorkspaceId
        }
        return String(root.workspaceSnapshot.workspaceId || "")
    }

    function channelMuteKey(workspaceId, channelId) {
        var normalizedWorkspaceId = String(workspaceId || "")
        var normalizedChannelId = String(channelId || "")
        return normalizedWorkspaceId.length > 0 && normalizedChannelId.length > 0
            ? normalizedWorkspaceId + "::" + normalizedChannelId
            : ""
    }

    function channelIdMuted(workspaceId, channelId) {
        var key = root.channelMuteKey(workspaceId, channelId)
        return key.length > 0 && root.mutedChannels[key] === true
    }

    function channelMuted(channel) {
        var row = channel || {}
        return root.channelIdMuted(root.currentWorkspaceId(), row.channelId)
    }

    function channelArchived(channel) {
        var row = channel || {}
        return row.archived === true
    }

    function channelMemberCount(channel) {
        var row = channel || {}
        if (row.memberCount !== undefined && row.memberCount !== null) {
            var parsed = Number(row.memberCount)
            if (!isNaN(parsed) && parsed >= 0) {
                var count = Math.floor(parsed)
                if (!root.channelIsDirectMessage(row) || count > 0) {
                    return count
                }
            }
        }
        if (root.channelIsDirectMessage(row)) {
            var directIds = root.channelDirectMessageParticipantDeviceIds(row)
            return directIds.length > 0 ? directIds.length : 2
        }
        return Boolean(row.isPrivate) ? 0 : root.memberCount
    }

    function channelMemberDeviceIds(channel) {
        var row = channel || {}
        var ids = row.memberDeviceIds || []
        var normalized = []
        for (var i = 0; i < ids.length; i += 1) {
            var id = String(ids[i] || "").trim()
            if (id.length > 0 && normalized.indexOf(id) === -1) {
                normalized.push(id)
            }
        }
        return normalized
    }

    function channelDirectMessageParticipantDeviceIds(channel) {
        var row = channel || {}
        var ids = row.directMessageParticipantDeviceIds || []
        if (ids.length === 0) {
            ids = row.memberDeviceIds || []
        }
        var normalized = []
        for (var i = 0; i < ids.length; i += 1) {
            var id = String(ids[i] || "").trim()
            if (id.length > 0 && normalized.indexOf(id) === -1) {
                normalized.push(id)
            }
        }
        return normalized
    }

    function deviceIdListContains(deviceIds, deviceId) {
        var target = String(deviceId || "").trim()
        if (target.length === 0) {
            return false
        }
        var ids = deviceIds || []
        for (var i = 0; i < ids.length; i += 1) {
            if (String(ids[i] || "") === target) {
                return true
            }
        }
        return false
    }

    function memberByDeviceId(deviceId) {
        var target = String(deviceId || "").trim()
        if (target.length === 0) {
            return null
        }
        for (var i = 0; i < root.members.length; i += 1) {
            if (String(root.members[i].deviceId || "") === target) {
                return root.members[i]
            }
        }
        return null
    }

    function memberAvatarId(member) {
        var row = member || ({})
        var resolvedAvatarId = root.avatarIdForDevice(
            String(row.deviceId || ""))
        if (AvatarCatalog.isValid(resolvedAvatarId)) {
            return resolvedAvatarId
        }
        var directAvatarId = String(row.avatarId || "").trim()
        return AvatarCatalog.isValid(directAvatarId) ? directAvatarId : ""
    }

    function avatarIdForDevice(deviceId) {
        var target = String(deviceId || "").trim()
        if (target.length === 0) {
            return ""
        }
        for (var linkIndex = 0;
                linkIndex < root.personDeviceLinks.length;
                linkIndex += 1) {
            var link = root.personDeviceLinks[linkIndex] || ({})
            if (String(link.deviceId || "").trim() !== target) {
                continue
            }
            var linkedAvatarId = String(
                link.personAvatarId || "").trim()
            if (AvatarCatalog.isValid(linkedAvatarId)) {
                return linkedAvatarId
            }
            var linkedDeviceAvatarId = String(
                link.deviceAvatarId || "").trim()
            if (AvatarCatalog.isValid(linkedDeviceAvatarId)) {
                return linkedDeviceAvatarId
            }
        }
        for (var memberIndex = 0;
                memberIndex < root.members.length;
                memberIndex += 1) {
            var member = root.members[memberIndex] || ({})
            if (String(member.deviceId || "").trim() === target
                    && AvatarCatalog.isValid(member.avatarId)) {
                return String(member.avatarId)
            }
        }
        for (var profileIndex = 0;
                profileIndex < root.profiles.length;
                profileIndex += 1) {
            var profile = root.profiles[profileIndex] || ({})
            if (String(profile.deviceId || "").trim() === target
                    && AvatarCatalog.isValid(profile.avatarId)) {
                return String(profile.avatarId)
            }
        }
        return ""
    }

    function usedWorkspaceAvatarIds() {
        var ids = []
        for (var i = 0; i < root.members.length; i += 1) {
            var member = root.members[i] || ({})
            var avatarId = root.memberAvatarId(member)
            if (!AvatarCatalog.isValid(avatarId)) {
                avatarId = AvatarCatalog.deterministicAvatarId(
                    root.currentWorkspaceId(),
                    String(member.deviceId || ""))
            }
            if (AvatarCatalog.isValid(avatarId)
                    && ids.indexOf(avatarId) === -1) {
                ids.push(avatarId)
            }
        }
        return ids
    }

    function directMessageParticipantDeviceId(channel) {
        var ids = root.channelDirectMessageParticipantDeviceIds(channel)
        var localDeviceId = String(chaftController.deviceId || "").trim()
        for (var i = 0; i < ids.length; i += 1) {
            var deviceId = String(ids[i] || "").trim()
            if (deviceId.length > 0 && deviceId !== localDeviceId) {
                return deviceId
            }
        }
        return ids.length === 1 ? String(ids[0] || "") : ""
    }

    function directMessageAvatarId(channel) {
        var deviceId = root.directMessageParticipantDeviceId(channel)
        var member = root.memberByDeviceId(deviceId)
        return member === null
            ? root.avatarIdForDevice(deviceId)
            : root.memberAvatarId(member)
    }

    function memberAccessRow(member, deviceId) {
        var row = member || ({})
        var normalizedDeviceId = String(deviceId || row.deviceId || "").trim()
        var displayName = member === null ? "" : String(row.displayName || "").trim()
        var hasDisplayName = displayName.length > 0
        return ({
            deviceId: normalizedDeviceId,
            displayLabel: !hasDisplayName
                ? root.unnamedPersonLabel(normalizedDeviceId)
                : displayName,
            grantDisplayLabel: !hasDisplayName ? "Unnamed teammate" : displayName,
            supportLabel: !hasDisplayName && normalizedDeviceId.length > 0
                ? "Support code " + root.shortDeviceId(normalizedDeviceId)
                : "",
            roleLabel: member === null ? "" : root.roleLabel(row.role)
        })
    }

    function membersWithDeviceIds(deviceIds) {
        var rows = []
        var ids = deviceIds || []
        for (var i = 0; i < ids.length; i += 1) {
            var deviceId = String(ids[i] || "").trim()
            if (deviceId.length === 0) {
                continue
            }
            rows.push(root.memberAccessRow(root.memberByDeviceId(deviceId), deviceId))
        }
        return rows
    }

    function channelAccessHistory(channel) {
        return ((channel || {}).accessHistory || [])
    }

    function recentChannelAccessHistory(limit) {
        var rows = root.selectedChannelAccessHistory || []
        var max = Number(limit || 0)
        return max > 0 ? rows.slice(0, max) : rows
    }

    function channelAccessHistoryRows(limit, expanded) {
        var rows = root.selectedChannelAccessHistory || []
        if (expanded) {
            return rows
        }
        var max = Number(limit || 0)
        return max > 0 ? rows.slice(0, max) : rows
    }

    function channelAccessHistoryToggleText(limit, expanded) {
        var rows = root.selectedChannelAccessHistory || []
        if (expanded) {
            return "Show less"
        }
        return "Show all " + String(rows.length)
    }

    function channelAccessHistoryActorText(row) {
        var actor = String((row || {}).actorDisplayName || "").trim()
        if (actor.length === 0) {
            var deviceId = String((row || {}).actorDeviceId || "").trim()
            actor = deviceId.length > 0 ? root.unnamedPersonLabel(deviceId) : "Unknown"
        }
        var time = root.physicalMsTimeLabel((row || {}).physicalMs)
        return time.length > 0 ? "By " + actor + " - " + time : "By " + actor
    }

    function privateChannelGrantCandidates() {
        if (!root.selectedChannelPrivate || root.selectedChannelDirectMessage) {
            return []
        }
        var rows = []
        var existing = root.selectedChannelMemberDeviceIds || []
        for (var i = 0; i < root.members.length; i += 1) {
            var member = root.members[i]
            var deviceId = String(member.deviceId || "").trim()
            if (deviceId.length > 0 && !root.deviceIdListContains(existing, deviceId)) {
                rows.push(root.memberAccessRow(member, deviceId))
            }
        }
        var unnamedCount = 0
        for (var j = 0; j < rows.length; j += 1) {
            if (String(rows[j].supportLabel || "").length > 0) {
                unnamedCount += 1
            }
        }
        if (unnamedCount > 1) {
            var unnamedIndex = 0
            for (var k = 0; k < rows.length; k += 1) {
                if (String(rows[k].supportLabel || "").length > 0) {
                    unnamedIndex += 1
                    rows[k].grantDisplayLabel = "Unnamed teammate " + String(unnamedIndex)
                }
            }
        }
        return rows
    }

    function peopleCountLabel(count) {
        var parsed = Number(count || 0)
        var value = (!isNaN(parsed) && parsed >= 0) ? Math.floor(parsed) : 0
        return String(value) + " " + (value === 1 ? "person" : "people")
    }

    function humanList(labels) {
        var rows = labels || []
        if (rows.length === 0) {
            return ""
        }
        if (rows.length === 1) {
            return String(rows[0] || "")
        }
        if (rows.length === 2) {
            return String(rows[0] || "") + " and " + String(rows[1] || "")
        }
        return String(rows.slice(0, rows.length - 1).join(", "))
            + ", and "
            + String(rows[rows.length - 1] || "")
    }

    function privateRoomAccessMemberLabels(limit, includeLocal) {
        var rows = root.selectedChannelAccessMembers || []
        var labels = []
        var max = Number(limit || 0)
        var localDeviceId = String(chaftController.deviceId || "").trim()
        for (var i = 0; i < rows.length; i += 1) {
            var row = rows[i] || ({})
            var deviceId = String(row.deviceId || "").trim()
            if (!includeLocal && deviceId.length > 0 && deviceId === localDeviceId) {
                continue
            }
            var label = String(row.displayLabel || "").trim()
            if (label.length === 0 && deviceId.length > 0) {
                label = root.unnamedPersonLabel(deviceId)
            }
            if (label.length === 0) {
                continue
            }
            labels.push(label)
            if (max > 0 && labels.length >= max) {
                break
            }
        }
        return labels
    }

    function privateRoomAccessPeopleText() {
        var rows = root.selectedChannelAccessMembers || []
        var labels = []
        var localDeviceId = String(chaftController.deviceId || "").trim()
        for (var i = 0; i < rows.length; i += 1) {
            var row = rows[i] || ({})
            var deviceId = String(row.deviceId || "").trim()
            var label = String(row.displayLabel || "").trim()
            if (label.length === 0 && deviceId.length > 0) {
                label = root.unnamedPersonLabel(deviceId)
            }
            if (label.length === 0) {
                continue
            }
            if (deviceId.length > 0 && deviceId === localDeviceId) {
                label += " (you)"
            }
            var role = String(row.roleLabel || "").trim()
            if (role.length > 0) {
                label += " - " + role
            }
            labels.push(label)
        }
        return labels.length > 0
            ? labels.join(", ")
            : "No room members are listed in this snapshot."
    }

    function privateRoomHistoryHelperLabel() {
        var labels = root.privateRoomAccessMemberLabels(2, false)
        if (labels.length > 0) {
            return root.humanList(labels)
        }
        return "someone with this room's history"
    }

    function selectedChannelPeopleSummary() {
        var count = root.selectedChannelMemberCount
        if (root.selectedChannelDirectMessage) {
            return root.peopleCountLabel(count) + " in this conversation"
        }
        if (root.selectedChannelPrivate) {
            return root.peopleCountLabel(count) + " can read this private room"
        }
        if (count === 1) {
            return "1 person in the workspace can read this room"
        }
        return "All " + root.peopleCountLabel(count) + " in the workspace can read this room"
    }

    function selectedConversationNoun() {
        return root.selectedChannelDirectMessage ? "conversation" : "room"
    }

    function selectedMuteAccessibleName() {
        return (root.selectedChannelMuted ? "Unmute " : "Mute ")
            + root.selectedConversationNoun()
    }

    function selectedMuteTooltip() {
        if (root.selectedChannelMuted) {
            return "Let this " + root.selectedConversationNoun()
                + " count toward notifications again"
        }
        return root.selectedChannelDirectMessage
            ? "Keep unread in this conversation quiet"
            : "Keep unread here quiet"
    }

    function selectedTimelineEmptyText() {
        var name = String(root.selectedChannelDisplayName || "").trim()
        if (root.normalizedSearchQuery.length > 0) {
            return root.selectedChannelDirectMessage && name.length > 0
                ? "No matching messages with " + name
                : "No matching messages"
        }
        if (root.selectedChannelDirectMessage) {
            return name.length > 0
                ? "No messages with " + name + " yet"
                : "No messages in this conversation yet"
        }
        if (root.selectedChannelPrivate) {
            return "No messages in this private room yet"
        }
        if (root.selectedChannelArchived) {
            return "No messages in this archived room yet"
        }
        return "No messages in this room yet"
    }

    function privateRoomReadabilityTitle() {
        if (root.smokePrivateRoomRepairFailureActive) {
            return "Waiting for room history"
        }
        if (root.selectedChannelLockedMessageCount > 0) {
            return "Needs room access here"
        }
        if (root.selectedChannelLoadedMessageCount > 0) {
            return "Loaded messages readable"
        }
        return "Waiting for room history"
    }

    function privateRoomReadabilityText() {
        if (root.smokePrivateRoomRepairFailureActive) {
            return "History did not load here yet. Ask someone with access to keep Chaft open, then fetch again."
        }
        var locked = Number(root.selectedChannelLockedMessageCount || 0)
        if (locked > 0) {
            return String(locked) + " loaded "
                + (locked === 1 ? "message is" : "messages are")
                + " locked here. Fetch history from a teammate who can read "
                + (locked === 1 ? "it" : "them")
                + ", or ask an admin to add you again."
        }
        if (root.selectedChannelLoadedMessageCount > 0) {
            return "Messages already loaded in this private room are readable here. "
                + "Older history may still arrive when a teammate is reachable."
        }
        return "No messages are loaded in this private room yet. "
            + "Fetch history from a teammate to check what can load here."
    }

    function privateRoomKeyRefreshText() {
        if (root.canManageWorkspaceAccess()) {
            return "Use after adding or removing access."
        }
        return "Admins can protect new messages after access changes."
    }

    function privateRoomHistoryHelpText() {
        var helper = root.privateRoomHistoryHelperLabel()
        if (root.selectedChannelLockedMessageCount > 0) {
            return "Copy a note for " + helper
                + " to keep Chaft open. If it stays locked, ask an admin to add you again."
        }
        if (root.selectedChannelLoadedMessageCount === 0) {
            return "Copy a note for " + helper
                + " to keep Chaft open while you fetch history."
        }
        return "Copy a note for a teammate if older private history is missing."
    }

    function privateRoomHistoryRepairActionVisible() {
        return root.smokePrivateRoomRepairFailureActive
            || (root.selectedChannelPrivate
            && !root.selectedChannelDirectMessage
            && (root.selectedChannelLockedMessageCount > 0
                || root.selectedChannelLoadedMessageCount === 0))
    }

    function privateRoomHistoryRepairFailedVisible() {
        return root.privateRoomHistoryRepairActionVisible()
            && root.selectedChannelKey.length > 0
            && (root.smokePrivateRoomRepairFailureActive
                || root.lastPrivateRoomHistoryRepairFailedChannelId === root.selectedChannelKey)
    }

    function privateRoomHistoryRepairFailedText() {
        if (!root.privateRoomHistoryRepairFailedVisible()) {
            return ""
        }
        var helper = root.privateRoomHistoryHelperLabel()
        if (root.preferredSyncPeerEndpoint().length === 0
                && !root.smokePrivateRoomRepairSavedAddressActive) {
            return "No teammate address is saved yet. Add one from someone with this room's history, or open People & Access to message someone who can help."
        }
        if (chaftController.syncInFlight) {
            return "Checking the saved teammate address now. If history still does not load, ask "
                + helper + " to keep Chaft open."
        }
        return "The saved teammate address did not answer. Ask " + helper
            + " to keep Chaft open, or open People & Access to message someone who can help."
    }

    function privateRoomHistoryRepairActionLabel() {
        if (root.preferredSyncPeerEndpoint().length > 0 && chaftController.syncInFlight) {
            return "Fetching..."
        }
        if (root.privateRoomHistoryRepairFailedVisible()
                && root.preferredSyncPeerEndpoint().length > 0) {
            return "Fetch again"
        }
        return root.preferredSyncPeerEndpoint().length > 0 ? "Fetch history" : "Add address"
    }

    function privateRoomHistoryRepairChangeAddressVisible() {
        return root.privateRoomHistoryRepairFailedVisible()
            && root.preferredSyncPeerEndpoint().length > 0
    }

    function privateRoomHistoryRepairActionEnabled() {
        return root.preferredSyncPeerEndpoint().length === 0
            || !root.workspaceOperationInFlight
    }

    function privateRoomHistoryRepairActionTooltip() {
        if (root.preferredSyncPeerEndpoint().length > 0 && chaftController.syncInFlight) {
            return "Chaft is already fetching history"
        }
        return root.preferredSyncPeerEndpoint().length > 0
            ? "Fetch history from a reachable teammate"
            : "Add a teammate address"
    }

    function handlePrivateRoomHistoryRepairAction() {
        if (root.preferredSyncPeerEndpoint().length === 0) {
            return root.focusPeerAddressField()
        }
        root.lastPrivateRoomHistoryRepairFailedChannelId = ""
        if (root.pullWorkspaceFromPreferredPeer()) {
            root.pendingPrivateRoomHistoryRepairCompletion = true
            root.pendingPrivateRoomHistoryRepairChannelId = root.selectedChannelKey
            root.pendingPrivateRoomHistoryRepairRoomName = root.selectedChannelDisplayName
            toastHost.show("info", "Fetching room history", "", "", 3000)
            return true
        }
        root.lastPrivateRoomHistoryRepairFailedChannelId = root.selectedChannelKey
        toastHost.show(
            "warning",
            "History could not fetch yet. Ask someone with room access to keep Chaft open, then fetch again.",
            "Copy help",
            "copy-private-room-help",
            7000)
        return false
    }

    function handlePrivateRoomHistoryRepairCompletion() {
        if (!root.pendingPrivateRoomHistoryRepairCompletion
                || chaftController.syncInFlight) {
            return
        }
        var status = String(chaftController.syncStatus || "").trim()
        if (status.length === 0) {
            return
        }
        var channelId = String(root.pendingPrivateRoomHistoryRepairChannelId || "").trim()
        var roomName = String(root.pendingPrivateRoomHistoryRepairRoomName || "private room").trim()
        root.pendingPrivateRoomHistoryRepairCompletion = false
        root.pendingPrivateRoomHistoryRepairChannelId = ""
        root.pendingPrivateRoomHistoryRepairRoomName = ""
        if (root.historySyncStatusSucceeded(status)) {
            if (channelId.length > 0
                    && root.lastPrivateRoomHistoryRepairFailedChannelId === channelId) {
                root.lastPrivateRoomHistoryRepairFailedChannelId = ""
            }
            toastHost.show(
                "success",
                "History fetch finished. Check #" + roomName + " for newly loaded messages.",
                "",
                "",
                4500)
            return
        }
        root.lastPrivateRoomHistoryRepairFailedChannelId = channelId.length > 0
            ? channelId
            : root.selectedChannelKey
        toastHost.show(
            "warning",
            "History still needs a reachable teammate. Ask someone with room access to keep Chaft open, then fetch again.",
            "Copy help",
            "copy-private-room-help",
            8000)
    }

    function privateRoomHelpCopyText() {
        var workspaceName = String(root.workspaceSnapshot.name || "Workspace").trim()
        var roomName = String(root.selectedChannelDisplayName || "room").trim()
        var helper = root.privateRoomHistoryHelperLabel()
        var nextStep = root.selectedChannelLockedMessageCount > 0
            ? "Keep Chaft open while I fetch history. If it stays locked, an admin may need to add me again."
            : "Keep Chaft open while I fetch history."
        var lines = [
            "Please help me fetch private-room history in Chaft.",
            nextStep,
            "",
            "Workspace: " + workspaceName,
            "Room: #" + roomName,
            "Status: " + root.privateRoomReadabilityTitle(),
            "What I see: " + root.privateRoomReadabilityText(),
            "Who can help: " + helper,
            "People with room access: " + root.privateRoomAccessPeopleText(),
            "This device: " + root.supportDeviceCodeLabel(chaftController.deviceId)
        ]
        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length > 0) {
            lines.push("Saved teammate address: " + endpoint)
        }
        if (root.privateRoomHistoryRepairFailedVisible()) {
            lines.push(root.preferredSyncPeerEndpoint().length > 0
                ? "Last fetch: the saved teammate address did not answer. Send a fresh address if this one changed."
                : "Last fetch: no reachable teammate address was saved.")
        }
        return lines.join("\n")
    }

    function copyPrivateRoomHelpNote() {
        return root.copyTextToClipboard(
            root.privateRoomHelpCopyText(),
            "private room help"
        )
    }

    function privateRoomKeyRefreshUnavailableReason() {
        if (!root.runtimeWorkReady) {
            return "Open a workspace to protect new messages."
        }
        if (!root.canManageWorkspaceAccess()) {
            return root.workspaceAccessUnavailableReason()
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current access change first."
        }
        if (root.selectedChannelKey.length === 0) {
            return "Choose a room first."
        }
        return "Protect new messages in this private room."
    }

    function confirmRefreshSelectedPrivateRoomKey() {
        if (!root.selectedChannelPrivate || root.selectedChannelDirectMessage
                || root.selectedChannelKey.length === 0) {
            return false
        }
        confirmDialog.ask(
            "Protect new messages",
            "Protect future messages in #" + root.selectedChannelDisplayName
                + "? People who still have room access keep access.",
            "Protect",
            "rotate-channel-key:" + root.selectedChannelKey,
            false)
        return true
    }

    function grantSelectedPrivateRoomAccess(comboBox) {
        if (!root.runtimeWorkReady || !root.canManageWorkspaceAccess()
                || root.selectedChannelKey.length === 0) {
            return false
        }
        var candidates = root.selectedChannelGrantCandidates || []
        var index = comboBox ? comboBox.currentIndex : 0
        if (index < 0 || index >= candidates.length) {
            return false
        }
        var deviceId = String(candidates[index].deviceId || "").trim()
        return deviceId.length > 0
            && chaftController.addChannelMember(root.selectedChannelKey, deviceId)
    }

    function confirmRevokeSelectedChannelMember(deviceId, displayLabel) {
        var normalizedDeviceId = String(deviceId || "").trim()
        if (normalizedDeviceId.length === 0 || root.selectedChannelKey.length === 0) {
            return false
        }
        var label = String(displayLabel || "").trim()
        if (label.length === 0) {
            label = "this person"
        }
        confirmDialog.ask(
            "Remove private-room access",
            "Remove " + label + " from this private room? New messages here will be protected from them after the access refresh completes.",
            "Remove",
            "revoke-channel-member:" + root.selectedChannelKey + "::" + normalizedDeviceId,
            true)
        return true
    }

    function toggleSelectedChannelMuted() {
        if (!root.runtimeWorkReady || root.selectedChannelKey.length === 0) {
            return false
        }
        return chaftController.setChannelMuted(
            root.currentWorkspaceId(),
            root.selectedChannelKey,
            !root.selectedChannelMuted)
    }

    function toggleSelectedChannelArchived() {
        if (!root.runtimeWorkReady || root.selectedChannelKey.length === 0
                || root.selectedChannelDirectMessage) {
            return false
        }
        return chaftController.updateChannelArchive(
            root.selectedChannelKey,
            !root.selectedChannelArchived)
    }

    function confirmLeaveSelectedPrivateRoom() {
        if (!root.selectedChannelCanLeave) {
            return false
        }
        confirmDialog.ask(
            "Leave room",
            "Leave this private room? It will disappear from your sidebar on this device. Ask an admin to add you again if you need access later.",
            "Leave",
            "leave-private-room:" + root.selectedChannelKey,
            true)
        return true
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

    function channelIsDirectMessage(channel) {
        var row = channel || {}
        if (row.directMessage === true) {
            return true
        }
        var name = String(row.name || "").trim().toLowerCase()
        return Boolean(row.isPrivate) && name.indexOf("dm-") === 0
    }

    function channelDisplayName(channel) {
        var row = channel || {}
        var name = String(row.name || "").trim()
        if (!root.channelIsDirectMessage(row)) {
            return name.length > 0 ? name : "general"
        }
        var participantLabel = root.directMessageParticipantLabel(row)
        if (participantLabel.length > 0) {
            return participantLabel
        }
        if (name.toLowerCase().indexOf("dm-") !== 0) {
            if (name.length > 0) {
                return name
            }
            var fallbackDeviceLabel = root.directMessageParticipantDeviceLabel(row)
            return fallbackDeviceLabel.length > 0 ? fallbackDeviceLabel : "Direct message"
        }
        var label = name.slice(3).replace(/-/g, " ").trim()
        if (label.length > 0) {
            return root.titleCaseLabel(label)
        }
        var deviceLabel = root.directMessageParticipantDeviceLabel(row)
        return deviceLabel.length > 0 ? deviceLabel : "Direct message"
    }

    function directMessageParticipantLabel(channel) {
        var ids = root.channelDirectMessageParticipantDeviceIds(channel)
        var localDeviceId = String(chaftController.deviceId || "").trim()
        var fallbackDeviceId = ""
        for (var i = 0; i < ids.length; i += 1) {
            var deviceId = String(ids[i] || "").trim()
            if (deviceId.length === 0) {
                continue
            }
            if (deviceId !== localDeviceId) {
                var member = root.memberByDeviceId(deviceId)
                if (member !== null) {
                    var displayName = String(member.displayName || "").trim()
                    if (displayName.length > 0) {
                        return displayName
                    }
                }
                return ""
            }
            fallbackDeviceId = deviceId
        }
        if (ids.length === 1 && fallbackDeviceId.length > 0) {
            return "You"
        }
        return ""
    }

    function directMessageParticipantDeviceLabel(channel) {
        var ids = root.channelDirectMessageParticipantDeviceIds(channel)
        var localDeviceId = String(chaftController.deviceId || "").trim()
        for (var i = 0; i < ids.length; i += 1) {
            var deviceId = String(ids[i] || "").trim()
            if (deviceId.length > 0 && deviceId !== localDeviceId) {
                return root.unnamedPersonLabel(deviceId)
            }
        }
        return ""
    }

    function startDirectMessage(deviceId, displayLabel) {
        var normalizedDeviceId = String(deviceId || "").trim()
        var localDeviceId = String(chaftController.deviceId || "").trim()
        var normalizedDisplayLabel = normalizedDeviceId.length > 0
                && normalizedDeviceId === localDeviceId
            ? "You"
            : displayLabel
        var existing = root.existingDirectMessageChannel(
            normalizedDeviceId, normalizedDisplayLabel)
        if (String(existing.channelId || "").length > 0) {
            return root.selectChannelId(existing.channelId, true)
        }
        return root.runtimeWorkReady
            && chaftController.createDirectMessage(
                normalizedDeviceId, normalizedDisplayLabel)
    }

    function titleCaseLabel(label) {
        return String(label || "").split(/\s+/).filter(function(part) {
            return part.length > 0
        }).map(function(part) {
            return part.slice(0, 1).toUpperCase() + part.slice(1)
        }).join(" ")
    }

    function directMessageSlug(displayLabel, deviceId) {
        var source = String(displayLabel || "").trim().toLowerCase()
        if (source.length === 0) {
            source = String(deviceId || "").trim().slice(0, 12).toLowerCase()
        }
        var slug = ""
        var lastDash = false
        for (var i = 0; i < source.length; i += 1) {
            var ch = source.charAt(i)
            var isDigit = ch >= "0" && ch <= "9"
            var isLetter = ch.toLowerCase() !== ch.toUpperCase()
            if (isLetter || isDigit) {
                slug += ch
                lastDash = false
            } else if (!lastDash && slug.length > 0) {
                slug += "-"
                lastDash = true
            }
        }
        while (slug.charAt(slug.length - 1) === "-") {
            slug = slug.slice(0, -1)
        }
        return slug.length > 0 ? slug : "person"
    }

    function existingDirectMessageChannel(deviceId, displayLabel) {
        var expectedName = "dm-" + root.directMessageSlug(displayLabel, deviceId)
        var source = root.channels || []
        var targetDeviceId = String(deviceId || "").trim()
        for (var i = 0; i < source.length; i += 1) {
            var channel = source[i] || {}
            var participantIds = root.channelDirectMessageParticipantDeviceIds(channel)
            for (var participantIndex = 0; participantIndex < participantIds.length; participantIndex += 1) {
                if (String(participantIds[participantIndex] || "").trim() === targetDeviceId) {
                    return channel
                }
            }
            if (root.channelIsDirectMessage(channel)
                    && String(channel.name || "").trim().toLowerCase() === expectedName) {
                return channel
            }
        }
        return ({})
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
                || normalized.indexOf("verified") !== -1) {
            return Tokens.success
        }
        return Tokens.textMuted
    }

    function positiveUnreadCount(value) {
        var count = Number(value || 0)
        return isFinite(count) && count > 0 ? count : 0
    }

    function unreadCountFromRows(rows, workspaceId) {
        var total = 0
        var source = rows || []
        var sourceWorkspaceId = String(workspaceId || root.currentWorkspaceId())
        for (var i = 0; i < source.length; i += 1) {
            var row = source[i] || {}
            if (root.channelIdMuted(sourceWorkspaceId, row.channelId)
                    || root.channelArchived(row)) {
                continue
            }
            total += root.positiveUnreadCount(row.unreadCount)
        }
        return total
    }

    function workspaceSummaryUnreadCount(summary) {
        var row = summary || {}
        var workspaceId = String(row.workspaceId || "")
        var unreadChannels = row.unreadChannels || []
        if (unreadChannels.length > 0 && workspaceId.length > 0) {
            return root.unreadCountFromRows(unreadChannels, workspaceId)
        }
        return root.positiveUnreadCount(row.unreadCount)
    }

    function workspaceRailUnreadCount(workspace) {
        var row = workspace || {}
        var workspaceId = String(row.workspaceId || "")
        var selectedWorkspaceId = String(chaftController.selectedWorkspaceId
            || root.workspaceSnapshot.workspaceId
            || "")
        if (workspaceId.length > 0 && workspaceId === selectedWorkspaceId) {
            return root.unreadCountFromRows(root.channels, selectedWorkspaceId)
        }
        return root.workspaceSummaryUnreadCount(row)
    }

    function windowUnreadCount() {
        var selectedWorkspaceId = String(chaftController.selectedWorkspaceId
            || root.workspaceSnapshot.workspaceId
            || "")
        var total = root.unreadCountFromRows(root.channels, selectedWorkspaceId)
        var summaries = chaftController.workspaceSummaries || []
        for (var i = 0; i < summaries.length; i += 1) {
            var summary = summaries[i] || {}
            var summaryId = String(summary.workspaceId || "")
            if (summaryId.length > 0 && summaryId === selectedWorkspaceId) {
                continue
            }
            total += root.workspaceSummaryUnreadCount(summary)
        }
        return total
    }

    function unreadCountTitleLabel(count) {
        var value = root.positiveUnreadCount(count)
        return value > 99 ? "99+" : String(value)
    }

    function notificationWorkspaceScopeLabel() {
        var count = (root.workspaceRailItems || []).length
        if (count > 1) {
            return "all " + String(count) + " workspaces"
        }
        return "this workspace"
    }

    function notificationScopeText() {
        if (!chaftController.notificationsEnabled) {
            return "Notifications are off. Chaft still catches up when opened."
        }
        return "Alerts cover unmuted rooms in "
            + root.notificationWorkspaceScopeLabel()
            + " while Chaft is open."
    }

    function notificationQuietRoomsText() {
        return "Muted and archived rooms stay quiet. Add Chaft to startup for live alerts after sign-in."
    }

    function unreadNotificationMessage(count) {
        var value = root.positiveUnreadCount(count)
        if (value === 1) {
            return "1 unread message"
        }
        return String(value) + " unread messages"
    }

    function unreadNotificationPreviewMessage(count) {
        if (!chaftController.notificationPreviewEnabled) {
            return root.unreadNotificationMessage(count)
                + (root.positiveUnreadCount(count) === 1
                    ? ". Open Chaft to read it."
                    : ". Open Chaft to read them.")
        }
        var channels = root.channels || []
        for (var i = 0; i < channels.length; i += 1) {
            var channel = channels[i] || {}
            if (root.channelMuted(channel)
                    || root.channelArchived(channel)
                    || root.positiveUnreadCount(channel.unreadCount) <= 0) {
                continue
            }
            var label = root.channelDisplayName(channel)
            var activity = root.channelActivityLabel(channel)
            if (activity.length > 0) {
                return label + " - " + activity
            }
            if (label.length > 0) {
                return root.unreadNotificationMessage(count) + " in " + label
            }
        }
        var selectedWorkspaceId = String(chaftController.selectedWorkspaceId
            || root.workspaceSnapshot.workspaceId
            || "")
        var summaries = chaftController.workspaceSummaries || []
        for (var j = 0; j < summaries.length; j += 1) {
            var summary = summaries[j] || {}
            var summaryId = String(summary.workspaceId || "")
            if (summaryId.length > 0 && summaryId === selectedWorkspaceId) {
                continue
            }
            var summaryUnread = root.workspaceSummaryUnreadCount(summary)
            if (summaryUnread <= 0) {
                continue
            }
            var workspaceName = root.workspaceDisplayName(summary)
            if (workspaceName.length > 0) {
                return root.unreadNotificationMessage(summaryUnread)
                    + " in " + workspaceName
            }
        }
        return root.unreadNotificationMessage(count)
    }

    function updateUnreadNotificationBaseline() {
        root.lastNotificationUnreadCount = root.totalUnreadCount
    }

    function handleUnreadNotificationCountChanged() {
        var count = root.totalUnreadCount
        if (!root.unreadNotificationsReady) {
            root.lastNotificationUnreadCount = count
            return
        }
        if (count > root.lastNotificationUnreadCount
                && !root.active
                && root.runtimeWorkReady
                && chaftController.notificationsEnabled) {
            chaftController.showDesktopNotification(
                "Chaft",
                root.unreadNotificationPreviewMessage(count),
                chaftController.notificationSoundEnabled)
        }
        root.lastNotificationUnreadCount = count
    }

    function restoredWindowDimension(key, fallback, minimum, maximum) {
        var geometry = chaftController.windowGeometry || {}
        var value = Number(geometry[key] || fallback)
        if (!isFinite(value)) {
            return fallback
        }
        return Math.max(minimum, Math.min(maximum, Math.round(value)))
    }

    function restoredWindowWidth() {
        return root.restoredWindowDimension("width", 1440, root.minimumWidth, 7680)
    }

    function restoredWindowHeight() {
        return root.restoredWindowDimension("height", 820, root.minimumHeight, 4320)
    }

    function persistWindowGeometry() {
        if (root.width < root.minimumWidth || root.height < root.minimumHeight) {
            return
        }
        chaftController.windowGeometry = ({
            width: Math.round(root.width),
            height: Math.round(root.height)
        })
    }

    function scheduleWindowGeometryPersist() {
        windowGeometryPersistTimer.restart()
    }

    function loadPersistedComposerDrafts() {
        if (!root.runtimeAccessReady) {
            root.clearPersistedDrafts()
            return
        }
        root.composerDrafts = chaftController.composerDrafts || ({})
    }

    function persistComposerDrafts() {
        if (!root.runtimeAccessReady) {
            chaftController.composerDrafts = ({})
            return
        }
        chaftController.composerDrafts = root.composerDrafts || ({})
    }

    function scheduleComposerDraftPersist() {
        if (!root.runtimeAccessReady) {
            root.clearPersistedDrafts()
            return
        }
        composerDraftPersistTimer.restart()
    }

    function clearPersistedDrafts() {
        composerDraftPersistTimer.stop()
        root.composerDrafts = ({})
        chaftController.composerDrafts = ({})
    }

    function startDemoTour() {
        if (chaftController.hasRuntimeWorkspace || chaftController.rawEventStoreMode) {
            return
        }
        root.demoTourActive = true
    }

    function exitDemoTour() {
        root.demoTourActive = false
    }

    function applyThemeChoice(themeId) {
        var id = String(themeId || "")
        if (id.length === 0) {
            return
        }
        if (root.systemThemeMode) {
            if (Themes.themeById(id).dark) {
                chaftController.darkThemeId = id
            } else {
                chaftController.lightThemeId = id
            }
        } else {
            chaftController.themeId = id
        }
    }

    function applyPendingSmokeArchivedChannelSelection() {
        if (!root.pendingSmokeArchivedChannelSelection) {
            return
        }
        var rows = root.channels || []
        for (var i = 0; i < rows.length; i += 1) {
            if (String(rows[i].name || "") === "design"
                    && rows[i].archived === true) {
                root.pendingSmokeArchivedChannelSelection = false
                root.selectChannelId(rows[i].channelId, true)
                Qt.callLater(function() {
                    if (root.selectedChannelArchived) {
                        channelDetailsPopup.open()
                    }
                })
                return
            }
        }
    }

    function applyPendingSmokePrivateChannelDetailsSelection() {
        if (!root.pendingSmokePrivateChannelDetailsSelection
                && !root.pendingSmokePrivateChannelRepairFailedSelection) {
            return
        }
        var repairFailed = root.pendingSmokePrivateChannelRepairFailedSelection
        var savedAddress = root.pendingSmokePrivateChannelRepairFailedSavedAddress
        var rows = root.channels || []
        for (var i = 0; i < rows.length; i += 1) {
            if (String(rows[i].name || "") === "vault"
                    && rows[i].isPrivate === true) {
                root.pendingSmokePrivateChannelDetailsSelection = false
                root.pendingSmokePrivateChannelRepairFailedSelection = false
                root.pendingSmokePrivateChannelRepairFailedSavedAddress = false
                if (repairFailed) {
                    root.lastPrivateRoomHistoryRepairFailedChannelId = rows[i].channelId
                }
                root.selectChannelId(rows[i].channelId, true)
                Qt.callLater(function() {
                    if (root.selectedChannelPrivate) {
                        channelDetailsPopup.open()
                    }
                })
                return
            }
        }
    }

    function applyPendingSmokeSetupRoomAccessSelection() {
        if (!root.pendingSmokeSetupRoomAccessSelection) {
            return
        }
        var rows = root.channels || []
        for (var i = 0; i < rows.length; i += 1) {
            if (String(rows[i].name || "") === "vault"
                    && rows[i].isPrivate === true) {
                root.pendingSmokeSetupRoomAccessSelection = false
                root.selectChannelId(rows[i].channelId, true)
                Qt.callLater(function() {
                    if (root.selectedChannelPrivate) {
                        channelDetailsPopup.open()
                    }
                })
                return
            }
        }
    }

    function applyPendingSmokePrivateChannelInspectorSelection() {
        if (!root.pendingSmokePrivateChannelInspectorSelection) {
            return
        }
        var rows = root.channels || []
        for (var i = 0; i < rows.length; i += 1) {
            if (String(rows[i].name || "") === "vault"
                    && rows[i].isPrivate === true) {
                root.pendingSmokePrivateChannelInspectorSelection = false
                root.syncDrawerOpen = false
                chaftController.inspectorPinned = true
                root.inspectorSmokeScrollOffsetY = 0
                root.selectChannelId(rows[i].channelId, true)
                return
            }
        }
    }

    function applySmokeUiState() {
        var state = String(chaftController.smokeUiState || "")
        if (state === "setup") {
            root.openSettings("profile")
        } else if (state === "setup-identity") {
            root.openSettings("profile")
        } else if (state === "setup-avatar-picker") {
            root.openSettings("profile")
            Qt.callLater(function() {
                setupPanel.openProfileAvatarPicker()
            })
        } else if (state === "setup-add-device") {
            root.openSettings("devices")
        } else if (state === "setup-access-updates") {
            root.openSettings("devices")
        } else if (state === "setup-security") {
            root.openSettings("advanced")
        } else if (state === "setup-backup") {
            root.openSettings("backup")
        } else if (state === "setup-room-access") {
            root.pendingSmokeSetupRoomAccessSelection = true
            root.applyPendingSmokeSetupRoomAccessSelection()
        } else if (state === "setup-people") {
            root.openPeopleAccess(false)
        } else if (state === "setup-invite-dialog") {
            root.openPeopleAccess(true)
        } else if (state === "setup-request-review") {
            root.openAccessRequestsPanel()
            smokeRequestReviewTimer.restart()
        } else if (state === "setup-invite") {
            root.openPeopleAccess(false)
            smokeInvitePackageTimer.restart()
        } else if (state === "setup-approval-invite") {
            root.openPeopleAccess(false)
            Qt.callLater(function() {
                setupPanel.approvalInviteModeEnabled = true
            })
            smokeApprovalInvitePackageTimer.restart()
        } else if (state === "setup-invite-lost") {
            root.openPeopleAccess(false)
            Qt.callLater(function() { setupPanel.openInvitationsSection() })
        } else if (state === "setup-request") {
            root.openAccessRequestsPanel()
        } else if (state === "setup-request-approved") {
            root.openAccessRequestsPanel()
            smokeApprovedRequestInviteTimer.restart()
        } else if (state === "setup-request-lost") {
            root.openAccessRequestsPanel()
        } else if (state === "setup-request-reinvite") {
            root.openAccessRequestsPanel()
        } else if (state === "drawer") {
            root.syncDrawerOpen = true
        } else if (state === "drawer-advanced") {
            root.syncDrawerOpen = true
            root.syncAdvancedToolsOpen = true
        } else if (state === "first-sync-waiting") {
            smokeFirstSyncWaitingTimer.restart()
        } else if (state === "first-sync-recovery") {
            smokeFirstSyncRecoveryTimer.restart()
        } else if (state === "member-roles") {
            root.openSmokeMemberRoles()
        } else if (state === "direct-message") {
            smokeDirectMessageTimer.restart()
        } else if (state === "palette") {
            commandPalette.open()
        } else if (state === "entry") {
            root.openWorkspaceEntry("create")
        } else if (state === "entry-join") {
            root.openWorkspaceEntry("join")
        } else if (state === "entry-restore") {
            root.openWorkspaceEntry("join", "restore")
            Qt.callLater(function() {
                root.loadWorkspaceCredentialText(JSON.stringify({
                    schemaVersion: 1,
                    workspaceId: "wrk_visual_smoke",
                    exporterDeviceId: "dev_visual_smoke_admin",
                    kdf: {
                        algorithm: "argon2id",
                        salt: "visual-smoke-salt",
                        memoryKiB: 19456,
                        iterations: 2,
                        parallelism: 1
                    },
                    sealedPayload: "visual-smoke-sealed-payload"
                }, null, 2))
                workspaceEntryDialog.recoveryPassphraseText = "visual-smoke-passphrase"
            })
        } else if (state === "entry-restore-failed") {
            root.openWorkspaceEntry("join", "restore")
            Qt.callLater(function() {
                root.loadWorkspaceCredentialText(JSON.stringify({
                    schemaVersion: 1,
                    workspaceId: "wrk_visual_smoke",
                    exporterDeviceId: "dev_visual_smoke_admin",
                    kdf: {
                        algorithm: "argon2id",
                        salt: "visual-smoke-salt",
                        memoryKiB: 19456,
                        iterations: 2,
                        parallelism: 1
                    },
                    sealedPayload: "visual-smoke-sealed-payload"
                }, null, 2))
                workspaceEntryDialog.prepareRecoveryFailureForSmoke()
            })
        } else if (state === "entry-join-invite") {
            root.openWorkspaceEntry("join")
            root.loadWorkspaceCredentialText(JSON.stringify({
                kind: "chaft.workspace-invite.v2",
                schemaVersion: 2,
                workspaceId: "wrk_visual_smoke",
                workspaceName: "Visual Smoke",
                inviteId: "inv_visual_smoke_secure",
                displayName: "Sam Rivera",
                inviterDeviceId: "dev_visual_smoke_admin",
                inviterDisplayName: "Mira Chen",
                inviterPublicKey: "visual-smoke-public-key",
                inviterSignature: "visual-smoke-signature",
                capabilityPublicKey: "visual-smoke-capability-public-key",
                capabilitySecret: "visual-smoke-one-time-capability",
                role: "member",
                peerEndpoint: "direct+tcp://127.0.0.1:44944",
                syncExpectation: "history_after_claim",
                createdAt: "2026-07-10T12:00:00Z",
                expiresAt: root.inviteExpiresAtIso(30)
            }, null, 2))
        } else if (state === "entry-approval-invite") {
            root.openWorkspaceEntry("join")
            root.loadWorkspaceCredentialText(JSON.stringify({
                kind: "chaft.workspace-invite.v1",
                schemaVersion: 1,
                workspaceId: "wrk_visual_smoke",
                workspaceName: "Visual Smoke",
                inviteId: "inv_visual_smoke_approval",
                inviteeDeviceId: "dev_visual_smoke_joiner",
                inviteeDisplayName: "Sam Rivera",
                inviterDeviceId: "dev_visual_smoke_admin",
                inviterDisplayName: "Mira Chen",
                role: "member",
                approvalPolicy: "admin_required",
                syncExpectation: "waiting_for_admin_approval",
                peerEndpoint: "direct+tcp://127.0.0.1:44944",
                expiresAt: root.inviteExpiresAtIso(30)
            }, null, 2))
        } else if (state === "entry-workspace-card") {
            root.openWorkspaceEntry("join")
            Qt.callLater(function() {
                root.loadWorkspaceCredentialText(JSON.stringify({
                    kind: "chaft.workspace-card.v1",
                    schemaVersion: 1,
                    workspaceId: "wrk_visual_smoke",
                    workspaceName: "Visual Smoke",
                    accessPolicy: "request_access",
                    adminDeviceId: "dev_visual_smoke_admin",
                    adminDisplayName: "Mira Chen",
                    peerEndpoint: "direct+tcp://127.0.0.1:44944",
                    createdAt: "2026-07-07T12:00:00Z"
                }, null, 2))
            })
        } else if (state === "entry-workspace-card-invite-only") {
            root.openWorkspaceEntry("join")
            Qt.callLater(function() {
                root.loadWorkspaceCredentialText(JSON.stringify({
                    kind: "chaft.workspace-card.v1",
                    schemaVersion: 1,
                    workspaceId: "wrk_visual_smoke",
                    workspaceName: "Visual Smoke",
                    accessPolicy: "invite_only",
                    adminDeviceId: "dev_visual_smoke_admin",
                    adminDisplayName: "Mira Chen",
                    peerEndpoint: "direct+tcp://127.0.0.1:44944",
                    createdAt: "2026-07-07T12:00:00Z"
                }, null, 2))
            })
        } else if (state === "entry-request-sent") {
            root.openWorkspaceEntry("join")
            Qt.callLater(function() {
                root.loadWorkspaceCredentialText(JSON.stringify({
                    kind: "chaft.workspace-card.v1",
                    schemaVersion: 1,
                    workspaceId: "wrk_visual_smoke",
                    workspaceName: "Visual Smoke",
                    accessPolicy: "request_access",
                    adminDeviceId: "dev_visual_smoke_admin",
                    adminDisplayName: "Mira Chen",
                    peerEndpoint: "direct+tcp://127.0.0.1:44944",
                    createdAt: "2026-07-07T12:00:00Z"
                }, null, 2))
                smokeEntryRequestSentTimer.restart()
            })
        } else if (state === "post-create") {
            postCreateExportDialog.open()
        } else if (state === "post-create-recovery") {
            postCreateExportDialog.open()
            postCreateExportDialog.recoverySetupOpen = true
        } else if (state === "add-workspace") {
            smokeAddWorkspaceTimer.restart()
        } else if (state === "channel-details") {
            channelDetailsPopup.open()
        } else if (state === "private-channel-details") {
            root.pendingSmokePrivateChannelDetailsSelection = true
            root.applyPendingSmokePrivateChannelDetailsSelection()
        } else if (state === "private-channel-access") {
            root.channelAccessToolsOpen = true
            root.pendingSmokePrivateChannelDetailsSelection = true
            root.applyPendingSmokePrivateChannelDetailsSelection()
        } else if (state === "private-channel-repair-failed") {
            root.channelAccessToolsOpen = true
            root.pendingSmokePrivateChannelRepairFailedSelection = true
            root.applyPendingSmokePrivateChannelDetailsSelection()
        } else if (state === "private-channel-repair-saved") {
            root.channelAccessToolsOpen = true
            root.pendingSmokePrivateChannelRepairFailedSelection = true
            root.pendingSmokePrivateChannelRepairFailedSavedAddress = true
            root.applyPendingSmokePrivateChannelDetailsSelection()
        } else if (state === "private-channel-inspector") {
            root.pendingSmokePrivateChannelInspectorSelection = true
            root.applyPendingSmokePrivateChannelInspectorSelection()
        } else if (state === "channel-archived") {
            root.pendingSmokeArchivedChannelSelection = true
            root.applyPendingSmokeArchivedChannelSelection()
        } else if (state === "external-link") {
            smokeExternalLinkTimer.restart()
        }
    }

    function openSmokeMemberRoles() {
        root.syncDrawerOpen = false
        root.inspectorItemKey = ""
        chaftController.inspectorPinned = true
        root.inspectorSmokeScrollOffsetY = root.smokeMemberRolesFallbackOffset()
        smokeMemberRolesScrollTimer.restart()
    }

    function smokeMemberRolesFallbackOffset() {
        return Math.max(420, Math.round(root.height * 0.34))
    }

    function scrollInspectorToPeople() {
        var targetY = Math.max(
            root.smokeMemberRolesFallbackOffset(),
            inspectorPeopleSectionHeader.y - Tokens.space2)
        var contentY = Math.max(0, targetY)
        root.inspectorSmokeScrollOffsetY = contentY
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
        var existingValue = String(existingDrafts[key] || "")
        if (draft.trim().length > 0 && existingValue === draft) {
            return
        }
        if (draft.trim().length === 0
                && !Object.prototype.hasOwnProperty.call(existingDrafts, key)) {
            return
        }
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
        root.scheduleComposerDraftPersist()
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

    function beginComposerOperation(kind, workspaceId, channelId,
                                    replyToMessageId, messageId, text,
                                    filePath) {
        if (root.composerOperationPending) {
            return false
        }
        root.pendingComposerOperationKind = String(kind || "")
        root.pendingComposerOperationWorkspaceId = String(workspaceId || "")
        root.pendingComposerOperationChannelId = String(channelId || "")
        root.pendingComposerOperationReplyToMessageId =
            String(replyToMessageId || "")
        root.pendingComposerOperationMessageId = String(messageId || "")
        root.pendingComposerOperationText = String(text || "")
        root.pendingComposerOperationFilePath = String(filePath || "")
        return root.composerOperationPending
    }

    function clearPendingComposerOperation() {
        root.pendingComposerOperationKind = ""
        root.pendingComposerOperationWorkspaceId = ""
        root.pendingComposerOperationChannelId = ""
        root.pendingComposerOperationReplyToMessageId = ""
        root.pendingComposerOperationMessageId = ""
        root.pendingComposerOperationText = ""
        root.pendingComposerOperationFilePath = ""
    }

    function pendingComposerContextMatches(kind, workspaceId, channelId,
                                           replyToMessageId, messageId,
                                           filePath) {
        if (root.pendingComposerOperationKind !== String(kind || "")
                || root.pendingComposerOperationWorkspaceId
                    !== String(workspaceId || "")) {
            return false
        }
        if (channelId !== undefined
                && root.pendingComposerOperationChannelId
                    !== String(channelId || "")) {
            return false
        }
        if (replyToMessageId !== undefined
                && root.pendingComposerOperationReplyToMessageId
                    !== String(replyToMessageId || "")) {
            return false
        }
        if (messageId !== undefined
                && root.pendingComposerOperationMessageId
                    !== String(messageId || "")) {
            return false
        }
        return filePath === undefined
            || root.pendingComposerOperationFilePath === String(filePath || "")
    }

    function completePendingMessageSend(workspaceId, channelId,
                                        replyToMessageId, success, message) {
        if (!root.pendingComposerContextMatches(
                    "message", workspaceId, channelId, replyToMessageId,
                    undefined, undefined)) {
            return
        }
        var sentText = root.pendingComposerOperationText
        var draftKey = root.composerDraftKey(workspaceId, channelId)
        var storedDraft = String(root.composerDrafts[draftKey] || "")
        var selectedContext = root.currentWorkspaceId() === String(workspaceId || "")
            && root.selectedChannelKey === String(channelId || "")
            && root.editingMessageId.length === 0
        var visibleDraftUnchanged = selectedContext
            && composer.draftText() === sentText
        if (success) {
            if (storedDraft === sentText) {
                root.clearDraftForWorkspaceChannel(workspaceId, channelId)
            }
            if (visibleDraftUnchanged) {
                composer.clearDraft()
                if (root.replyTargetMessageId
                        === String(replyToMessageId || "")) {
                    root.cancelReplyMessage()
                }
            }
        } else {
            toastHost.show(
                "error",
                String(message || "Message was not sent.")
                    + " Your draft is still here.",
                "",
                "",
                8000)
        }
        root.clearPendingComposerOperation()
    }

    function completePendingMessageEdit(workspaceId, messageId, success,
                                        message) {
        if (!root.pendingComposerContextMatches(
                    "edit", workspaceId, undefined, undefined, messageId,
                    undefined)) {
            return
        }
        var editedText = root.pendingComposerOperationText
        var visibleEditUnchanged =
            root.currentWorkspaceId() === String(workspaceId || "")
            && root.editingMessageId === String(messageId || "")
            && composer.draftText() === editedText
        root.clearPendingComposerOperation()
        if (success) {
            if (visibleEditUnchanged) {
                root.cancelEditMessage()
            }
        } else {
            toastHost.show(
                "error",
                String(message || "Message was not updated.")
                    + " Your edit is still here.",
                "",
                "",
                8000)
        }
    }

    function completePendingAttachmentSend(workspaceId, channelId,
                                           replyToMessageId, filePath,
                                           success, message) {
        if (!root.pendingComposerContextMatches(
                    "attachment", workspaceId, channelId, replyToMessageId,
                    undefined, filePath)) {
            return
        }
        var sentText = root.pendingComposerOperationText
        var draftKey = root.composerDraftKey(workspaceId, channelId)
        var storedDraft = String(root.composerDrafts[draftKey] || "")
        var selectedContext = root.currentWorkspaceId() === String(workspaceId || "")
            && root.selectedChannelKey === String(channelId || "")
            && root.editingMessageId.length === 0
        var visibleDraftUnchanged = selectedContext
            && composer.draftText() === sentText
        if (success) {
            if (storedDraft === sentText) {
                root.clearDraftForWorkspaceChannel(workspaceId, channelId)
            }
            if (visibleDraftUnchanged) {
                composer.clearDraft()
                if (root.replyTargetMessageId
                        === String(replyToMessageId || "")) {
                    root.cancelReplyMessage()
                }
            }
        } else {
            toastHost.show(
                "error",
                String(message || "File was not sent.")
                    + " Your message is still here; attach the file again to retry.",
                "",
                "",
                9000)
        }
        root.clearPendingComposerOperation()
    }

    function queueCountLabel(value, singular, plural) {
        return String(value) + " " + (value === 1 ? singular : plural)
    }

    function publishQueueStatusText() {
        if (root.publishQueueError.length > 0) {
            return "Sharing needs attention"
        }
        if (root.queuedMissingBlobCount > 0) {
            return "Files need retry"
        }
        if (root.queuedSkippedGapCount > 0) {
            return "History needs update"
        }
        if (root.queuedPublishableEventCount > 0 || root.queuedBackupEventCount > 0) {
            return "Local history available"
        }
        return "No eligible local events"
    }

    function publishQueueDetailText() {
        if (root.publishQueueError.length > 0) {
            return root.publishQueueError
        }

        var parts = []
        if (root.queuedPublishableEventCount > 0) {
            parts.push(root.queueCountLabel(
                root.queuedPublishableEventCount,
                "local event available to peers",
                "local events available to peers"))
        }
        if (root.queuedBackupEventCount > 0) {
            parts.push(root.queueCountLabel(
                root.queuedBackupEventCount,
                "backup-eligible event",
                "backup-eligible events"))
        }
        if (root.queuedMissingBlobCount > 0) {
            parts.push(root.queueCountLabel(root.queuedMissingBlobCount, "file to retry", "files to retry"))
        }
        if (root.queuedSkippedGapCount > 0) {
            parts.push(root.queueCountLabel(root.queuedSkippedGapCount, "history item to fetch", "history items to fetch"))
        }
        return parts.length > 0 ? parts.join(" | ") : "No publishable local events"
    }

    function storageHealthStatusText() {
        if (root.storageHealthError.length > 0) {
            return "History check failed"
        }
        if (!root.storageHealthKnown) {
            return "Checking history"
        }
        if (root.storageHealthHasIssue) {
            return "History needs review"
        }
        return "History healthy"
    }

    function storageHealthDetailText() {
        if (root.storageHealthError.length > 0) {
            return root.storageHealthError
        }
        if (!root.storageHealthKnown) {
            return "Checking history"
        }
        var parts = []
        if (root.storageCorruptEventCount > 0) {
            parts.push(root.queueCountLabel(root.storageCorruptEventCount, "damaged item", "damaged items"))
        }
        if (root.storageNonServableParseableEventCount > 0) {
            parts.push(root.queueCountLabel(root.storageNonServableParseableEventCount, "untrusted item", "untrusted items"))
        }
        if (root.storagePoisonedMetadataCount > 0) {
            parts.push(root.queueCountLabel(root.storagePoisonedMetadataCount, "history record mismatch", "history record mismatches"))
        }
        if (root.storagePromotableMetadataCount > 0) {
            parts.push(root.queueCountLabel(root.storagePromotableMetadataCount, "history record gap", "history record gaps"))
        }
        if (parts.length > 0) {
            return parts.join(" | ")
        }

        return "History healthy | " + root.queueCountLabel(root.storageServableEventCount, "available item", "available items")
    }

    function backupDeviceReviewCount() {
        var statuses = chaftController.backupPeerStatuses || {}
        var count = 0
        for (var endpoint in statuses) {
            if (Object.prototype.hasOwnProperty.call(statuses, endpoint)
                    && root.backupPeerSuspectScore(statuses[endpoint]) > 0) {
                count += 1
            }
        }
        return count
    }

    function safetyChatStatusText() {
        if (!chaftController.hasRuntimeWorkspace) {
            return "Open a workspace to check safety."
        }
        if (!root.runtimeAccessReady) {
            return "Unlock this workspace before reviewing safety."
        }
        if (!root.runtimeWorkReady) {
            return "Open the live workspace before changing safety settings."
        }
        if (root.storageHealthHasIssue) {
            return "You can keep using Chaft, but history on this device needs review."
        }
        if (root.backupDeviceReviewCount() > 0) {
            return "You can keep chatting. Review backup addresses before relying on them."
        }
        return "You can keep chatting. No safety issues are visible on this device."
    }

    function safetyNextActionText() {
        if (!chaftController.hasRuntimeWorkspace) {
            return "Open or join a workspace first."
        }
        if (!root.runtimeAccessReady) {
            return "Unlock the workspace, then check access again."
        }
        if (root.storageMetadataRepairSuggested) {
            return "Fix history, then check again."
        }
        if (root.storageHealthHasIssue) {
            return "Review the history warning before sharing or refreshing access."
        }
        if (root.backupDeviceReviewCount() > 0) {
            return "Review backup addresses marked Needs review."
        }
        return "No action needed unless access looks wrong."
    }

    function safetySupportDetailText() {
        if (root.storageHealthHasIssue) {
            return "Share this history status if support asks: " + root.storageHealthDetailText()
        }
        if (root.backupDeviceReviewCount() > 0) {
            return "Share the backup address marked Needs review if someone helps you troubleshoot."
        }
        return "Use message support info or Export access record only when someone asks."
    }

    function safetySupportCopyText() {
        return [
            "Chaft safety summary",
            "Status: " + root.safetyChatStatusText(),
            "Next action: " + root.safetyNextActionText(),
            "Support detail: " + root.safetySupportDetailText()
        ].join("\n")
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
            + " - "
            + root.byteSizeLabel(Number((attachment && attachment.byteLen) || 0))
            + ((attachment && attachment.localBlobAvailable === false) ? " - missing on this device" : "")
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
            if (root.timelineItemKey(root.inspectorSelectedItemSnapshot) === key) {
                return root.inspectorSelectedItemSnapshot
            }
        }
        return ({})
    }

    function selectInspectorItem(item) {
        var key = root.timelineItemKey(item)
        if (key.length === 0) {
            return false
        }
        var itemChannelId = String((item && item.channelId) || "")
        if (root.hasChannelId(itemChannelId)
                && itemChannelId
                    !== String(root.selectedChannel.channelId || "")) {
            if (!root.selectChannelId(itemChannelId, false)) {
                return false
            }
        }
        root.inspectorSelectedItemSnapshot = item
        root.inspectorItemKey = key
        return true
    }

    function timelineMessageItemById(messageId) {
        var targetId = String(messageId || "")
        var collections = [
            root.selectedTimeline || [],
            root.channelTimeline || [],
            root.timeline || []
        ]
        for (var collectionIndex = 0;
                collectionIndex < collections.length;
                collectionIndex += 1) {
            var items = collections[collectionIndex]
            for (var itemIndex = 0; itemIndex < items.length; itemIndex += 1) {
                var item = items[itemIndex] || {}
                if (String(item.messageId || "") === targetId) {
                    return item
                }
            }
        }
        return null
    }

    function positionTimelineMessageById(messageId) {
        var targetId = String(messageId || "")
        for (var i = 0; i < root.selectedTimeline.length; i += 1) {
            if (String((root.selectedTimeline[i] || {}).messageId || "")
                    === targetId) {
                timelineView.positionViewAtIndex(i, ListView.Center)
                timelineView.forceActiveFocus()
                return true
            }
        }
        return false
    }

    function selectedTimelineContainsMessageId(messageId) {
        var targetId = String(messageId || "")
        for (var i = 0; i < root.selectedTimeline.length; i += 1) {
            if (String((root.selectedTimeline[i] || {}).messageId || "")
                    === targetId) {
                return true
            }
        }
        return false
    }

    function openTimelineMessageById(messageId) {
        var targetId = String(messageId || "")
        if (targetId.length === 0) {
            return false
        }
        var item = root.timelineMessageItemById(targetId)
        if (item !== null) {
            var visibleInCurrentTimeline =
                root.selectedTimelineContainsMessageId(targetId)
            if (!root.selectInspectorItem(item)) {
                return false
            }
            // Keep direct search-result navigation in its result context. A
            // related message that does not match the query must leave search
            // so the loaded room timeline can reveal it.
            if (root.searchHasTerms && !visibleInCurrentTimeline) {
                searchField.text = ""
                root.searchQuery = ""
            }
            Qt.callLater(function() {
                root.positionTimelineMessageById(targetId)
            })
            return true
        }
        toastHost.show(
            "info",
            "That message is outside the loaded history. Load earlier messages "
                + "or search for it to open it.",
            "",
            "",
            6000)
        return false
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

    function countLoadedMessages(items) {
        items = items || []
        var count = 0
        for (var i = 0; i < items.length; i += 1) {
            var kind = String(items[i].kind || "")
            var deleted = items[i].deleted === true
            if (!deleted && (kind === "message" || kind === "encrypted_message")) {
                count += 1
            }
        }
        return count
    }

    function countLockedMessages(items) {
        items = items || []
        var count = 0
        for (var i = 0; i < items.length; i += 1) {
            var kind = String(items[i].kind || "")
            var deleted = items[i].deleted === true
            if (kind === "encrypted_message" && items[i].bodyDecrypted !== true && !deleted) {
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
        var deviceId = String(item.authorDeviceId || "")
        return deviceId.length > 0 ? root.unnamedPersonLabel(deviceId) : ""
    }

    function timelineKindLabel(item) {
        if (!item || !item.kind) {
            return ""
        }
        if (item.kind === "missing_history_gap") {
            return "Needs earlier history"
        }
        if (item.kind === "invalid_signature") {
            return "Message could not be verified"
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
                || root.workspaceOperationInFlight
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

    function scheduleControllerIdleWork() {
        root.controllerIdleWorkPending = true
        if (root.controllerIdleWorkScheduled) {
            return
        }
        root.controllerIdleWorkScheduled = true
        Qt.callLater(function() {
            root.controllerIdleWorkScheduled = false
            if (!root.controllerIdleWorkPending || !root.runtimeWorkReady) {
                return
            }
            if (String(root.pendingEntryDisplayName || "").trim().length > 0
                    && root.localDeviceMembershipReady()) {
                if (root.pendingEntryDisplayNameUpdateInFlight
                        || root.workspaceOperationInFlight) {
                    return
                }
                if (root.applyPendingEntryDisplayName()) {
                    return
                }
            }
            if (root.pendingJoinPeerEndpointTargetsCurrentWorkspace()) {
                if (root.workspaceOperationInFlight) {
                    return
                }
                root.controllerIdleWorkPending = false
                if (!root.pullPendingJoinPeerIfReady()
                        && root.pendingJoinPeerEndpointTargetsCurrentWorkspace()) {
                    root.controllerIdleWorkPending = true
                }
                return
            }
            if (root.workspaceOperationInFlight) {
                return
            }
            root.controllerIdleWorkPending = false
            root.requestSelectedChannelTimelineIfNeeded()
        })
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

    function searchStatusText() {
        if (!root.searchHasTerms) {
            return ""
        }
        if (!root.runtimeWorkReady) {
            return "Unlock the workspace to search its history."
        }
        if (!root.indexedSearchHasTerms) {
            return "Showing loaded matches in this room. Workspace search needs letters or numbers."
        }
        if (root.searchResultsLoading) {
            return "Showing loaded matches while workspace search runs..."
        }
        var messageCount = root.messageSearchHitCountLabel()
        var conversationCount =
            (chaftController.channelSearchResults || []).length
        return "Workspace: " + messageCount
            + (messageCount === "1" ? " message" : " messages")
            + " · " + String(conversationCount)
            + (conversationCount === 1 ? " conversation" : " conversations")
    }

    function localDeviceDisplayName() {
        for (var i = 0; i < root.profiles.length; i += 1) {
            if (root.profiles[i].deviceId === chaftController.deviceId) {
                return String(root.profiles[i].displayName || "")
            }
        }
        return ""
    }

    function localDeviceAvatarId() {
        for (var i = 0; i < root.profiles.length; i += 1) {
            if (root.profiles[i].deviceId === chaftController.deviceId) {
                return String(root.profiles[i].avatarId || "")
            }
        }
        return ""
    }

    function localLinkedPersonDisplayName() {
        var localDeviceId = String(chaftController.deviceId || "").trim()
        if (localDeviceId.length === 0) {
            return ""
        }
        var linkedPersonId = ""
        for (var linkIndex = 0;
                linkIndex < root.personDeviceLinks.length;
                linkIndex += 1) {
            var link = root.personDeviceLinks[linkIndex] || ({})
            if (String(link.deviceId || "").trim() !== localDeviceId) {
                continue
            }
            var linkedDisplayName = String(
                link.personDisplayName || "").trim()
            if (linkedDisplayName.length > 0) {
                return linkedDisplayName
            }
            linkedPersonId = String(link.personId || "").trim()
            break
        }
        if (linkedPersonId.length === 0) {
            return ""
        }
        for (var profileIndex = 0;
                profileIndex < root.personProfiles.length;
                profileIndex += 1) {
            var profile = root.personProfiles[profileIndex] || ({})
            if (String(profile.personId || "").trim() === linkedPersonId) {
                return String(profile.displayName || "")
            }
        }
        return ""
    }

    function localLinkedPersonAvatarId() {
        var localDeviceId = String(chaftController.deviceId || "").trim()
        if (localDeviceId.length === 0) {
            return ""
        }
        var linkedPersonId = ""
        for (var linkIndex = 0;
                linkIndex < root.personDeviceLinks.length;
                linkIndex += 1) {
            var link = root.personDeviceLinks[linkIndex] || ({})
            if (String(link.deviceId || "").trim() !== localDeviceId) {
                continue
            }
            var linkedAvatarId = String(
                link.personAvatarId || "").trim()
            if (linkedAvatarId.length > 0) {
                return linkedAvatarId
            }
            linkedPersonId = String(link.personId || "").trim()
            break
        }
        if (linkedPersonId.length === 0) {
            return ""
        }
        for (var profileIndex = 0;
                profileIndex < root.personProfiles.length;
                profileIndex += 1) {
            var profile = root.personProfiles[profileIndex] || ({})
            if (String(profile.personId || "").trim() === linkedPersonId) {
                return String(profile.avatarId || "")
            }
        }
        return ""
    }

    function localDeviceMembershipReady() {
        var localDeviceId = String(chaftController.deviceId || "").trim()
        if (localDeviceId.length === 0) {
            return false
        }
        for (var i = 0; i < root.members.length; i += 1) {
            if (String(root.members[i].deviceId || "").trim() === localDeviceId) {
                return true
            }
        }
        return false
    }

    function pendingEntryDisplayNameConfirmed() {
        var displayName = String(root.pendingEntryDisplayName || "").trim()
        var avatarId = String(root.pendingEntryAvatarId || "").trim()
        if (displayName.length === 0) {
            return false
        }
        var targetWorkspaceId = String(
            root.pendingEntryDisplayNameWorkspaceId || "").trim()
        return (targetWorkspaceId.length === 0
                || root.currentWorkspaceId() === targetWorkspaceId)
            && root.localDeviceDisplayName().trim() === displayName
            && root.localLinkedPersonDisplayName().trim() === displayName
            && (avatarId.length === 0
                || (root.localDeviceAvatarId().trim() === avatarId
                    && root.localLinkedPersonAvatarId().trim() === avatarId))
    }

    function pendingEntryDisplayNameRequestForCurrentWorkspace() {
        var workspaceId = root.currentWorkspaceId()
        if (workspaceId.length === 0) {
            return ({})
        }

        var candidates = []
        var persistedRequests = chaftController.pendingJoinRequests || ({})
        for (var key in persistedRequests) {
            if (!Object.prototype.hasOwnProperty.call(
                    persistedRequests, key)) {
                continue
            }
            var persistedRequest = persistedRequests[key] || ({})
            var requestId = String(
                persistedRequest.requestId || "").trim()
            var displayName = String(
                persistedRequest.displayName || "").trim()
            var avatarId = String(
                persistedRequest.avatarId || "").trim()
            var status = String(
                persistedRequest.status || "").trim()
            if (String(persistedRequest.workspaceId || "").trim()
                        !== workspaceId
                    || (status !== "profile_pending"
                        && status !== "profile_written")
                    || displayName.length === 0
                    || chaftController.deviceDisplayNameValidationError(
                        displayName).length > 0
                    || (avatarId.length > 0
                        && chaftController.avatarIdValidationError(
                            avatarId).length > 0)) {
                continue
            }
            candidates.push({
                key: key,
                requestId: requestId,
                displayName: displayName,
                avatarId: avatarId,
                createdAt: String(persistedRequest.createdAt || ""),
                writeSucceeded: status === "profile_written"
            })
        }
        if (candidates.length === 0) {
            return ({})
        }
        candidates.sort(function(left, right) {
            return String(right.createdAt || "").localeCompare(
                String(left.createdAt || ""))
        })
        var selectedName = candidates[0].displayName
        var selectedAvatarId = candidates[0].avatarId
        for (var candidateIndex = 1;
                candidateIndex < candidates.length;
                candidateIndex += 1) {
            // Concurrent approvals with different identities are ambiguous.
            // Wait for an explicit in-memory import correlation instead of
            // guessing which persisted request should name this device.
            if (candidates[candidateIndex].displayName !== selectedName
                    || candidates[candidateIndex].avatarId
                        !== selectedAvatarId) {
                return ({})
            }
        }
        candidates.sort(function(left, right) {
            if (left.writeSucceeded !== right.writeSucceeded) {
                return left.writeSucceeded ? -1 : 1
            }
            return String(right.createdAt || "").localeCompare(
                String(left.createdAt || ""))
        })
        return candidates[0]
    }

    function persistPendingEntryDisplayNameState(status) {
        var workspaceId = String(
            root.pendingEntryDisplayNameWorkspaceId || "").trim()
        var displayName = String(
            root.pendingEntryDisplayName || "").trim()
        var avatarId = String(
            root.pendingEntryAvatarId || "").trim()
        var requestId = String(
            root.pendingEntryDisplayNameRequestId || "").trim()
        if (workspaceId.length === 0 || displayName.length === 0) {
            return false
        }

        var next = root.copyMap(chaftController.pendingJoinRequests || ({}))
        var key = String(
            root.pendingEntryDisplayNameRequestKey || "").trim()
        if (key.length === 0 && requestId.length > 0) {
            for (var candidateKey in next) {
                if (Object.prototype.hasOwnProperty.call(next, candidateKey)
                        && String((next[candidateKey] || {}).requestId || "")
                            .trim() === requestId) {
                    key = candidateKey
                    break
                }
            }
        }
        if (key.length === 0) {
            key = requestId.length > 0
                ? requestId
                : "profile:" + workspaceId
        }

        var row = root.copyMap(
            Object.prototype.hasOwnProperty.call(next, key)
                ? next[key]
                : ({}))
        row.workspaceId = workspaceId
        if (root.currentWorkspaceId() === workspaceId) {
            row.workspaceName = String(
                root.workspaceSnapshot.name || row.workspaceName || "").trim()
        }
        row.displayName = displayName
        if (avatarId.length > 0) {
            row.avatarId = avatarId
        } else {
            delete row.avatarId
        }
        row.status = String(status || "profile_pending").trim()
        row.createdAt = String(row.createdAt || (new Date()).toISOString())
        delete row.error
        if (requestId.length > 0) {
            row.requestId = requestId
        } else {
            delete row.requestId
            row.sourceType = "profile_finalization"
        }
        if (String(row.artifact || "").trim().length === 0) {
            row.sourceType = "profile_finalization"
            // The sanitizer requires a non-empty artifact. Persist only a
            // secret-free lifecycle marker; never retain imported credentials
            // or a workspace encryption key for this UI recovery state.
            row.artifact = JSON.stringify({
                kind: "chaft.pending-profile.v1",
                schemaVersion: 1,
                workspaceId: workspaceId
            })
        }
        next[key] = row
        root.pendingEntryDisplayNameRequestKey = key
        return chaftController.storePendingJoinRequests(next)
    }

    function clearPendingEntryDisplayNamePersistence(key, requestId) {
        var normalizedKey = String(key || "").trim()
        var normalizedRequestId = String(requestId || "").trim()
        var next = root.copyMap(chaftController.pendingJoinRequests || ({}))
        if (normalizedKey.length > 0
                && Object.prototype.hasOwnProperty.call(next, normalizedKey)) {
            delete next[normalizedKey]
            return chaftController.storePendingJoinRequests(next)
        }
        if (normalizedRequestId.length > 0) {
            return root.clearPendingAccessRequestForRequestId(
                normalizedRequestId)
        }
        return false
    }

    function restorePendingEntryDisplayNameFromRequests() {
        var currentWorkspaceId = root.currentWorkspaceId()
        if (!root.runtimeWorkReady || currentWorkspaceId.length === 0) {
            return false
        }
        var pendingWorkspaceId = String(
            root.pendingEntryDisplayNameWorkspaceId || "").trim()
        if (String(root.pendingEntryDisplayName || "").trim().length > 0) {
            if (pendingWorkspaceId !== currentWorkspaceId) {
                if (root.pendingEntryDisplayNameUpdateInFlight
                        || root.pendingEntryDisplayNameRequestKey.length === 0) {
                    return false
                }
                // The old workspace's marker remains durable. Release only
                // its in-memory state so the current workspace can recover its
                // own pending identity; switching back will restore this one.
                root.clearPendingEntryDisplayName()
            } else {
                if (root.pendingEntryDisplayNameRequestKey.length === 0) {
                    var matchingRequest =
                        root.pendingEntryDisplayNameRequestForCurrentWorkspace()
                    if (String(matchingRequest.displayName || "").trim()
                            === String(root.pendingEntryDisplayName || "").trim()
                            && String(matchingRequest.avatarId || "").trim()
                            === String(root.pendingEntryAvatarId || "").trim()) {
                        root.pendingEntryDisplayNameRequestId = String(
                            matchingRequest.requestId || "").trim()
                        root.pendingEntryDisplayNameRequestKey = String(
                            matchingRequest.key || "").trim()
                        root.pendingEntryDisplayNameWriteSucceeded = Boolean(
                            matchingRequest.writeSucceeded)
                    }
                }
                return true
            }
        }

        var request = root.pendingEntryDisplayNameRequestForCurrentWorkspace()
        var displayName = String(request.displayName || "").trim()
        var avatarId = String(request.avatarId || "").trim()
        var requestId = String(request.requestId || "").trim()
        if (displayName.length === 0
                || String(request.key || "").trim().length === 0) {
            return false
        }
        root.pendingEntryDisplayName = displayName
        root.pendingEntryAvatarId = avatarId
        root.pendingEntryDisplayNameWorkspaceId = currentWorkspaceId
        root.pendingEntryDisplayNameRequestId = requestId
        root.pendingEntryDisplayNameRequestKey = String(
            request.key || "").trim()
        root.pendingEntryDisplayNameWriteSucceeded = Boolean(
            request.writeSucceeded)
        root.pendingEntryDisplayNameRetryAttempt = 0
        root.pendingEntryDisplayNameReconcileAttempt = 0
        return true
    }

    function schedulePendingEntryDisplayNameRetry() {
        if (String(root.pendingEntryDisplayName || "").trim().length === 0
                || root.pendingEntryDisplayNameUpdateInFlight
                || root.pendingEntryDisplayNameWriteSucceeded
                || root.pendingEntryDisplayNameRetryAttempt
                    >= root.pendingEntryDisplayNameRetryLimit) {
            return false
        }
        root.pendingEntryDisplayNameRetryAttempt += 1
        pendingEntryDisplayNameRetryTimer.interval = Math.min(
            8000,
            500 * Math.pow(2, root.pendingEntryDisplayNameRetryAttempt))
        pendingEntryDisplayNameRetryTimer.restart()
        return true
    }

    function schedulePendingEntryDisplayNameReconciliation() {
        if (String(root.pendingEntryDisplayName || "").trim().length === 0
                || !root.pendingEntryDisplayNameWriteSucceeded) {
            return false
        }
        root.pendingEntryDisplayNameReconcileAttempt = Math.min(
            root.pendingEntryDisplayNameReconcileAttempt + 1, 5)
        pendingEntryDisplayNameRetryTimer.interval = Math.min(
            8000,
            500 * Math.pow(2,
                root.pendingEntryDisplayNameReconcileAttempt))
        pendingEntryDisplayNameRetryTimer.restart()
        return true
    }

    function reconcilePendingEntryDisplayName() {
        if (!root.pendingEntryDisplayNameWriteSucceeded) {
            return root.applyPendingEntryDisplayName()
        }
        if (root.pendingEntryDisplayNameConfirmed()) {
            return root.completePendingEntryDisplayName()
        }
        var targetWorkspaceId = String(
            root.pendingEntryDisplayNameWorkspaceId || "").trim()
        if (!root.runtimeWorkReady
                || targetWorkspaceId.length === 0
                || root.currentWorkspaceId() !== targetWorkspaceId
                || root.workspaceOperationInFlight
                || root.pendingEntryDisplayNameUpdateInFlight) {
            return root.schedulePendingEntryDisplayNameReconciliation()
        }
        if (chaftController.reconcileRuntimeSnapshotIfIdle()) {
            return true
        }
        return root.schedulePendingEntryDisplayNameReconciliation()
    }

    function applyPendingEntryDisplayName() {
        var displayName = String(root.pendingEntryDisplayName || "").trim()
        var avatarId = String(root.pendingEntryAvatarId || "").trim()
        if (displayName.length === 0) {
            return false
        }
        if (!root.runtimeWorkReady || root.currentWorkspaceId().length === 0) {
            return false
        }
        var targetWorkspaceId = String(
            root.pendingEntryDisplayNameWorkspaceId || "").trim()
        if (targetWorkspaceId.length > 0
                && root.currentWorkspaceId() !== targetWorkspaceId) {
            return false
        }
        if (root.pendingEntryDisplayNameConfirmed()) {
            return root.completePendingEntryDisplayName()
        }
        if (root.pendingEntryDisplayNameWriteSucceeded) {
            return root.schedulePendingEntryDisplayNameReconciliation()
        }
        if (!root.localDeviceMembershipReady()
                || root.workspaceOperationInFlight
                || root.pendingEntryDisplayNameUpdateInFlight
                || root.pendingEntryDisplayNameRetryAttempt
                    >= root.pendingEntryDisplayNameRetryLimit) {
            return false
        }

        pendingEntryDisplayNameRetryTimer.stop()
        root.pendingEntryDisplayNameUpdateName = displayName
        root.pendingEntryDisplayNameUpdateAvatarId = avatarId
        root.pendingEntryDisplayNameUpdateWorkspaceId =
            root.currentWorkspaceId()
        root.pendingEntryDisplayNameUpdateInFlight = true
        var accepted = avatarId.length > 0
            ? chaftController.updateDeviceProfileWithAvatar(
                displayName, avatarId)
            : chaftController.updateDeviceProfile(displayName)
        if (accepted) {
            return true
        }
        root.pendingEntryDisplayNameUpdateInFlight = false
        root.pendingEntryDisplayNameUpdateName = ""
        root.pendingEntryDisplayNameUpdateAvatarId = ""
        root.pendingEntryDisplayNameUpdateWorkspaceId = ""
        root.schedulePendingEntryDisplayNameRetry()
        return false
    }

    function handleDeviceProfileUpdateFinished(workspaceId, displayName,
                                               success, message) {
        var completedWorkspaceId = String(workspaceId || "").trim()
        var completedDisplayName = String(displayName || "").trim()
        if (!root.pendingEntryDisplayNameUpdateInFlight
                || completedWorkspaceId
                    !== root.pendingEntryDisplayNameUpdateWorkspaceId
                || completedDisplayName
                    !== root.pendingEntryDisplayNameUpdateName) {
            return
        }

        root.pendingEntryDisplayNameUpdateInFlight = false
        root.pendingEntryDisplayNameUpdateName = ""
        root.pendingEntryDisplayNameUpdateAvatarId = ""
        root.pendingEntryDisplayNameUpdateWorkspaceId = ""
        if (String(root.pendingEntryDisplayName || "").trim().length === 0) {
            root.scheduleControllerIdleWork()
            return
        }
        if (root.pendingEntryDisplayNameConfirmed()) {
            var confirmedRequestId = String(
                root.pendingEntryDisplayNameRequestId || "").trim()
            var confirmedRequestKey = String(
                root.pendingEntryDisplayNameRequestKey || "").trim()
            root.clearPendingEntryDisplayName()
            root.clearPendingEntryDisplayNamePersistence(
                confirmedRequestKey, confirmedRequestId)
            root.scheduleControllerIdleWork()
            return
        }
        if (success) {
            // The signed event was appended. From this point onward, only
            // reconcile snapshots; repeating updateDeviceProfile would append
            // duplicate profile events when UI refresh is merely delayed.
            root.pendingEntryDisplayNameWriteSucceeded = true
            root.pendingEntryDisplayNameRetryAttempt = 0
            root.pendingEntryDisplayNameReconcileAttempt = 0
            root.persistPendingEntryDisplayNameState("profile_written")
            root.schedulePendingEntryDisplayNameReconciliation()
        } else {
            root.schedulePendingEntryDisplayNameRetry()
        }
    }

    function completePendingEntryDisplayName() {
        if (!root.pendingEntryDisplayNameConfirmed()) {
            return false
        }
        var requestId = String(
            root.pendingEntryDisplayNameRequestId || "").trim()
        var requestKey = String(
            root.pendingEntryDisplayNameRequestKey || "").trim()
        root.clearPendingEntryDisplayName()
        root.clearPendingEntryDisplayNamePersistence(requestKey, requestId)
        return true
    }

    function clearPendingEntryDisplayName() {
        root.pendingEntryDisplayName = ""
        root.pendingEntryAvatarId = ""
        root.pendingEntryDisplayNameWorkspaceId = ""
        root.pendingEntryDisplayNameRequestId = ""
        root.pendingEntryDisplayNameRequestKey = ""
        root.pendingEntryDisplayNameUpdateInFlight = false
        root.pendingEntryDisplayNameUpdateName = ""
        root.pendingEntryDisplayNameUpdateAvatarId = ""
        root.pendingEntryDisplayNameUpdateWorkspaceId = ""
        root.pendingEntryDisplayNameWriteSucceeded = false
        root.pendingEntryDisplayNameRetryAttempt = 0
        root.pendingEntryDisplayNameReconcileAttempt = 0
        pendingEntryDisplayNameRetryTimer.stop()
    }

    function memberLabel(member) {
        var displayName = String(member.displayName || "").trim()
        if (displayName.length > 0) {
            return displayName
        }
        var deviceId = String(member.deviceId || "")
        return root.unnamedPersonLabel(deviceId)
    }

    function unnamedPersonLabel(deviceId) {
        return "Unnamed teammate"
    }

    function memberInitial(member) {
        var label = root.memberLabel(member)
        return label.length > 0 ? label.slice(0, 1).toUpperCase() : "?"
    }

    function normalizedRole(role) {
        return String(role || "").trim().toLowerCase()
    }

    function normalizedWorkspaceAccessPolicy(policy) {
        var normalized = String(policy || "").trim().toLowerCase()
        if (normalized === "request_access" || normalized === "discoverable") {
            return normalized
        }
        return "invite_only"
    }

    function workspaceAccessPolicyLabel(policy) {
        switch (root.normalizedWorkspaceAccessPolicy(policy)) {
        case "request_access":
            return "People can request access"
        case "discoverable":
            return "Discoverable not live"
        case "invite_only":
        default:
            return "Invite only"
        }
    }

    function workspaceAccessPolicyDescription(policy) {
        switch (root.normalizedWorkspaceAccessPolicy(policy)) {
        case "request_access":
            return "New people can send a request for an owner or admin to approve."
        case "discoverable":
            return "Discovery is not live yet. Use request links or direct invites for now."
        case "invite_only":
        default:
            return "Only people with an invite from an owner or admin can join."
        }
    }

    function workspaceAccessPolicyAllowsRequests(policy) {
        var normalized = root.normalizedWorkspaceAccessPolicy(policy)
        return normalized === "request_access" || normalized === "discoverable"
    }

    function roleLabel(role) {
        switch (root.normalizedRole(role)) {
        case "owner":
            return "Owner"
        case "admin":
            return "Admin"
        case "guest":
            return "Guest"
        case "member":
        default:
            return "Member"
        }
    }

    function roleDescription(role) {
        switch (root.normalizedRole(role)) {
        case "owner":
            return "Owners can manage the workspace, invites, access, and security-critical settings."
        case "admin":
            return "Admins can invite people, manage access, and help keep the workspace reachable."
        case "guest":
            return "Guests can read and send messages only in spaces they are allowed to access."
        case "member":
        default:
            return "Members can join conversations and create regular rooms, but cannot manage workspace access."
        }
    }

    function localMember() {
        var localDeviceId = String(chaftController.deviceId || "")
        if (localDeviceId.length === 0) {
            return null
        }
        for (var i = 0; i < root.members.length; i += 1) {
            if (String(root.members[i].deviceId || "") === localDeviceId) {
                return root.members[i]
            }
        }
        return null
    }

    function localRole() {
        var member = root.localMember()
        return member === null ? "" : root.normalizedRole(member.role)
    }

    function canManageWorkspaceAccess() {
        var role = root.localRole()
        return role === "owner" || role === "admin"
    }

    function canManageWorkspaceOwners() {
        return root.localRole() === "owner"
    }

    function workspaceAccessUnavailableReason() {
        if (!root.runtimeWorkReady) {
            return "Open a workspace before changing access."
        }
        var role = root.localRole()
        if (role.length === 0) {
            return "Your role is still loading."
        }
        return "Only owners and admins can invite people or change workspace access."
    }

    function memberRoleOptions(member) {
        var options = []
        var role = root.normalizedRole(member ? member.role : "")
        if (root.canManageWorkspaceOwners()) {
            options.push({ label: "Owner", role: "owner" })
            options.push({ label: "Admin", role: "admin" })
        }
        if (role !== "owner" && (role !== "admin" || root.canManageWorkspaceOwners())) {
            options.push({ label: "Member", role: "member" })
            options.push({ label: "Guest", role: "guest" })
        }
        return options
    }

    function canChangeMemberRole(member) {
        if (!root.runtimeWorkReady || !root.canManageWorkspaceAccess() || member === null) {
            return false
        }
        var deviceId = String(member.deviceId || "")
        if (deviceId.length === 0 || deviceId === String(chaftController.deviceId || "")) {
            return false
        }
        var role = root.normalizedRole(member.role)
        return (role !== "owner" && role !== "admin") || root.canManageWorkspaceOwners()
    }

    function memberRoleUnavailableReason(member) {
        if (!root.runtimeWorkReady) {
            return "Open a workspace before changing roles."
        }
        if (!root.canManageWorkspaceAccess()) {
            return root.workspaceAccessUnavailableReason()
        }
        if (member !== null
                && String(member.deviceId || "") === String(chaftController.deviceId || "")) {
            return "You cannot change your own role here."
        }
        if (member !== null) {
            var role = root.normalizedRole(member.role)
            if ((role === "owner" || role === "admin")
                    && !root.canManageWorkspaceOwners()) {
                return "Only an owner can change admin or owner roles."
            }
        }
        return "Choose a different role first."
    }

    function canRemoveMember(member) {
        if (!root.runtimeWorkReady || !root.canManageWorkspaceAccess() || member === null) {
            return false
        }
        var deviceId = String(member.deviceId || "")
        if (deviceId.length === 0 || deviceId === String(chaftController.deviceId || "")) {
            return false
        }
        var role = root.normalizedRole(member.role)
        return (role !== "owner" && role !== "admin") || root.canManageWorkspaceOwners()
    }

    function memberRemovalUnavailableReason(member) {
        if (!root.runtimeWorkReady) {
            return "Open a workspace before removing people."
        }
        if (!root.canManageWorkspaceAccess()) {
            return root.workspaceAccessUnavailableReason()
        }
        if (member !== null
                && String(member.deviceId || "") === String(chaftController.deviceId || "")) {
            return "You cannot remove your own access here."
        }
        if (member !== null) {
            var role = root.normalizedRole(member.role)
            if ((role === "owner" || role === "admin")
                    && !root.canManageWorkspaceOwners()) {
                return "Only an owner can remove admins or owners."
            }
        }
        return "This person cannot be removed right now."
    }

    function confirmMemberRemoval(deviceId, displayLabel) {
        var normalizedDeviceId = String(deviceId || "").trim()
        if (normalizedDeviceId.length === 0) {
            return false
        }
        var label = String(displayLabel || "").trim()
        if (label.length === 0) {
            label = "this person"
        }
        confirmDialog.ask(
            "Remove " + label + "?",
            "Remove " + label + " from this workspace? New messages will be protected from them after the access refresh completes.",
            "Remove",
            "remove-member:" + normalizedDeviceId,
            true)
        return true
    }

    function confirmMemberRoleChange(deviceId, displayLabel, role) {
        var normalizedDeviceId = String(deviceId || "").trim()
        var normalizedRole = root.normalizedRole(role)
        if (normalizedDeviceId.length === 0 || normalizedRole.length === 0) {
            return false
        }
        var label = String(displayLabel || "").trim()
        if (label.length === 0) {
            label = "this person"
        }
        var member = root.memberByDeviceId(normalizedDeviceId)
        var currentRole = root.normalizedRole(member ? member.role : "")
        if (currentRole === normalizedRole) {
            return false
        }
        var nextRoleLabel = root.roleLabel(normalizedRole)
        var currentRoleLabel = root.roleLabel(currentRole)
        var grantsAccessManagement = normalizedRole === "admin"
            || normalizedRole === "owner"
        var removesAccessManagement = (currentRole === "admin"
                || currentRole === "owner")
            && !grantsAccessManagement
        var consequence = root.roleDescription(normalizedRole)
        if (grantsAccessManagement) {
            consequence += " This grants access-management privileges, including the ability to invite or remove people."
        } else if (removesAccessManagement) {
            consequence += " They will no longer be able to manage workspace access."
        }
        confirmDialog.ask(
            "Make " + label + " " + nextRoleLabel.toLowerCase() + "?",
            "Change from " + currentRoleLabel + " to " + nextRoleLabel
                + ". " + consequence
                + " The change takes effect after their device receives it.",
            "Change to " + nextRoleLabel,
            "update-member-role:" + normalizedDeviceId + "::" + normalizedRole,
            grantsAccessManagement || removesAccessManagement)
        return true
    }

    function inviteExpiresAtIso(days) {
        var count = Number(days)
        if (!isFinite(count) || count <= 0) {
            return ""
        }
        return new Date(Date.now() + count * 24 * 60 * 60 * 1000).toISOString()
    }

    function inviteExpiryLabel(expiresAt) {
        var value = String(expiresAt || "").trim()
        if (value.length === 0) {
            return "Never expires"
        }
        var timestamp = Date.parse(value)
        if (!isFinite(timestamp)) {
            return "Expiry unknown"
        }
        var delta = timestamp - Date.now()
        if (delta <= 0) {
            return "Expired"
        }
        var days = Math.ceil(delta / (24 * 60 * 60 * 1000))
        return days === 1 ? "Expires in 1 day" : "Expires in " + days + " days"
    }

    function inviteExpired(expiresAt) {
        var value = String(expiresAt || "").trim()
        if (value.length === 0) {
            return false
        }
        var timestamp = Date.parse(value)
        return isFinite(timestamp) && timestamp <= Date.now()
    }

    function inviteMaxClaims(invite) {
        var value = Number(invite && invite.maxClaims)
        if (!isFinite(value) || value < 1) {
            return 1
        }
        return Math.floor(value)
    }

    function inviteClaimCount(invite) {
        var maximum = root.inviteMaxClaims(invite)
        var value = Number(invite && invite.claimCount)
        if (!isFinite(value) || value < 0) {
            return 0
        }
        return Math.min(maximum, Math.floor(value))
    }

    function inviteRemainingClaims(invite) {
        var maximum = root.inviteMaxClaims(invite)
        var explicit = invite && invite.remainingClaims !== undefined
                && invite.remainingClaims !== null
            ? Number(invite.remainingClaims)
            : NaN
        if (isFinite(explicit) && explicit >= 0) {
            return Math.min(maximum, Math.floor(explicit))
        }
        return Math.max(0, maximum - root.inviteClaimCount(invite))
    }

    function inviteClaimLimitLabel(invite) {
        var maximum = root.inviteMaxClaims(invite)
        return maximum === 1 ? "1 join" : maximum + " joins"
    }

    function inviteClaimAvailabilityLabel(invite) {
        var maximum = root.inviteMaxClaims(invite)
        var remaining = root.inviteRemainingClaims(invite)
        if (remaining === 0) {
            return "No joins remaining"
        }
        if (maximum === 1) {
            return "1 join remaining"
        }
        return remaining + " of " + maximum + " joins remaining"
    }

    function inviteClaimUsageLabel(invite) {
        var maximum = root.inviteMaxClaims(invite)
        var used = root.inviteClaimCount(invite)
        if (used === 0) {
            return "Not used"
        }
        if (maximum === 1) {
            return "1 join used"
        }
        return used + " of " + maximum + " joins used"
    }

    function inviteApprovalLabel(policy) {
        var value = String(policy || "preapproved").trim()
        return value === "admin_required" ? "Needs admin approval" : "Approved when created"
    }

    function inviteApprovalBlocksJoin(policy) {
        return String(policy || "preapproved").trim() === "admin_required"
    }

    function inviteSyncExpectation(invite, fallbackPeerEndpoint) {
        var policy = String((invite && invite.approvalPolicy) || "preapproved").trim()
        if (root.inviteApprovalBlocksJoin(policy)) {
            return "waiting_for_admin_approval"
        }
        var explicit = String((invite && invite.syncExpectation) || "").trim()
        if (explicit === "auto_fetch_from_invite_source"
                || explicit === "needs_reachable_teammate"
                || explicit === "waiting_for_admin_approval") {
            return explicit
        }
        var endpoint = String((invite && invite.peerEndpoint) || fallbackPeerEndpoint || "").trim()
        return endpoint.length > 0
            ? "auto_fetch_from_invite_source"
            : "needs_reachable_teammate"
    }

    function inviteSyncExpectationLabel(invite, fallbackPeerEndpoint) {
        var expectation = root.inviteSyncExpectation(invite, fallbackPeerEndpoint)
        if (expectation === "waiting_for_admin_approval") {
            return "Waiting for admin approval"
        }
        if (expectation === "auto_fetch_from_invite_source") {
            return "History can load from the invite"
        }
        return "Needs a reachable teammate"
    }

    function inviteSyncExpectationMessage(invite, fallbackPeerEndpoint) {
        var expectation = root.inviteSyncExpectation(invite, fallbackPeerEndpoint)
        if (expectation === "waiting_for_admin_approval") {
            return "Ask a workspace admin to approve this invite before messages can load."
        }
        if (expectation === "auto_fetch_from_invite_source") {
            return "Chaft will join and start loading history from the invite."
        }
        return "Chaft can join now. History loads when someone with this workspace is reachable."
    }

    function isOpenMlsKeyPackage(keyPackage) {
        return String(keyPackage.protocol || "").indexOf("openmls/key-package") === 0
    }

    function workspaceInitial(workspace) {
        var name = String(workspace.name || workspace.workspaceId || "C").trim()
        return name.length > 0 ? name.slice(0, 1).toUpperCase() : "C"
    }

    function workspaceDisplayName(workspace) {
        var row = workspace || {}
        var name = String(row.name || row.workspaceName || "").trim()
        if (name.length > 0) {
            return name
        }
        var workspaceId = String(row.workspaceId || "").trim()
        return workspaceId.length > 0
            ? root.shortAccessIdentifier(workspaceId)
            : "Workspace"
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

    function openWorkspaceEntry(mode, intent) {
        if (root.composerContextChangeBlocked()) {
            return false
        }
        root.workspaceEntryMode = mode === "create" ? "create" : "join"
        root.workspaceEntryIntent = String(intent || root.workspaceEntryMode)
        workspaceEntryDialog.open()
        return true
    }

    function openReceivedApprovalInvite(forceOpen) {
        var inviteText = String(chaftController.keyTransferJson || "").trim()
        if (inviteText.length === 0
                || !chaftController.keyTransferFromJoinResponseInbox
                || (!root.keyTransferIsInvitePackage()
                    && !root.keyTransferIsInviteResponse())) {
            return false
        }
        if (workspaceEntryDialog.createOperationPending) {
            toastHost.show(
                "success",
                "Workspace access received. Finish creating the workspace; Chaft will keep it ready to open.",
                "Open when ready",
                "open-received-approval",
                9000)
            return false
        }
        if (root.composerContextBusy) {
            toastHost.show(
                "success",
                "Workspace access received. Finish the current message "
                    + "operation, then open it from the notification.",
                "Open when ready",
                "open-received-approval",
                9000)
            return false
        }
        var response = root.parsedCredentialObject(inviteText)
        var responseRequestId = String((response && response.requestId) || "").trim()
        var pendingRequest = root.pendingAccessRequestRowByRequestId(responseRequestId)
        var pendingDisplayName = String(
            (pendingRequest && pendingRequest.displayName) || "").trim()
        var existingText = String(workspaceEntryDialog.credentialsText || "").trim()
        var secureResponse = root.keyTransferIsInviteResponse()
        var shouldForce = forceOpen === true
        var waitingForThisApproval = workspaceEntryDialog.visible
            && workspaceEntryDialog.joinRequestPrepared
            && responseRequestId.length > 0
            && responseRequestId
                === String(workspaceEntryDialog.joinRequestPreparedRequestId || "").trim()
        if (workspaceEntryDialog.visible
                && existingText.length > 0
                && existingText !== inviteText
                && !waitingForThisApproval
                && !shouldForce) {
            toastHost.show(
                "success",
                secureResponse
                    ? "Encrypted access received. Finish the open handoff or open it when ready."
                    : "Approval received. Finish the open handoff or open the new invite when ready.",
                "Open",
                "open-received-approval",
                9000)
            return false
        }

        workspaceEntryDialog.beginReceivedApproval(
            pendingDisplayName, responseRequestId)
        root.openWorkspaceEntry("join", "received-approval")
        Qt.callLater(function() {
            var currentInviteText = String(chaftController.keyTransferJson || "").trim()
            if (!chaftController.keyTransferFromJoinResponseInbox
                    || currentInviteText.length === 0
                    || currentInviteText !== inviteText) {
                return
            }
            workspaceEntryDialog.clearCredentialImportFailure()
            root.loadWorkspaceCredentialText(currentInviteText)
            var endpoint = root.credentialPeerEndpoint(currentInviteText)
            if (endpoint.length > 0) {
                workspaceEntryDialog.peerEndpointText = endpoint
            }
        })
        toastHost.show(
            "success",
            secureResponse
                ? "Encrypted access received. Review it and join when ready."
                : "Approval received. Review the invite and join when ready.",
            "Open",
            "open-received-approval",
            7000)
        return true
    }

    function openAddWorkspaceChooser() {
        if (root.composerContextChangeBlocked()) {
            return false
        }
        addWorkspacePopup.open()
        return true
    }

    function chooseWorkspaceEntry(mode, intent) {
        addWorkspacePopup.close()
        return root.openWorkspaceEntry(mode, intent)
    }

    function chooseDemoWorkspace() {
        addWorkspacePopup.close()
        root.startDemoTour()
    }

    function openWorkspaceCredentialFile() {
        workspaceCredentialDialog.open()
    }

    function parsedJsonObject(text) {
        try {
            var parsed = JSON.parse(String(text || ""))
            return parsed && typeof parsed === "object" && !Array.isArray(parsed)
                ? parsed
                : null
        } catch (error) {
            return null
        }
    }

    function artifactPayloadFromLink(text) {
        var value = String(text || "").trim()
        if (value.indexOf("chaft-invite:") !== 0
                && value.indexOf("chaft-request:") !== 0
                && value.indexOf("chaft-workspace:") !== 0) {
            return ""
        }
        var payloadIndex = value.indexOf("payload=")
        var payload = payloadIndex >= 0
            ? value.slice(payloadIndex + 8)
            : value.slice(value.indexOf(":") + 1)
        var ampIndex = payload.indexOf("&")
        if (ampIndex >= 0) {
            payload = payload.slice(0, ampIndex)
        }
        if (payload.charAt(0) === "?") {
            payload = payload.slice(1)
        }
        if (payload.length === 0) {
            return ""
        }
        try {
            return decodeURIComponent(payload)
        } catch (error) {
            return ""
        }
    }

    function credentialObjectFromArtifactObject(parsed) {
        if (parsed === null) {
            return null
        }
        var kind = String(parsed.kind || "")
        if (kind === "chaft.invite-file.v1"
                && parsed.invite !== undefined
                && parsed.invite !== null
                && typeof parsed.invite === "object"
                && !Array.isArray(parsed.invite)) {
            return parsed.invite
        }
        if (kind === "chaft.join-request-file.v1"
                && parsed.request !== undefined
                && parsed.request !== null
                && typeof parsed.request === "object"
                && !Array.isArray(parsed.request)) {
            return parsed.request
        }
        if (kind === "chaft.workspace-card-file.v1"
                && parsed.card !== undefined
                && parsed.card !== null
                && typeof parsed.card === "object"
                && !Array.isArray(parsed.card)) {
            return parsed.card
        }
        return null
    }

    function credentialPayloadText(credentials) {
        var text = String(credentials || "").trim()
        var linkPayload = root.artifactPayloadFromLink(text)
        if (linkPayload.length > 0) {
            text = linkPayload
        }
        var parsed = root.parsedJsonObject(text)
        var artifactObject = root.credentialObjectFromArtifactObject(parsed)
        return artifactObject === null ? text : JSON.stringify(artifactObject)
    }

    function parsedCredentialObject(credentials) {
        return root.parsedJsonObject(root.credentialPayloadText(credentials))
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

    function credentialWorkspaceId(credentials) {
        var parsed = root.parsedCredentialObject(credentials)
        if (parsed === null) {
            return ""
        }
        if (parsed.workspaceId !== undefined) {
            return String(parsed.workspaceId || "").trim()
        }
        var workspaceKey = root.credentialWorkspaceKeyObject(parsed)
        if (workspaceKey !== null && workspaceKey.workspaceId !== undefined) {
            return String(workspaceKey.workspaceId || "").trim()
        }
        var recoveryBundle = root.credentialRecoveryBundleObject(parsed)
        if (recoveryBundle !== null && recoveryBundle.workspaceId !== undefined) {
            return String(recoveryBundle.workspaceId || "").trim()
        }
        return ""
    }

    function credentialRecoveryBundleObject(parsed) {
        if (parsed === null) {
            return null
        }
        if (parsed.recoveryBundle !== undefined
                && parsed.recoveryBundle !== null
                && typeof parsed.recoveryBundle === "object"
                && !Array.isArray(parsed.recoveryBundle)) {
            return parsed.recoveryBundle
        }
        if (parsed.schemaVersion !== undefined
                && parsed.workspaceId !== undefined
                && parsed.exporterDeviceId !== undefined
                && parsed.kdf !== undefined
                && parsed.sealedPayload !== undefined) {
            return parsed
        }
        return null
    }

    function credentialWorkspaceKeyObject(parsed) {
        if (parsed === null) {
            return null
        }
        if (String(parsed.kind || "") === "chaft.workspace-invite.v1"
                && parsed.workspaceKey !== undefined
                && parsed.workspaceKey !== null
                && typeof parsed.workspaceKey === "object"
                && !Array.isArray(parsed.workspaceKey)) {
            return parsed.workspaceKey
        }
        if (parsed.workspaceKey !== undefined
                && parsed.workspaceKey !== null
                && typeof parsed.workspaceKey === "object"
                && !Array.isArray(parsed.workspaceKey)) {
            return parsed.workspaceKey
        }
        if (parsed.workspaceId !== undefined
                && (parsed.aes256GcmSivKey !== undefined
                    || parsed.aes_256_gcm_siv_key !== undefined)) {
            return parsed
        }
        return null
    }

    function shortAccessIdentifier(value) {
        var text = String(value || "").trim()
        return text.length > 18 ? text.slice(0, 8) + "..." + text.slice(text.length - 6) : text
    }

    function credentialSummaryRow(label, value) {
        var normalized = String(value || "").trim()
        return normalized.length > 0
            ? ({ label: String(label || ""), value: normalized })
            : null
    }

    function workspaceCardAdminLabel(card) {
        var adminName = String((card && card.adminDisplayName) || "").trim()
        if (adminName.length > 0) {
            return adminName
        }
        return "a workspace admin"
    }

    function workspaceCardRequestRouteLabel(card) {
        var endpoint = String((card && card.peerEndpoint) || "").trim()
        if (endpoint.length === 0) {
            return ""
        }
        return root.workspaceCardAdminLabel(card) + " in Chaft"
    }

    function credentialImportSummary(credentials, restoreMode, peerEndpoint, passphrase) {
        var text = String(credentials || "").trim()
        if (text.length === 0) {
            return ({
                title: "",
                message: "",
                rows: [],
                canImport: false,
                warning: false
            })
        }

        var parsed = root.parsedCredentialObject(text)
        if (parsed === null) {
            return ({
                title: "This does not look like a Chaft file",
                message: "Open or paste an invite, request link, decryption key kit, or access file.",
                rows: [],
                canImport: false,
                warning: true
            })
        }

        var kind = String(parsed.kind || "")
        var rows = []
        var row
        if (kind === "chaft.workspace-card.v1") {
            var cardAllowsRequests = root.workspaceAccessPolicyAllowsRequests(parsed.accessPolicy)
            row = root.credentialSummaryRow("Workspace", parsed.workspaceName
                || root.shortAccessIdentifier(parsed.workspaceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Who can join",
                root.workspaceAccessPolicyLabel(parsed.accessPolicy))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Shared by", parsed.adminDisplayName
                || root.shortDeviceId(parsed.adminDeviceId))
            if (row !== null) {
                rows.push(row)
            }
            row = cardAllowsRequests
                ? root.credentialSummaryRow("Send request to",
                    root.workspaceCardRequestRouteLabel(parsed))
                : root.credentialSummaryRow("Invite contact",
                    root.workspaceCardAdminLabel(parsed))
            if (row !== null) {
                rows.push(row)
            }
            return ({
                title: cardAllowsRequests ? "Request link" : "Invite required",
                message: cardAllowsRequests
                    ? "This link does not grant access. Use it to send an access request for an owner or admin to approve."
                    : "This link does not grant access. Ask an owner or admin for a Chaft invite to join.",
                rows: rows,
                canImport: false,
                warning: false
            })
        }

        if (kind === "chaft.workspace-join-request.v1") {
            row = root.credentialSummaryRow("From", parsed.displayName || "Unnamed teammate")
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Workspace", parsed.workspaceName
                || root.shortAccessIdentifier(parsed.workspaceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Support code", root.shortDeviceId(parsed.deviceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Note", parsed.note)
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Send to", parsed.deliveryDisplayName
                || (String(parsed.deliveryDeviceId || "").trim().length > 0
                    ? "the workspace admin"
                    : ""))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Started with", root.joinRequestSourceLabel(parsed))
            if (row !== null) {
                rows.push(row)
            }
            return ({
                title: "Access request",
                message: "Send this to a workspace admin. It is not an invite, so it cannot join the workspace yet.",
                rows: rows,
                canImport: false,
                warning: true
            })
        }

        if (kind === "chaft.workspace-invite.v1") {
            var inviteExpired = root.inviteExpired(parsed.expiresAt)
            var approvalBlocksJoin = root.inviteApprovalBlocksJoin(parsed.approvalPolicy)
            row = root.credentialSummaryRow("Workspace", parsed.workspaceName
                || root.shortAccessIdentifier(parsed.workspaceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Invited by", parsed.inviterDisplayName
                || root.shortDeviceId(parsed.inviterDeviceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow(
                "Invite label", parsed.inviteeDisplayName)
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Access", root.roleLabel(parsed.role || "member"))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Expires", root.inviteExpiryLabel(parsed.expiresAt))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Approval", root.inviteApprovalLabel(parsed.approvalPolicy))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("History",
                root.inviteSyncExpectationLabel(parsed, peerEndpoint))
            if (row !== null) {
                rows.push(row)
            }
            return ({
                title: inviteExpired
                    ? "Invite expired"
                    : (approvalBlocksJoin
                        ? "Invite needs approval"
                        : "Invite ready"),
                message: inviteExpired
                    ? "Ask a workspace admin for a new invite."
                    : root.inviteSyncExpectationMessage(parsed, peerEndpoint),
                rows: rows,
                canImport: !inviteExpired && !approvalBlocksJoin,
                warning: inviteExpired || approvalBlocksJoin
            })
        }

        if (kind === "chaft.workspace-invite.v2") {
            var claimInviteExpired = root.inviteExpired(parsed.expiresAt)
            row = root.credentialSummaryRow("Workspace", parsed.workspaceName
                || root.shortAccessIdentifier(parsed.workspaceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Invited by", parsed.inviterDisplayName
                || root.shortDeviceId(parsed.inviterDeviceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Access", root.roleLabel(parsed.role || "member"))
            if (row !== null) {
                rows.push(row)
            }
            var inviteDeviceLimit = root.inviteMaxClaims(parsed)
            row = root.credentialSummaryRow("Maximum joins",
                inviteDeviceLimit === 1
                    ? "1 join"
                    : inviteDeviceLimit + " joins")
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Expires", root.inviteExpiryLabel(parsed.expiresAt))
            if (row !== null) {
                rows.push(row)
            }
            return ({
                title: claimInviteExpired ? "Invite expired" : "Secure invite ready",
                message: claimInviteExpired
                    ? "Ask a workspace admin for a new invite."
                    : "Join from this device. Workspace access stays encrypted until the owner responds.",
                rows: rows,
                canImport: false,
                warning: claimInviteExpired
            })
        }

        if (kind === "chaft.workspace-invite-response.v1") {
            var responseTargetsThisDevice = String(parsed.inviteeDeviceId || "").trim()
                === String(chaftController.deviceId || "").trim()
            row = root.credentialSummaryRow("Workspace", parsed.workspaceName
                || root.shortAccessIdentifier(parsed.workspaceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Access", root.roleLabel(parsed.role || "member"))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Approved by",
                root.shortDeviceId(parsed.responderDeviceId))
            if (row !== null) {
                rows.push(row)
            }
            return ({
                title: responseTargetsThisDevice
                    ? "Secure access approved"
                    : "Approval belongs to another device",
                message: responseTargetsThisDevice
                    ? "This encrypted approval can be opened only on the device that requested access."
                    : "Open this approval on the device that originally requested access.",
                rows: rows,
                canImport: responseTargetsThisDevice,
                warning: !responseTargetsThisDevice
            })
        }

        var recoveryBundle = root.credentialRecoveryBundleObject(parsed)
        if (recoveryBundle !== null) {
            row = root.credentialSummaryRow("Workspace",
                root.shortAccessIdentifier(recoveryBundle.workspaceId || parsed.workspaceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("Passphrase",
                String(passphrase || "").trim().length > 0 ? "Ready" : "Required")
            if (row !== null) {
                rows.push(row)
            }
            return ({
                title: "Decryption key kit found",
                message: String(passphrase || "").trim().length > 0
                    ? "This imports local decryption keys. If this device is not already authorized, an invite is required before Chaft can show or send workspace content."
                    : "Enter the passphrase used when this kit was saved. Keep the kit private; it is not an invite or an access approval.",
                rows: rows,
                canImport: String(passphrase || "").trim().length > 0,
                warning: String(passphrase || "").trim().length === 0
            })
        }

        var workspaceKey = root.credentialWorkspaceKeyObject(parsed)
        if (workspaceKey !== null) {
            row = root.credentialSummaryRow("Workspace",
                parsed.workspaceName || root.shortAccessIdentifier(workspaceKey.workspaceId || parsed.workspaceId))
            if (row !== null) {
                rows.push(row)
            }
            row = root.credentialSummaryRow("History",
                String(peerEndpoint || "").trim().length > 0 ? "Ready to fetch" : "Fetch later")
            if (row !== null) {
                rows.push(row)
            }
            return ({
                title: "Workspace ready to join",
                message: String(peerEndpoint || "").trim().length > 0
                    ? "Chaft can join and fetch history from the saved teammate address."
                    : "Chaft can join now. Add a reachable teammate later to fetch history.",
                rows: rows,
                canImport: true,
                warning: false
            })
        }

        return ({
            title: "Unknown Chaft file",
            message: "This file is valid, but it is not an invite, request link, decryption key kit, or access file.",
            rows: [],
            canImport: false,
            warning: true
        })
    }

    function credentialCanSubmit(credentials, restoreMode, passphrase) {
        return root.credentialImportSummary(
            credentials, restoreMode, "", passphrase).canImport === true
    }

    function credentialImportFailureSummary(source, status) {
        var rawStatus = String(status || "").trim()
        var normalized = rawStatus.toLowerCase()
        var recovery = String(source || "") === "recovery"
        var detail = rawStatus.length > 160
            ? rawStatus.slice(0, 157) + "..."
            : rawStatus
        var passphraseLike = normalized.indexOf("open failed") >= 0
            || normalized.indexOf("openfailed") >= 0
            || normalized.indexOf("crypto") >= 0
            || normalized.indexOf("passphrase") >= 0
        if (recovery && passphraseLike) {
            return ({
                title: "Couldn't import key kit",
                message: "The passphrase may be wrong. Re-enter it, or open a newer decryption key kit.",
                detail: detail
            })
        }
        if (recovery) {
            return ({
                title: "Couldn't import key kit",
                message: "Open the decryption key kit again. If access is also missing, ask an admin for a new invite.",
                detail: detail
            })
        }
        return ({
            title: "Couldn't join workspace",
            message: "Open the latest invite, request link, or access file. Ask an admin for a new invite if it still fails.",
            detail: detail
        })
    }

    function clearPendingWorkspaceImport() {
        root.pendingWorkspaceImportActive = false
        root.pendingWorkspaceImportWorkspaceId = ""
        root.pendingWorkspaceImportRequestId = ""
        root.pendingWorkspaceImportPeerEndpoint = ""
        root.pendingWorkspaceImportSource = ""
        root.pendingWorkspaceImportInboxArtifact = ""
        workspaceEntryDialog.credentialImportPending = false
    }

    function handleWorkspaceCredentialImportFinished(source, workspaceId,
                                                     success, message) {
        if (!root.pendingWorkspaceImportActive) {
            return
        }
        var normalizedSource = String(source || "").trim()
        if (normalizedSource !== root.pendingWorkspaceImportSource) {
            return
        }
        var expectedWorkspaceId = root.pendingWorkspaceImportWorkspaceId
        var importedWorkspaceId = String(workspaceId || "").trim()
        if (success && expectedWorkspaceId.length > 0
                && importedWorkspaceId.length > 0
                && importedWorkspaceId !== expectedWorkspaceId) {
            success = false
            message = "The imported access belongs to a different workspace."
        }
        if (success) {
            var completedWorkspaceId = importedWorkspaceId.length > 0
                ? importedWorkspaceId
                : expectedWorkspaceId
            var requestId = root.pendingWorkspaceImportRequestId
            var peerEndpoint = root.pendingWorkspaceImportPeerEndpoint
            var inboxArtifact = root.pendingWorkspaceImportInboxArtifact
            var privateRoomCount = normalizedSource === "recovery"
                ? chaftController.lastRecoveryImportedChannelCount
                : -1
            var holdRequestUntilProfileConfirmed =
                String(root.pendingEntryDisplayName || "").trim().length > 0
                && root.pendingEntryDisplayNameWorkspaceId
                    === completedWorkspaceId
            if (holdRequestUntilProfileConfirmed) {
                root.pendingEntryDisplayNameRequestId = requestId
                root.persistPendingEntryDisplayNameState("profile_pending")
            } else if (requestId.length > 0) {
                root.clearPendingAccessRequestForRequestId(requestId)
            } else {
                root.clearPendingAccessRequestForWorkspace(completedWorkspaceId)
            }
            // A different approval can arrive while this import is running.
            // Acknowledge only the exact inbox artifact that started it, never
            // whichever response happens to be staged at completion time.
            if (normalizedSource === "access"
                    && inboxArtifact.length > 0
                    && chaftController.keyTransferFromJoinResponseInbox
                    && String(chaftController.keyTransferJson || "").trim()
                        === inboxArtifact) {
                chaftController.acknowledgeCurrentJoinResponseInboxEntry()
            }
            root.clearPendingWorkspaceImport()
            workspaceEntryDialog.clearCredentialImportFailure()
            if (peerEndpoint.length > 0) {
                chaftController.defaultPeerEndpoint = peerEndpoint
                root.pendingJoinPeerEndpoint = peerEndpoint
                root.rememberJoinWaitingForPeer(
                    completedWorkspaceId,
                    false,
                    normalizedSource,
                    privateRoomCount)
                root.scheduleControllerIdleWork()
            } else {
                root.rememberJoinWaitingForPeer(
                    completedWorkspaceId,
                    true,
                    normalizedSource,
                    privateRoomCount)
            }
            workspaceEntryDialog.close()
            return
        }
        var failedSource = root.pendingWorkspaceImportSource
        if (root.pendingEntryDisplayNameWorkspaceId
                === root.pendingWorkspaceImportWorkspaceId) {
            root.clearPendingEntryDisplayName()
        }
        root.clearPendingWorkspaceImport()
        workspaceEntryDialog.showCredentialImportFailure(
            failedSource, String(message || "Workspace import failed."))
    }

    function workspaceCredentialApprovalContext(credentials) {
        var parsed = root.parsedCredentialObject(credentials)
        var kind = String((parsed && parsed.kind) || "").trim()
        var candidateRequestId = String(
            (parsed && parsed.requestId) || "").trim()
        var pendingRequest = candidateRequestId.length > 0
            ? root.pendingAccessRequestRowByRequestId(candidateRequestId)
            : ({})
        var pendingDisplayName = String(
            (pendingRequest && pendingRequest.displayName) || "").trim()
        // A legacy approval can be reopened from disk outside the original
        // received-approval action. Only restore its identity when the exact
        // request is still recorded locally; never trust an invite-supplied
        // display name as the joiner's identity.
        var matchingLegacyApproval = kind === "chaft.workspace-invite.v1"
            && candidateRequestId.length > 0
            && (root.workspaceEntryIntent === "received-approval"
                || pendingDisplayName.length > 0)
        var recognized = kind === "chaft.workspace-invite-response.v1"
            || matchingLegacyApproval
        return {
            recognized: recognized,
            requestId: recognized ? candidateRequestId : "",
            pendingDisplayName: recognized ? pendingDisplayName : ""
        }
    }

    function rebindWorkspaceEntryIdentity(credentials) {
        var approval = root.workspaceCredentialApprovalContext(credentials)
        workspaceEntryDialog.bindCredentialIdentity(
            String(approval.pendingDisplayName || "").trim(),
            String(approval.requestId || "").trim(),
            approval.recognized === true)
    }

    function loadWorkspaceCredentialText(text) {
        var normalized = String(text || "").trim()
        if (normalized.length === 0) {
            return false
        }
        workspaceEntryDialog.credentialsText = normalized
        root.rebindWorkspaceEntryIdentity(normalized)
        var endpoint = root.credentialPeerEndpoint(normalized)
        if (endpoint.length > 0
                && workspaceEntryDialog.peerEndpointText.trim().length === 0) {
            workspaceEntryDialog.peerEndpointText = endpoint
        }
        return true
    }

    function loadWorkspaceCredentialUrl(fileUrl) {
        var text = chaftController.readCredentialFile(root.localPathFromUrl(fileUrl))
        return text.length > 0 && root.loadWorkspaceCredentialText(text)
    }

    function submitWorkspaceCreate() {
        if (!root.runtimeAccessReady) {
            return false
        }
        if (root.composerContextBusy) {
            workspaceEntryDialog.createOperationError =
                "Finish the current message operation before changing workspaces."
            return false
        }
        if (workspaceEntryDialog.createOperationPending) {
            return false
        }
        if (root.localDeviceDisplayName().trim().length === 0
                && workspaceEntryDialog.displayNameText.trim().length === 0) {
            return false
        }
        var displayNameError = chaftController.deviceDisplayNameValidationError(
            workspaceEntryDialog.displayNameText)
        if (displayNameError.length > 0) {
            workspaceEntryDialog.createOperationError = displayNameError
            return false
        }
        var avatarError = chaftController.avatarIdValidationError(
            workspaceEntryDialog.avatarIdText)
        if (avatarError.length > 0) {
            workspaceEntryDialog.createOperationError = avatarError
            return false
        }
        workspaceEntryDialog.createOperationError = ""
        if (chaftController.createWorkspace(
                    workspaceEntryDialog.createNameText,
                    workspaceEntryDialog.createChannelText,
                    workspaceEntryDialog.createAccessPolicyText)) {
            root.pendingWorkspaceCreateDisplayName =
                workspaceEntryDialog.displayNameText.trim()
            root.pendingWorkspaceCreateAvatarId =
                workspaceEntryDialog.avatarIdText.trim()
            workspaceEntryDialog.createOperationPending = true
            return true
        }
        workspaceEntryDialog.createOperationError = String(
            chaftController.syncStatus || "Could not create the workspace.")
        return false
    }

    function handleWorkspaceCreateFinished(workspaceId, success, selected,
                                           message) {
        if (!workspaceEntryDialog.createOperationPending) {
            return
        }
        workspaceEntryDialog.createOperationPending = false
        var createdWorkspaceId = String(workspaceId || "").trim()
        if (!success || createdWorkspaceId.length === 0) {
            root.pendingWorkspaceCreateDisplayName = ""
            root.pendingWorkspaceCreateAvatarId = ""
            workspaceEntryDialog.createOperationError = String(
                message || "Could not create the workspace.")
            return
        }

        var displayName = String(
            root.pendingWorkspaceCreateDisplayName || "").trim()
        var avatarId = String(
            root.pendingWorkspaceCreateAvatarId || "").trim()
        root.pendingWorkspaceCreateDisplayName = ""
        root.pendingWorkspaceCreateAvatarId = ""
        if (displayName.length > 0) {
            root.clearPendingEntryDisplayName()
            root.pendingEntryDisplayName = displayName
            root.pendingEntryAvatarId = avatarId
            root.pendingEntryDisplayNameWorkspaceId = createdWorkspaceId
            root.persistPendingEntryDisplayNameState("profile_pending")
        }
        root.setKeyKitReminder(createdWorkspaceId, true)
        root.pendingPostCreateExport = true
        root.pendingPostCreateWorkspaceId = createdWorkspaceId
        workspaceEntryDialog.createOperationError = ""
        workspaceEntryDialog.close()
        Qt.callLater(function() {
            if (chaftController.keyTransferFromJoinResponseInbox
                    && (root.keyTransferIsInvitePackage()
                        || root.keyTransferIsInviteResponse())) {
                root.openReceivedApprovalInvite(false)
            }
        })

        if (selected === true) {
            root.applyPendingEntryDisplayName()
            Qt.callLater(function() {
                root.openPostCreateCheckpointIfReady()
            })
            return
        }
        toastHost.show(
            "success",
            "Workspace created. Open it from the workspace switcher to finish setup.",
            "",
            "",
            6500)
    }

    function keyKitReminderPending(workspaceId) {
        var normalizedWorkspaceId = String(workspaceId || "").trim()
        var reminders = chaftController.keyKitReminders || ({})
        return normalizedWorkspaceId.length > 0
            && reminders[normalizedWorkspaceId] === true
    }

    function setKeyKitReminder(workspaceId, pending) {
        var normalizedWorkspaceId = String(workspaceId || "").trim()
        if (normalizedWorkspaceId.length === 0) {
            return false
        }
        var existing = chaftController.keyKitReminders || ({})
        var reminders = ({})
        for (var key in existing) {
            if (Object.prototype.hasOwnProperty.call(existing, key)
                    && existing[key] === true) {
                reminders[key] = true
            }
        }
        if (pending) {
            reminders[normalizedWorkspaceId] = true
        } else {
            delete reminders[normalizedWorkspaceId]
        }
        return chaftController.storeKeyKitReminders(reminders)
    }

    function openPostCreateCheckpointIfReady() {
        var workspaceId = root.currentWorkspaceId()
        var immediateReminder = root.pendingPostCreateExport
            && workspaceId.length > 0
            && workspaceId === root.pendingPostCreateWorkspaceId
        if ((!immediateReminder
                && !root.keyKitReminderPending(workspaceId))
                || !root.runtimeWorkReady
                || workspaceId.length === 0
                || root.workspaceOperationInFlight
                || root.pendingEntryDisplayNameUpdateInFlight
                || postCreateExportDialog.visible) {
            return false
        }
        if (immediateReminder) {
            root.pendingPostCreateExport = false
            root.pendingPostCreateWorkspaceId = ""
        }
        postCreateExportDialog.workspaceId = workspaceId
        postCreateExportDialog.openedForNewWorkspace = true
        postCreateExportDialog.keyKitBackedUp = false
        postCreateExportDialog.explicitlyDeferred = false
        postCreateExportDialog.recoverySetupOpen = true
        postCreateExportDialog.open()
        return true
    }

    function submitWorkspaceJoin() {
        var credentials = workspaceEntryDialog.credentialsText.trim()
        var parsedCredential = root.parsedCredentialObject(credentials)
        var recoveryBundle = root.credentialRecoveryBundleObject(
            parsedCredential)
        var isRecoveryRestore = recoveryBundle !== null
        if (!root.runtimeAccessReady) {
            workspaceEntryDialog.showCredentialImportFailure(
                isRecoveryRestore ? "recovery" : "access",
                "Workspace access is not ready yet.")
            return false
        }
        if (root.composerContextBusy) {
            workspaceEntryDialog.showCredentialImportFailure(
                isRecoveryRestore ? "recovery" : "access",
                "Finish the current message operation before changing workspaces.")
            return false
        }
        if (credentials.length === 0) {
            return false
        }
        // Correctness must not depend on the zero-delay paste helper having
        // fired before the user presses Join.
        root.rebindWorkspaceEntryIdentity(credentials)
        if (workspaceEntryDialog.joinIdentityVisible
                && workspaceEntryDialog.displayNameText.trim().length === 0) {
            return false
        }
        workspaceEntryDialog.clearCredentialImportFailure()
        if (!root.credentialCanSubmit(
                    credentials,
                    workspaceEntryDialog.restoreMode,
                    workspaceEntryDialog.recoveryPassphraseText)) {
            return false
        }
        var packagePeerEndpoint = root.credentialPeerEndpoint(credentials)
        var peerEndpoint = workspaceEntryDialog.peerEndpointText.trim()
        if (peerEndpoint.length === 0 && packagePeerEndpoint.length > 0) {
            peerEndpoint = packagePeerEndpoint
            workspaceEntryDialog.peerEndpointText = peerEndpoint
        }
        // Whitespace-only input is blank, but leading and trailing whitespace
        // in a real passphrase is significant and must reach crypto unchanged.
        var passphrase = workspaceEntryDialog.recoveryPassphraseText
        var credentialJson = root.credentialJsonForImport(credentials, passphrase)
        var credentialKind = parsedCredential === null
            ? ""
            : String(parsedCredential.kind || "")
        var approvalContext = root.workspaceCredentialApprovalContext(credentials)
        var responseRequestId = approvalContext.recognized === true
            ? String(approvalContext.requestId || "").trim()
            : ""
        var pendingRequest = approvalContext.recognized === true
            ? root.pendingAccessRequestRowByRequestId(responseRequestId)
            : ({})
        // A decryption key kit does not carry device identity. Keep the
        // current local identity and do not overwrite its profile with a name
        // copied from whichever workspace was selected before the import.
        var entryDisplayName = isRecoveryRestore
            ? ""
            : workspaceEntryDialog.displayNameText.trim()
        var entryAvatarId = isRecoveryRestore
            ? ""
            : workspaceEntryDialog.avatarIdText.trim()
        var requestedDisplayName = String(
            (pendingRequest && pendingRequest.displayName) || "").trim()
        var requestedAvatarId = String(
            (pendingRequest && pendingRequest.avatarId) || "").trim()
        var requestedDisplayNameError = requestedDisplayName.length > 0
            ? chaftController.deviceDisplayNameValidationError(
                requestedDisplayName)
            : ""
        if (requestedDisplayName.length > 0
                && requestedDisplayNameError.length === 0) {
            entryDisplayName = requestedDisplayName
        }
        if (AvatarCatalog.isValid(requestedAvatarId)) {
            entryAvatarId = requestedAvatarId
        }
        if (!isRecoveryRestore && entryDisplayName.length > 0) {
            var entryDisplayNameError =
                chaftController.deviceDisplayNameValidationError(entryDisplayName)
            if (entryDisplayNameError.length > 0) {
                if (requestedDisplayNameError.length > 0) {
                    workspaceEntryDialog.receivedApprovalDisplayNamePreserved = false
                }
                workspaceEntryDialog.displayNameEditing = true
                return false
            }
            var entryAvatarError =
                chaftController.avatarIdValidationError(entryAvatarId)
            if (entryAvatarError.length > 0) {
                workspaceEntryDialog.displayNameEditing = true
                return false
            }
        }
        var accepted = credentialKind === "chaft.workspace-invite-response.v1"
            ? chaftController.importWorkspaceInviteResponse(JSON.stringify(parsedCredential))
            : (isRecoveryRestore
                ? chaftController.importRecoveryBundle(credentialJson, passphrase)
                : chaftController.importWorkspaceKey(credentialJson))
        if (accepted) {
            if (entryDisplayName.length > 0) {
                root.clearPendingEntryDisplayName()
                root.pendingEntryDisplayName = entryDisplayName
                root.pendingEntryAvatarId = entryAvatarId
                root.pendingEntryDisplayNameWorkspaceId =
                    root.credentialWorkspaceId(credentials)
                root.pendingEntryDisplayNameRequestId = responseRequestId
            } else {
                root.clearPendingEntryDisplayName()
            }
            root.pendingWorkspaceImportActive = true
            root.pendingWorkspaceImportWorkspaceId = root.credentialWorkspaceId(credentials)
            root.pendingWorkspaceImportRequestId = responseRequestId
            root.pendingWorkspaceImportPeerEndpoint = peerEndpoint
            root.pendingWorkspaceImportSource = isRecoveryRestore ? "recovery" : "access"
            var stagedInboxArtifact = String(
                chaftController.keyTransferJson || "").trim()
            root.pendingWorkspaceImportInboxArtifact =
                !isRecoveryRestore
                    && chaftController.keyTransferFromJoinResponseInbox
                    && stagedInboxArtifact === credentials
                ? credentials
                : ""
            workspaceEntryDialog.credentialImportPending = true
        } else {
            workspaceEntryDialog.showCredentialImportFailure(
                isRecoveryRestore ? "recovery" : "access",
                chaftController.syncStatus)
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

    function composerContextChangeBlocked() {
        if (root.composerOperationPending) {
            toastHost.show(
                "info",
                "Wait for the current message operation to finish before "
                    + "leaving this conversation.",
                "",
                "",
                4000)
            return true
        }
        if (root.editingMessageId.length > 0) {
            toastHost.show(
                "info",
                "Save or cancel the message edit before leaving this conversation.",
                "",
                "",
                4000)
            return true
        }
        return false
    }

    function lockWorkspace() {
        if (root.composerContextChangeBlocked()) {
            return false
        }
        chaftController.clearRuntimeUnlock()
        return true
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
        if (root.selectedChannelId !== normalizedChannelId
                && root.composerContextChangeBlocked()) {
            return false
        }
        root.saveCurrentDraft()
        root.mainDestination = "conversation"
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
        if (normalizedWorkspaceId
                !== String(chaftController.selectedWorkspaceId || "")
                && root.composerContextChangeBlocked()) {
            return false
        }
        root.saveCurrentDraft()
        root.mainDestination = "conversation"
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
        root.mainDestination = "conversation"
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
            return root.openTimelineMessageById(
                root.runtimeSearchTimeline[0].messageId)
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
        if (!root.conversationDestination) {
            return root.closeMainDestination(true)
        }
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
        if (!root.runtimeWorkReady || channelId.length === 0) {
            return
        }
        if (!root.active
                || !root.conversationDestination
                || root.normalizedSearchQuery.length > 0
                || !timelineView.visible
                || !root.selectedChannelTimelineReady
                || root.timelineChannelId !== channelId
                || timelineView.pendingInitialScroll
                || timelineView.pendingUnreadScroll
                || timelineView.preservingPrepend
                || !timelineView.followLatest
                || !timelineView.isNearLatest()) {
            return
        }
        if (root.workspaceOperationInFlight
                || root.pendingEntryDisplayNameUpdateInFlight) {
            markReadDebounce.restart()
            return
        }
        chaftController.markChannelRead(channelId)
    }

    function beginEditMessage(messageId, body) {
        var normalizedMessageId = String(messageId || "")
        if (normalizedMessageId.length === 0) {
            return false
        }
        if (root.composerOperationPending) {
            toastHost.show(
                "info",
                "Wait for the current message operation to finish.",
                "",
                "",
                4000)
            return false
        }
        if (root.editingMessageId.length > 0
                && root.editingMessageId !== normalizedMessageId) {
            toastHost.show(
                "info",
                "Save or cancel the current edit before editing another message.",
                "",
                "",
                4000)
            return false
        }
        root.saveCurrentDraft()
        root.cancelReplyMessage()
        root.editingMessageId = normalizedMessageId
        composer.setDraft(String(body || ""))
        return true
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
        if (root.composerOperationPending) {
            toastHost.show(
                "info",
                "Wait for the current message operation to finish.",
                "",
                "",
                4000)
            return false
        }
        if (root.editingMessageId.length > 0) {
            toastHost.show(
                "info",
                "Save or cancel the current edit before replying.",
                "",
                "",
                4000)
            return false
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

    function keyTransferObject() {
        return root.parsedCredentialObject(chaftController.keyTransferJson)
    }

    function keyTransferIsInvitePackage() {
        var parsed = root.keyTransferObject()
        return parsed !== null
            && (String(parsed.kind || "") === "chaft.workspace-invite.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite.v2")
    }

    function keyTransferIsInviteResponse() {
        var parsed = root.keyTransferObject()
        return parsed !== null
            && String(parsed.kind || "") === "chaft.workspace-invite-response.v1"
    }

    function keyTransferIsJoinRequest() {
        var parsed = root.keyTransferObject()
        return parsed !== null
            && (String(parsed.kind || "") === "chaft.workspace-join-request.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite-claim.v1")
            && parsed.deviceId !== undefined
    }

    function keyTransferIsWorkspaceCard() {
        var parsed = root.keyTransferObject()
        return parsed !== null
            && String(parsed.kind || "") === "chaft.workspace-card.v1"
            && parsed.workspaceId !== undefined
    }

    function workspaceCardObjectFromCredentials(credentials) {
        var parsed = root.parsedCredentialObject(credentials)
        return parsed !== null
            && String(parsed.kind || "") === "chaft.workspace-card.v1"
            ? parsed
            : null
    }

    function workspaceCardLabel(card) {
        var name = String((card && card.workspaceName) || "").trim()
        if (name.length > 0) {
            return name
        }
        return root.shortAccessIdentifier(card ? card.workspaceId : "")
    }

    function keyTransferIsRecoveryBundle() {
        var parsed = root.keyTransferObject()
        return parsed !== null
            && parsed.schemaVersion !== undefined
            && parsed.workspaceId !== undefined
            && parsed.exporterDeviceId !== undefined
            && parsed.kdf !== undefined
            && parsed.sealedPayload !== undefined
    }

    function keyTransferIsAccessFile() {
        var parsed = root.keyTransferObject()
        return root.credentialWorkspaceKeyObject(parsed) !== null
            || (parsed !== null
                && parsed.channelId !== undefined
                && (parsed.aes256GcmSivKey !== undefined
                    || parsed.aes_256_gcm_siv_key !== undefined))
    }

    function keyTransferLabel() {
        if (root.keyTransferIsInvitePackage()) {
            return "invite"
        }
        if (root.keyTransferIsInviteResponse()) {
            return "secure access"
        }
        if (root.keyTransferIsJoinRequest()) {
            var request = root.keyTransferObject()
            return String((request && request.kind) || "")
                === "chaft.workspace-invite-claim.v1"
                ? "join request"
                : "access request"
        }
        if (root.keyTransferIsWorkspaceCard()) {
            return "request card"
        }
        if (root.keyTransferIsRecoveryBundle()) {
            return "decryption key kit"
        }
        if (root.keyTransferIsAccessFile()) {
            return "access file"
        }
        return "support detail"
    }

    function inviteArtifactObject(invite) {
        return ({
            kind: "chaft.invite-file.v1",
            schemaVersion: 1,
            workspaceId: String(invite.workspaceId || ""),
            workspaceName: String(invite.workspaceName || ""),
            inviteId: String(invite.inviteId || ""),
            inviterDisplayName: String(invite.inviterDisplayName || ""),
            inviterDeviceId: String(invite.inviterDeviceId || ""),
            inviteeDisplayName: String(invite.inviteeDisplayName || ""),
            inviteeDeviceId: String(invite.inviteeDeviceId || ""),
            role: String(invite.role || "member"),
            peerEndpoint: String(invite.peerEndpoint || ""),
            createdAt: String(invite.createdAt || ""),
            expiresAt: String(invite.expiresAt || ""),
            approvalPolicy: String(invite.approvalPolicy || "preapproved"),
            syncExpectation: root.inviteSyncExpectation(invite, ""),
            invite: invite
        })
    }

    function joinRequestArtifactObject(request) {
        return ({
            kind: "chaft.join-request-file.v1",
            schemaVersion: 1,
            requestId: String(request.requestId || ""),
            workspaceId: String(request.workspaceId || ""),
            workspaceName: String(request.workspaceName || ""),
            displayName: String(request.displayName || ""),
            deviceId: String(request.deviceId || ""),
            note: String(request.note || ""),
            deliveryDisplayName: String(request.deliveryDisplayName || ""),
            deliveryDeviceId: String(request.deliveryDeviceId || ""),
            deliveryPeerEndpoint: String(request.deliveryPeerEndpoint || ""),
            sourceType: String(request.sourceType || ""),
            sourceInviteId: String(request.sourceInviteId || ""),
            sourceDisplayName: String(request.sourceDisplayName || ""),
            sourceApprovalPolicy: String(request.sourceApprovalPolicy || ""),
            createdAt: String(request.createdAt || ""),
            request: request
        })
    }

    function workspaceCardArtifactObject(card) {
        return ({
            kind: "chaft.workspace-card-file.v1",
            schemaVersion: 1,
            workspaceId: String(card.workspaceId || ""),
            workspaceName: String(card.workspaceName || ""),
            accessPolicy: root.normalizedWorkspaceAccessPolicy(card.accessPolicy),
            peerEndpoint: String(card.peerEndpoint || ""),
            adminDisplayName: String(card.adminDisplayName || ""),
            adminDeviceId: String(card.adminDeviceId || ""),
            createdAt: String(card.createdAt || ""),
            card: card
        })
    }

    function keyTransferArtifactObject() {
        return root.credentialArtifactForFileText(chaftController.keyTransferJson)
    }

    function credentialArtifactForFileText(text) {
        var parsed = root.parsedJsonObject(String(text || "").trim())
        if (parsed === null) {
            return null
        }
        var kind = String(parsed.kind || "")
        if (kind === "chaft.invite-file.v1"
                || kind === "chaft.join-request-file.v1"
                || kind === "chaft.workspace-card-file.v1") {
            return parsed
        }
        if (kind === "chaft.workspace-invite.v1"
                || kind === "chaft.workspace-invite.v2") {
            return root.inviteArtifactObject(parsed)
        }
        if (kind === "chaft.workspace-join-request.v1"
                || kind === "chaft.workspace-invite-claim.v1") {
            return root.joinRequestArtifactObject(parsed)
        }
        if (kind === "chaft.workspace-card.v1") {
            return root.workspaceCardArtifactObject(parsed)
        }
        return parsed
    }

    function artifactLink(prefix, artifact) {
        if (artifact === null) {
            return ""
        }
        return prefix + "?payload=" + encodeURIComponent(JSON.stringify(artifact))
    }

    function keyTransferCopyText() {
        var parsed = root.keyTransferObject()
        if (parsed === null) {
            return chaftController.keyTransferJson
        }
        if (String(parsed.kind || "") === "chaft.workspace-invite.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite.v2") {
            return root.artifactLink("chaft-invite:", root.inviteArtifactObject(parsed))
        }
        if (String(parsed.kind || "") === "chaft.workspace-join-request.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite-claim.v1") {
            return root.artifactLink("chaft-request:", root.joinRequestArtifactObject(parsed))
        }
        if (String(parsed.kind || "") === "chaft.workspace-card.v1") {
            return root.artifactLink("chaft-workspace:", root.workspaceCardArtifactObject(parsed))
        }
        return chaftController.keyTransferJson
    }

    function keyTransferCopyLabel() {
        if (root.keyTransferIsInvitePackage()) {
            return "invite link"
        }
        if (root.keyTransferIsJoinRequest()) {
            return root.keyTransferLabel()
        }
        if (root.keyTransferIsWorkspaceCard()) {
            return "request link"
        }
        return root.keyTransferLabel()
    }

    function keyTransferFileText() {
        var artifact = root.keyTransferArtifactObject()
        return artifact === null ? chaftController.keyTransferJson : JSON.stringify(artifact, null, 2)
    }

    function keyTransferFileExtension(label) {
        var normalized = String(label || root.keyTransferLabel()).toLowerCase()
        if (normalized.indexOf("invite claim") >= 0
                || normalized.indexOf("join request") >= 0
                || normalized.indexOf("access request") >= 0) {
            return ".chaftrequest"
        }
        if (normalized.indexOf("invite") >= 0) {
            return ".chaftinvite"
        }
        if (normalized.indexOf("workspace card") >= 0
                || normalized.indexOf("request card") >= 0
                || normalized.indexOf("request link") >= 0) {
            return ".chaftworkspace"
        }
        if (normalized.indexOf("recovery") >= 0
                || normalized.indexOf("key kit") >= 0) {
            return ".chaftrecovery"
        }
        if (normalized.indexOf("access file") >= 0
                || normalized.indexOf("workspace access") >= 0
                || normalized.indexOf("room access") >= 0
                || normalized.indexOf("secure access") >= 0) {
            return ".chaftaccess"
        }
        return ".json"
    }

    function keyTransferNameFilters(label) {
        var normalized = String(label || root.keyTransferLabel()).toLowerCase()
        var extension = root.keyTransferFileExtension(label)
        var olderSupportFilter = "Older support files (*.json)"
        if (extension === ".chaftinvite") {
            return [ "Chaft invites (*.chaftinvite)", olderSupportFilter, "All files (*)" ]
        }
        if (extension === ".chaftrequest") {
            if (normalized.indexOf("join request") >= 0) {
                return [ "Chaft join requests (*.chaftrequest)", olderSupportFilter, "All files (*)" ]
            }
            return normalized.indexOf("invite claim") >= 0
                ? [ "Chaft join requests (*.chaftrequest)", olderSupportFilter, "All files (*)" ]
                : [ "Chaft access requests (*.chaftrequest)", olderSupportFilter, "All files (*)" ]
        }
        if (extension === ".chaftworkspace") {
            return [ "Chaft request cards (*.chaftworkspace)", olderSupportFilter, "All files (*)" ]
        }
        if (extension === ".chaftrecovery") {
            return [ "Chaft decryption key kits (*.chaftrecovery)", olderSupportFilter, "All files (*)" ]
        }
        if (extension === ".chaftaccess") {
            return [ "Chaft access files (*.chaftaccess)", olderSupportFilter, "All files (*)" ]
        }
        return [ olderSupportFilter, "All files (*)" ]
    }

    function workspaceCredentialNameFilters(restoreMode) {
        var allChaft = "All Chaft credentials (*.chaftinvite *.chaftrequest *.chaftworkspace *.chaftrecovery *.chaftaccess)"
        var olderSupportFilter = "Older support files (*.json)"
        if (restoreMode) {
            return [
                "Chaft decryption key kits (*.chaftrecovery)",
                allChaft,
                olderSupportFilter,
                "All files (*)"
            ]
        }
        return [
            allChaft,
            "Chaft invites (*.chaftinvite)",
            "Chaft request cards (*.chaftworkspace)",
            "Chaft access requests (*.chaftrequest)",
            "Chaft decryption key kits (*.chaftrecovery)",
            "Chaft access files (*.chaftaccess)",
            olderSupportFilter,
            "All files (*)"
        ]
    }

    function fileNameSegment(value, fallback, maxLength) {
        var source = String(value || "").trim()
        var limit = Math.max(8, maxLength || 48)
        var segment = ""
        var lastSeparator = false
        for (var i = 0; i < source.length && segment.length < limit; i += 1) {
            var ch = source.charAt(i)
            var code = source.charCodeAt(i)
            var alnum = (code >= 48 && code <= 57)
                || (code >= 65 && code <= 90)
                || (code >= 97 && code <= 122)
            var allowedPunctuation = ch === "." || ch === "-" || ch === "_"
            if (alnum || allowedPunctuation) {
                segment += ch
                lastSeparator = false
            } else if (!lastSeparator && segment.length > 0) {
                segment += " "
                lastSeparator = true
            }
        }
        while (segment.length > 0) {
            var tail = segment.charAt(segment.length - 1)
            if (tail !== " " && tail !== "." && tail !== "-" && tail !== "_") {
                break
            }
            segment = segment.slice(0, -1)
        }
        while (segment.length > 0) {
            var head = segment.charAt(0)
            if (head !== " " && head !== "." && head !== "-" && head !== "_") {
                break
            }
            segment = segment.slice(1)
        }
        if (segment.length > 0) {
            return segment
        }
        if (fallback === "") {
            return ""
        }
        return String(fallback || "Workspace")
    }

    function artifactSuggestedFileName(artifact, label) {
        var value = artifact || ({})
        var normalized = String(label || root.keyTransferLabel()).toLowerCase()
        var extension = root.keyTransferFileExtension(normalized)
        var workspace = root.fileNameSegment(
            value.workspaceName || value.workspaceId,
            "Workspace",
            48)
        var person = root.fileNameSegment(
            value.inviteeDisplayName || value.displayName
                || value.deliveryDisplayName || value.inviteeDeviceId
                || value.deviceId,
            "",
            40)
        var date = root.fileNameSegment(String(value.createdAt || "").slice(0, 10), "", 16)
        var parts = [ "Chaft", workspace ]
        if (extension === ".chaftinvite") {
            parts.push("Invite")
            if (person.length > 0) {
                parts.push(person)
            }
        } else if (extension === ".chaftrequest") {
            parts.push("Access Request")
            if (person.length > 0) {
                parts.push(person)
            }
        } else if (extension === ".chaftworkspace") {
            parts.push("Request Card")
        } else if (extension === ".chaftrecovery") {
            parts.push("Decryption Key Kit")
        } else if (extension === ".chaftaccess") {
            parts.push("Access File")
        } else {
            parts.push("Credentials")
        }
        if (date.length > 0) {
            parts.push(date)
        }
        return parts.join(" - ") + extension
    }

    function keyTransferSuggestedFileName(label) {
        return root.artifactSuggestedFileName(
            root.keyTransferArtifactObject() || ({}),
            label)
    }

    function fileUrlFromLocalPath(path) {
        var normalized = String(path || "").replace(/\\/g, "/")
        if (normalized.length === 0) {
            return ""
        }
        if (Qt.platform.os === "windows") {
            return "file:///" + encodeURI(normalized)
        }
        if (normalized.charAt(0) !== "/") {
            return encodeURI(normalized)
        }
        return "file://" + encodeURI(normalized)
    }

    function keyTransferSuggestedFileUrl(label) {
        return root.suggestedFileUrl(root.keyTransferSuggestedFileName(label))
    }

    function credentialSuggestedFileUrl(text, label) {
        var artifact = root.credentialArtifactForFileText(text) || ({})
        return root.suggestedFileUrl(root.artifactSuggestedFileName(artifact, label))
    }

    function suggestedFileUrl(fileName) {
        var folder = StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        if (String(folder || "").length === 0) {
            folder = StandardPaths.writableLocation(StandardPaths.HomeLocation)
        }
        if (String(folder || "").length === 0) {
            return fileName
        }
        return root.fileUrlFromLocalPath(String(folder).replace(/\/$/, "") + "/" + fileName)
    }

    function joinRequestSourceLabel(request) {
        var sourceType = String((request && request.sourceType) || "").trim()
        var sourceName = String((request && request.sourceDisplayName) || "").trim()
        if (sourceType === "approval_invite") {
            return sourceName.length > 0
                ? "Approval invite from " + sourceName
                : "Approval invite"
        }
        if (sourceType === "workspace_card") {
            return sourceName.length > 0
                ? "Request link from " + sourceName
                : "Request link"
        }
        if (sourceType === "invite_claim") {
            return sourceName.length > 0
                ? "Secure invite from " + sourceName
                : "Secure invite"
        }
        return ""
    }

    function copyMap(source) {
        var row = {}
        var value = source || {}
        for (var key in value) {
            if (Object.prototype.hasOwnProperty.call(value, key)) {
                row[key] = value[key]
            }
        }
        return row
    }

    function pendingAccessRequestRows(requests) {
        var rows = []
        var source = requests || {}
        for (var key in source) {
            if (Object.prototype.hasOwnProperty.call(source, key)) {
                var row = root.copyMap(source[key])
                var persistedStatus = String(
                    row.status || "").trim()
                if (persistedStatus === "profile_pending"
                        || persistedStatus === "profile_written") {
                    continue
                }
                row.key = key
                row.workspaceLabel = String(row.workspaceName || "").trim().length > 0
                    ? String(row.workspaceName || "").trim()
                    : (String(row.workspaceId || "").trim().length > 0
                        ? root.shortAccessIdentifier(row.workspaceId)
                        : "Workspace admin")
                row.displayLabel = String(row.displayName || "").trim().length > 0
                    ? String(row.displayName || "").trim()
                    : root.localDeviceDisplayName()
                row.deliveryLabel = String(row.deliveryDisplayName || "").trim().length > 0
                    ? String(row.deliveryDisplayName || "").trim()
                    : (String(row.deliveryDeviceId || "").trim().length > 0
                        ? "the workspace admin"
                        : "an owner or admin")
                row.deliveryHasAddress = String(row.deliveryPeerEndpoint || "").trim().length > 0
                row.sourceLabel = root.joinRequestSourceLabel(row)
                row.status = String(row.status || "ready_to_send").trim()
                var secureClaim = String(row.sourceType || "").trim()
                    === "invite_claim"
                var badgeLabels = {
                    approved: "Approved",
                    closed: "Closed",
                    copied: "Copied",
                    declined: "Declined",
                    file_ready: "File ready",
                    ready_to_send: "Ready",
                    send_failed: "Not sent",
                    sending: "Sending",
                    sent: "Waiting",
                    sent_unpersisted: "Storage",
                    unverified_response: "Review"
                }
                var titleLabels = {
                    approved: "Approval received",
                    closed: "Request closed",
                    copied: "Request link copied",
                    declined: "Request declined",
                    file_ready: "Request file ready",
                    ready_to_send: "Request ready",
                    send_failed: "Request not sent",
                    sending: "Sending request",
                    sent: "Waiting for approval",
                    sent_unpersisted: "Sent; status not saved",
                    unverified_response: "Unverified response"
                }
                if (secureClaim) {
                    badgeLabels.approved = "Ready"
                    titleLabels = {
                        approved: "Encrypted access received",
                        closed: "Join request closed",
                        copied: "Join request copied",
                        declined: "Join request declined",
                        file_ready: "Join request ready",
                        ready_to_send: "Join request ready",
                        send_failed: "Join request not sent",
                        sending: "Sending join request",
                        sent: "Waiting for access",
                        sent_unpersisted: "Sent; status not saved",
                        unverified_response: "Unverified response"
                    }
                }
                if (row.key === root.pendingAccessRequestSendingKey) {
                    row.status = "sending"
                } else if (row.key === root.pendingAccessRequestSentUnpersistedKey
                           && (row.status === "ready_to_send"
                               || row.status === "sending")) {
                    row.status = "sent_unpersisted"
                } else if (row.status === "sending") {
                    // The app restarted before the in-flight send resolved.
                    // Keep the persisted artifact available for a safe retry.
                    row.status = "ready_to_send"
                } else if (!Object.prototype.hasOwnProperty.call(
                               badgeLabels, row.status)) {
                    row.status = "ready_to_send"
                }
                row.isTerminalStatus = row.status === "approved"
                    || row.status === "declined"
                    || row.status === "closed"
                row.statusBadgeLabel = badgeLabels[row.status]
                row.statusTitle = titleLabels[row.status]
                row.canSendDirect = row.deliveryHasAddress
                    && root.runtimeAccessReady
                    && !row.isTerminalStatus
                    && row.status !== "sent_unpersisted"
                    && String(row.artifact || "").trim().length > 0
                row.canShareRequest = !row.isTerminalStatus
                    && String(row.artifact || "").trim().length > 0
                row.canOpenInvite = row.status !== "declined"
                    && row.status !== "closed"
                    && row.status !== "unverified_response"
                row.canCheckResponse = row.deliveryHasAddress
                    && root.runtimeAccessReady
                    && !row.isTerminalStatus
                    && String(row.workspaceId || "").trim().length > 0
                    && row.status !== "ready_to_send"
                    && row.status !== "sending"
                row.statusMessage = root.pendingAccessRequestStatusMessage(row)
                row.receiptLabel = root.pendingAccessRequestReceiptLabel(row)
                rows.push(row)
            }
        }
        rows.sort(function(a, b) {
            return String(b.createdAt || "").localeCompare(String(a.createdAt || ""))
        })
        return rows
    }

    function pendingAccessRequestRowByKey(key) {
        var normalizedKey = String(key || "").trim()
        if (normalizedKey.length === 0) {
            return ({})
        }
        var rows = root.pendingAccessRequests || []
        for (var i = 0; i < rows.length; i++) {
            if (String((rows[i] || {}).key || "").trim() === normalizedKey
                    || String((rows[i] || {}).requestId || "").trim()
                        === normalizedKey) {
                return rows[i]
            }
        }
        return ({})
    }

    function pendingAccessRequestRowByRequestId(requestId) {
        var normalizedRequestId = String(requestId || "").trim()
        if (normalizedRequestId.length === 0) {
            return ({})
        }
        var rows = root.pendingAccessRequests || []
        for (var i = 0; i < rows.length; i++) {
            if (String((rows[i] || {}).requestId || "").trim()
                    === normalizedRequestId) {
                return rows[i]
            }
        }
        return ({})
    }

    function recordPendingAccessRequestFromArtifact(status, artifactText,
                                                    selectedAvatarId) {
        var normalizedArtifactText = String(artifactText || "").trim()
        var request = root.parsedCredentialObject(normalizedArtifactText)
        var kind = String((request && request.kind) || "")
        if (kind !== "chaft.workspace-join-request.v1"
                && kind !== "chaft.workspace-invite-claim.v1") {
            return false
        }
        var normalizedStatus = String(status || "ready_to_send").trim()
        if (normalizedStatus !== "sent"
                && normalizedStatus !== "send_failed"
                && normalizedStatus !== "copied"
                && normalizedStatus !== "file_ready"
                && normalizedStatus !== "sending") {
            normalizedStatus = "ready_to_send"
        }
        var workspaceId = String(request.workspaceId || "").trim()
        var requestId = String(request.requestId || "").trim()
        // A workspace can have more than one access handoff in flight. The
        // cryptographic request ID is the correlation key; workspace-keyed
        // entries are read and migrated only for compatibility with older
        // saved state.
        var key = requestId.length > 0 ? requestId : workspaceId
        if (key.length === 0) {
            return false
        }
        if (normalizedArtifactText.length === 0) {
            return false
        }
        var next = root.copyMap(chaftController.pendingJoinRequests || ({}))
        var existingKey = Object.prototype.hasOwnProperty.call(next, key)
            ? key
            : ""
        if (existingKey.length === 0 && requestId.length > 0) {
            for (var candidateKey in next) {
                if (Object.prototype.hasOwnProperty.call(next, candidateKey)
                        && String((next[candidateKey] || {}).requestId || "").trim()
                            === requestId) {
                    existingKey = candidateKey
                    break
                }
            }
        }
        var existing = root.copyMap(
            existingKey.length > 0 ? next[existingKey] : ({}))
        if (existingKey.length > 0 && existingKey !== key) {
            delete next[existingKey]
        }
        var now = (new Date()).toISOString()
        var avatarId = String(selectedAvatarId || "").trim()
        if (!AvatarCatalog.isValid(avatarId)) {
            avatarId = String(existing.avatarId || "").trim()
        }
        if (!AvatarCatalog.isValid(avatarId)
                && workspaceEntryDialog.visible
                && String(request.displayName || "").trim()
                    === workspaceEntryDialog.displayNameText.trim()
                && AvatarCatalog.isValid(
                    workspaceEntryDialog.avatarIdText)) {
            avatarId = workspaceEntryDialog.avatarIdText.trim()
        }
        next[key] = {
            requestId: requestId,
            workspaceId: workspaceId,
            workspaceName: String(request.workspaceName || "").trim(),
            displayName: String(request.displayName || "").trim(),
            avatarId: AvatarCatalog.isValid(avatarId) ? avatarId : "",
            deliveryDisplayName: String(request.deliveryDisplayName || "").trim(),
            deliveryDeviceId: String(request.deliveryDeviceId || "").trim(),
            deliveryPeerEndpoint: String(request.deliveryPeerEndpoint || "").trim(),
            sourceType: String(request.sourceType || "").trim(),
            sourceInviteId: String(request.sourceInviteId || "").trim(),
            sourceDisplayName: String(request.sourceDisplayName || "").trim(),
            sourceApprovalPolicy: String(request.sourceApprovalPolicy || "").trim(),
            status: normalizedStatus,
            createdAt: String(existing.createdAt || now),
            sentAt: normalizedStatus === "sent"
                ? now
                : String(existing.sentAt || ""),
            lastAttemptAt: normalizedStatus === "send_failed"
                    || normalizedStatus === "sending"
                ? now
                : String(existing.lastAttemptAt || ""),
            artifact: normalizedArtifactText
        }
        return chaftController.storePendingJoinRequests(next)
    }

    function recordPendingAccessRequestFromCurrentJoinRequest(status) {
        if (!root.keyTransferIsJoinRequest()) {
            return false
        }
        return root.recordPendingAccessRequestFromArtifact(
            status, root.keyTransferFileText(),
            workspaceEntryDialog.avatarIdText)
    }

    function pendingAccessRequestCopyText(row) {
        var artifactText = String((row && row.artifact) || "").trim()
        var artifact = root.parsedCredentialObject(artifactText)
        if (artifact !== null) {
            return root.artifactLink("chaft-request:", artifact)
        }
        return artifactText
    }

    function copyPendingAccessRequest(row) {
        if ((row && row.isTerminalStatus) || !(row && row.canShareRequest)) {
            return false
        }
        var text = root.pendingAccessRequestCopyText(row)
        if (text.length === 0) {
            return false
        }
        var secureClaim = String((row && row.sourceType) || "").trim()
            === "invite_claim"
        if (!root.copyTextToClipboard(
                text, secureClaim ? "join request" : "access request")) {
            return false
        }
        if (String((row && row.status) || "").trim() !== "sent") {
            root.updatePendingAccessRequestStatus(row, "copied", "")
        }
        return true
    }

    function openSavePendingAccessRequestDialog(row) {
        if ((row && row.isTerminalStatus) || !(row && row.canShareRequest)) {
            return false
        }
        var artifactText = String((row && row.artifact) || "").trim()
        if (artifactText.length === 0) {
            return false
        }
        root.pendingAccessRequestSaveText = artifactText
        root.pendingAccessRequestSaveKey = String(
            (row && (row.key || row.workspaceId || row.requestId)) || "").trim()
        root.pendingAccessRequestSaveLabel = String(
            (row && row.sourceType) || "").trim() === "invite_claim"
            ? "join request"
            : "access request"
        pendingAccessRequestSaveDialog.selectedFile =
            root.credentialSuggestedFileUrl(
                artifactText, root.pendingAccessRequestSaveLabel)
        pendingAccessRequestSaveDialog.open()
        return true
    }

    function pendingAccessRequestPayload(row) {
        var artifactText = String((row && row.artifact) || "").trim()
        if (artifactText.length === 0) {
            return null
        }
        var parsed = root.parsedCredentialObject(artifactText)
        return parsed !== null
            && (String(parsed.kind || "") === "chaft.workspace-join-request.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite-claim.v1")
            && String(parsed.deviceId || "").trim().length > 0
            ? parsed
            : null
    }

    function updatePendingAccessRequestStatus(row, status, error) {
        var key = String((row && row.key) || (row && row.workspaceId) || "").trim()
        if (key.length === 0) {
            return false
        }
        var next = root.copyMap(chaftController.pendingJoinRequests || ({}))
        if (!Object.prototype.hasOwnProperty.call(next, key)) {
            return false
        }
        var updated = root.copyMap(next[key])
        updated.status = String(status || "ready_to_send").trim()
        if (updated.status === "sent") {
            updated.sentAt = (new Date()).toISOString()
            updated.lastAttemptAt = updated.sentAt
        } else if (updated.status === "send_failed") {
            updated.lastAttemptAt = (new Date()).toISOString()
        } else if (updated.status === "ready_to_send"
                   && root.pendingAccessRequestSendingKey === key) {
            updated.lastAttemptAt = (new Date()).toISOString()
        }
        if (String(error || "").trim().length > 0) {
            updated.error = String(error || "").trim()
        } else {
            delete updated.error
        }
        next[key] = updated
        var stored = chaftController.storePendingJoinRequests(next)
        if (stored && key === root.pendingAccessRequestSentUnpersistedKey) {
            root.pendingAccessRequestSentUnpersistedKey = ""
        }
        return stored
    }

    function sendPendingAccessRequest(row) {
        if ((row && row.isTerminalStatus) || !(row && row.canSendDirect)) {
            return false
        }
        var key = String((row && row.key) || (row && row.workspaceId) || "").trim()
        var endpoint = String((row && row.deliveryPeerEndpoint) || "").trim()
        var payload = root.pendingAccessRequestPayload(row)
        if (key.length === 0 || endpoint.length === 0 || payload === null) {
            return false
        }
        if (chaftController.joinRequestSubmitInFlight) {
            return false
        }
        root.pendingAccessRequestSendingKey = key
        if (!root.updatePendingAccessRequestStatus(row, "ready_to_send", "")) {
            root.pendingAccessRequestSendingKey = ""
            toastHost.show(
                "warning",
                "Could not save the access handoff. Free disk space or restore write access, then try again.",
                "",
                "",
                6000)
            return false
        }
        if (!chaftController.submitWorkspaceJoinRequestDirect(
                    endpoint,
                    String(payload.workspaceId || row.workspaceId || "").trim(),
                    JSON.stringify(payload))) {
            root.pendingAccessRequestSendingKey = ""
            root.updatePendingAccessRequestStatus(row, "send_failed",
                                                  chaftController.syncStatus)
            var secureClaim = String((row && row.sourceType) || "").trim()
                === "invite_claim"
            toastHost.show(
                "warning",
                secureClaim
                    ? "Join request not sent. Copy or save it, or try again when "
                        + row.deliveryLabel + " is reachable."
                    : "Request not sent. Copy the request link or try again when "
                        + row.deliveryLabel + " is reachable.",
                "",
                "",
                6000)
            return false
        }
        return true
    }

    function pullPendingAccessRequestResponse(row, userInitiated) {
        if (!(row && row.canCheckResponse)) {
            return false
        }
        if (chaftController.accessEnvelopePullInFlight) {
            return false
        }
        var endpoint = String((row && row.deliveryPeerEndpoint) || "").trim()
        var workspaceId = String((row && row.workspaceId) || "").trim()
        if (endpoint.length === 0 || workspaceId.length === 0) {
            return false
        }
        var requestId = String((row && row.requestId) || "").trim()
        if (String((row && row.status) || "") === "sent_unpersisted") {
            // Retry the durable status write, but do not block a response
            // check: the request artifact and correlation ID were saved before
            // network submission.
            root.updatePendingAccessRequestStatus(row, "sent", "")
        }
        if (requestId.length === 0
                || !chaftController.pullAccessResponsesFromPeer(
                    endpoint, workspaceId, requestId)) {
            return false
        }
        if (userInitiated) {
            toastHost.show(
                "info",
                "Checking for approval from " + String(row.deliveryLabel || "the workspace admin") + ".",
                "",
                "",
                3000)
        }
        return true
    }

    function checkPendingAccessRequestResponse(row) {
        return root.pullPendingAccessRequestResponse(row, true)
    }

    function autoCheckPendingAccessRequestResponse() {
        if (!root.runtimeAccessReady || chaftController.accessEnvelopePullInFlight) {
            return false
        }
        var rows = root.pendingAccessRequests || []
        if (rows.length === 0) {
            root.pendingAccessResponseAutoCheckLastKey = ""
            return false
        }
        var startIndex = 0
        for (var i = 0; i < rows.length; i++) {
            if (String((rows[i] || {}).key || "").trim()
                    === root.pendingAccessResponseAutoCheckLastKey) {
                startIndex = i + 1
                break
            }
        }
        for (var offset = 0; offset < rows.length; offset++) {
            var row = rows[(startIndex + offset) % rows.length] || ({})
            if (!row.canCheckResponse) {
                continue
            }
            root.pendingAccessResponseAutoCheckLastKey =
                String(row.key || row.workspaceId || "").trim()
            return root.pullPendingAccessRequestResponse(row, false)
        }
        return false
    }

    function dismissPendingAccessRequest(row) {
        var key = String((row && row.key) || (row && row.workspaceId) || "").trim()
        if (key.length === 0) {
            return false
        }
        var next = root.copyMap(chaftController.pendingJoinRequests || ({}))
        if (!Object.prototype.hasOwnProperty.call(next, key)) {
            return false
        }
        delete next[key]
        var stored = chaftController.storePendingJoinRequests(next)
        if (stored && key === root.pendingAccessRequestSentUnpersistedKey) {
            root.pendingAccessRequestSentUnpersistedKey = ""
        }
        return stored
    }

    function confirmDismissPendingAccessRequest(row) {
        var key = String((row && row.key) || (row && row.workspaceId) || "").trim()
        if (key.length === 0) {
            return false
        }
        var workspace = String((row && row.workspaceLabel) || "this workspace").trim()
        var secureClaim = String((row && row.sourceType) || "").trim()
            === "invite_claim"
        confirmDialog.ask(
            secureClaim ? "Hide join request" : "Hide access request",
            secureClaim
                ? "Hide the join request reminder for " + workspace
                    + "? Keep the copied request or saved file before hiding. You can still open encrypted access here when it arrives."
                : "Hide the access request reminder for " + workspace
                    + "? Keep the copied link or saved file before hiding. You can still open an invite here after an admin approves.",
            "Hide",
            "dismiss-pending-access-request:" + key,
            false)
        return true
    }

    function clearPendingAccessRequestForRequestId(requestId) {
        var normalizedRequestId = String(requestId || "").trim()
        if (normalizedRequestId.length === 0) {
            return false
        }
        var next = root.copyMap(chaftController.pendingJoinRequests || ({}))
        var removedKeys = []
        for (var key in next) {
            if (Object.prototype.hasOwnProperty.call(next, key)
                    && (String(key).trim() === normalizedRequestId
                        || String((next[key] || {}).requestId || "").trim()
                            === normalizedRequestId)) {
                removedKeys.push(key)
                delete next[key]
            }
        }
        if (removedKeys.length === 0) {
            return false
        }
        var stored = chaftController.storePendingJoinRequests(next)
        if (stored) {
            for (var index = 0; index < removedKeys.length; index++) {
                if (removedKeys[index] === root.pendingAccessRequestSentUnpersistedKey) {
                    root.pendingAccessRequestSentUnpersistedKey = ""
                    break
                }
            }
        }
        return stored
    }

    function clearPendingAccessRequestForWorkspace(workspaceId) {
        var workspaceKey = String(workspaceId || "").trim()
        if (workspaceKey.length === 0) {
            return false
        }
        var next = root.copyMap(chaftController.pendingJoinRequests || ({}))
        var removedKeys = []
        if (Object.prototype.hasOwnProperty.call(next, workspaceKey)) {
            // Legacy state used workspaceId as the storage key. This is an
            // exact compatibility match and is safe to remove.
            removedKeys.push(workspaceKey)
        } else {
            for (var key in next) {
                if (Object.prototype.hasOwnProperty.call(next, key)
                        && String((next[key] || {}).workspaceId || "").trim()
                            === workspaceKey) {
                    removedKeys.push(key)
                }
            }
        }
        // Without a request ID, never guess between concurrent handoffs for
        // the same workspace.
        if (removedKeys.length !== 1) {
            return false
        }
        delete next[removedKeys[0]]
        var stored = chaftController.storePendingJoinRequests(next)
        if (stored) {
            for (var index = 0; index < removedKeys.length; index++) {
                if (removedKeys[index] === root.pendingAccessRequestSentUnpersistedKey) {
                    root.pendingAccessRequestSentUnpersistedKey = ""
                    break
                }
            }
        }
        return stored
    }

    function outputPathWithExtension(path, extension) {
        var value = String(path || "").trim()
        if (value.length === 0) {
            return value
        }
        var slashIndex = Math.max(value.lastIndexOf("/"), value.lastIndexOf("\\"))
        var fileName = value.slice(slashIndex + 1)
        if (fileName.indexOf(".") >= 0) {
            return value
        }
        return value + String(extension || ".json")
    }

    function copyKeyTransferArtifact(label) {
        return root.copyTextToClipboard(
            root.keyTransferCopyText(),
            String(label || root.keyTransferCopyLabel()))
    }

    function nextKeyTransferFileSaveToken() {
        root.keyTransferFileSaveSequence += 1
        return "key-transfer-save-" + String(Date.now()) + "-"
            + String(root.keyTransferFileSaveSequence)
    }

    function openSaveKeyTransferDialog(label, operationToken,
                                       trackPendingAccessRequest) {
        if (saveKeyTransferDialog.visible
                || chaftController.keyTransferJson.length === 0) {
            return false
        }
        var transferText = root.keyTransferFileText()
        if (String(transferText || "").trim().length === 0) {
            return false
        }
        var transfer = root.keyTransferObject()
        saveKeyTransferDialog.transferText = transferText
        saveKeyTransferDialog.transferKind = String(
            (transfer && transfer.kind) || "")
        saveKeyTransferDialog.operationToken = String(
            operationToken || root.nextKeyTransferFileSaveToken())
        saveKeyTransferDialog.trackPendingAccessRequest =
            trackPendingAccessRequest === true
        saveKeyTransferDialog.transferLabel = String(label || root.keyTransferLabel())
        saveKeyTransferDialog.selectedFile = root.credentialSuggestedFileUrl(
            transferText, saveKeyTransferDialog.transferLabel)
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
        return root.copyTextToClipboard(root.inspectorItem.eventId || "", "support ID")
    }

    function copyInspectorMessageId() {
        return root.copyTextToClipboard(root.inspectorItem.messageId || "", "message support ID")
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
        return root.copyTextToClipboard(selector, "file support ID")
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
        if (!root.runtimeWorkReady
                || root.workspaceOperationInFlight
                || root.pendingEntryDisplayNameUpdateInFlight
                || root.pendingJoinPeerEndpointTargetsCurrentWorkspace()
                || !root.autoSyncEnabled) {
            return
        }
        if (chaftController.peerHosting
                && root.hostedRuntimeReconcileDue) {
            if (root.reconcileHostedRuntimeIfReady()) {
                return
            }
            root.hostedRuntimeReconcileDue = false
        }
        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length > 0) {
            if (chaftController.syncWorkspaceIfIdle(endpoint)
                    && chaftController.peerHosting) {
                // Ensure inbound writes are materialized before another
                // network sync can occupy the serialized workspace worker.
                root.hostedRuntimeReconcileDue = true
            }
        }
    }

    function reconcileHostedRuntimeIfReady() {
        if (root.runtimeWorkReady
                && chaftController.peerHosting
                && !root.workspaceOperationInFlight
                && !root.pendingEntryDisplayNameUpdateInFlight) {
            if (chaftController.reconcileRuntimeSnapshotIfIdle()) {
                root.hostedRuntimeReconcileDue = false
                return true
            }
        }
        return false
    }

    function rememberJoinWaitingForPeer(workspaceId, notify, source, privateRoomCount) {
        root.pendingJoinAwaitingWorkspaceId = String(workspaceId || "").trim()
        root.pendingJoinAwaitingSource = String(source || "access").trim()
        root.pendingJoinRecoveryPrivateRoomCount = root.pendingJoinAwaitingSource === "recovery"
            ? Number(privateRoomCount === undefined ? -1 : privateRoomCount)
            : -1
        root.pendingJoinAwaitingReachablePeer = true
        if (notify !== false) {
            var toastMessage = root.pendingJoinAwaitingSource === "recovery"
                ? "Decryption keys imported. " + root.recoveryPrivateRoomRestoreText()
                    + " Membership and missing history may still require an invite or reachable teammate."
                : "Workspace joined. Waiting for a reachable teammate to fetch history."
            toastHost.show(
                "info",
                toastMessage,
                "",
                "",
                5000)
        }
    }

    function clearJoinWaitingForPeer() {
        root.pendingJoinAwaitingReachablePeer = false
        root.pendingJoinAwaitingWorkspaceId = ""
        root.pendingJoinAwaitingSource = ""
        root.pendingJoinRecoveryPrivateRoomCount = -1
    }

    function historySyncStatusSucceeded(status) {
        var normalized = String(status || "").trim().toLowerCase()
        return normalized.indexOf("fetched ") === 0
            || normalized.indexOf("synced ") === 0
    }

    function maybeClearJoinWaitingForPeerFromStatus() {
        var targetWorkspaceId = String(
            root.pendingJoinAwaitingWorkspaceId || "").trim()
        if (root.pendingJoinAwaitingReachablePeer
                && targetWorkspaceId.length > 0
                && root.currentWorkspaceId() === targetWorkspaceId
                && String(root.pendingJoinPeerEndpoint || "").trim().length === 0
                && root.historySyncStatusSucceeded(chaftController.syncStatus)) {
            root.clearJoinWaitingForPeer()
        }
    }

    function firstSyncWaitingTitleText() {
        return root.pendingJoinAwaitingSource === "recovery"
            ? "Keys imported"
            : "History not here yet"
    }

    function recoveryPrivateRoomRestoreText() {
        var count = Number(root.pendingJoinRecoveryPrivateRoomCount)
        if (count > 0) {
            return "Keys imported for " + String(count) + " private "
                + (count === 1 ? "room" : "rooms")
                + "."
        }
        if (count === 0) {
            return "No private-room keys were included in this key kit."
        }
        return "Private rooms may need a reachable teammate."
    }

    function firstSyncWaitingDetailText() {
        if (root.pendingJoinAwaitingSource === "recovery") {
            return root.preferredSyncPeerEndpoint().length > 0
                ? root.recoveryPrivateRoomRestoreText()
                    + " Fetch history from a teammate who has access."
                : root.recoveryPrivateRoomRestoreText()
                    + " Add a teammate address to fetch history."
        }
        return root.preferredSyncPeerEndpoint().length > 0
            ? "Your teammate address is saved. Fetch when they are online."
            : "Add an address from a teammate who has this workspace history."
    }

    function firstSyncWaitingActionLabel() {
        return root.preferredSyncPeerEndpoint().length > 0 ? "Fetch history" : "Add address"
    }

    function firstSyncWaitingHelpNeedText() {
        if (root.preferredSyncPeerEndpoint().length > 0) {
            return "Please keep Chaft open while I fetch history from your saved address."
        }
        return "Please open Chaft, send me your address, and keep Chaft open while I fetch history."
    }

    function firstSyncWaitingHelpCopyText() {
        var lines = [
            "Chaft history help",
            "Workspace: " + String(root.workspaceSnapshot.name || "Workspace").trim(),
            "Status: " + root.firstSyncWaitingTitleText(),
            "What Chaft sees: " + root.firstSyncWaitingDetailText(),
            "What I need: " + root.firstSyncWaitingHelpNeedText(),
            "This device: " + root.supportDeviceCodeLabel(chaftController.deviceId)
        ]
        var endpoint = root.preferredSyncPeerEndpoint()
        if (endpoint.length > 0) {
            lines.push("Saved teammate address: " + endpoint)
        }
        return lines.join("\n")
    }

    function copyFirstSyncWaitingHelpNote() {
        return root.copyTextToClipboard(
            root.firstSyncWaitingHelpCopyText(),
            "history help"
        )
    }

    function confirmHideFirstSyncWaiting() {
        confirmDialog.ask(
            "Hide history reminder",
            "Hide this history reminder? You can still add a teammate address and fetch history later.",
            "Hide",
            "hide-first-sync-waiting",
            false)
        return true
    }

    function focusPeerAddressField() {
        if (channelDetailsPopup.opened) {
            channelDetailsPopup.close()
        }
        root.syncDrawerOpen = true
        Qt.callLater(function() {
            peerEndpointField.forceActiveFocus()
            peerEndpointField.selectAll()
        })
        toastHost.show(
            "info",
            "Ask someone with this history to open Chaft, then paste the address they share.",
            "",
            "",
            4500)
        return true
    }

    function openPeopleAccessForPrivateRoomHelp() {
        if (channelDetailsPopup.opened) {
            channelDetailsPopup.close()
        }
        root.openPeopleAccess(false)
        toastHost.show(
            "info",
            "Message someone with this room's history, or ask an admin to add you again.",
            "",
            "",
            4500)
        return true
    }

    function handleFirstSyncWaitingAction() {
        return root.preferredSyncPeerEndpoint().length > 0
            ? root.pullWorkspaceFromPreferredPeer()
            : root.focusPeerAddressField()
    }

    function pendingJoinPeerEndpointTargetsCurrentWorkspace() {
        var endpoint = String(root.pendingJoinPeerEndpoint || "").trim()
        var targetWorkspaceId = String(
            root.pendingJoinAwaitingWorkspaceId || "").trim()
        return endpoint.length > 0
            && targetWorkspaceId.length > 0
            && root.currentWorkspaceId() === targetWorkspaceId
    }

    function pullPendingJoinPeerIfReady() {
        var endpoint = String(root.pendingJoinPeerEndpoint || "").trim()
        if (!root.pendingJoinPeerEndpointTargetsCurrentWorkspace()
                || !root.runtimeWorkReady
                || root.workspaceOperationInFlight
                || root.pendingEntryDisplayNameUpdateInFlight) {
            return false
        }
        peerEndpointField.text = endpoint
        toastHost.show("info", "Fetching workspace history", "", "", 3000)
        root.pendingJoinPullCompletion = true
        if (chaftController.pullWorkspace(endpoint)) {
            root.pendingJoinPeerEndpoint = ""
            root.autoSyncEnabled = true
            return true
        }
        root.pendingJoinPullCompletion = false
        return false
    }

    function handlePendingJoinPullStatus() {
        if (!root.pendingJoinPullCompletion
                || String(root.pendingJoinPeerEndpoint || "").trim().length > 0
                || root.workspaceOperationInFlight) {
            return
        }
        var status = String(chaftController.syncStatus || "").trim()
        if (status.length === 0) {
            return
        }
        root.pendingJoinPullCompletion = false
        if (status.toLowerCase().indexOf("fetched ") === 0) {
            root.clearJoinWaitingForPeer()
            if (chaftController.peerHosting) {
                chaftController.refreshHostedPeerEndpointHint()
            }
            toastHost.show("success", "History fetched from invite", "", "", 4000)
        } else {
            toastHost.show("warning", "History could not fetch yet. Try again when a teammate is reachable.", "", "", 5000)
        }
    }

    function repairHistoryFromPeer() {
        if (!root.runtimeWorkReady || root.workspaceOperationInFlight) {
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
        return root.runtimeWorkReady
            && !root.workspaceOperationInFlight
            && endpoint.length > 0
            && chaftController.syncWorkspace(endpoint)
    }

    function publishWorkspaceToPreferredPeer() {
        var endpoint = root.preferredSyncPeerEndpoint()
        return root.runtimeWorkReady
            && !root.workspaceOperationInFlight
            && endpoint.length > 0
            && chaftController.publishWorkspace(endpoint)
    }

    function backupWorkspaceToPreferredPeer() {
        var endpoint = root.preferredManualBackupPeerEndpoint()
        return root.runtimeWorkReady
            && !root.workspaceOperationInFlight
            && endpoint.length > 0
            && chaftController.backupWorkspace(endpoint)
    }

    function pullWorkspaceFromPreferredPeer() {
        var endpoint = root.preferredSyncPeerEndpoint()
        return root.runtimeWorkReady
            && !root.workspaceOperationInFlight
            && endpoint.length > 0
            && chaftController.pullWorkspace(endpoint)
    }

    function retryBlobTransfersWithPreferredPeers() {
        var endpoint = root.preferredRetryPeerEndpoint()
        return root.runtimeWorkReady
            && !root.workspaceOperationInFlight
            && chaftController.retryBlobTransfers(endpoint)
    }

    function repairStorageMetadata() {
        return root.runtimeWorkReady
            && root.storageMetadataRepairSuggested
            && chaftController.repairWorkspaceStorageMetadata()
    }

    function publishEventWithTrustSnapshotToPreferredPeer(eventId) {
        var endpoint = root.preferredSyncPeerEndpoint()
        return root.runtimeWorkReady
            && !root.workspaceOperationInFlight
            && endpoint.length > 0
            && chaftController.publishEventWithTrustSnapshot(eventId, endpoint)
    }

    function backupConfiguredPeerIfReady() {
        var backupPeers = chaftController.backupPeerEndpoints || []
        if (!root.autoBackupEnabled || !root.runtimeWorkReady
                || root.workspaceOperationInFlight || !root.hasAutoBackupTargets) {
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
        root.channelAccessHistoryExpanded = false
        root.inspectorAccessHistoryExpanded = false
        root.restoreSelectedDraft(false)
        root.resetTimelineForChannelContext()
        root.scheduleControllerIdleWork()
        root.scheduleMarkSelectedChannelRead()
    }
    onNormalizedSearchQueryChanged: {
        if (root.normalizedSearchQuery.length > 0) {
            timelineView.resetToBeginningOnNextModel()
        } else {
            root.resetTimelineForChannelContext()
            root.scheduleControllerIdleWork()
        }
    }
    onInspectorItemKeyChanged: {
        if (root.inspectorItemKey.length === 0) {
            root.inspectorSelectedItemSnapshot = ({})
        }
    }
    onChannelsChanged: {
        root.applyPendingSmokeArchivedChannelSelection()
        root.applyPendingSmokePrivateChannelDetailsSelection()
        root.applyPendingSmokePrivateChannelInspectorSelection()
    }
    Component.onCompleted: {
        root.loadPersistedComposerDrafts()
        root.updateUnreadNotificationBaseline()
        root.ensureSelectedChannelInSnapshot()
        root.restorePendingEntryDisplayNameFromRequests()
        root.restoreSelectedDraft(false)
        root.resetTimelineForChannelContext()
        root.scheduleControllerIdleWork()
        root.scheduleMarkSelectedChannelRead()
        root.applySmokeUiState()
        Qt.callLater(function() {
            root.updateUnreadNotificationBaseline()
            root.unreadNotificationsReady = true
            root.openPostCreateCheckpointIfReady()
        })
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
            root.updateUnreadNotificationBaseline()
            return
        }
        root.updateUnreadNotificationBaseline()
        root.hostedRuntimeReconcileDue = true
        root.restorePendingEntryDisplayNameFromRequests()
        root.applyPendingEntryDisplayName()
        var pendingJoinPullStarted = root.pullPendingJoinPeerIfReady()
        root.scheduleControllerIdleWork()
        root.scheduleMarkSelectedChannelRead()
        if (root.indexedSearchHasTerms) {
            searchDebounce.restart()
        }
        if (root.autoSyncEnabled && !pendingJoinPullStarted
                && !root.pendingJoinPeerEndpointTargetsCurrentWorkspace()) {
            root.syncSelectedPeerIfReady()
        }
        if (root.autoBackupEnabled) {
            root.backupConfiguredPeerIfReady()
        }
        Qt.callLater(function() {
            root.openPostCreateCheckpointIfReady()
        })
    }

    Timer {
        id: searchDebounce
        interval: 250
        repeat: false
        onTriggered: {
            if (!root.runtimeWorkReady) {
                return
            }
            if (root.indexedSearchHasTerms) {
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
        id: pendingEntryDisplayNameRetryTimer
        interval: 1000
        repeat: false
        onTriggered: {
            if (root.pendingEntryDisplayNameWriteSucceeded) {
                root.reconcilePendingEntryDisplayName()
            } else {
                root.applyPendingEntryDisplayName()
            }
        }
    }

    Timer {
        id: autoSyncTimer
        interval: 3000
        repeat: true
        running: root.runtimeWorkReady && root.autoSyncEnabled
        onTriggered: root.syncSelectedPeerIfReady()
    }

    Timer {
        id: hostedRuntimeReconcileTimer
        interval: 2000
        repeat: true
        running: root.runtimeWorkReady && chaftController.peerHosting
        onTriggered: root.reconcileHostedRuntimeIfReady()
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

    Timer {
        id: pendingAccessResponseAutoCheckTimer
        interval: 5000
        repeat: true
        running: root.runtimeAccessReady
            && root.pendingAccessRequests.length > 0
            && !chaftController.accessEnvelopePullInFlight
        onTriggered: root.autoCheckPendingAccessRequestResponse()
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
            root.handleAccessRequestNotification()
            root.scheduleMarkSelectedChannelRead()
            root.restorePendingEntryDisplayNameFromRequests()
            root.applyPendingEntryDisplayName()
            root.scheduleControllerIdleWork()
            if (root.autoBackupEnabled) {
                autoBackupDebounce.restart()
            }
            if (String(chaftController.smokeUiState || "") === "member-roles"
                    && root.memberCount > 1) {
                smokeMemberRolesScrollTimer.restart()
            }
            root.applyPendingSmokeSetupRoomAccessSelection()
            root.openPostCreateCheckpointIfReady()
        }
        function onMessageSendFinished(workspaceId, channelId,
                                       replyToMessageId, success, message) {
            root.completePendingMessageSend(
                workspaceId, channelId, replyToMessageId, success, message)
        }
        function onMessageEditFinished(workspaceId, messageId, success,
                                       message) {
            root.completePendingMessageEdit(
                workspaceId, messageId, success, message)
        }
        function onAttachmentSendFinished(workspaceId, channelId,
                                          replyToMessageId, filePath,
                                          success, message) {
            root.completePendingAttachmentSend(
                workspaceId, channelId, replyToMessageId, filePath,
                success, message)
        }
        function onSelectedWorkspaceChanged() {
            root.hostedRuntimeReconcileDue = true
            root.inspectorItemKey = ""
            root.searchQuery = ""
            searchField.text = ""
            root.resetAccessRequestNotificationBaseline()
            timelineView.resetToLatestOnNextModel()
            Qt.callLater(function() {
                root.restorePendingEntryDisplayNameFromRequests()
                root.applyPendingEntryDisplayName()
                root.scheduleControllerIdleWork()
                root.openPostCreateCheckpointIfReady()
            })
        }
        function onBackupPeerEndpointsChanged() {
            if (!root.hasAutoBackupTargets) {
                root.autoBackupEnabled = false
            } else if (root.autoBackupEnabled) {
                root.backupConfiguredPeerIfReady()
            }
        }
        function onHostedPeerChanged() {
            root.hostedRuntimeReconcileDue = true
            if (chaftController.peerHosting) {
                root.reconcileHostedRuntimeIfReady()
            }
        }
        function onKeyTransferJsonChanged() {
            if (postCreateExportDialog.openRecoverySaveWhenReady
                    && postCreateExportDialog.keyKitReady) {
                postCreateExportDialog.openRecoverySaveWhenReady = false
                root.openSaveKeyTransferDialog("decryption key kit")
            }
            if (chaftController.keyTransferFromJoinResponseInbox
                    && (root.keyTransferIsInvitePackage()
                        || root.keyTransferIsInviteResponse())) {
                root.openReceivedApprovalInvite(false)
            }
        }
        function onAutoBackupEnabledChanged() {
            root.autoBackupEnabled = chaftController.autoBackupEnabled
        }
        function onLastCreatedChannelChanged() {
            var channelId = String(chaftController.lastCreatedChannelId || "")
            if (channelId.length > 0) {
                root.selectChannelId(channelId, true)
            }
        }
        function onSyncInFlightChanged() {
            if (!chaftController.syncInFlight) {
                root.handlePendingJoinPullStatus()
                root.handlePrivateRoomHistoryRepairCompletion()
                root.applyPendingEntryDisplayName()
                root.scheduleMarkSelectedChannelRead()
                root.scheduleControllerIdleWork()
                Qt.callLater(function() {
                    root.openPostCreateCheckpointIfReady()
                })
                pendingJoinPullCompletionTimer.restart()
                pendingPrivateRoomHistoryRepairCompletionTimer.restart()
            }
        }
        function onTimelineLoadInFlightChanged() {
            if (!chaftController.timelineLoadInFlight) {
                root.applyPendingEntryDisplayName()
                root.scheduleMarkSelectedChannelRead()
                if (root.controllerIdleWorkPending) {
                    root.scheduleControllerIdleWork()
                }
            }
        }
        function onWorkspaceOperationInFlightChanged() {
            if (!root.workspaceOperationInFlight) {
                root.restorePendingEntryDisplayNameFromRequests()
                root.applyPendingEntryDisplayName()
                root.scheduleMarkSelectedChannelRead()
                root.scheduleControllerIdleWork()
            }
        }
        function onSyncStatusChanged() {
            root.handlePendingJoinPullStatus()
            root.handlePrivateRoomHistoryRepairCompletion()
            root.maybeClearJoinWaitingForPeerFromStatus()
        }
        function onWorkspaceCredentialImportFinished(source, workspaceId,
                                                     success, message) {
            root.handleWorkspaceCredentialImportFinished(
                source, workspaceId, success, message)
        }
        function onDeviceProfileUpdateFinished(workspaceId, displayName,
                                               success, message) {
            root.handleDeviceProfileUpdateFinished(
                workspaceId, displayName, success, message)
            root.scheduleMarkSelectedChannelRead()
            Qt.callLater(function() {
                root.openPostCreateCheckpointIfReady()
            })
        }
        function onWorkspaceCreateFinished(workspaceId, success, selected,
                                           message) {
            root.handleWorkspaceCreateFinished(
                workspaceId, success, selected, message)
        }
        function onPendingJoinRequestsChanged() {
            root.restorePendingEntryDisplayNameFromRequests()
            root.applyPendingEntryDisplayName()
            var key = root.pendingAccessRequestSentUnpersistedKey
            if (key.length === 0) {
                return
            }
            var requests = chaftController.pendingJoinRequests || ({})
            if (!Object.prototype.hasOwnProperty.call(requests, key)) {
                root.pendingAccessRequestSentUnpersistedKey = ""
                return
            }
            var status = String((requests[key] || {}).status || "").trim()
            if (status !== "ready_to_send" && status !== "sending") {
                root.pendingAccessRequestSentUnpersistedKey = ""
            }
        }
        function onJoinRequestDirectSubmitCompleted(requestId, success, message) {
            if (root.pendingAccessRequestSendingKey.length === 0) {
                return
            }
            var row = root.pendingAccessRequestRowByKey(root.pendingAccessRequestSendingKey)
            if (String(row.requestId || "").trim()
                    !== String(requestId || "").trim()) {
                return
            }
            var deliveryLabel = String(row.deliveryLabel || "the workspace admin")
            var secureClaim = String(row.sourceType || "").trim()
                === "invite_claim"
            row.key = root.pendingAccessRequestSendingKey
            root.pendingAccessRequestSendingKey = ""
            var statusSaved = root.updatePendingAccessRequestStatus(
                row,
                success ? "sent" : "send_failed",
                success ? "" : String(message || ""))
            if (success && !statusSaved) {
                root.pendingAccessRequestSentUnpersistedKey = row.key
                toastHost.show(
                    "warning",
                    "Request sent, but its status could not be saved. Restore disk access; Chaft will keep checking for the response.",
                    "",
                    "",
                    8000)
                return
            }
            var toastMessage = success
                ? (secureClaim
                    ? "Join request sent to " + deliveryLabel
                        + ". Chaft will check for encrypted access."
                    : "Request sent to " + deliveryLabel
                        + ". Wait for their invite after approval.")
                : (secureClaim
                    ? "Join request not sent. Copy or save it, or try again when "
                        + deliveryLabel + " is reachable."
                    : "Request not sent. Copy the request link or try again when "
                        + deliveryLabel + " is reachable.")
            toastHost.show(
                success ? "success" : "warning",
                toastMessage,
                "",
                "",
                success ? 4500 : 6000)
        }
        function onRuntimeUnlockChanged() {
            if (chaftController.runtimeUnlockRequired) {
                root.runtimeUnlockDismissed = false
            }
            if (!chaftController.runtimeUnlocked) {
                root.saveCurrentDraft()
                root.inspectorItemKey = ""
                root.replyTarget = ({})
                root.editingMessageId = ""
                root.searchQuery = ""
                root.suppressComposerDraftSave = true
                composer.clearDraft()
                root.suppressComposerDraftSave = false
                attachmentDialog.pendingText = ""
                attachmentDialog.pendingWorkspaceId = ""
                attachmentDialog.pendingChannelId = ""
                attachmentDialog.pendingReplyToMessageId = ""
            } else {
                root.loadPersistedComposerDrafts()
                root.restoreSelectedDraft(false)
            }
        }
    }

    Connections {
        target: timelineView
        function onContentYChanged() {
            root.scheduleMarkSelectedChannelRead()
        }
        function onMovementEnded() {
            root.scheduleMarkSelectedChannelRead()
        }
    }

    WorkspaceEntryDialog {
        id: workspaceEntryDialog
        app: root
    }

    Dialog {
        id: postCreateExportDialog
        property string workspaceId: ""
        property bool advancedOpen: false
        property bool openRecoverySaveWhenReady: false
        property bool recoverySetupOpen: false
        property bool openedForNewWorkspace: false
        property bool keyKitBackedUp: false
        property bool explicitlyDeferred: false
        readonly property bool keyKitReady:
            chaftController.keyTransferJson.length > 0
            && root.keyTransferIsRecoveryBundle()
            && root.credentialWorkspaceId(chaftController.keyTransferJson)
                === postCreateExportDialog.workspaceId

        modal: true
        width: Math.min(root.width - 48, 560)
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        closePolicy: Popup.NoAutoClose
        title: "Save current decryption keys"

        ColumnLayout {
            anchors.fill: parent
            spacing: Tokens.space3

            Text {
                Layout.fillWidth: true
                text: (root.workspaceSnapshot.name || "Your workspace")
                    + " is ready. Save a private decryption key kit before "
                    + "inviting people. Back up this device's runtime separately. "
                    + "For admin continuity, authorize and promote another trusted "
                    + "owner. This kit imports saved keys, but cannot authorize a "
                    + "replacement device or restore identity, membership, or owner role."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeSm
                wrapMode: Text.WordWrap
            }

            Rectangle {
                Layout.fillWidth: true
                visible: false
                implicitHeight: postCreateChecklist.implicitHeight + Tokens.space3 * 2
                radius: Tokens.radiusSm
                color: Tokens.surfaceRaised
                border.width: 1
                border.color: Tokens.borderSubtle

                ColumnLayout {
                    id: postCreateChecklist
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Tokens.space3
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: root.localDeviceDisplayName().trim().length > 0
                            ? "Name set: " + root.localDeviceDisplayName().trim()
                            : "Set your name in Profile when you're ready"
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeSm
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.fillWidth: true
                        text: "Invite teammates from People & Access when you're ready."
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeSm
                        wrapMode: Text.WordWrap
                    }

                    Text {
                        Layout.fillWidth: true
                        text: postCreateExportDialog.keyKitReady
                            ? "Decryption key kit ready. Store it privately, keep its passphrase separate, and never send it as an invite."
                            : "Save a new key kit after major access or key changes so it contains the latest decryption keys."
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeSm
                        wrapMode: Text.WordWrap
                    }
                }
            }

            LabeledField {
                id: postCreateRecoveryPassphraseField
                Layout.fillWidth: true
                visible: postCreateExportDialog.recoverySetupOpen
                    && !postCreateExportDialog.keyKitReady
                label: "Key kit passphrase"
                placeholderText: "Use a long, unique passphrase"
                echoMode: TextInput.Password
                requiredField: true
                errorText: text.length > 0 && text.length < 12
                    ? "Use at least 12 characters."
                    : ""
                supportText: "Keep this passphrase separate from the key kit."
                onAccepted: {
                    if (text.trim().length > 0) {
                        postCreateRecoveryPassphraseConfirmationField.forceFieldFocus()
                    }
                }
            }

            LabeledField {
                id: postCreateRecoveryPassphraseConfirmationField
                Layout.fillWidth: true
                visible: postCreateRecoveryPassphraseField.visible
                label: "Confirm passphrase"
                placeholderText: "Enter the passphrase again"
                echoMode: TextInput.Password
                requiredField: true
                errorText: text.length > 0
                    && text !== postCreateRecoveryPassphraseField.text
                    ? "Passphrases do not match."
                    : ""
                onAccepted: {
                    if (postCreateRecoveryPassphraseField.text.trim().length > 0
                            && postCreateRecoveryPassphraseField.text.length >= 12
                            && text === postCreateRecoveryPassphraseField.text
                            && root.runtimeWorkReady
                            && !root.workspaceOperationInFlight
                            && !root.pendingEntryDisplayNameUpdateInFlight) {
                        postCreateExportDialog.openRecoverySaveWhenReady =
                            chaftController.exportRecoveryBundle(
                                postCreateRecoveryPassphraseField.text)
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                Button {
                    id: postCreateKeyKitButton
                    Layout.fillWidth: true
                    text: postCreateExportDialog.keyKitReady
                        ? "Save decryption key kit"
                        : (postCreateExportDialog.recoverySetupOpen
                            ? "Create decryption key kit"
                            : "Decryption key kit")
                    enabled: root.runtimeWorkReady
                        && !chaftController.keyTransferInFlight
                        && !root.workspaceOperationInFlight
                        && !root.pendingEntryDisplayNameUpdateInFlight
                        && (postCreateExportDialog.keyKitReady
                            || !postCreateExportDialog.recoverySetupOpen
                            || (postCreateRecoveryPassphraseField.text.trim().length > 0
                                && postCreateRecoveryPassphraseField.text.length >= 12
                                && postCreateRecoveryPassphraseConfirmationField.text
                                    === postCreateRecoveryPassphraseField.text))
                    onClicked: {
                        if (postCreateExportDialog.keyKitReady) {
                            root.openSaveKeyTransferDialog("decryption key kit")
                            return
                        }
                        if (!postCreateExportDialog.recoverySetupOpen) {
                            postCreateExportDialog.recoverySetupOpen = true
                            Qt.callLater(function() {
                                postCreateRecoveryPassphraseField.forceFieldFocus()
                            })
                            return
                        }
                        postCreateExportDialog.openRecoverySaveWhenReady =
                            chaftController.exportRecoveryBundle(
                                postCreateRecoveryPassphraseField.text)
                    }

                    background: Rectangle {
                        radius: Tokens.radiusSm
                        color: postCreateKeyKitButton.down
                            ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.38)
                            : Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b,
                                      postCreateKeyKitButton.enabled ? 0.24 : 0.1)
                        border.width: postCreateKeyKitButton.visualFocus ? 2 : 1
                        border.color: postCreateKeyKitButton.enabled
                            ? Tokens.accent
                            : Tokens.borderSubtle
                    }

                    contentItem: Text {
                        text: postCreateKeyKitButton.text
                        color: postCreateKeyKitButton.enabled
                            ? Tokens.textStrong
                            : Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeSm
                        font.weight: Font.Medium
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                }

                Button {
                    Layout.fillWidth: true
                    text: "Invite people"
                    enabled: root.runtimeWorkReady
                        && !chaftController.keyTransferInFlight
                        && !postCreateExportDialog.openRecoverySaveWhenReady
                    onClicked: {
                        if (!postCreateExportDialog.keyKitBackedUp) {
                            confirmDialog.ask(
                                "Invite without saving keys?",
                                "A decryption key kit has not been saved in durable private storage. Save it first, or confirm that you want to invite people now.",
                                "Invite anyway",
                                "invite-without-post-create-key-kit",
                                false)
                            return
                        }
                        postCreateExportDialog.close()
                        root.openPeopleAccess(true)
                    }
                }
            }

            Text {
                Layout.fillWidth: true
                visible: postCreateExportDialog.recoverySetupOpen
                    || postCreateExportDialog.keyKitReady
                text: "This kit is a point-in-time copy of decryption keys. "
                    + "It does not restore identity or workspace authorization."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                wrapMode: Text.WordWrap
            }

            Button {
                Layout.fillWidth: true
                visible: postCreateExportDialog.keyKitReady
                text: postCreateExportDialog.advancedOpen ? "Hide advanced" : "Advanced"
                onClicked: postCreateExportDialog.advancedOpen = !postCreateExportDialog.advancedOpen
            }

            TextArea {
                Layout.fillWidth: true
                Layout.preferredHeight: 132
                visible: postCreateExportDialog.keyKitReady
                    && postCreateExportDialog.advancedOpen
                readOnly: true
                text: chaftController.keyTransferJson
                Accessible.name: "Decryption key kit"
                color: Tokens.textStrong
                wrapMode: TextEdit.WrapAnywhere
                background: Rectangle {
                    radius: Tokens.radiusMd
                    color: Tokens.sidebarInput
                }
            }

            RowLayout {
                Layout.fillWidth: true
                visible: postCreateExportDialog.keyKitReady
                    && postCreateExportDialog.advancedOpen
                spacing: Tokens.space2

                Button {
                    Layout.fillWidth: true
                    text: "Copy " + root.keyTransferLabel()
                    enabled: postCreateExportDialog.keyKitReady
                    onClicked: {
                        if (root.copyTextToClipboard(
                                chaftController.keyTransferJson,
                                root.keyTransferLabel())) {
                            toastHost.show(
                                "info",
                                "Copied. Save it in durable private storage before marking it complete.",
                                "",
                                "",
                                6000)
                        }
                    }
                }

                Button {
                    Layout.fillWidth: true
                    text: "Save " + root.keyTransferLabel()
                    enabled: postCreateExportDialog.keyKitReady
                    onClicked: root.openSaveKeyTransferDialog(root.keyTransferLabel())
                }
            }

            Button {
                id: postCreateStartButton
                Layout.fillWidth: true
                text: postCreateExportDialog.keyKitBackedUp
                    ? "Open workspace"
                    : "Not now"
                enabled: !chaftController.keyTransferInFlight
                    && !postCreateExportDialog.openRecoverySaveWhenReady
                onClicked: {
                    if (!postCreateExportDialog.keyKitBackedUp) {
                        postCreateExportDialog.explicitlyDeferred = true
                        root.setKeyKitReminder(
                            postCreateExportDialog.workspaceId, false)
                    }
                    postCreateExportDialog.close()
                }
            }
        }

        Connections {
            target: root
            function onKeyTransferFileSaveFinished(success, label,
                                                   artifactKind,
                                                   operationToken) {
                if (postCreateExportDialog.visible && success
                        && String(label || "").toLowerCase()
                            .indexOf("key kit") >= 0) {
                    postCreateExportDialog.keyKitBackedUp = true
                    root.setKeyKitReminder(
                        postCreateExportDialog.workspaceId, false)
                }
            }
        }

        onOpened: {
            Qt.callLater(function() {
                if (postCreateExportDialog.keyKitReady
                        && postCreateKeyKitButton.enabled) {
                    postCreateKeyKitButton.forceActiveFocus()
                } else if (postCreateRecoveryPassphraseField.visible) {
                    postCreateRecoveryPassphraseField.forceFieldFocus()
                } else {
                    postCreateKeyKitButton.forceActiveFocus()
                }
            })
        }
        onClosed: {
            var shouldRemind = postCreateExportDialog.openedForNewWorkspace
                && !postCreateExportDialog.keyKitBackedUp
                && postCreateExportDialog.explicitlyDeferred
            postCreateRecoveryPassphraseField.text = ""
            postCreateRecoveryPassphraseConfirmationField.text = ""
            postCreateExportDialog.advancedOpen = false
            postCreateExportDialog.openRecoverySaveWhenReady = false
            postCreateExportDialog.recoverySetupOpen = false
            postCreateExportDialog.openedForNewWorkspace = false
            postCreateExportDialog.keyKitBackedUp = false
            postCreateExportDialog.explicitlyDeferred = false
            postCreateExportDialog.workspaceId = ""
            if (shouldRemind) {
                toastHost.show(
                    "warning",
                    "No decryption key kit was saved. Create one later from "
                        + "Settings. Back up this device's runtime; for admin "
                        + "continuity, promote another trusted owner.",
                    "",
                    "",
                    9000)
            }
        }
    }

    Dialog {
        id: runtimeUnlockDialog
        modal: true
        width: Math.min(root.width - 48, 420)
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        visible: chaftController.runtimeUnlockRequired && !root.runtimeUnlockDismissed
        closePolicy: Popup.NoAutoClose
        title: "Unlock workspace"

        ColumnLayout {
            anchors.fill: parent
            spacing: 10

            Text {
                Layout.fillWidth: true
                text: "Enter the workspace passphrase to read messages and manage access on this device."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeSm
                wrapMode: Text.WordWrap
            }

            TextField {
                id: runtimePassphraseField
                Layout.fillWidth: true
                placeholderText: "Workspace passphrase"
                echoMode: TextInput.Password
                color: Tokens.textStrong
                placeholderTextColor: Tokens.textMuted
                Accessible.name: "Workspace passphrase"
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
                    text: "Not now"
                    Accessible.name: "Unlock workspace later"
                    onClicked: {
                        runtimePassphraseField.text = ""
                        root.runtimeUnlockDismissed = true
                    }
                }

                Button {
                    Layout.fillWidth: true
                    text: "Unlock workspace"
                    enabled: runtimePassphraseField.text.trim().length > 0
                    Accessible.name: "Unlock workspace"
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
            var filePath = root.localPathFromUrl(selectedFile).trim()
            var sent = false
            if (!root.composerOperationPending
                    && pendingWorkspaceId === root.currentWorkspaceId()) {
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
                root.beginComposerOperation(
                    "attachment",
                    pendingWorkspaceId,
                    pendingChannelId,
                    pendingReplyToMessageId,
                    "",
                    pendingText,
                    filePath)
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
        id: workspaceCredentialDialog
        title: workspaceEntryDialog.keyKitMode
            ? "Open decryption key kit"
            : "Open invite, request link, decryption key kit, or access file"
        fileMode: FileDialog.OpenFile
        nameFilters: root.workspaceCredentialNameFilters(
            workspaceEntryDialog.keyKitMode)
        onAccepted: {
            root.loadWorkspaceCredentialUrl(selectedFile)
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
        property string transferLabel: "support detail"
        property string transferText: ""
        property string transferKind: ""
        property string operationToken: ""
        property bool trackPendingAccessRequest: false
        title: "Save " + transferLabel
        fileMode: FileDialog.SaveFile
        nameFilters: root.keyTransferNameFilters(transferLabel)
        onAccepted: {
            var savedLabel = transferLabel
            var savedText = transferText
            var savedKind = transferKind
            var savedToken = operationToken
            var shouldTrackPendingAccessRequest = trackPendingAccessRequest
            var outputPath = root.outputPathWithExtension(
                root.localPathFromUrl(selectedFile),
                root.keyTransferFileExtension(savedLabel))
            var saved = chaftController.saveTextFile(
                outputPath,
                savedText,
                savedLabel)
            if (saved && shouldTrackPendingAccessRequest) {
                saved = root.recordPendingAccessRequestFromArtifact(
                    "file_ready", savedText)
            }
            root.keyTransferFileSaveFinished(
                saved, savedLabel, savedKind, savedToken)
            transferLabel = "support detail"
            transferText = ""
            transferKind = ""
            operationToken = ""
            trackPendingAccessRequest = false
        }
        onRejected: {
            transferLabel = "support detail"
            transferText = ""
            transferKind = ""
            operationToken = ""
            trackPendingAccessRequest = false
        }
    }

    FileDialog {
        id: pendingAccessRequestSaveDialog
        title: "Save " + root.pendingAccessRequestSaveLabel
        fileMode: FileDialog.SaveFile
        nameFilters: root.pendingAccessRequestSaveLabel === "join request"
            ? [ "Chaft join requests (*.chaftrequest)", "Older support files (*.json)", "All files (*)" ]
            : [ "Chaft access requests (*.chaftrequest)", "Older support files (*.json)", "All files (*)" ]
        onAccepted: {
            var outputPath = root.outputPathWithExtension(
                root.localPathFromUrl(selectedFile),
                ".chaftrequest")
            var saved = chaftController.saveTextFile(
                outputPath,
                root.pendingAccessRequestSaveText,
                root.pendingAccessRequestSaveLabel)
            if (saved && root.pendingAccessRequestSaveKey.length > 0) {
                var pendingRow = root.pendingAccessRequestRowByKey(
                    root.pendingAccessRequestSaveKey)
                if (String(pendingRow.status || "").trim() !== "sent") {
                    root.updatePendingAccessRequestStatus(
                        pendingRow, "file_ready", "")
                }
            }
            root.pendingAccessRequestSaveText = ""
            root.pendingAccessRequestSaveKey = ""
            root.pendingAccessRequestSaveLabel = "access request"
        }
        onRejected: {
            root.pendingAccessRequestSaveText = ""
            root.pendingAccessRequestSaveKey = ""
            root.pendingAccessRequestSaveLabel = "access request"
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: root.workspaceRailWidth
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

                    BrandMark {
                        width: 40
                        height: 40
                    }

                    Repeater {
                        model: root.workspaceRailItems
                        delegate: WorkspaceRailItem {
                            id: workspaceRailDelegate
                            required property int index
                            required property var modelData

                            workspaceId: String(workspaceRailDelegate.modelData.workspaceId || "")
                            workspaceName: root.workspaceDisplayName(
                                workspaceRailDelegate.modelData)
                            initial: root.workspaceInitial(workspaceRailDelegate.modelData)
                            selected: String(workspaceRailDelegate.modelData.workspaceId || "") === chaftController.selectedWorkspaceId
                            actionable: root.runtimeAccessReady
                            unreadCount: root.workspaceRailUnreadCount(workspaceRailDelegate.modelData)
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
                Accessible.description: "Join, create, or import a decryption key kit"
                enabled: root.runtimeAccessReady && !root.composerContextBusy
                onClicked: root.openAddWorkspaceChooser()
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

            Popup {
                id: addWorkspacePopup
                parent: addWorkspaceButton
                x: addWorkspaceButton.width + Tokens.space2
                y: Math.min(0, addWorkspaceButton.height - implicitHeight)
                width: 252
                padding: Tokens.space2
                modal: true
                focus: true
                closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

                background: Rectangle {
                    radius: Tokens.radiusMd
                    color: Tokens.surfaceRaised
                    border.width: 1
                    border.color: Tokens.borderSubtle
                }

                contentItem: ColumnLayout {
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: "Add workspace"
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeLg
                        font.weight: Font.Bold
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.fillWidth: true
                        visible: false
                        text: chaftController.hasRuntimeWorkspace
                            ? "Join with an invite, request link, or access file; create another workspace; or import a decryption key kit."
                            : "Open an invite, request link, or access file; create a workspace; import a decryption key kit; or explore a demo."
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        wrapMode: Text.WordWrap
                    }

                    Button {
                        id: addWorkspaceJoinButton
                        Layout.fillWidth: true
                        text: "Join workspace"
                        Accessible.name: text
                        Accessible.description: "Open an invite, request link, or access file"
                        onClicked: root.chooseWorkspaceEntry("join")

                        background: Rectangle {
                            radius: Tokens.radiusSm
                            color: addWorkspaceJoinButton.hovered
                                ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                                : Tokens.surfaceBase
                            border.width: addWorkspaceJoinButton.visualFocus ? 2 : 1
                            border.color: addWorkspaceJoinButton.visualFocus ? Tokens.accent : Tokens.borderSubtle
                        }

                        contentItem: RowLayout {
                            spacing: Tokens.space2

                            Text {
                                text: "⇄"
                                color: Tokens.accent
                                font.pixelSize: Tokens.fontSizeMd
                                font.weight: Font.Bold
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1

                                Text {
                                    Layout.fillWidth: true
                                    text: addWorkspaceJoinButton.text
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: false
                                    text: "Open an invite or access file"
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 2
                                }
                            }
                        }
                    }

                    Button {
                        id: addWorkspaceCreateButton
                        Layout.fillWidth: true
                        text: "Create workspace"
                        variant: "primary"
                        Accessible.name: text
                        Accessible.description: "Start a new workspace"
                        onClicked: root.chooseWorkspaceEntry("create")

                        background: Rectangle {
                            radius: Tokens.radiusSm
                            color: addWorkspaceCreateButton.hovered
                                ? Qt.lighter(Tokens.accent, 1.06)
                                : Tokens.accent
                            border.width: addWorkspaceCreateButton.visualFocus ? 2 : 1
                            border.color: Tokens.accent
                        }

                        contentItem: RowLayout {
                            spacing: Tokens.space2

                            Text {
                                text: "#"
                                color: Tokens.onAccent
                                font.pixelSize: Tokens.fontSizeMd
                                font.weight: Font.Bold
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1

                                Text {
                                    Layout.fillWidth: true
                                    text: addWorkspaceCreateButton.text
                                    color: Tokens.onAccent
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: false
                                    text: "Start a private space, then invite teammates any time"
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 2
                                }
                            }
                        }
                    }

                    Button {
                        id: addWorkspaceRestoreButton
                        Layout.fillWidth: true
                        text: "Import key kit"
                        variant: "quiet"
                        Accessible.name: text
                        Accessible.description: "Import saved decryption keys"
                        onClicked: root.chooseWorkspaceEntry("join", "restore")

                        background: Rectangle {
                            radius: Tokens.radiusSm
                            color: addWorkspaceRestoreButton.hovered
                                ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                                : Tokens.surfaceBase
                            border.width: addWorkspaceRestoreButton.visualFocus ? 2 : 1
                            border.color: addWorkspaceRestoreButton.visualFocus ? Tokens.accent : Tokens.borderSubtle
                        }

                        contentItem: RowLayout {
                            spacing: Tokens.space2

                            Text {
                                text: "↺"
                                color: Tokens.accent
                                font.pixelSize: Tokens.fontSizeMd
                                font.weight: Font.Bold
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1

                                Text {
                                    Layout.fillWidth: true
                                    text: addWorkspaceRestoreButton.text
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: false
                                    text: "Import keys; an unauthorized device still needs an invite"
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 2
                                }
                            }
                        }
                    }

                    Button {
                        id: addWorkspaceDemoButton
                        Layout.fillWidth: true
                        text: "Explore demo workspace"
                        variant: "quiet"
                        visible: !chaftController.hasRuntimeWorkspace
                        enabled: !chaftController.hasRuntimeWorkspace
                        Accessible.name: text
                        Accessible.description: "Preview Chaft without saving or sharing anything"
                        onClicked: root.chooseDemoWorkspace()

                        background: Rectangle {
                            radius: Tokens.radiusSm
                            color: addWorkspaceDemoButton.hovered && addWorkspaceDemoButton.enabled
                                ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                                : Tokens.surfaceBase
                            border.width: addWorkspaceDemoButton.visualFocus ? 2 : 1
                            border.color: addWorkspaceDemoButton.visualFocus ? Tokens.accent : Tokens.borderSubtle
                        }

                        contentItem: RowLayout {
                            spacing: Tokens.space2

                            Text {
                                text: "▶"
                                color: addWorkspaceDemoButton.enabled ? Tokens.accent : Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeMd
                                font.weight: Font.Bold
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1

                                Text {
                                    Layout.fillWidth: true
                                    text: addWorkspaceDemoButton.text
                                    color: addWorkspaceDemoButton.enabled ? Tokens.textStrong : Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: addWorkspaceDemoButton.enabled
                                        ? "Explore a read-only sample workspace"
                                        : "Available before opening a workspace"
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 2
                                }
                            }
                        }
                    }
                }
            }
        }

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: root.navigationSidebarWidth
            color: Tokens.sidebar

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    Text {
                        Layout.fillWidth: true
                        text: root.workspaceSnapshot.name || "Chaft"
                        color: Tokens.sidebarTextStrong
                        font.pixelSize: Tokens.fontSizeXl
                        font.weight: Font.Bold
                        elide: Text.ElideRight
                    }

                    Text {
                        visible: !root.hasWorkspaceContent
                        text: "Private workspace chat"
                        color: Tokens.sidebarTextMuted
                        font.pixelSize: Tokens.fontSizeXs
                    }
                }

                TextField {
                    id: searchField
                    Layout.fillWidth: true
                    visible: root.hasWorkspaceContent
                    placeholderText: "Search or jump"
                    Accessible.name: "Search or jump"
                    Accessible.description: root.searchHasTerms
                        ? "Search messages, rooms, and DMs"
                        : "Find messages, rooms, DMs, or jump to a conversation"
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
                    visible: root.searchHasTerms
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: root.searchStatusText()
                        color: Tokens.sidebarTextMuted
                        font.pixelSize: Tokens.fontSizeXs
                        elide: Text.ElideRight
                        Accessible.role: Accessible.StaticText
                        Accessible.name: text
                    }

                    Button {
                        text: "Clear"
                        Layout.preferredWidth: 58
                        Accessible.name: "Clear search"
                        onClicked: {
                            searchField.text = ""
                            root.searchQuery = ""
                            searchField.forceActiveFocus()
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    spacing: 6

                    Text {
                        Layout.fillWidth: true
                        text: "Rooms"
                        color: Tokens.sidebarTextMuted
                        font.pixelSize: Tokens.fontSizeXs
                        font.weight: Font.DemiBold
                    }

                    Button {
                        id: newChannelButton
                        text: "+"
                        Accessible.name: "New room"
                        Accessible.description: "Open the room creation form"
                        implicitWidth: 30
                        enabled: root.runtimeWorkReady
                        onClicked: newChannelPopup.open()
                        ToolTip.visible: hovered
                        ToolTip.text: "New room"
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
                                text: "Room name"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                            }

                            TextField {
                                id: channelNameField
                                Layout.fillWidth: true
                                placeholderText: "e.g. launch-plan"
                                Accessible.name: "Room name"
                                onAccepted: newChannelPopup.createFromForm()
                            }

                            CheckBox {
                                id: privateChannelCheck
                                text: "Private room"
                                Accessible.name: "Private room"
                                Accessible.description: checked
                                    ? "New room will require explicit member grants"
                                    : "New room will be visible to workspace members"
                            }

                            Button {
                                Layout.fillWidth: true
                                text: privateChannelCheck.checked ? "Create private room" : "Create room"
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
                    Layout.fillHeight: true
                    visible: root.hasWorkspaceContent
                    Layout.preferredHeight: 1
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
                                visible: root.filteredRoomChannels.length === 0
                                    && root.filteredArchivedChannels.length === 0
                                    && root.filteredDirectMessageChannels.length === 0
                                Layout.fillWidth: true
                                text: root.channels.length === 0
                                    && root.runtimeWorkReady
                                    && root.normalizedSearchQuery.length === 0
                                    ? "No rooms yet. Press + to create one."
                                    : "No matching conversations"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeSm
                                wrapMode: Text.WordWrap
                            }

                            Repeater {
                                model: root.filteredRoomChannels
                                delegate: SidebarItem {
                                    id: channelSidebarDelegate
                                    required property var modelData

                                    label: root.channelDisplayName(channelSidebarDelegate.modelData)
                                    secondaryLabel: root.channelSidebarLabel(channelSidebarDelegate.modelData)
                                    selected: channelSidebarDelegate.modelData.channelId === root.selectedChannel.channelId
                                    unreadCount: channelSidebarDelegate.modelData.unreadCount
                                    privateChannel: channelSidebarDelegate.modelData.isPrivate
                                    hasDraft: root.draftTextForChannel(channelSidebarDelegate.modelData.channelId).trim().length > 0
                                    muted: root.channelMuted(channelSidebarDelegate.modelData)
                                    archived: root.channelArchived(channelSidebarDelegate.modelData)
                                    onActivated: root.selectChannelId(channelSidebarDelegate.modelData.channelId, true)
                                }
                            }

                            Text {
                                visible: root.filteredArchivedChannels.length > 0
                                Layout.fillWidth: true
                                Layout.topMargin: 8
                                text: "Archived"
                                color: Tokens.sidebarTextMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                            }

                            Repeater {
                                model: root.filteredArchivedChannels
                                delegate: SidebarItem {
                                    id: archivedChannelSidebarDelegate
                                    required property var modelData

                                    label: root.channelDisplayName(archivedChannelSidebarDelegate.modelData)
                                    secondaryLabel: root.channelSidebarLabel(archivedChannelSidebarDelegate.modelData)
                                    selected: archivedChannelSidebarDelegate.modelData.channelId === root.selectedChannel.channelId
                                    unreadCount: archivedChannelSidebarDelegate.modelData.unreadCount
                                    privateChannel: archivedChannelSidebarDelegate.modelData.isPrivate
                                    hasDraft: root.draftTextForChannel(archivedChannelSidebarDelegate.modelData.channelId).trim().length > 0
                                    muted: root.channelMuted(archivedChannelSidebarDelegate.modelData)
                                    archived: true
                                    onActivated: root.selectChannelId(archivedChannelSidebarDelegate.modelData.channelId, true)
                                }
                            }

                            Text {
                                visible: root.filteredDirectMessageChannels.length > 0
                                Layout.fillWidth: true
                                Layout.topMargin: 8
                                text: "Direct messages"
                                color: Tokens.sidebarTextMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                            }

                            Repeater {
                                model: root.filteredDirectMessageChannels
                                delegate: SidebarItem {
                                    id: directMessageSidebarDelegate
                                    required property var modelData

                                    label: root.channelDisplayName(directMessageSidebarDelegate.modelData)
                                    secondaryLabel: root.channelSidebarLabel(directMessageSidebarDelegate.modelData)
                                    selected: directMessageSidebarDelegate.modelData.channelId === root.selectedChannel.channelId
                                    unreadCount: directMessageSidebarDelegate.modelData.unreadCount
                                    privateChannel: true
                                    directMessage: true
                                    avatarId: root.directMessageAvatarId(
                                        directMessageSidebarDelegate.modelData)
                                    avatarWorkspaceId: root.currentWorkspaceId()
                                    avatarIdentityId: root.directMessageParticipantDeviceId(
                                        directMessageSidebarDelegate.modelData)
                                    hasDraft: root.draftTextForChannel(directMessageSidebarDelegate.modelData.channelId).trim().length > 0
                                    muted: root.channelMuted(directMessageSidebarDelegate.modelData)
                                    archived: false
                                    onActivated: root.selectChannelId(directMessageSidebarDelegate.modelData.channelId, true)
                                }
                            }

                            Button {
                                Layout.fillWidth: true
                                visible: root.channelCount > root.channels.length
                                text: "Load more rooms"
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
                    id: peopleAccessButton
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    text: "People & Access"
                    checkable: true
                    checked: root.peopleAccessDestination
                    Accessible.name: "People & Access"
                    Accessible.description: root.waitingAccessRequestCount > 0
                        ? root.accessRequestCountLabel(root.waitingAccessRequestCount) + " waiting"
                        : "Manage members, invites, requests, and roles"
                    onClicked: root.openPeopleAccess(false)
                    ToolTip.visible: hovered && root.waitingAccessRequestCount > 0
                    ToolTip.text: root.accessRequestCountLabel(root.waitingAccessRequestCount)
                        + " waiting"

                    contentItem: RowLayout {
                        spacing: Tokens.space2

                        Text {
                            Layout.fillWidth: true
                            text: peopleAccessButton.text
                            color: peopleAccessButton.enabled ? Tokens.textStrong : Tokens.textMuted
                            font: peopleAccessButton.font
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                        }

                        Rectangle {
                            visible: root.waitingAccessRequestCount > 0
                            Layout.preferredWidth: Math.max(22, accessRequestBadgeText.implicitWidth + 10)
                            Layout.preferredHeight: 20
                            radius: Tokens.radiusSm
                            color: Qt.rgba(
                                Tokens.accent.r,
                                Tokens.accent.g,
                                Tokens.accent.b,
                                0.14)
                            border.width: 1
                            border.color: Tokens.accent

                            Text {
                                id: accessRequestBadgeText
                                anchors.centerIn: parent
                                text: root.accessRequestBadgeLabel(root.waitingAccessRequestCount)
                                color: Tokens.accent
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                            }
                        }
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: chaftController.deviceId.length > 0
                        || chaftController.hasRuntimeWorkspace
                    text: "Settings"
                    checkable: true
                    checked: root.settingsDestination
                    Accessible.name: "Settings"
                    Accessible.description: "Open personal and workspace settings"
                    onClicked: root.openSettings(root.settingsCategory)
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: Tokens.surfaceBase

            ColumnLayout {
                id: conversationView
                anchors.fill: parent
                visible: root.conversationDestination
                spacing: 0

                EmptyWorkspaceView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: !root.hasWorkspaceContent
                    app: root
                }

                Rectangle {
                    visible: root.demoTourActive
                    Layout.fillWidth: true
                    Layout.preferredHeight: visible ? 34 : 0
                    color: Qt.rgba(
                        Tokens.accent.r,
                        Tokens.accent.g,
                        Tokens.accent.b,
                        0.08)
                    border.color: Tokens.accent

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 18
                        anchors.rightMargin: 12
                        spacing: Tokens.space2

                        Text {
                            Layout.fillWidth: true
                            text: "Demo workspace preview - nothing here is saved or shared."
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.Medium
                            elide: Text.ElideRight
                        }

                        Button {
                            text: "Exit demo"
                            Accessible.name: "Exit demo workspace preview"
                            onClicked: root.exitDemoTour()
                        }
                    }
                }

                Rectangle {
                    id: channelHeaderPanel
                    readonly property int syncControlsExpandedHeight: Math.max(
                        46,
                        syncControlsFlow.childrenRect.height + 18)

                    visible: root.hasWorkspaceContent
                    Layout.fillWidth: true
                    Layout.preferredHeight: visible
                        ? root.channelHeaderHeight
                            + (chaftController.hasRuntimeWorkspace && root.syncDrawerOpen
                                ? channelHeaderPanel.syncControlsExpandedHeight
                                : 0)
                        : 0
                    color: Tokens.surfaceBase

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        RowLayout {
                            Layout.fillWidth: true
                            Layout.preferredHeight: root.channelHeaderHeight
                            Layout.leftMargin: 18
                            Layout.rightMargin: 18
                            spacing: 10

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: (root.selectedChannelDirectMessage ? "@ " : "# ")
                                        + root.selectedChannelDisplayName
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeXl
                                    font.weight: Font.Bold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    visible: root.selectedChannelTopic.length > 0
                                        && !root.selectedChannelDirectMessage
                                    Layout.fillWidth: true
                                    text: root.selectedChannelTopic
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeSm
                                    elide: Text.ElideRight
                                }
                            }

                            Button {
                                id: channelDetailsButton
                                visible: root.runtimeWorkReady
                                    && root.selectedChannelKey.length > 0
                                text: root.selectedChannelDirectMessage ? "Chat" : "Room"
                                Layout.preferredWidth: 64
                                Accessible.name: root.selectedChannelDirectMessage
                                    ? "Conversation actions"
                                    : "Room actions"
                                onClicked: channelHeaderActionsMenu.open()
                                ToolTip.visible: hovered
                                ToolTip.text: root.selectedChannelDirectMessage
                                    ? "Conversation actions"
                                    : "Room actions"

                                Menu {
                                    id: channelHeaderActionsMenu
                                    y: channelDetailsButton.height

                                    MenuItem {
                                        visible: !root.selectedChannelDirectMessage
                                        text: "Edit room"
                                        onTriggered: Qt.callLater(function() {
                                            channelDetailsPopup.open()
                                        })
                                    }

                                    MenuItem {
                                        text: root.selectedChannelMuted ? "Unmute" : "Mute"
                                        onTriggered: root.toggleSelectedChannelMuted()
                                    }
                                }
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
                                        + " message security notice"
                                        + (root.channelCryptoExceptionCount === 1 ? "" : "s")
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

                        Popup {
                            id: channelDetailsPopup
                            property string viewMode: "details"
                            parent: channelDetailsButton
                            y: channelDetailsButton.height + 4
                            x: channelDetailsButton.width - width
                            width: Math.min(380, root.width - 48)
                            height: Math.min(
                                channelDetailsContent.implicitHeight + topPadding + bottomPadding,
                                root.height - 48)
                            padding: Tokens.space3
                            modal: true
                            focus: true
                            closePolicy: Popup.CloseOnEscape
                            onOpened: {
                                channelDetailsPopup.viewMode = root.selectedChannelPrivate
                                    && root.channelAccessToolsOpen
                                    ? "access"
                                    : "details"
                                channelDetailsNameField.text = root.selectedChannelDisplayName
                                channelDetailsTopicField.text = root.selectedChannelTopic
                                if (channelDetailsPopup.viewMode === "details") {
                                    channelDetailsNameField.forceActiveFocus()
                                } else {
                                    channelAccessModeButton.forceActiveFocus()
                                }
                            }
                            onClosed: {
                                channelDetailsNameField.text = ""
                                channelDetailsTopicField.text = ""
                                root.channelAccessToolsOpen = false
                                channelDetailsPopup.viewMode = "details"
                            }

                            function saveFromForm() {
                                var nextName = channelDetailsNameField.text.trim()
                                var nextTopic = channelDetailsTopicField.text.trim()
                                if (nextName.length === 0) {
                                    return
                                }
                                if (nextName === root.selectedChannelDisplayName
                                        && nextTopic === root.selectedChannelTopic) {
                                    channelDetailsPopup.close()
                                    return
                                }
                                if (root.runtimeWorkReady
                                        && chaftController.updateChannelDetails(
                                            root.selectedChannelKey, nextName, nextTopic)) {
                                    channelDetailsPopup.close()
                                }
                            }

                            background: Rectangle {
                                radius: Tokens.radiusMd
                                color: Tokens.surfaceRaised
                                border.width: 1
                                border.color: Tokens.borderSubtle
                            }

                            contentItem: ScrollView {
                                id: channelDetailsScroll
                                clip: true
                                contentWidth: availableWidth
                                contentHeight: channelDetailsContent.implicitHeight
                                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                                ColumnLayout {
                                    id: channelDetailsContent
                                    width: channelDetailsScroll.availableWidth
                                    height: implicitHeight
                                    spacing: Tokens.space2

                                Text {
                                    text: "Room"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    visible: root.selectedChannelPrivate
                                        && !root.selectedChannelDirectMessage
                                    spacing: Tokens.space1

                                    Button {
                                        Layout.fillWidth: true
                                        text: "Details"
                                        checkable: true
                                        checked: channelDetailsPopup.viewMode === "details"
                                        onClicked: channelDetailsPopup.viewMode = "details"
                                    }

                                    Button {
                                        id: channelAccessModeButton
                                        Layout.fillWidth: true
                                        text: "Access & security"
                                        checkable: true
                                        checked: channelDetailsPopup.viewMode === "access"
                                        onClicked: channelDetailsPopup.viewMode = "access"
                                    }
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: channelDetailsPopup.viewMode === "access"
                                    text: root.selectedChannelPeopleSummary()
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: channelDetailsPopup.viewMode === "details"
                                    spacing: 4

                                    Text {
                                        Layout.fillWidth: true
                                        text: "Name"
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideRight
                                    }

                                    TextField {
                                        id: channelDetailsNameField
                                        Layout.fillWidth: true
                                        placeholderText: "Room name"
                                        placeholderTextColor: Tokens.textMuted
                                        color: Tokens.textStrong
                                        selectByMouse: true
                                        Accessible.name: "Room name"
                                        onAccepted: channelDetailsPopup.saveFromForm()
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: channelDetailsPopup.viewMode === "details"
                                    spacing: 4

                                    Text {
                                        Layout.fillWidth: true
                                        text: "Topic"
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideRight
                                    }

                                    TextArea {
                                        id: channelDetailsTopicField
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 82
                                        placeholderText: "Add a topic"
                                        placeholderTextColor: Tokens.textMuted
                                        color: Tokens.textStrong
                                        wrapMode: TextEdit.Wrap
                                        selectByMouse: true
                                        Accessible.name: "Room topic"
                                        background: Rectangle {
                                            radius: Tokens.radiusSm
                                            color: Tokens.surfaceBase
                                            border.width: 1
                                            border.color: channelDetailsTopicField.activeFocus
                                                ? Tokens.accent
                                                : Tokens.borderSubtle
                                        }
                                    }
                                }

                                ColumnLayout {
                                    visible: root.selectedChannelPrivate
                                        && !root.selectedChannelDirectMessage
                                        && channelDetailsPopup.viewMode === "access"
                                    Layout.fillWidth: true
                                    spacing: Tokens.space2

                                    RowLayout {
                                        Layout.fillWidth: true

                                        Text {
                                            Layout.fillWidth: true
                                            text: "People with access"
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeSm
                                            font.weight: Font.DemiBold
                                        }

                                        Button {
                                            text: root.channelAccessToolsOpen
                                                ? "Hide history"
                                                : "History & security"
                                            checkable: true
                                            checked: root.channelAccessToolsOpen
                                            onToggled: root.channelAccessToolsOpen = checked
                                            Accessible.name: text
                                        }
                                    }

                                    Rectangle {
                                        visible: root.channelAccessToolsOpen
                                        Layout.fillWidth: true
                                        implicitHeight: privateRoomReadabilityLayout.implicitHeight
                                            + Tokens.space3 * 2
                                        radius: Tokens.radiusSm
                                        color: root.selectedChannelLockedMessageCount > 0
                                            ? Tokens.warningSurface
                                            : (root.selectedChannelLoadedMessageCount > 0
                                                ? Tokens.secureSurface
                                                : Tokens.surfaceBase)
                                        border.width: 1
                                        border.color: root.selectedChannelLockedMessageCount > 0
                                            ? Tokens.warning
                                            : (root.selectedChannelLoadedMessageCount > 0
                                                ? Tokens.secure
                                                : Tokens.borderSubtle)

                                        Accessible.role: Accessible.StaticText
                                        Accessible.name: root.privateRoomReadabilityTitle()
                                            + ". "
                                            + root.privateRoomReadabilityText()

                                        ColumnLayout {
                                            id: privateRoomReadabilityLayout
                                            anchors.fill: parent
                                            anchors.margins: Tokens.space3
                                            spacing: Tokens.space1

                                            Text {
                                                Layout.fillWidth: true
                                                text: root.privateRoomReadabilityTitle()
                                                color: root.selectedChannelLockedMessageCount > 0
                                                    ? Tokens.warningText
                                                    : (root.selectedChannelLoadedMessageCount > 0
                                                        ? Tokens.secure
                                                        : Tokens.textStrong)
                                                font.pixelSize: Tokens.fontSizeXs
                                                font.weight: Font.DemiBold
                                                wrapMode: Text.WordWrap
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: root.privateRoomReadabilityText()
                                                color: root.selectedChannelLockedMessageCount > 0
                                                    ? Tokens.warningText
                                                    : Tokens.textMuted
                                                font.pixelSize: Tokens.fontSizeXs
                                                wrapMode: Text.WordWrap
                                            }

                                            RowLayout {
                                                visible: root.privateRoomHistoryRepairActionVisible()
                                                Layout.alignment: Qt.AlignRight
                                                Layout.topMargin: Tokens.space1
                                                spacing: Tokens.space2

                                                Button {
                                                    visible: root.privateRoomHistoryRepairChangeAddressVisible()
                                                    text: "Change address"
                                                    Accessible.name: "Change teammate address for private room history"
                                                    onClicked: root.focusPeerAddressField()
                                                    ToolTip.visible: hovered
                                                    ToolTip.text: "Paste a different teammate address"
                                                }

                                                Button {
                                                    text: root.privateRoomHistoryRepairActionLabel()
                                                    enabled: root.privateRoomHistoryRepairActionEnabled()
                                                    Accessible.name: text + " for private room history"
                                                    onClicked: root.handlePrivateRoomHistoryRepairAction()
                                                    ToolTip.visible: hovered
                                                    ToolTip.text: root.privateRoomHistoryRepairActionTooltip()
                                                }
                                            }

                                            Text {
                                                visible: root.privateRoomHistoryRepairFailedVisible()
                                                Layout.fillWidth: true
                                                text: root.privateRoomHistoryRepairFailedText()
                                                color: Tokens.warningText
                                                font.pixelSize: Tokens.fontSizeXs
                                                wrapMode: Text.WordWrap
                                            }

                                            RowLayout {
                                                visible: root.privateRoomHistoryRepairFailedVisible()
                                                Layout.alignment: Qt.AlignRight
                                                spacing: Tokens.space2

                                                Button {
                                                    text: "Message someone"
                                                    Accessible.name: "Open People & Access to message someone about private room history"
                                                    onClicked: root.openPeopleAccessForPrivateRoomHelp()
                                                    ToolTip.visible: hovered
                                                    ToolTip.text: "Find someone who can help with this room"
                                                }

                                                Button {
                                                    text: "Copy help"
                                                    Accessible.name: "Copy private room repair help"
                                                    onClicked: root.copyPrivateRoomHelpNote()
                                                    ToolTip.visible: hovered
                                                    ToolTip.text: "Copy what to send to someone who can help with this room"
                                                }
                                            }
                                        }
                                    }

                                    ColumnLayout {
                                        visible: root.channelAccessToolsOpen
                                        Layout.fillWidth: true
                                        spacing: 4

                                        Text {
                                            Layout.fillWidth: true
                                            text: "Recent access"
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.DemiBold
                                            elide: Text.ElideRight
                                        }

                                        Repeater {
                                            model: root.channelAccessHistoryRows(
                                                3, root.channelAccessHistoryExpanded)

                                            delegate: ColumnLayout {
                                                id: channelAccessHistoryRow
                                                required property var modelData

                                                Layout.fillWidth: true
                                                spacing: 1

                                                Text {
                                                    Layout.fillWidth: true
                                                    text: String(channelAccessHistoryRow.modelData.title || "")
                                                    color: Tokens.textStrong
                                                    font.pixelSize: Tokens.fontSizeXs
                                                    elide: Text.ElideRight
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    text: String(channelAccessHistoryRow.modelData.detail || "")
                                                    color: Tokens.textMuted
                                                    font.pixelSize: Tokens.fontSizeXs
                                                    maximumLineCount: 2
                                                    elide: Text.ElideRight
                                                    wrapMode: Text.WordWrap
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    text: root.channelAccessHistoryActorText(channelAccessHistoryRow.modelData)
                                                    color: Tokens.textMuted
                                                    font.pixelSize: Tokens.fontSizeXs
                                                    elide: Text.ElideRight
                                                }
                                            }
                                        }

                                        Button {
                                            visible: root.selectedChannelAccessHistory.length > 3
                                            Layout.alignment: Qt.AlignLeft
                                            text: root.channelAccessHistoryToggleText(
                                                3, root.channelAccessHistoryExpanded)
                                            Accessible.name: text + " room access history"
                                            onClicked: root.channelAccessHistoryExpanded =
                                                !root.channelAccessHistoryExpanded
                                        }

                                        Text {
                                            visible: root.selectedChannelAccessHistory.length === 0
                                            Layout.fillWidth: true
                                            text: "No access changes loaded on this device yet."
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            wrapMode: Text.WordWrap
                                        }
                                    }

                                    RowLayout {
                                        visible: root.channelAccessToolsOpen
                                            && !root.privateRoomHistoryRepairFailedVisible()
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 2

                                            Text {
                                                Layout.fillWidth: true
                                                text: "History help"
                                                color: Tokens.textStrong
                                                font.pixelSize: Tokens.fontSizeXs
                                                font.weight: Font.DemiBold
                                                elide: Text.ElideRight
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: root.privateRoomHistoryHelpText()
                                                color: Tokens.textMuted
                                                font.pixelSize: Tokens.fontSizeXs
                                                maximumLineCount: 2
                                                elide: Text.ElideRight
                                                wrapMode: Text.WordWrap
                                            }
                                        }

                                        Button {
                                            text: "Copy help"
                                            Layout.preferredWidth: 96
                                            enabled: root.runtimeWorkReady
                                            Accessible.name: "Copy private room help"
                                            onClicked: root.copyPrivateRoomHelpNote()
                                            ToolTip.visible: hovered
                                            ToolTip.text: "Copy what to send to someone who can help with this room"
                                        }
                                    }

                                    RowLayout {
                                        visible: root.channelAccessToolsOpen
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 2

                                            Text {
                                                Layout.fillWidth: true
                                                text: "Protect new messages"
                                                color: Tokens.textStrong
                                                font.pixelSize: Tokens.fontSizeXs
                                                font.weight: Font.DemiBold
                                                elide: Text.ElideRight
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: root.privateRoomKeyRefreshText()
                                                color: Tokens.textMuted
                                                font.pixelSize: Tokens.fontSizeXs
                                                wrapMode: Text.WordWrap
                                            }
                                        }

                                        Button {
                                            text: "Protect"
                                            Layout.preferredWidth: 82
                                            enabled: root.selectedChannelCanRefreshKey
                                            Accessible.name: "Protect new private-room messages"
                                            onClicked: root.confirmRefreshSelectedPrivateRoomKey()
                                            ToolTip.visible: hovered
                                            ToolTip.text: enabled
                                                ? "Protect future messages in this private room"
                                                : root.privateRoomKeyRefreshUnavailableReason()
                                        }
                                    }

                                    ListView {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: Math.min(104, contentHeight)
                                        clip: true
                                        interactive: contentHeight > height
                                        spacing: 4
                                        model: root.selectedChannelAccessMembers

                                        delegate: RowLayout {
                                            id: channelAccessMemberRow
                                            required property var modelData

                                            width: ListView.view.width
                                            height: 32
                                            spacing: Tokens.space2

                                            ColumnLayout {
                                                Layout.fillWidth: true
                                                spacing: 0

                                                Text {
                                                    Layout.fillWidth: true
                                                    text: String(channelAccessMemberRow.modelData.displayLabel || "")
                                                    color: Tokens.textStrong
                                                    font.pixelSize: Tokens.fontSizeXs
                                                    font.weight: Font.DemiBold
                                                    elide: Text.ElideRight
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    text: String(channelAccessMemberRow.modelData.roleLabel || "")
                                                    visible: text.length > 0
                                                    color: Tokens.textMuted
                                                    font.pixelSize: Tokens.fontSizeXs
                                                    elide: Text.ElideRight
                                                }
                                            }

                                            Button {
                                                visible: root.canManageWorkspaceAccess()
                                                    && String(channelAccessMemberRow.modelData.deviceId || "") !== String(chaftController.deviceId || "")
                                                text: "Remove"
                                                Layout.preferredWidth: 74
                                                enabled: root.runtimeWorkReady
                                                    && root.selectedChannelKey.length > 0
                                                onClicked: root.confirmRevokeSelectedChannelMember(
                                                    channelAccessMemberRow.modelData.deviceId,
                                                    channelAccessMemberRow.modelData.displayLabel)
                                                ToolTip.visible: hovered
                                                ToolTip.text: "Remove private-room access"
                                            }
                                        }
                                    }

                                    RowLayout {
                                        visible: root.canManageWorkspaceAccess()
                                            && root.selectedChannelGrantCandidates.length > 0
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        ComboBox {
                                            id: channelAccessGrantCombo
                                            Layout.fillWidth: true
                                            model: root.selectedChannelGrantCandidates
                                            textRole: "grantDisplayLabel"
                                            enabled: root.runtimeWorkReady
                                            readonly property var selectedCandidate:
                                                currentIndex >= 0
                                                    ? (root.selectedChannelGrantCandidates[currentIndex] || ({}))
                                                    : ({})
                                            Accessible.name: "Person to add to private room"
                                            Accessible.description: String(
                                                selectedCandidate.supportLabel || "")
                                            ToolTip.visible: hovered
                                                && String(selectedCandidate.supportLabel || "").length > 0
                                            ToolTip.text: String(selectedCandidate.supportLabel || "")
                                        }

                                        Button {
                                            text: "Add"
                                            Layout.preferredWidth: 58
                                            enabled: root.runtimeWorkReady
                                                && channelAccessGrantCombo.currentIndex >= 0
                                            onClicked: root.grantSelectedPrivateRoomAccess(channelAccessGrantCombo)
                                            ToolTip.visible: hovered && !enabled
                                            ToolTip.text: "Choose a person first"
                                        }
                                    }

                                    Text {
                                        visible: root.canManageWorkspaceAccess()
                                            && root.selectedChannelGrantCandidates.length === 0
                                        Layout.fillWidth: true
                                        text: root.memberCount > root.members.length
                                            ? "Load more people to add someone else."
                                            : "Everyone shown here already has access."
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }

                                    Button {
                                        visible: root.canManageWorkspaceAccess()
                                            && root.memberCount > root.members.length
                                        Layout.fillWidth: true
                                        text: "Load more people"
                                        enabled: root.runtimeWorkReady
                                        onClicked: chaftController.loadMoreMembers()
                                    }

                                    Text {
                                        visible: !root.canManageWorkspaceAccess()
                                        Layout.fillWidth: true
                                        text: root.workspaceAccessUnavailableReason()
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }
                                }

                                Button {
                                    id: roomActionsButton
                                    visible: !root.selectedChannelDirectMessage
                                        && channelDetailsPopup.viewMode === "details"
                                    Layout.alignment: Qt.AlignLeft
                                    text: "Room actions"
                                    Accessible.name: "Room actions"
                                    onClicked: roomActionsMenu.open()

                                    Menu {
                                        id: roomActionsMenu
                                        y: roomActionsButton.height

                                        MenuItem {
                                            text: root.selectedChannelArchived
                                                ? "Restore room"
                                                : "Archive room"
                                            onTriggered: {
                                                if (root.toggleSelectedChannelArchived()) {
                                                    channelDetailsPopup.close()
                                                }
                                            }
                                        }

                                        MenuSeparator {
                                            visible: root.selectedChannelCanLeave
                                        }

                                        MenuItem {
                                            visible: root.selectedChannelCanLeave
                                            text: "Leave room"
                                            onTriggered: {
                                                channelDetailsPopup.close()
                                                root.confirmLeaveSelectedPrivateRoom()
                                            }
                                        }
                                    }
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: Tokens.space2

                                    Item {
                                        Layout.fillWidth: true
                                    }

                                    Button {
                                        text: channelDetailsPopup.viewMode === "details"
                                            ? "Cancel"
                                            : "Close"
                                        Layout.preferredWidth: 72
                                        onClicked: channelDetailsPopup.close()
                                    }

                                    Button {
                                        visible: channelDetailsPopup.viewMode === "details"
                                        text: "Save"
                                        Layout.preferredWidth: 72
                                        enabled: channelDetailsNameField.text.trim().length > 0
                                        onClicked: channelDetailsPopup.saveFromForm()
                                    }
                                }
                            }
                        }
                        }

                        Item {
                            id: syncControlsContainer
                            Layout.fillWidth: true
                            Layout.preferredHeight: channelHeaderPanel.syncControlsExpandedHeight
                            visible: chaftController.hasRuntimeWorkspace && root.syncDrawerOpen
                            clip: true

                            Flow {
                                id: syncControlsFlow
                                x: 18
                                y: 9
                                width: Math.max(0, syncControlsContainer.width - 36)
                                spacing: 8

                                Text {
                                    width: syncControlsFlow.width
                                    height: implicitHeight
                                    text: root.syncDrawerSummaryText()
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeSm
                                    wrapMode: Text.WordWrap
                                }

                                Button {
                                    visible: root.syncAdvancedToolsOpen
                                    enabled: !chaftController.peerHostingInFlight
                                        && (chaftController.peerHosting || root.runtimeWorkReady)
                                    text: chaftController.peerHosting ? "Stop" : "Host direct address"
                                    onClicked: {
                                        if (chaftController.peerHosting) {
                                            chaftController.stopLocalPeer()
                                        } else {
                                            var listenAddress = localPeerListenField.text.trim()
                                            chaftController.startLocalPeer(
                                                listenAddress.length > 0
                                                    ? listenAddress
                                                    : "127.0.0.1:0")
                                        }
                                    }
                                    ToolTip.visible: hovered
                                    ToolTip.text: chaftController.peerHosting
                                        ? "Stop hosting this address"
                                        : "Start a local listener. The default loopback address works only on this device; enter a LAN address to share with another device."
                                }

                                Button {
                                    visible: root.syncAdvancedToolsOpen
                                        && !chaftController.peerHosting
                                    text: "Use public relay"
                                    enabled: root.runtimeWorkReady
                                        && !chaftController.peerHostingInFlight
                                    onClicked: chaftController.startLocalIrohPeer()
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Use encrypted public relay for this hosted address. "
                                        + "Discovery remains subject to operator policy."
                                }

                                Button {
                                    visible: root.syncAdvancedToolsOpen
                                        && !chaftController.peerHosting
                                    text: root.customReachableAddressOpen
                                        ? "Hide advanced"
                                        : "Advanced address"
                                    checkable: true
                                    checked: root.customReachableAddressOpen
                                    onToggled: root.customReachableAddressOpen = checked
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Show manual connection address"
                                }

                                TextField {
                                    id: localPeerListenField
                                    visible: root.syncAdvancedToolsOpen
                                        && root.customReachableAddressOpen
                                        && !chaftController.peerHosting
                                    width: 132
                                    enabled: !chaftController.peerHostingInFlight
                                    placeholderText: "Optional address"
                                    Accessible.name: "Custom connection address"
                                    color: Tokens.textStrong
                                    placeholderTextColor: Tokens.textMuted
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Blank uses 127.0.0.1:0 (this device only). Enter a LAN listen address to host for another device."
                                    background: Rectangle {
                                        radius: Tokens.radiusMd
                                        color: Tokens.surfaceRaised
                                        border.color: Tokens.borderSubtle
                                    }
                                }

                                Text {
                                    width: 132
                                    height: 30
                                    visible: root.syncAdvancedToolsOpen
                                        && chaftController.peerHosting
                                    text: chaftController.hostedPeerEndpoint
                                    color: Tokens.textMuted
                                    font.family: Tokens.fontMono
                                    font.pixelSize: Tokens.fontSizeSm
                                    elide: Text.ElideMiddle
                                }

                                Button {
                                    visible: root.syncAdvancedToolsOpen
                                        && chaftController.peerHosting
                                        && chaftController.hostedPeerEndpoint.length > 0
                                    text: "Copy"
                                    width: 58
                                    onClicked: root.copyTextToClipboard(
                                        chaftController.hostedPeerEndpoint,
                                        "sharing address"
                                    )
                                }

                                StatusChip {
                                    visible: root.publishQueueIssueCount > 0
                                        || root.publishQueueError.length > 0
                                        || root.syncAdvancedToolsOpen
                                    text: root.publishQueueStatusText()
                                    description: root.publishQueueDetailText()
                                    warning: root.publishQueueIssueCount > 0 || root.publishQueueError.length > 0
                                    secure: false
                                    maxWidth: 260
                                }

                                StatusChip {
                                    visible: root.storageHealthKnown
                                        && (root.storageHealthHasIssue || root.syncAdvancedToolsOpen)
                                    text: root.storageHealthStatusText()
                                    description: root.storageHealthDetailText()
                                    warning: root.storageHealthHasIssue
                                }

                                Button {
                                    visible: root.storageMetadataRepairSuggested
                                    enabled: root.runtimeWorkReady
                                        && !root.workspaceOperationInFlight
                                    text: "Fix history"
                                    onClicked: root.repairStorageMetadata()
                                }

                                Text {
                                    visible: root.syncAdvancedToolsOpen
                                    width: 116
                                    height: 30
                                    text: "Teammate address"
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    verticalAlignment: Text.AlignVCenter
                                }

                                TextField {
                                    id: peerEndpointField
                                    width: 220
                                    placeholderText: "Paste a teammate address"
                                    color: Tokens.textStrong
                                    placeholderTextColor: Tokens.textMuted
                                    Component.onCompleted: text = chaftController.defaultPeerEndpoint
                                    onTextEdited: root.peerEndpointFormError = ""
                                    onAccepted: root.connectPeerEndpointFromField()
                                    Accessible.name: "Teammate address"
                                    Accessible.description: root.peerEndpointFormError
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

                                Button {
                                    text: "Save address"
                                    enabled: root.peerEndpointFormIsValid()
                                        && root.peerEndpointFormValue()
                                            !== String(chaftController.defaultPeerEndpoint || "").trim()
                                    onClicked: root.savePeerEndpointFromField()
                                    ToolTip.visible: hovered && !enabled
                                    ToolTip.text: root.peerEndpointFormIsValid()
                                        ? "This address is already saved"
                                        : "Enter a valid teammate address"
                                }

                                Text {
                                    visible: root.peerEndpointFormError.length > 0
                                    width: 220
                                    height: visible ? 30 : 0
                                    text: root.peerEndpointFormError
                                    color: Tokens.warningText
                                    font.pixelSize: Tokens.fontSizeXs
                                    verticalAlignment: Text.AlignVCenter
                                }

                                PeerRouteChip {
                                    visible: root.syncAdvancedToolsOpen
                                        || root.activePeerRouteIsWarning()
                                    label: root.activePeerRouteLabel()
                                    detail: root.activePeerRouteDetail()
                                    warning: root.activePeerRouteIsWarning()
                                }

                                CheckBox {
                                    id: liveUpdatesCheckBox
                                    enabled: root.autoSyncEnabled
                                        || (root.runtimeWorkReady
                                            && root.peerEndpointFormIsValid())
                                    text: "Automatic updates"
                                    checked: root.autoSyncEnabled
                                    onToggled: root.autoSyncEnabled = checked
                                    ToolTip.visible: hovered
                                    ToolTip.text: root.peerEndpointFormIsValid()
                                        ? "Periodically update from the saved teammate address"
                                        : root.autoSyncEnabled
                                            ? "Turn off automatic updates or enter a valid teammate address"
                                            : "Enter a valid teammate address before enabling automatic updates"
                                }

                                Button {
                                    enabled: root.runtimeWorkReady
                                        && !root.workspaceOperationInFlight
                                        && root.peerEndpointFormIsValid()
                                    text: chaftController.syncInFlight
                                        ? "Updating..."
                                        : "Update now"
                                    onClicked: root.connectPeerEndpointFromField()
                                }

                                Button {
                                    id: syncMaintenanceButton
                                    visible: root.syncAdvancedToolsOpen
                                    text: "Maintenance"
                                    Accessible.name: "Sync maintenance actions"
                                    onClicked: syncMaintenanceMenu.open()

                                    Menu {
                                        id: syncMaintenanceMenu
                                        y: syncMaintenanceButton.height

                                        MenuItem {
                                            text: "Share history"
                                            enabled: root.runtimeWorkReady
                                                && !root.workspaceOperationInFlight
                                                && root.preferredSyncPeerEndpoint().length > 0
                                            onTriggered: root.publishWorkspaceToPreferredPeer()
                                        }

                                        MenuItem {
                                            text: "Back up now"
                                            enabled: root.runtimeWorkReady
                                                && !root.workspaceOperationInFlight
                                                && root.preferredManualBackupPeerEndpoint().length > 0
                                            onTriggered: root.backupWorkspaceToPreferredPeer()
                                        }

                                        MenuItem {
                                            text: "Retry files"
                                            enabled: root.runtimeWorkReady
                                                && !root.workspaceOperationInFlight
                                                && (root.preferredRetryPeerEndpoint().length > 0
                                                    || (chaftController.backupPeerEndpoints || []).length > 0)
                                            onTriggered: root.retryBlobTransfersWithPreferredPeers()
                                        }

                                        MenuItem {
                                            text: "Fetch history"
                                            enabled: root.runtimeWorkReady
                                                && !root.workspaceOperationInFlight
                                                && root.preferredSyncPeerEndpoint().length > 0
                                            onTriggered: root.pullWorkspaceFromPreferredPeer()
                                        }

                                        MenuSeparator {}

                                        MenuItem {
                                            text: "Clean up local files"
                                            enabled: root.runtimeWorkReady
                                                && !root.workspaceOperationInFlight
                                            onTriggered: chaftController.pruneBlobs()
                                        }
                                    }
                                }

                                Button {
                                    text: root.syncAdvancedToolsOpen
                                        ? "Hide options"
                                        : "More options"
                                    checkable: true
                                    checked: root.syncAdvancedToolsOpen
                                    onToggled: {
                                        root.syncAdvancedToolsOpen = checked
                                        if (!checked) {
                                            root.customReachableAddressOpen = false
                                        }
                                    }
                                    Accessible.name: text
                                }
                            }
                        }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        visible: root.joinWaitingForPeerBannerVisible
                        implicitHeight: firstSyncWaitingRow.implicitHeight + Tokens.space3 * 2
                        radius: Tokens.radiusSm
                        color: Tokens.surfaceRaised
                        border.width: 1
                        border.color: Tokens.borderSubtle

                        RowLayout {
                            id: firstSyncWaitingRow
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: Tokens.space3
                            spacing: Tokens.space3

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: root.firstSyncWaitingTitleText()
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: root.firstSyncWaitingDetailText()
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                }
                            }

                            Button {
                                text: root.firstSyncWaitingActionLabel()
                                Layout.preferredWidth: 112
                                variant: "primary"
                                enabled: root.runtimeWorkReady
                                    && (root.preferredSyncPeerEndpoint().length === 0
                                        || !root.workspaceOperationInFlight)
                                onClicked: root.handleFirstSyncWaitingAction()
                            }

                            Button {
                                id: firstSyncWaitingMoreButton
                                text: "More"
                                Layout.preferredWidth: 72
                                Accessible.name: "More history options"
                                onClicked: firstSyncWaitingMenu.open()

                                Menu {
                                    id: firstSyncWaitingMenu
                                    y: firstSyncWaitingMoreButton.height

                                    MenuItem {
                                        text: "Copy help note"
                                        enabled: root.runtimeAccessReady
                                        onTriggered: root.copyFirstSyncWaitingHelpNote()
                                    }

                                    MenuItem {
                                        text: "Hide reminder"
                                        onTriggered: root.confirmHideFirstSyncWaiting()
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
                        enabled: !root.workspaceOperationInFlight
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
                    workspaceId: root.currentWorkspaceId()
                    emptyText: root.selectedTimelineEmptyText()
                    actionsEnabled: root.runtimeWorkReady
                        && !root.composerOperationPending
                    historyRepairEnabled: root.runtimeWorkReady
                        && !root.workspaceOperationInFlight
                        && root.preferredSyncPeerEndpoint().length > 0
                    historyRepairHasAddress: root.preferredSyncPeerEndpoint().length > 0
                    historyRepairBusy: chaftController.syncInFlight
                    autoFollowLatest: root.normalizedSearchQuery.length === 0
                    openReactionPickerOnLoad: String(chaftController.smokeUiState || "") === "reaction-picker"
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
                    onReplyParentRequested: function(messageId) {
                        root.openTimelineMessageById(messageId)
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
                    onExternalLinkRequested: function(link) {
                        root.requestExternalLink(link)
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
                    onCryptoBadgeRequested: function (title, message) {
                        root.showInfo(title, message)
                    }
                }

                ComposerBar {
                    id: composer
                    visible: root.hasWorkspaceContent
                    Layout.fillWidth: true
                    channelName: root.selectedChannelDisplayName
                    directMessage: root.selectedChannelDirectMessage
                    editMode: root.editingMessageId.length > 0
                    replyMode: root.replyTargetMessageId.length > 0
                    replyLabel: root.replyTargetMessageId.length > 0
                        ? root.replyTargetLabel(root.replyTarget)
                        : ""
                    replyAvatarId: String(root.replyTarget.authorAvatarId || "")
                    replyWorkspaceId: root.currentWorkspaceId()
                    replyIdentityId: String(root.replyTarget.authorDeviceId || "")
                    replyDisplayName: root.itemAuthorLabel(root.replyTarget)
                    operationPending: root.composerOperationPending
                    enabled: root.runtimeWorkReady && root.selectedChannelKey.length > 0
                    onDraftChanged: function(text) {
                        if (!root.suppressComposerDraftSave) {
                            root.saveSelectedDraftText(text)
                        }
                    }
                    onSendRequested: function(text) {
                        if (root.composerOperationPending) {
                            return
                        }
                        var workspaceId = root.currentWorkspaceId()
                        var channelId = root.selectedChannelKey
                        var replyToMessageId = root.replyTargetMessageId
                        var sent = root.replyTargetMessageId.length > 0
                            ? chaftController.sendMessageReply(
                                channelId,
                                replyToMessageId,
                                text
                            )
                            : chaftController.sendMessage(channelId, text)
                        if (sent) {
                            root.beginComposerOperation(
                                "message",
                                workspaceId,
                                channelId,
                                replyToMessageId,
                                "",
                                text,
                                "")
                        }
                    }
                    onAttachRequested: function(text) {
                        if (root.composerOperationPending) {
                            return
                        }
                        attachmentDialog.pendingText = text
                        attachmentDialog.pendingWorkspaceId = root.currentWorkspaceId()
                        attachmentDialog.pendingChannelId = root.selectedChannelKey
                        attachmentDialog.pendingReplyToMessageId = root.replyTargetMessageId
                        attachmentDialog.open()
                    }
                    onSaveEditRequested: function(text) {
                        if (root.composerOperationPending) {
                            return
                        }
                        var workspaceId = root.currentWorkspaceId()
                        var messageId = root.editingMessageId
                        if (chaftController.editMessage(messageId, text)) {
                            root.beginComposerOperation(
                                "edit",
                                workspaceId,
                                "",
                                "",
                                messageId,
                                text,
                                "")
                        }
                    }
                    onCancelEditRequested: root.cancelEditMessage()
                    onCancelReplyRequested: root.cancelReplyMessage()
                }
            }

            SettingsView {
                id: setupPanel
                anchors.fill: parent
                visible: !root.conversationDestination
                    && (chaftController.deviceId.length > 0
                        || chaftController.hasRuntimeWorkspace)
                app: root
                destination: root.mainDestination
                category: root.settingsCategory
                onCloseRequested: root.closeMainDestination(true)
                onCategoryRequested: function(categoryId) {
                    root.openSettings(categoryId)
                }
                onPeopleAccessRequested: function(focusInvite) {
                    root.openPeopleAccess(focusInvite)
                }
            }
        }

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: root.inspectorPanelWidth
            Layout.minimumWidth: visible ? root.inspectorPanelWidth : 0
            visible: root.conversationDestination
                && root.hasWorkspaceContent
                && ((root.pinnedInspectorAvailable && chaftController.inspectorPinned)
                    || root.inspectorItemKey.length > 0)
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
                    id: inspectorContentItem
                    width: inspectorScroll.availableWidth
                    height: inspectorColumn.implicitHeight + 28

                    ColumnLayout {
                        id: inspectorColumn
                        x: 14
                        y: 14 - root.inspectorSmokeScrollOffsetY
                        width: Math.max(0, parent.width - 28)
                        spacing: 14

                        ColumnLayout {
                            id: inspectorPeopleSectionHeader
                            Layout.fillWidth: true
                            spacing: 8

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    Text {
                                        Layout.fillWidth: true
                                        text: root.selectedChannelDisplayName.length > 0
                                            ? (root.selectedChannelDirectMessage ? "@ " : "# ")
                                                + root.selectedChannelDisplayName
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

                                    Text {
                                        visible: root.selectedChannelTopic.length > 0
                                            && !root.selectedChannelDirectMessage
                                        Layout.fillWidth: true
                                        text: root.selectedChannelTopic
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        elide: Text.ElideRight
                                        maximumLineCount: 2
                                        wrapMode: Text.WordWrap
                                    }
                                }

                                Button {
                                    visible: root.pinnedInspectorAvailable
                                    text: chaftController.inspectorPinned ? "Unpin" : "Pin"
                                    Layout.preferredWidth: 64
                                    Accessible.name: chaftController.inspectorPinned
                                        ? "Unpin inspector"
                                        : "Pin inspector"
                                    onClicked: chaftController.inspectorPinned = !chaftController.inspectorPinned
                                    ToolTip.visible: hovered
                                    ToolTip.text: chaftController.inspectorPinned
                                        ? "Inspector stays open; unpin to open only on selection"
                                        : "Keep the inspector open even with nothing selected"
                                }

                                Button {
                                    visible: root.inspectorItemKey.length > 0
                                    text: "Close"
                                    Layout.preferredWidth: 64
                                    Accessible.name: "Close message details"
                                    onClicked: root.inspectorItemKey = ""
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Button {
                                    visible: root.runtimeWorkReady
                                        && root.selectedChannelKey.length > 0
                                        && !root.selectedChannelDirectMessage
                                    text: "Edit"
                                    Layout.fillWidth: true
                                    Accessible.name: "Edit room details"
                                    onClicked: channelDetailsPopup.open()
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Edit room name and topic"
                                }

                                Button {
                                    visible: root.runtimeWorkReady
                                        && root.selectedChannelKey.length > 0
                                    text: root.selectedChannelMuted ? "Unmute" : "Mute"
                                    Layout.fillWidth: true
                                    Accessible.name: root.selectedMuteAccessibleName()
                                    onClicked: root.toggleSelectedChannelMuted()
                                    ToolTip.visible: hovered
                                    ToolTip.text: root.selectedMuteTooltip()
                                }

                                Rectangle {
                                    Layout.preferredWidth: Math.max(72, channelKindText.implicitWidth + 18)
                                    Layout.preferredHeight: 32
                                    radius: Tokens.radiusSm
                                    color: root.selectedChannelArchived
                                        ? Tokens.surfaceRaised
                                        : (root.selectedChannelPrivate ? Tokens.secureSurface : Tokens.surfaceBase)
                                    border.color: Tokens.borderSubtle

                                    Text {
                                        id: channelKindText
                                        anchors.centerIn: parent
                                        text: root.selectedChannelArchived
                                            ? "Archived"
                                            : (root.selectedChannelDirectMessage
                                                ? "Direct"
                                                : (root.selectedChannelPrivate ? "Private" : "Open"))
                                        color: root.selectedChannelPrivate && !root.selectedChannelArchived
                                            ? Tokens.secure
                                            : Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }
                        }

                        GridLayout {
                            Layout.fillWidth: true
                            visible: root.inspectorItemKey.length === 0
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
                                        text: "Loaded messages"
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
                                        text: "People"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String(root.selectedChannelMemberCount)
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
                                        text: "Needs review"
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
                                        text: "Recent files"
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
                            visible: root.selectedChannelPrivate
                                && !root.selectedChannelDirectMessage
                            Layout.fillWidth: true
                            implicitHeight: inspectorPrivateRoomReadabilityLayout.implicitHeight
                                + Tokens.space3 * 2
                            radius: Tokens.radiusSm
                            color: root.selectedChannelLockedMessageCount > 0
                                ? Tokens.warningSurface
                                : (root.selectedChannelLoadedMessageCount > 0
                                    ? Tokens.secureSurface
                                    : Tokens.surfaceBase)
                            border.width: 1
                            border.color: root.selectedChannelLockedMessageCount > 0
                                ? Tokens.warning
                                : (root.selectedChannelLoadedMessageCount > 0
                                    ? Tokens.secure
                                    : Tokens.borderSubtle)

                            Accessible.role: Accessible.StaticText
                            Accessible.name: root.privateRoomReadabilityTitle()
                                + ". "
                                + root.privateRoomReadabilityText()

                            ColumnLayout {
                                id: inspectorPrivateRoomReadabilityLayout
                                anchors.fill: parent
                                anchors.margins: Tokens.space3
                                spacing: Tokens.space1

                                Text {
                                    Layout.fillWidth: true
                                    text: root.privateRoomReadabilityTitle()
                                    color: root.selectedChannelLockedMessageCount > 0
                                        ? Tokens.warningText
                                        : (root.selectedChannelLoadedMessageCount > 0
                                            ? Tokens.secure
                                            : Tokens.textStrong)
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                    wrapMode: Text.WordWrap
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: root.privateRoomReadabilityText()
                                    color: root.selectedChannelLockedMessageCount > 0
                                        ? Tokens.warningText
                                        : Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                }

                                RowLayout {
                                    visible: root.privateRoomHistoryRepairActionVisible()
                                    Layout.alignment: Qt.AlignRight
                                    Layout.topMargin: Tokens.space1
                                    spacing: Tokens.space2

                                    Button {
                                        visible: root.privateRoomHistoryRepairChangeAddressVisible()
                                        text: "Change address"
                                        Accessible.name: "Change teammate address for private room history"
                                        onClicked: root.focusPeerAddressField()
                                        ToolTip.visible: hovered
                                        ToolTip.text: "Paste a different teammate address"
                                    }

                                    Button {
                                        text: root.privateRoomHistoryRepairActionLabel()
                                        enabled: root.privateRoomHistoryRepairActionEnabled()
                                        Accessible.name: text + " for private room history"
                                        onClicked: root.handlePrivateRoomHistoryRepairAction()
                                        ToolTip.visible: hovered
                                        ToolTip.text: root.privateRoomHistoryRepairActionTooltip()
                                    }
                                }

                                Text {
                                    visible: root.privateRoomHistoryRepairFailedVisible()
                                    Layout.fillWidth: true
                                    text: root.privateRoomHistoryRepairFailedText()
                                    color: Tokens.warningText
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                }

                                RowLayout {
                                    visible: root.privateRoomHistoryRepairFailedVisible()
                                    Layout.alignment: Qt.AlignRight
                                    spacing: Tokens.space2

                                    Button {
                                        text: "Message someone"
                                        Accessible.name: "Open People & Access to message someone about private room history"
                                        onClicked: root.openPeopleAccessForPrivateRoomHelp()
                                        ToolTip.visible: hovered
                                        ToolTip.text: "Find someone who can help with this room"
                                    }

                                    Button {
                                        text: "Copy help"
                                        Accessible.name: "Copy private room repair help"
                                        onClicked: root.copyPrivateRoomHelpNote()
                                        ToolTip.visible: hovered
                                        ToolTip.text: "Copy what to send to someone who can help with this room"
                                    }
                                }
                            }
                        }

                        ColumnLayout {
                            visible: root.selectedChannelPrivate
                                && !root.selectedChannelDirectMessage
                            Layout.fillWidth: true
                            spacing: 4

                            Text {
                                Layout.fillWidth: true
                                text: "Recent access"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Repeater {
                                model: root.channelAccessHistoryRows(
                                    2, root.inspectorAccessHistoryExpanded)

                                delegate: ColumnLayout {
                                    id: inspectorAccessHistoryRow
                                    required property var modelData

                                    Layout.fillWidth: true
                                    spacing: 1

                                    Text {
                                        Layout.fillWidth: true
                                        text: String(inspectorAccessHistoryRow.modelData.title || "")
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXs
                                        elide: Text.ElideRight
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: String(inspectorAccessHistoryRow.modelData.detail || "")
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        maximumLineCount: 2
                                        elide: Text.ElideRight
                                        wrapMode: Text.WordWrap
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: root.channelAccessHistoryActorText(inspectorAccessHistoryRow.modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        elide: Text.ElideRight
                                    }
                                }
                            }

                            Button {
                                visible: root.selectedChannelAccessHistory.length > 2
                                Layout.alignment: Qt.AlignLeft
                                text: root.channelAccessHistoryToggleText(
                                    2, root.inspectorAccessHistoryExpanded)
                                Accessible.name: text + " room access history"
                                onClicked: root.inspectorAccessHistoryExpanded =
                                    !root.inspectorAccessHistoryExpanded
                            }

                            Text {
                                visible: root.selectedChannelAccessHistory.length === 0
                                Layout.fillWidth: true
                                text: "No access changes loaded here yet."
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }
                        }

                        RowLayout {
                            visible: root.selectedChannelPrivate
                                && !root.selectedChannelDirectMessage
                                && !root.privateRoomHistoryRepairFailedVisible()
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: "History help"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: root.privateRoomHistoryHelpText()
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    maximumLineCount: 2
                                    elide: Text.ElideRight
                                    wrapMode: Text.WordWrap
                                }
                            }

                            Button {
                                text: "Copy"
                                Layout.preferredWidth: 82
                                enabled: root.runtimeWorkReady
                                Accessible.name: "Copy private room help"
                                onClicked: root.copyPrivateRoomHelpNote()
                                ToolTip.visible: hovered
                                ToolTip.text: "Copy what to send to someone who can help with this room"
                            }
                        }

                        RowLayout {
                            visible: root.selectedChannelPrivate
                                && !root.selectedChannelDirectMessage
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: "Protect new messages"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: root.privateRoomKeyRefreshText()
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    maximumLineCount: 2
                                    elide: Text.ElideRight
                                    wrapMode: Text.WordWrap
                                }
                            }

                            Button {
                                text: "Protect"
                                Layout.preferredWidth: 82
                                enabled: root.selectedChannelCanRefreshKey
                                Accessible.name: "Protect new private-room messages"
                                onClicked: root.confirmRefreshSelectedPrivateRoomKey()
                                ToolTip.visible: hovered
                                ToolTip.text: enabled
                                    ? "Protect future messages in this private room"
                                    : root.privateRoomKeyRefreshUnavailableReason()
                            }
                        }

                        RowLayout {
                            visible: root.runtimeWorkReady
                                && root.selectedChannelKey.length > 0
                                && !root.selectedChannelDirectMessage
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: root.selectedChannelArchived
                                        ? "Archived room"
                                        : "Active room"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: root.selectedChannelArchived
                                        ? "Kept under Archived until restored."
                                        : "Visible in the Rooms list."
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }
                            }

                            Button {
                                text: root.selectedChannelArchived ? "Restore" : "Archive"
                                Layout.preferredWidth: 82
                                Accessible.name: root.selectedChannelArchived
                                    ? "Restore room"
                                    : "Archive room"
                                onClicked: root.toggleSelectedChannelArchived()
                                ToolTip.visible: hovered
                                ToolTip.text: root.selectedChannelArchived
                                    ? "Move this room back to Rooms"
                                    : "Move this room out of the active Rooms list"
                            }
                        }

                        RowLayout {
                            visible: root.selectedChannelCanLeave
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: "Room access"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: "Leave this room here."
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }
                            }

                            Button {
                                text: "Leave"
                                Layout.preferredWidth: 82
                                Accessible.name: "Leave private room"
                                onClicked: root.confirmLeaveSelectedPrivateRoom()
                                ToolTip.visible: hovered
                                ToolTip.text: "Remove your access to this private room"
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
                                color: root.inspectorItemIsSelected
                                    ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.16)
                                    : Tokens.surfaceBase
                                border.color: root.inspectorItemIsSelected
                                    ? Tokens.accent
                                    : Tokens.borderSubtle

                                Text {
                                    id: messageModeText
                                    anchors.centerIn: parent
                                    text: root.inspectorItemIsSelected ? "Selected" : "Latest"
                                    color: root.inspectorItemIsSelected ? Tokens.accent : Tokens.textMuted
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

                                    AvatarMark {
                                        Layout.preferredWidth: 32
                                        Layout.preferredHeight: 32
                                        warning: root.inspectorItem.kind === "missing_history_gap"
                                            || root.inspectorItem.kind === "invalid_signature"
                                        avatarId: String(root.inspectorItem.authorAvatarId || "")
                                        workspaceId: root.currentWorkspaceId()
                                        identityId: String(root.inspectorItem.authorDeviceId || "")
                                        displayName: root.itemAuthorLabel(root.inspectorItem)
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 2

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.inspectorItem.kind === "missing_history_gap"
                                                ? "History missing"
                                                : root.inspectorItem.kind === "invalid_signature"
                                                    ? "Security check failed"
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

                                GridLayout {
                                    Layout.fillWidth: true
                                    columns: 2
                                    columnSpacing: 6
                                    rowSpacing: 6

                                    Button {
                                        text: root.inspectorItem.bodyTruncated ? "Copy preview" : "Copy text"
                                        Layout.fillWidth: true
                                        Layout.columnSpan: 2
                                        enabled: root.inspectorBodyCopyText().length > 0
                                        onClicked: root.copyInspectorBody()
                                    }

                                    Button {
                                        text: "Copy support ID"
                                        Layout.fillWidth: true
                                        visible: root.inspectorDetailsOpen
                                        enabled: String(root.inspectorItem.eventId || "").length > 0
                                        onClicked: root.copyInspectorEventId()
                                    }

                                    Button {
                                        text: "Copy message support ID"
                                        Layout.fillWidth: true
                                        visible: root.inspectorDetailsOpen
                                            && String(root.inspectorItem.messageId || "").length > 0
                                        enabled: String(root.inspectorItem.messageId || "").length > 0
                                        onClicked: root.copyInspectorMessageId()
                                    }
                                }

                                Button {
                                    text: root.inspectorDetailsOpen
                                        ? "Hide support details"
                                        : "Support details"
                                    Layout.preferredWidth: 142
                                    onClicked: root.inspectorDetailsOpen = !root.inspectorDetailsOpen
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Technical message identifiers and counts"
                                }

                                GridLayout {
                                    Layout.fillWidth: true
                                    visible: root.inspectorDetailsOpen
                                    columns: 2
                                    columnSpacing: 10
                                    rowSpacing: 4

                                    Text {
                                        text: "Support ID"
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
                            workspaceId: root.currentWorkspaceId()
                            replyCount: root.inspectorThreadReplyCount
                            replyPreviews: root.inspectorThreadReplyPreviews
                            runtimeReady: root.runtimeWorkReady
                            messageId: String(root.inspectorItem.messageId || "")
                            messageDeleted: Boolean(root.inspectorItem.deleted)
                            onReplyRequested: root.beginReplyMessage(root.inspectorItem)
                            onReplySelected: function(messageId) {
                                root.openTimelineMessageById(messageId)
                            }
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
                                color: root.autoBackupEnabled
                                    ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.14)
                                    : Tokens.surfaceBase
                                border.color: root.autoBackupEnabled
                                    ? Tokens.accent
                                    : Tokens.borderSubtle

                                Text {
                                    id: backupAutoText
                                    anchors.centerIn: parent
                                    text: root.autoBackupEnabled ? "Auto" : "Manual"
                                    color: root.autoBackupEnabled ? Tokens.accent : Tokens.textMuted
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
                                        text: "Backup addresses"
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
                                        text: "This device"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: chaftController.peerHosting
                                            ? "Sharing address"
                                            : (chaftController.peerHostingInFlight ? "Updating" : "Not sharing")
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
                                        text: "Local events"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                    }

                                    Text {
                                        text: String(root.queuedPublishableEventCount)
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
                                color: root.publishQueueIssueCount > 0 ? Tokens.warningSurface : Tokens.surfaceBase
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 1

                                    Text {
                                        text: "Needs retry"
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
                                        text: "History"
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
                                && !root.workspaceOperationInFlight
                            text: "Fix history"
                            onClicked: root.repairStorageMetadata()
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            visible: root.peerEndpointHints.length > 0
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                text: "Shared addresses"
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
                                syncInFlight: root.workspaceOperationInFlight
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
                            text: "No backup addresses"
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
                                text: "People"
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

                        Button {
                            Layout.fillWidth: true
                            text: "Manage people"
                            onClicked: root.openPeopleAccess(false)
                        }

                        ListView {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.min(320, contentHeight)
                            visible: false
                            clip: true
                            interactive: contentHeight > height
                            spacing: 6
                            model: root.members

                            delegate: MemberRow {
                                id: memberRowDelegate
                                required property var modelData

                                width: ListView.view.width
                                deviceId: String(memberRowDelegate.modelData.deviceId || "")
                                avatarId: root.memberAvatarId(memberRowDelegate.modelData)
                                workspaceId: root.currentWorkspaceId()
                                displayLabel: root.memberLabel(memberRowDelegate.modelData)
                                initial: root.memberInitial(memberRowDelegate.modelData)
                                roleLabel: root.roleLabel(memberRowDelegate.modelData.role)
                                roleValue: root.normalizedRole(memberRowDelegate.modelData.role)
                                roleOptions: root.memberRoleOptions(memberRowDelegate.modelData)
                                owner: memberRowDelegate.modelData.role === "owner"
                                localDevice: memberRowDelegate.deviceId === chaftController.deviceId
                                canMessage: root.runtimeWorkReady
                                showRoleEditor: chaftController.hasRuntimeWorkspace
                                    && root.canManageWorkspaceAccess()
                                    && memberRowDelegate.deviceId !== chaftController.deviceId
                                canChangeRole: root.canChangeMemberRole(memberRowDelegate.modelData)
                                roleUnavailableReason: root.memberRoleUnavailableReason(memberRowDelegate.modelData)
                                showRemoveAction: chaftController.hasRuntimeWorkspace
                                    && memberRowDelegate.deviceId !== chaftController.deviceId
                                canRemove: root.canRemoveMember(memberRowDelegate.modelData)
                                removeUnavailableReason: root.memberRemovalUnavailableReason(
                                    memberRowDelegate.modelData)
                                onCopyDeviceRequested: function (deviceId) {
                                    root.copyTextToClipboard(deviceId, "support code")
                                }
                                onMessageRequested: function (deviceId, displayLabel) {
                                    root.startDirectMessage(deviceId, displayLabel)
                                }
                                onRoleChangeRequested: function (deviceId, role) {
                                    root.confirmMemberRoleChange(
                                        deviceId,
                                        memberRowDelegate.displayLabel,
                                        role)
                                }
                                onRemoveRequested: function (deviceId, displayLabel) {
                                    root.confirmMemberRemoval(deviceId, displayLabel)
                                }
                            }
                        }

                        Button {
                            Layout.fillWidth: true
                            visible: false
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
                                text: "Access changes"
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
                            text: "No access changes waiting"
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
                                canManageAccess: root.canManageWorkspaceAccess()
                                accessUnavailableReason: root.workspaceAccessUnavailableReason()
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
