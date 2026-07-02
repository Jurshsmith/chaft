import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

ListView {
    id: root
    property var timelineModel: []
    property string emptyText: "No messages yet"
    property bool actionsEnabled: false
    property bool historyRepairEnabled: false
    property bool autoFollowLatest: true
    property bool showChannelLabels: false
    property string selectedItemKey: ""
    property bool followLatest: true
    property bool pendingInitialScroll: true
    property bool pendingUnreadScroll: false
    property bool preservingPrepend: false
    property real preservedContentHeight: 0
    property real preservedContentY: 0
    property var quickReactions: ["+1", "ship", "eyes", "done"]
    readonly property bool showJumpToLatest: root.autoFollowLatest
        && root.count > 0
        && root.contentHeight > root.height + 48
        && !root.followLatest
        && !root.pendingInitialScroll
        && !root.pendingUnreadScroll
        && !root.preservingPrepend
    signal itemSelected(var item)
    signal reactionRequested(string messageId, string reaction)
    signal reactionRemoveRequested(string messageId, string reaction)
    signal replyRequested(var item)
    signal threadRequested(var item)
    signal editRequested(string messageId, string body)
    signal deleteRequested(string messageId)
    signal attachmentSaveRequested(string messageId, string attachmentSelector, string displayName)
    signal proofPublishRequested(string eventId)
    signal historyRepairRequested(string eventId)

    function authorLabel(authorDisplayName, authorDeviceId) {
        var displayName = String(authorDisplayName || "").trim()
        if (displayName.length > 0) {
            return displayName
        }
        return String(authorDeviceId || "")
    }

    function authorInitial(authorDisplayName, authorDeviceId) {
        var value = root.authorLabel(authorDisplayName, authorDeviceId)
        return value.length > 0 ? value.slice(0, 1).toUpperCase() : "?"
    }

    function timeLabel(physicalMs) {
        if (physicalMs === undefined || physicalMs === null) {
            return ""
        }
        var value = Number(physicalMs || 0)
        if (!isFinite(value) || value <= 0) {
            return ""
        }

        var date = new Date(value)
        if (isNaN(date.getTime())) {
            return ""
        }

        var now = new Date()
        if (date.getFullYear() === now.getFullYear()
                && date.getMonth() === now.getMonth()
                && date.getDate() === now.getDate()) {
            return Qt.formatDateTime(date, "HH:mm")
        }
        return Qt.formatDateTime(date, "MMM d HH:mm")
    }

    function channelLabel(channelName, channelId) {
        if (!root.showChannelLabels) {
            return ""
        }
        var name = String(channelName || "").trim()
        if (name.length > 0) {
            return "#" + name
        }

        var id = String(channelId || "").trim()
        return id.length > 0 ? "#" + id : ""
    }

    function compactInlineText(value, fallback) {
        var text = String(value || "").replace(/\s+/g, " ").trim()
        if (text.length === 0) {
            return fallback || ""
        }
        return text.length > 96 ? text.slice(0, 93) + "..." : text
    }

    function replyPreviewLabel(replyPreview) {
        var preview = replyPreview || {}
        var author = root.authorLabel(preview.authorDisplayName, preview.authorDeviceId)
        var body = root.compactInlineText(preview.body, "message")
        return author.length > 0 ? author + ": " + body : body
    }

    function threadReplyLabel(count) {
        var value = Number(count || 0)
        return value === 1 ? "1 reply" : String(value) + " replies"
    }

    function localReactionSet(myReactions) {
        var set = {}
        var source = myReactions || []
        for (var i = 0; i < source.length; i += 1) {
            var reaction = String(source[i] || "").trim()
            if (reaction.length > 0) {
                set[reaction] = true
            }
        }
        return set
    }

    function reactionEntries(reactions, myReactions) {
        var entries = []
        var source = reactions || {}
        var mine = root.localReactionSet(myReactions)
        for (var key in source) {
            if (Object.prototype.hasOwnProperty.call(source, key)) {
                entries.push({
                    reaction: key,
                    count: source[key],
                    mine: mine[key] === true
                })
            }
        }
        entries.sort(function(left, right) {
            return left.reaction.localeCompare(right.reaction)
        })
        return entries
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

    function reactionTotalCount(item) {
        var reactions = (item && item.reactions) || {}
        var visible = root.reactionKeyCount(reactions)
        var count = Number((item && item.reactionCount) === undefined
            ? visible
            : item.reactionCount)
        if (!isFinite(count)) {
            count = visible
        }
        return Math.max(visible, Math.max(0, count))
    }

    function reactionOverflowLabel(item) {
        var reactions = (item && item.reactions) || {}
        var hidden = root.reactionTotalCount(item) - root.reactionKeyCount(reactions)
        if (hidden <= 0) {
            return ""
        }
        return String(hidden) + (hidden === 1 ? " more reaction" : " more reactions")
    }

    function quickReactionEntries(myReactions) {
        var entries = []
        var existing = root.localReactionSet(myReactions)
        for (var i = 0; i < root.quickReactions.length; i += 1) {
            var reaction = String(root.quickReactions[i] || "").trim()
            if (reaction.length > 0
                    && !Object.prototype.hasOwnProperty.call(existing, reaction)) {
                entries.push(reaction)
            }
        }
        return entries
    }

    function attachmentEntries(attachments) {
        return attachments || []
    }

    function attachmentTotalCount(item) {
        var attachments = (item && item.attachments) || []
        var count = Number((item && item.attachmentCount) === undefined
            ? attachments.length
            : item.attachmentCount)
        if (!isFinite(count)) {
            count = attachments.length
        }
        return Math.max(attachments.length, Math.max(0, count))
    }

    function attachmentOverflowLabel(item) {
        var attachments = (item && item.attachments) || []
        var hidden = root.attachmentTotalCount(item) - attachments.length
        if (hidden <= 0) {
            return ""
        }
        return String(hidden) + (hidden === 1 ? " more file" : " more files")
    }

    function scrollToLatest() {
        if (root.count > 0) {
            root.positionViewAtEnd()
        }
        root.followLatest = true
        root.pendingInitialScroll = false
        root.pendingUnreadScroll = false
    }

    function scrollToOldest() {
        if (root.count > 0) {
            root.positionViewAtBeginning()
        }
        root.followLatest = false
        root.pendingInitialScroll = false
        root.pendingUnreadScroll = false
    }

    function isNearLatest() {
        return root.contentHeight <= root.height
            || root.contentY >= root.contentHeight - root.height - 48
    }

    function updateFollowLatestFromPosition() {
        if (root.autoFollowLatest
                && !root.pendingInitialScroll
                && !root.pendingUnreadScroll
                && !root.preservingPrepend) {
            root.followLatest = root.isNearLatest()
        }
    }

    function resetToLatestOnNextModel() {
        root.followLatest = true
        root.pendingInitialScroll = true
        root.pendingUnreadScroll = false
        if (root.autoFollowLatest) {
            Qt.callLater(function() {
                root.scrollToLatest()
            })
        }
    }

    function resetToBeginningOnNextModel() {
        root.followLatest = false
        root.pendingInitialScroll = false
        root.pendingUnreadScroll = false
        Qt.callLater(function() {
            root.positionViewAtBeginning()
        })
    }

    function unreadStartIndex() {
        var items = root.timelineModel || []
        for (var i = 0; i < items.length; i += 1) {
            if (Boolean(items[i].unreadDividerBefore)) {
                return i
            }
        }
        return -1
    }

    function scrollToUnreadStartOrLatest() {
        var unreadIndex = root.unreadStartIndex()
        if (unreadIndex >= 0) {
            root.positionViewAtIndex(unreadIndex, ListView.Beginning)
            root.followLatest = false
            root.pendingInitialScroll = false
            root.pendingUnreadScroll = false
            return
        }
        root.scrollToLatest()
    }

    function resetToUnreadOnNextModel() {
        root.followLatest = false
        root.pendingInitialScroll = false
        root.pendingUnreadScroll = true
        Qt.callLater(function() {
            root.scrollToUnreadStartOrLatest()
        })
    }

    function prepareForPrepend() {
        root.preservingPrepend = true
        root.preservedContentHeight = root.contentHeight
        root.preservedContentY = root.contentY
        root.followLatest = false
        root.pendingUnreadScroll = false
    }

    function cancelPrepend() {
        root.preservingPrepend = false
    }

    clip: true
    boundsBehavior: Flickable.StopAtBounds
    spacing: 0
    model: root.timelineModel

    Component.onCompleted: {
        if (root.autoFollowLatest) {
            Qt.callLater(function() {
                root.scrollToLatest()
            })
        }
    }

    onAutoFollowLatestChanged: {
        if (autoFollowLatest) {
            root.resetToLatestOnNextModel()
        } else {
            root.resetToBeginningOnNextModel()
        }
    }

    onCountChanged: {
        if (root.pendingUnreadScroll) {
            Qt.callLater(function() {
                root.scrollToUnreadStartOrLatest()
            })
        } else if (root.count > 0 && root.autoFollowLatest
                && !root.preservingPrepend
                && (root.pendingInitialScroll || root.followLatest)) {
            Qt.callLater(function() {
                root.scrollToLatest()
            })
        }
    }

    onContentHeightChanged: {
        if (root.preservingPrepend) {
            var delta = root.contentHeight - root.preservedContentHeight
            if (delta > 0) {
                root.contentY = root.preservedContentY + delta
            }
            root.preservingPrepend = false
        } else if (root.pendingUnreadScroll) {
            Qt.callLater(function() {
                root.scrollToUnreadStartOrLatest()
            })
        } else if (root.count > 0 && root.autoFollowLatest
                && (root.pendingInitialScroll || root.followLatest)) {
            Qt.callLater(function() {
                root.scrollToLatest()
            })
        }
    }

    onContentYChanged: root.updateFollowLatestFromPosition()

    onMovementEnded: {
        root.updateFollowLatestFromPosition()
    }

    Text {
        anchors.centerIn: parent
        visible: root.timelineModel.length === 0
        text: root.emptyText
        color: Tokens.textMuted
        font.pixelSize: 14
    }

    Rectangle {
        id: latestJump
        visible: root.showJumpToLatest
        z: 20
        x: Math.max(18, root.width - width - 22)
        y: root.contentY + root.height - height - 18
        width: Math.max(132, latestJumpLabel.implicitWidth + 32)
        height: 34
        radius: Tokens.radiusMd
        color: latestJumpMouse.containsMouse ? Tokens.secureSurface : Tokens.surfaceRaised
        border.color: Tokens.accent
        border.width: 1

        Text {
            id: latestJumpLabel
            anchors.centerIn: parent
            text: "Jump to latest"
            color: Tokens.accent
            font.pixelSize: 12
            font.weight: Font.DemiBold
        }

        MouseArea {
            id: latestJumpMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.scrollToLatest()
        }

        ToolTip.visible: latestJumpMouse.containsMouse
        ToolTip.text: "Scroll to newest messages"
    }

    delegate: Rectangle {
        id: row
        width: root.width
        readonly property string rowMessageId: String(modelData.messageId || "")
        readonly property string rowEventId: String(modelData.eventId || "")
        readonly property string rowItemKey: row.rowMessageId.length > 0 ? row.rowMessageId : row.rowEventId
        readonly property bool selectedRow: row.rowItemKey.length > 0 && row.rowItemKey === root.selectedItemKey
        readonly property bool historyGapRow: modelData.kind === "missing_history_gap"
        readonly property bool invalidSignatureRow: modelData.kind === "invalid_signature"
        readonly property bool warningRow: row.historyGapRow || row.invalidSignatureRow
        readonly property bool unreadDividerBefore: Boolean(modelData.unreadDividerBefore)
        readonly property bool messageDeleted: Boolean(modelData.deleted)
        readonly property int unreadOffset: row.unreadDividerBefore ? 32 : 0
        readonly property int bodyLineLimit: row.warningRow ? 1 : 8
        readonly property real bodyMaxHeight: Math.ceil(14 * 1.35 * row.bodyLineLimit)
        readonly property real contentAreaHeight: Math.max(
            row.warningRow ? 52 : 92,
            contentColumn.implicitHeight + 24
        )
        color: row.warningRow
            ? Tokens.warningSurface
            : (row.selectedRow ? Tokens.secureSurface : (index % 2 === 0 ? Tokens.surfaceBase : Tokens.surfaceRaised))
        border.color: row.selectedRow ? Tokens.secure : "transparent"
        border.width: row.selectedRow ? 1 : 0
        height: row.unreadOffset + row.contentAreaHeight

        Item {
            visible: row.unreadDividerBefore
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: 32

            RowLayout {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: 18
                anchors.rightMargin: 18
                spacing: 8

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: Tokens.accent
                    opacity: 0.55
                }

                Text {
                    text: "New messages"
                    color: Tokens.accent
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: Tokens.accent
                    opacity: 0.55
                }
            }
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.itemSelected(modelData)
        }

        RowLayout {
            id: contentRow
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.topMargin: row.unreadOffset + 12
            anchors.leftMargin: 18
            anchors.rightMargin: 18
            height: Math.max(36, contentColumn.implicitHeight)
            spacing: 12

            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                Layout.alignment: Qt.AlignTop
                radius: 7
                color: row.warningRow
                    ? Tokens.warning
                    : (modelData.encrypted ? Tokens.secure : Tokens.accent)

                Text {
                    anchors.centerIn: parent
                    text: row.warningRow
                        ? "!"
                        : root.authorInitial(modelData.authorDisplayName, modelData.authorDeviceId)
                    color: "white"
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                }
            }

            ColumnLayout {
                id: contentColumn
                Layout.fillWidth: true
                spacing: 3

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Text {
                        Layout.fillWidth: true
                        text: row.historyGapRow
                            ? "History gap"
                            : row.invalidSignatureRow
                                ? "Invalid signature"
                            : root.authorLabel(modelData.authorDisplayName, modelData.authorDeviceId)
                        color: Tokens.textStrong
                        font.pixelSize: 14
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.preferredWidth: Math.min(160, implicitWidth)
                        text: root.channelLabel(modelData.channelName, modelData.channelId)
                        visible: text.length > 0
                        color: Tokens.textMuted
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Text {
                        text: root.timeLabel(modelData.physicalMs)
                        visible: text.length > 0
                        color: Tokens.textMuted
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 28
                    visible: Boolean(modelData.replyPreview)
                    radius: Tokens.radiusSm
                    color: Tokens.surfaceBase
                    border.color: Tokens.borderSubtle

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 6

                        Rectangle {
                            Layout.preferredWidth: 3
                            Layout.fillHeight: true
                            Layout.topMargin: 6
                            Layout.bottomMargin: 6
                            radius: 2
                            color: Tokens.secure
                        }

                        Text {
                            Layout.fillWidth: true
                            text: root.replyPreviewLabel(modelData.replyPreview)
                            color: Tokens.textMuted
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Text {
                        id: bodyText
                        Layout.fillWidth: true
                        Layout.preferredHeight: Math.min(bodyText.implicitHeight, row.bodyMaxHeight)
                        Layout.maximumHeight: row.bodyMaxHeight
                        text: modelData.body
                        color: row.warningRow ? Tokens.warningText : Tokens.textStrong
                        font.pixelSize: 14
                        wrapMode: Text.Wrap
                        maximumLineCount: row.bodyLineLimit
                        elide: Text.ElideRight
                    }

                    Rectangle {
                        visible: modelData.encrypted
                        Layout.preferredHeight: 22
                        Layout.preferredWidth: 74
                        radius: Tokens.radiusSm
                        color: Tokens.secureSurface

                        Text {
                            anchors.centerIn: parent
                            text: "Encrypted"
                            color: Tokens.secure
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }
                    }

                    Rectangle {
                        visible: row.historyGapRow
                        Layout.preferredHeight: 24
                        Layout.preferredWidth: 62
                        radius: Tokens.radiusSm
                        color: gapRepairMouse.containsMouse && root.historyRepairEnabled
                            ? Tokens.secureSurface
                            : Tokens.surfaceRaised
                        border.color: root.historyRepairEnabled ? Tokens.borderSubtle : Tokens.warning
                        opacity: root.historyRepairEnabled ? 1.0 : 0.72

                        Text {
                            anchors.centerIn: parent
                            text: "Repair"
                            color: root.historyRepairEnabled ? Tokens.textMuted : Tokens.warningText
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }

                        MouseArea {
                            id: gapRepairMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: root.historyRepairEnabled
                                ? Qt.PointingHandCursor
                                : Qt.ArrowCursor
                            onClicked: {
                                if (root.historyRepairEnabled) {
                                    root.historyRepairRequested(row.rowEventId)
                                }
                            }
                        }

                        ToolTip.visible: gapRepairMouse.containsMouse
                        ToolTip.text: root.historyRepairEnabled
                            ? "Pull missing history from peer"
                            : "Set a peer endpoint to repair history"
                    }
                }

                Flow {
                    id: actionFlow
                    Layout.fillWidth: true
                    Layout.preferredHeight: visible ? Math.max(24, implicitHeight) : 0
                    visible: !row.warningRow
                    spacing: 6

                    Repeater {
                        model: root.attachmentEntries(modelData.attachments)

                        delegate: Rectangle {
                            id: attachmentChip
                            readonly property string attachmentBlobHash: String(modelData.blobHash || "")
                            readonly property string attachmentSelector: String(modelData.attachmentId || "").length > 0
                                ? String(modelData.attachmentId)
                                : attachmentBlobHash
                            readonly property string attachmentDisplayName: String(modelData.displayName || "attachment")
                            readonly property bool attachmentAvailable: modelData.localBlobAvailable !== false
                            width: Math.min(220, Math.max(92, attachmentLabel.implicitWidth + 18))
                            height: 24
                            radius: Tokens.radiusSm
                            color: attachmentChip.attachmentAvailable ? Tokens.surfaceRaised : Tokens.warningSurface
                            border.color: attachmentChip.attachmentAvailable ? Tokens.borderSubtle : Tokens.warning

                            Text {
                                id: attachmentLabel
                                anchors.centerIn: parent
                                width: parent.width - 14
                                text: attachmentChip.attachmentDisplayName
                                color: attachmentChip.attachmentAvailable ? Tokens.textStrong : Tokens.warningText
                                font.pixelSize: 12
                                font.weight: Font.DemiBold
                                elide: Text.ElideMiddle
                            }

                            ToolTip.visible: attachmentMouse.containsMouse
                            ToolTip.text: (modelData.mediaType || "application/octet-stream")
                                + " - "
                                + String(modelData.byteLen || 0)
                                + " bytes"
                                + (attachmentChip.attachmentAvailable ? "" : " - missing locally")

                            MouseArea {
                                id: attachmentMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: root.actionsEnabled
                                    && attachmentChip.attachmentAvailable
                                    && attachmentChip.attachmentSelector.length > 0
                                    ? Qt.PointingHandCursor
                                    : Qt.ArrowCursor
                                onClicked: {
                                    if (root.actionsEnabled
                                            && attachmentChip.attachmentAvailable
                                            && attachmentChip.attachmentSelector.length > 0) {
                                        root.attachmentSaveRequested(
                                            row.rowMessageId,
                                            attachmentChip.attachmentSelector,
                                            attachmentChip.attachmentDisplayName
                                        )
                                    }
                                }
                            }
                        }
                    }

                    Rectangle {
                        id: attachmentOverflowChip
                        readonly property string overflowLabel: root.attachmentOverflowLabel(modelData)
                        visible: overflowLabel.length > 0
                        width: Math.min(160, Math.max(78, attachmentOverflowText.implicitWidth + 18))
                        height: 24
                        radius: Tokens.radiusSm
                        color: Tokens.surfaceRaised
                        border.color: Tokens.borderSubtle

                        Text {
                            id: attachmentOverflowText
                            anchors.centerIn: parent
                            width: parent.width - 14
                            text: attachmentOverflowChip.overflowLabel
                            color: Tokens.textMuted
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }
                    }

                    Repeater {
                        model: root.reactionEntries(modelData.reactions, modelData.myReactions)

                        delegate: Rectangle {
                            id: reactionChip
                            readonly property bool mine: Boolean(modelData.mine)
                            width: Math.max(44, reactionLabel.implicitWidth + 16)
                            height: 24
                            radius: Tokens.radiusSm
                            color: reactionChip.mine
                                ? (reactionMouse.containsMouse && root.actionsEnabled
                                    ? Tokens.surfaceRaised
                                    : Tokens.secureSurface)
                                : Tokens.surfaceRaised
                            border.color: reactionChip.mine && reactionMouse.containsMouse && root.actionsEnabled
                                ? Tokens.secure
                                : (reactionChip.mine ? Tokens.secure : Tokens.borderSubtle)

                            Text {
                                id: reactionLabel
                                anchors.centerIn: parent
                                text: modelData.reaction + " " + modelData.count
                                color: reactionChip.mine ? Tokens.secure : Tokens.textMuted
                                font.pixelSize: 12
                                font.weight: Font.DemiBold
                            }

                            MouseArea {
                                id: reactionMouse
                                anchors.fill: parent
                                enabled: root.actionsEnabled
                                    && reactionChip.mine
                                    && Boolean(modelData.reaction)
                                    && Boolean(row.rowMessageId)
                                    && !row.messageDeleted
                                    && !row.warningRow
                                hoverEnabled: true
                                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                                onClicked: root.reactionRemoveRequested(
                                    row.rowMessageId,
                                    String(modelData.reaction || "")
                                )
                            }

                            ToolTip.visible: reactionMouse.containsMouse && reactionMouse.enabled
                            ToolTip.text: "Remove " + String(modelData.reaction || "")
                        }
                    }

                    Rectangle {
                        id: reactionOverflowChip
                        readonly property string overflowLabel: root.reactionOverflowLabel(modelData)
                        visible: overflowLabel.length > 0
                        width: Math.min(170, Math.max(92, reactionOverflowText.implicitWidth + 18))
                        height: 24
                        radius: Tokens.radiusSm
                        color: Tokens.surfaceRaised
                        border.color: Tokens.borderSubtle

                        Text {
                            id: reactionOverflowText
                            anchors.centerIn: parent
                            width: parent.width - 14
                            text: reactionOverflowChip.overflowLabel
                            color: Tokens.textMuted
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }
                    }

                    Repeater {
                        model: root.quickReactionEntries(modelData.myReactions)

                        delegate: Rectangle {
                            id: quickReactionChip
                            readonly property string reactionText: String(modelData || "")
                            width: Math.max(44, quickReactionLabel.implicitWidth + 16)
                            height: 24
                            radius: Tokens.radiusSm
                            visible: root.actionsEnabled
                                && Boolean(row.rowMessageId)
                                && !row.messageDeleted
                                && !row.warningRow
                            color: quickReactionMouse.containsMouse ? Tokens.secureSurface : Tokens.surfaceRaised
                            border.color: Tokens.borderSubtle

                            Text {
                                id: quickReactionLabel
                                anchors.centerIn: parent
                                text: quickReactionChip.reactionText
                                color: Tokens.textMuted
                                font.pixelSize: 12
                                font.weight: Font.DemiBold
                            }

                            MouseArea {
                                id: quickReactionMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.reactionRequested(
                                    row.rowMessageId,
                                    quickReactionChip.reactionText
                                )
                            }

                            ToolTip.visible: quickReactionMouse.containsMouse
                            ToolTip.text: "Add " + quickReactionChip.reactionText
                        }
                    }

                    Rectangle {
                        width: Math.max(70, threadReplyText.implicitWidth + 18)
                        height: 24
                        radius: Tokens.radiusSm
                        visible: Number(modelData.threadReplyCount || 0) > 0
                            && !row.warningRow
                        color: Tokens.secureSurface
                        border.color: Tokens.borderSubtle

                        Text {
                            id: threadReplyText
                            anchors.centerIn: parent
                            text: root.threadReplyLabel(modelData.threadReplyCount)
                            color: Tokens.secure
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }

                        ToolTip.visible: threadReplyMouse.containsMouse
                        ToolTip.text: modelData.threadLatestReply
                            ? "Latest: " + root.replyPreviewLabel(modelData.threadLatestReply)
                            : "Thread replies"

                        MouseArea {
                            id: threadReplyMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.threadRequested(modelData)
                        }
                    }

                    Rectangle {
                        width: 52
                        height: 24
                        radius: Tokens.radiusSm
                        visible: root.actionsEnabled && Boolean(modelData.messageId) && !modelData.deleted
                            && !row.warningRow
                        color: replyMouse.containsMouse ? Tokens.secureSurface : Tokens.surfaceRaised
                        border.color: Tokens.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            text: "Reply"
                            color: Tokens.textMuted
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }

                        MouseArea {
                            id: replyMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.replyRequested(modelData)
                        }

                        ToolTip.visible: replyMouse.containsMouse
                        ToolTip.text: "Reply in channel"
                    }

                    Rectangle {
                        width: 52
                        height: 24
                        radius: Tokens.radiusSm
                        visible: root.actionsEnabled && row.rowEventId.length > 0
                            && !row.warningRow
                        color: proofMouse.containsMouse ? Tokens.secureSurface : Tokens.surfaceRaised
                        border.color: Tokens.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            text: "Proof"
                            color: Tokens.textMuted
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }

                        MouseArea {
                            id: proofMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.proofPublishRequested(row.rowEventId)
                        }

                        ToolTip.visible: proofMouse.containsMouse
                        ToolTip.text: "Publish proof slice"
                    }

                    Rectangle {
                        width: 44
                        height: 24
                        radius: Tokens.radiusSm
                        visible: root.actionsEnabled && Boolean(modelData.messageId) && !modelData.deleted
                            && !Boolean(modelData.bodyTruncated)
                        color: editMouse.containsMouse ? Tokens.secureSurface : Tokens.surfaceRaised
                        border.color: Tokens.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            text: "Edit"
                            color: Tokens.textMuted
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }

                        MouseArea {
                            id: editMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.editRequested(modelData.messageId || "", modelData.body || "")
                        }

                        ToolTip.visible: editMouse.containsMouse
                        ToolTip.text: "Edit message"
                    }

                    Rectangle {
                        width: 56
                        height: 24
                        radius: Tokens.radiusSm
                        visible: root.actionsEnabled && Boolean(modelData.messageId) && !modelData.deleted
                        color: deleteMouse.containsMouse ? Tokens.warningSurface : Tokens.surfaceRaised
                        border.color: Tokens.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            text: "Delete"
                            color: Tokens.warningText
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }

                        MouseArea {
                            id: deleteMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.deleteRequested(modelData.messageId || "")
                        }

                        ToolTip.visible: deleteMouse.containsMouse
                        ToolTip.text: "Delete message"
                    }
                }
            }
        }
    }
}
