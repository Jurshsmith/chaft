pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

ListView {
    id: root
    property var timelineModel: []
    property string emptyText: "No messages yet"
    property string workspaceId: ""
    property bool actionsEnabled: false
    property bool historyRepairEnabled: false
    property bool historyRepairHasAddress: false
    property bool historyRepairBusy: false
    property bool autoFollowLatest: true
    property bool showChannelLabels: false
    property string selectedItemKey: ""
    property bool followLatest: true
    property bool pendingInitialScroll: true
    property bool pendingUnreadScroll: false
    property bool preservingPrepend: false
    property real preservedContentHeight: 0
    property real preservedContentY: 0
    property var quickReactions: ["👍", "🚀", "👀", "✅"]
    property var expandedMessageIds: ({})
    readonly property real readingWidth: Math.min(920, Math.max(0, root.width - 36))
    property var reactionChoices: [
        { emoji: "👍", label: "Thumbs up", keywords: "yes agree approve like" },
        { emoji: "👎", label: "Thumbs down", keywords: "no disagree dislike" },
        { emoji: "❤️", label: "Heart", keywords: "love favorite thanks" },
        { emoji: "😂", label: "Laugh", keywords: "funny haha lol" },
        { emoji: "🎉", label: "Celebrate", keywords: "party congrats win" },
        { emoji: "🚀", label: "Rocket", keywords: "ship launch go fast" },
        { emoji: "👀", label: "Eyes", keywords: "looking seen watch" },
        { emoji: "✅", label: "Done", keywords: "check complete approved" },
        { emoji: "🙌", label: "Raised hands", keywords: "praise hooray great" },
        { emoji: "🔥", label: "Fire", keywords: "hot strong great" },
        { emoji: "💯", label: "Hundred", keywords: "perfect exactly agree" },
        { emoji: "👏", label: "Clap", keywords: "applause nice" },
        { emoji: "🙏", label: "Thanks", keywords: "please thank grateful" },
        { emoji: "😄", label: "Smile", keywords: "happy glad" },
        { emoji: "😮", label: "Surprised", keywords: "wow unexpected" },
        { emoji: "😢", label: "Sad", keywords: "sorry upset" },
        { emoji: "😡", label: "Angry", keywords: "mad blocked bad" },
        { emoji: "🤔", label: "Thinking", keywords: "question maybe consider" },
        { emoji: "💡", label: "Idea", keywords: "lightbulb thought insight" },
        { emoji: "⭐", label: "Star", keywords: "important favorite" },
        { emoji: "📌", label: "Pin", keywords: "save remember" },
        { emoji: "⚠️", label: "Warning", keywords: "alert caution risk" },
        { emoji: "❓", label: "Question", keywords: "help ask unsure" },
        { emoji: "➕", label: "Plus one", keywords: "add support plus" }
    ]
    property bool openReactionPickerOnLoad: false
    property var pendingDeleteMessageIds: []
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
    signal replyParentRequested(string messageId)
    signal threadRequested(var item)
    signal editRequested(string messageId, string body)
    signal deleteRequested(string messageId)
    signal externalLinkRequested(string link)
    signal attachmentSaveRequested(string messageId, string attachmentSelector, string displayName)
    signal proofPublishRequested(string eventId)
    signal historyRepairRequested(string eventId)
    signal cryptoBadgeRequested(string title, string message)

    function authorLabel(authorDisplayName, authorDeviceId) {
        var displayName = String(authorDisplayName || "").trim()
        if (displayName.length > 0) {
            return displayName
        }
        var deviceId = String(authorDeviceId || "")
        return deviceId.length > 0 ? "Unnamed teammate" : ""
    }

    function shortDeviceId(deviceId) {
        var value = String(deviceId || "")
        return value.length > 14 ? value.slice(0, 7) + "..." + value.slice(value.length - 4) : value
    }

    function authorInitial(authorDisplayName, authorDeviceId) {
        var value = root.authorLabel(authorDisplayName, authorDeviceId)
        if (value.length === 0) {
            return "?"
        }
        var words = value.split(/[\s_-]+/).filter(function (word) { return word.length > 0 })
        if (words.length >= 2) {
            return String(words[0].slice(0, 1) + words[1].slice(0, 1)).toUpperCase()
        }
        return value.slice(0, 1).toUpperCase()
    }

    function exceptionLabel(kind) {
        if (kind === "missing_history_gap") {
            return "History missing"
        }
        if (kind === "invalid_signature") {
            return "Security check failed"
        }
        return ""
    }

    function dayLabel(physicalMs) {
        var value = Number(physicalMs || 0)
        if (!isFinite(value) || value <= 0) {
            return ""
        }
        var date = new Date(value)
        if (isNaN(date.getTime())) {
            return ""
        }
        return Qt.formatDateTime(date, "ddd, MMM d yyyy")
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

    function messageCanUseMarkdown(value) {
        // QTextDocument can fetch resources referenced by Markdown images or raw
        // HTML. Render messages containing either resource-bearing entry point
        // as plain text; safe Markdown still retains headings, emphasis, lists,
        // code, quotes, and normal links without rewriting message content.
        var source = String(value === undefined || value === null ? "" : value)
        var hasMarkdownImage = source.indexOf("![") !== -1
        var hasRawHtmlTag = /<\/?[A-Za-z][^>]*>/.test(source)
        return !hasMarkdownImage && !hasRawHtmlTag
    }

    function accessibleMessageLabel(item) {
        var source = item || {}
        var exception = root.exceptionLabel(String(source.kind || ""))
        if (exception.length > 0) {
            return exception
        }
        var author = root.authorLabel(source.authorDisplayName, source.authorDeviceId)
        return author.length > 0 ? "Message from " + author : "Message"
    }

    function setMessageExpanded(itemKey, expanded) {
        var key = String(itemKey || "")
        if (key.length === 0) {
            return
        }
        var next = Object.assign({}, root.expandedMessageIds || {})
        if (expanded) {
            next[key] = true
        } else {
            delete next[key]
        }
        root.expandedMessageIds = next
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
        for (var i = 0; i < root.quickReactions.length && entries.length < 2; i += 1) {
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
    ScrollBar.vertical: ScrollBar {
        policy: ScrollBar.AsNeeded
    }

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

    Column {
        anchors.centerIn: parent
        visible: root.timelineModel.length === 0
        width: Math.min(420, Math.max(0, root.width - 48))
        spacing: Tokens.space2

        Text {
            width: parent.width
            text: root.emptyText
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeLg
            font.weight: Font.DemiBold
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
        }

        Text {
            width: parent.width
            text: root.emptyText.indexOf("No matching") === 0
                ? "Try another search or clear the current terms."
                : "Send the first message to start the conversation."
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
        }
    }

    Button {
        id: latestJump
        objectName: "jumpToLatestButton"
        visible: root.showJumpToLatest
        z: 20
        x: Math.max(18, root.width - width - 22)
        y: root.contentY + root.height - height - 18
        width: Math.max(132, latestJumpLabel.implicitWidth + 32)
        height: 34
        text: "Jump to latest"
        activeFocusOnTab: visible

        Accessible.name: text
        Accessible.description: "Scroll to newest messages"

        background: Rectangle {
            radius: Tokens.radiusMd
            color: latestJump.hovered || latestJump.activeFocus
                ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.14)
                : Tokens.surfaceRaised
            border.color: Tokens.accent
            border.width: latestJump.activeFocus ? 2 : 1
        }

        contentItem: Text {
            id: latestJumpLabel
            text: latestJump.text
            color: Tokens.accent
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.DemiBold
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        onClicked: root.scrollToLatest()
        ToolTip.visible: hovered
        ToolTip.text: "Scroll to newest messages"
    }

    delegate: Rectangle {
        id: row
        objectName: "timelineMessageRow"
        required property int index
        required property var modelData
        width: root.width
        readonly property string rowMessageId: String(row.modelData.messageId || "")
        readonly property string rowEventId: String(row.modelData.eventId || "")
        readonly property string rowItemKey: row.rowMessageId.length > 0 ? row.rowMessageId : row.rowEventId
        readonly property bool selectedRow: row.rowItemKey.length > 0 && row.rowItemKey === root.selectedItemKey
        readonly property bool historyGapRow: row.modelData.kind === "missing_history_gap"
        readonly property bool invalidSignatureRow: row.modelData.kind === "invalid_signature"
        readonly property bool warningRow: row.historyGapRow || row.invalidSignatureRow
        readonly property bool unreadDividerBefore: Boolean(row.modelData.unreadDividerBefore)
        readonly property bool messageDeleted: Boolean(row.modelData.deleted)
        readonly property bool dayBoundary: Boolean(row.modelData.dayBoundary)
            && root.dayLabel(row.modelData.physicalMs).length > 0
        readonly property bool grouped: Boolean(row.modelData.groupedWithPrevious)
            && !row.warningRow && !row.unreadDividerBefore && !row.dayBoundary
        readonly property bool pendingDelete: row.rowMessageId.length > 0
            && root.pendingDeleteMessageIds.indexOf(row.rowMessageId) !== -1
        readonly property bool rowHovered: rowHover.hovered
        readonly property int dayOffset: row.dayBoundary ? 28 : 0
        readonly property int unreadOffset: (row.unreadDividerBefore ? 32 : 0) + row.dayOffset
        readonly property int bodyLineLimit: row.warningRow ? 1 : 8
        readonly property real bodyCollapsedMaxHeight: Math.ceil(
            Tokens.fontSizeMd * 1.35 * row.bodyLineLimit
        )
        readonly property bool bodyExpanded: root.expandedMessageIds[row.rowItemKey] === true
        readonly property bool bodyOverflowing: !row.warningRow
            && bodyText.contentHeight > row.bodyCollapsedMaxHeight + 1
        readonly property real contentAreaHeight: Math.max(
            row.warningRow ? 52 : (row.grouped ? 40 : 68),
            contentColumn.implicitHeight + 24
        )
        activeFocusOnTab: !row.pendingDelete
        visible: !row.pendingDelete
        color: row.warningRow
            ? Tokens.warningSurface
            : row.selectedRow
                ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.12)
                : row.rowHovered
                    ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.06)
                    : Tokens.surfaceBase
        border.color: row.selectedRow || row.activeFocus ? Tokens.accent : "transparent"
        border.width: row.activeFocus ? 2 : (row.selectedRow ? 1 : 0)
        height: row.pendingDelete ? 0 : row.unreadOffset + row.contentAreaHeight

        Accessible.role: Accessible.ListItem
        Accessible.name: root.accessibleMessageLabel(row.modelData)
        Accessible.description: row.warningRow
            ? "Timeline warning"
            : "Press Enter to open message actions. Message text is selectable."

        HoverHandler {
            id: rowHover
            enabled: !row.pendingDelete
        }

        Timer {
            id: smokeReactionPickerTimer
            interval: 850
            repeat: false
            onTriggered: {
                if (root.openReactionPickerOnLoad
                        && row.index === 0
                        && row.rowMessageId.length > 0
                        && !reactionPicker.visible) {
                    reactionPicker.open()
                }
            }
        }

        Component.onCompleted: {
            if (root.openReactionPickerOnLoad && row.index === 0) {
                smokeReactionPickerTimer.restart()
            }
        }

        Keys.onPressed: function (event) {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                    || event.key === Qt.Key_Space) {
                root.itemSelected(row.modelData)
                event.accepted = true
            } else if (event.key === Qt.Key_R && event.modifiers === Qt.NoModifier
                    && root.actionsEnabled && row.rowMessageId.length > 0
                    && !row.messageDeleted && !row.warningRow) {
                root.replyRequested(row.modelData)
                event.accepted = true
            } else if (event.key === Qt.Key_Menu
                    || (event.key === Qt.Key_F10
                        && (event.modifiers & Qt.ShiftModifier))) {
                rowMenu.popup()
                event.accepted = true
            }
        }

        Item {
            visible: row.dayBoundary
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: 28

            RowLayout {
                width: root.readingWidth
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: Tokens.borderSubtle
                }

                Text {
                    text: root.dayLabel(row.modelData.physicalMs)
                    color: Tokens.textMuted
                    font.family: Tokens.fontMono
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: Tokens.borderSubtle
                }
            }
        }

        Item {
            visible: row.unreadDividerBefore
            anchors.top: parent.top
            anchors.topMargin: row.dayOffset
            anchors.left: parent.left
            anchors.right: parent.right
            height: 32

            RowLayout {
                width: root.readingWidth
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.verticalCenter: parent.verticalCenter
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
                    font.pixelSize: Tokens.fontSizeSm
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
            scrollGestureEnabled: false
            acceptedButtons: Qt.LeftButton
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.itemSelected(row.modelData)
        }

        RowLayout {
            id: contentRow
            anchors.top: parent.top
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.topMargin: row.unreadOffset + 12
            width: root.readingWidth
            height: Math.max(36, contentColumn.implicitHeight)
            spacing: 12

            Item {
                Layout.preferredWidth: 36
                Layout.preferredHeight: row.grouped ? 18 : 36
                Layout.alignment: Qt.AlignTop

                TimelineAuthorMark {
                    anchors.fill: parent
                    visible: !row.grouped
                    Accessible.ignored: true
                    warning: row.warningRow
                    encrypted: row.modelData.encrypted
                    authorDeviceId: String(row.modelData.authorDeviceId || "")
                    authorAvatarId: String(row.modelData.authorAvatarId || "")
                    workspaceId: root.workspaceId
                    label: row.warningRow
                        ? ""
                        : root.authorInitial(row.modelData.authorDisplayName, row.modelData.authorDeviceId)
                }

                Text {
                    anchors.centerIn: parent
                    visible: row.grouped && row.rowHovered
                    text: root.timeLabel(row.modelData.physicalMs)
                    color: Tokens.textMuted
                    font.family: Tokens.fontMono
                    font.pixelSize: Tokens.fontSizeXs
                }
            }

            ColumnLayout {
                id: contentColumn
                Layout.fillWidth: true
                spacing: 3

                TimelineHeaderRow {
                    Layout.fillWidth: true
                    visible: !row.grouped
                    primaryLabel: row.historyGapRow
                        ? root.exceptionLabel(row.modelData.kind)
                        : row.invalidSignatureRow
                            ? root.exceptionLabel(row.modelData.kind)
                            : root.authorLabel(row.modelData.authorDisplayName, row.modelData.authorDeviceId)
                    channelLabel: root.channelLabel(row.modelData.channelName, row.modelData.channelId)
                    timeLabel: root.timeLabel(row.modelData.physicalMs)
                }

                Item {
                    id: parentReplyPreview
                    objectName: "replyParentPreview"
                    Layout.fillWidth: true
                    Layout.preferredHeight: visible ? 28 : 0
                    readonly property var preview: row.modelData.replyPreview || null
                    readonly property string parentMessageId: parentReplyPreview.preview
                        ? String(parentReplyPreview.preview.messageId || "")
                        : ""
                    readonly property string previewLabel: parentReplyPreview.preview
                        ? root.replyPreviewLabel(parentReplyPreview.preview)
                        : ""
                    visible: parentReplyPreview.previewLabel.length > 0
                    activeFocusOnTab: visible && parentMessageId.length > 0

                    Accessible.role: Accessible.Button
                    Accessible.name: "Go to replied message"
                    Accessible.description: previewLabel
                    Accessible.onPressAction: parentReplyPreview.activate()

                    function activate() {
                        if (parentReplyPreview.parentMessageId.length > 0) {
                            root.replyParentRequested(parentReplyPreview.parentMessageId)
                        }
                    }

                    TimelineReplyPreview {
                        anchors.fill: parent
                        Accessible.ignored: true
                        label: parentReplyPreview.previewLabel
                        avatarId: parentReplyPreview.preview
                            ? String(parentReplyPreview.preview.authorAvatarId || "")
                            : ""
                        authorDeviceId: parentReplyPreview.preview
                            ? String(parentReplyPreview.preview.authorDeviceId || "")
                            : ""
                        authorDisplayName: parentReplyPreview.preview
                            ? String(parentReplyPreview.preview.authorDisplayName || "")
                            : ""
                        workspaceId: root.workspaceId
                    }

                    Rectangle {
                        anchors.fill: parent
                        radius: Tokens.radiusSm
                        color: "transparent"
                        border.color: parentReplyPreview.activeFocus ? Tokens.accent : "transparent"
                        border.width: parentReplyPreview.activeFocus ? 2 : 0
                    }

                    MouseArea {
                        anchors.fill: parent
                        scrollGestureEnabled: false
                        enabled: parentReplyPreview.parentMessageId.length > 0
                        hoverEnabled: true
                        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onClicked: parentReplyPreview.activate()
                    }

                    Keys.onPressed: function (event) {
                        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                                || event.key === Qt.Key_Space) {
                            parentReplyPreview.activate()
                            event.accepted = true
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    TextEdit {
                        id: bodyText
                        objectName: "timelineMessageBody"
                        Layout.fillWidth: true
                        Layout.preferredHeight: row.bodyExpanded
                            ? bodyText.contentHeight
                            : Math.min(bodyText.contentHeight, row.bodyCollapsedMaxHeight)
                        text: String(row.modelData.body || "")
                        textFormat: row.warningRow
                                || !root.messageCanUseMarkdown(row.modelData.body)
                            ? TextEdit.PlainText
                            : TextEdit.MarkdownText
                        color: row.warningRow ? Tokens.warningText : Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeMd
                        wrapMode: TextEdit.Wrap
                        readOnly: true
                        selectByMouse: true
                        selectByKeyboard: true
                        persistentSelection: true
                        activeFocusOnTab: text.length > 0
                        cursorVisible: false
                        clip: !row.bodyExpanded

                        Accessible.role: Accessible.EditableText
                        Accessible.name: "Message text"
                        Accessible.description: "Read-only text. Select and copy as needed."
                        Accessible.readOnly: true

                        onLinkActivated: function(link) {
                            root.externalLinkRequested(link)
                        }
                    }

                    TimelineEncryptedBadge {
                        Layout.preferredHeight: visible ? 22 : 0
                        Layout.preferredWidth: visible ? 74 : 0
                        encrypted: row.modelData.encrypted
                        kind: String(row.modelData.kind || "")
                        warningRow: row.warningRow
                        bodyDecrypted: Boolean(row.modelData.bodyDecrypted)
                        messageDeleted: row.messageDeleted
                        onActivated: function (title, message) {
                            root.cryptoBadgeRequested(title, message)
                        }
                    }

                    TimelineRepairChip {
                        visible: row.historyGapRow
                        Layout.preferredHeight: 24
                        Layout.preferredWidth: 108
                        repairEnabled: root.historyRepairEnabled
                        addressAvailable: root.historyRepairHasAddress
                        busy: root.historyRepairBusy
                        onRepairRequested: root.historyRepairRequested(row.rowEventId)
                    }
                }

                TimelineActionChip {
                    id: bodyExpansionAction
                    objectName: "messageExpansionButton"
                    Layout.preferredWidth: 90
                    Layout.preferredHeight: 24
                    visible: row.bodyOverflowing || row.bodyExpanded
                    label: row.bodyExpanded ? "Show less" : "Show more"
                    tooltip: row.bodyExpanded
                        ? "Collapse message"
                        : "Read full message"
                    minimumWidth: 90
                    onActivated: {
                        root.followLatest = false
                        root.setMessageExpanded(row.rowItemKey, !row.bodyExpanded)
                    }
                }

                Flow {
                    id: actionFlow
                    Layout.fillWidth: true
                    Layout.preferredHeight: visible && implicitHeight > 0 ? implicitHeight : 0
                    visible: !row.warningRow
                    spacing: 6

                    Repeater {
                        model: root.attachmentEntries(row.modelData.attachments)

                        delegate: TimelineAttachmentChip {
                            id: attachmentChip
                            required property var modelData
                            readonly property string attachmentBlobHash: String(attachmentChip.modelData.blobHash || "")

                            messageId: row.rowMessageId
                            selector: String(attachmentChip.modelData.attachmentId || "").length > 0
                                ? String(attachmentChip.modelData.attachmentId)
                                : attachmentBlobHash
                            displayName: String(attachmentChip.modelData.displayName || "attachment")
                            mediaType: String(attachmentChip.modelData.mediaType || "application/octet-stream")
                            byteLen: Number(attachmentChip.modelData.byteLen || 0)
                            available: attachmentChip.modelData.localBlobAvailable !== false
                            actionsEnabled: root.actionsEnabled
                            onSaveRequested: function (messageId, selector, displayName) {
                                root.attachmentSaveRequested(messageId, selector, displayName)
                            }
                        }
                    }

                    TimelineOverflowChip {
                        label: root.attachmentOverflowLabel(row.modelData)
                        minimumWidth: 78
                        maximumWidth: 160
                    }

                    Repeater {
                        model: root.reactionEntries(row.modelData.reactions, row.modelData.myReactions)

                        delegate: TimelineReactionChip {
                            id: reactionChip
                            required property var modelData

                            messageId: row.rowMessageId
                            reaction: String(reactionChip.modelData.reaction || "")
                            count: Number(reactionChip.modelData.count || 0)
                            mine: Boolean(reactionChip.modelData.mine)
                            actionsEnabled: root.actionsEnabled
                            messageDeleted: row.messageDeleted
                            warningRow: row.warningRow
                            onRemoveRequested: function (messageId, reaction) {
                                root.reactionRemoveRequested(messageId, reaction)
                            }
                        }
                    }

                    TimelineOverflowChip {
                        label: root.reactionOverflowLabel(row.modelData)
                        minimumWidth: 92
                        maximumWidth: 170
                    }

                    TimelineThreadChip {
                        label: root.threadReplyLabel(row.modelData.threadReplyCount)
                        latestReplyLabel: row.modelData.threadLatestReply
                            ? root.replyPreviewLabel(row.modelData.threadLatestReply)
                            : ""
                        visible: Number(row.modelData.threadReplyCount || 0) > 0
                            && !row.warningRow
                        onOpenRequested: root.threadRequested(row.modelData)
                    }

                }
            }
        }

        Rectangle {
            id: hoverActions
            anchors.right: contentRow.right
            y: row.unreadOffset + 6
            width: hoverActionsRow.implicitWidth + 12
            height: 30
            radius: Tokens.radiusSm
            color: Tokens.surfaceRaised
            border.width: 1
            border.color: Tokens.borderSubtle
            readonly property bool shown: !row.warningRow && root.actionsEnabled && !row.pendingDelete
                && (row.rowHovered || row.activeFocus
                    || reactionPicker.visible || rowMenu.visible)
            opacity: shown ? 1 : 0
            visible: opacity > 0.01

            Behavior on opacity {
                enabled: Tokens.motionEnabled
                NumberAnimation {
                    duration: Tokens.motionQuickMs
                    easing.type: Easing.OutCubic
                }
            }

            Row {
                id: hoverActionsRow
                anchors.centerIn: parent
                spacing: 4

                Repeater {
                    model: root.quickReactionEntries(row.modelData.myReactions)

                    delegate: TimelineQuickReactionChip {
                        id: quickReactionChip
                        required property var modelData
                        readonly property string reactionText: String(quickReactionChip.modelData || "")

                        anchors.verticalCenter: hoverActionsRow.verticalCenter
                        messageId: row.rowMessageId
                        reaction: quickReactionChip.reactionText
                        actionsEnabled: root.actionsEnabled
                        messageDeleted: row.messageDeleted
                        warningRow: row.warningRow
                        onAddRequested: function (messageId, reaction) {
                            root.reactionRequested(messageId, reaction)
                        }
                    }
                }

                TimelineActionChip {
                    anchors.verticalCenter: hoverActionsRow.verticalCenter
                    label: "+"
                    tooltip: "Add reaction"
                    minimumWidth: 30
                    visible: Boolean(row.modelData.messageId) && !row.modelData.deleted
                    onActivated: reactionPicker.open()
                }

                TimelineActionChip {
                    anchors.verticalCenter: parent.verticalCenter
                    label: "Reply"
                    tooltip: "Reply in conversation"
                    minimumWidth: 52
                    visible: Boolean(row.modelData.messageId) && !row.modelData.deleted
                    onActivated: root.replyRequested(row.modelData)
                }

                TimelineActionChip {
                    anchors.verticalCenter: parent.verticalCenter
                    label: "⋯"
                    tooltip: "More actions"
                    minimumWidth: 30
                    visible: row.rowMessageId.length > 0 || row.rowEventId.length > 0
                    onActivated: rowMenu.popup()
                }
            }
        }

        ReactionPicker {
            id: reactionPicker

            anchorItem: hoverActions
            choices: root.reactionChoices
            myReactions: row.modelData.myReactions || []
            messageId: row.rowMessageId
            actionsEnabled: root.actionsEnabled
            messageDeleted: row.messageDeleted
            warningRow: row.warningRow
            onReactionRequested: function (messageId, reaction) {
                root.reactionRequested(messageId, reaction)
            }
            onReactionRemoveRequested: function (messageId, reaction) {
                root.reactionRemoveRequested(messageId, reaction)
            }
        }

        Menu {
            id: rowMenu

            MenuItem {
                text: "Edit message"
                enabled: root.actionsEnabled && Boolean(row.modelData.canEdit)
                    && Boolean(row.modelData.messageId)
                    && !row.modelData.deleted && !Boolean(row.modelData.bodyTruncated)
                onTriggered: root.editRequested(row.modelData.messageId || "", row.modelData.body || "")
            }

            MenuItem {
                text: "Share message diagnostics"
                enabled: root.actionsEnabled && row.rowEventId.length > 0
                onTriggered: root.proofPublishRequested(row.rowEventId)
            }

            MenuItem {
                text: "Delete message"
                enabled: root.actionsEnabled && Boolean(row.modelData.canDelete)
                    && Boolean(row.modelData.messageId)
                    && !row.modelData.deleted
                onTriggered: root.deleteRequested(row.modelData.messageId || "")
            }
        }
    }
}
