import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string channelName: "general"
    property bool directMessage: false
    property bool editMode: false
    property bool replyMode: false
    property string replyLabel: ""
    property string replyAvatarId: ""
    property string replyWorkspaceId: ""
    property string replyIdentityId: ""
    property string replyDisplayName: ""
    property bool operationPending: false
    property string blockedReason: ""
    property string blockedActionLabel: ""
    readonly property bool blocked: blockedReason.trim().length > 0
    signal sendRequested(string text)
    signal attachRequested(string text)
    signal saveEditRequested(string text)
    signal cancelEditRequested()
    signal cancelReplyRequested()
    signal draftChanged(string text)
    signal blockedActionRequested()
    readonly property int inputHeight: Math.min(92, Math.max(40, messageField.contentHeight + 12))

    function clearDraft() {
        messageField.text = ""
    }

    function setDraft(text) {
        messageField.text = text
        messageField.forceActiveFocus()
        messageField.selectAll()
    }

    function restoreDraft(text) {
        messageField.text = text
        messageField.cursorPosition = messageField.text.length
    }

    function draftText() {
        return messageField.text
    }

    function focusDraft() {
        messageField.forceActiveFocus()
        messageField.cursorPosition = messageField.text.length
    }

    function submitDraft() {
        if (!root.enabled || root.blocked || root.operationPending
                || messageField.text.trim().length === 0) {
            return false
        }
        if (root.editMode) {
            root.saveEditRequested(messageField.text)
        } else {
            root.sendRequested(messageField.text)
        }
        return true
    }

    function attachDraft() {
        if (!root.enabled || root.blocked || root.editMode || root.operationPending) {
            return false
        }
        root.attachRequested(messageField.text)
        return true
    }

    function cancelEdit() {
        if (!root.editMode || root.operationPending) {
            return false
        }
        root.cancelEditRequested()
        return true
    }

    height: ((root.editMode || root.replyMode) ? 70 : 50) + root.inputHeight
        + (root.blocked ? 34 : 0)
    color: Tokens.surfaceBase

    Rectangle {
        anchors.fill: parent
        anchors.margins: 14
        radius: Tokens.radiusMd
        color: Tokens.surfaceRaised
        border.color: Tokens.borderSubtle

        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 4

            RowLayout {
                visible: root.blocked
                Layout.fillWidth: true
                Layout.preferredHeight: visible ? 30 : 0
                spacing: 8

                Text {
                    objectName: "composerBlockedReason"
                    Layout.fillWidth: true
                    text: root.blockedReason
                    color: Tokens.warningText
                    font.pixelSize: Tokens.fontSizeSm
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    Accessible.role: Accessible.AlertMessage
                    Accessible.name: text
                }

                Button {
                    objectName: "composerBlockedAction"
                    visible: root.blockedActionLabel.length > 0
                    text: root.blockedActionLabel
                    implicitWidth: 108
                    enabled: root.enabled && !root.operationPending
                    onClicked: root.blockedActionRequested()
                }
            }

            Text {
                visible: root.editMode
                Layout.fillWidth: true
                text: "Editing message"
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeSm
                elide: Text.ElideRight
            }

            RowLayout {
                visible: root.replyMode && !root.editMode
                Layout.fillWidth: true
                spacing: 6

                AvatarMark {
                    Layout.preferredWidth: 22
                    Layout.preferredHeight: 22
                    avatarId: root.replyAvatarId
                    workspaceId: root.replyWorkspaceId
                    identityId: root.replyIdentityId
                    displayName: root.replyDisplayName
                }

                Text {
                    Layout.fillWidth: true
                    text: "Replying to " + root.replyLabel
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeSm
                    elide: Text.ElideRight
                }

                Button {
                    text: "Cancel"
                    implicitWidth: 72
                    onClicked: root.cancelReplyRequested()
                }
            }

            RowLayout {
                Layout.fillWidth: true

                ScrollView {
                    id: messageScroll
                    Layout.fillWidth: true
                    Layout.preferredHeight: root.inputHeight
                    clip: true
                    contentWidth: availableWidth
                    contentHeight: messageField.height
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                    ScrollBar.vertical.policy: messageField.contentHeight > root.inputHeight
                        ? ScrollBar.AsNeeded
                        : ScrollBar.AlwaysOff

                    TextArea {
                        id: messageField
                        objectName: "composerMessageField"
                        width: messageScroll.availableWidth
                        height: Math.max(root.inputHeight, contentHeight)
                        placeholderText: root.editMode
                            ? "Edit message"
                            : "Message " + (root.directMessage ? "@" : "#") + root.channelName
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
                        // A missing private-room key blocks delivery, not
                        // drafting. Keep the local draft fully editable.
                        enabled: root.enabled
                        wrapMode: TextEdit.Wrap
                        selectByMouse: true
                        padding: 0
                        topPadding: 9
                        bottomPadding: 7
                        background: Item {}
                        onTextChanged: root.draftChanged(text)
                        Keys.onReturnPressed: function(event) {
                            if (event.modifiers & Qt.ShiftModifier) {
                                event.accepted = false
                            } else {
                                root.submitDraft()
                                event.accepted = true
                            }
                        }
                        Keys.onEnterPressed: function(event) {
                            if (event.modifiers & Qt.ShiftModifier) {
                                event.accepted = false
                            } else {
                                root.submitDraft()
                                event.accepted = true
                            }
                        }
                    }
                }

                Button {
                    visible: root.editMode
                    text: "Cancel"
                    implicitWidth: 76
                    enabled: !root.operationPending
                    Layout.alignment: Qt.AlignBottom
                    onClicked: root.cancelEdit()
                }

                Button {
                    id: attachButton
                    visible: !root.editMode
                    text: "Attach"
                    implicitWidth: 72
                    enabled: root.enabled && !root.blocked && !root.operationPending
                    Layout.alignment: Qt.AlignBottom
                    Accessible.name: "Attach file"
                    Accessible.description: "Choose a file to attach to this draft"
                    onClicked: root.attachDraft()
                    ToolTip.visible: hovered
                    ToolTip.text: "Attach a file"
                }

                Button {
                    id: submitButton
                    text: root.operationPending
                        ? (root.editMode ? "Saving..." : "Sending...")
                        : (root.editMode ? "Save" : "Send")
                    implicitWidth: root.operationPending ? 92 : 78
                    enabled: root.enabled && !root.blocked && !root.operationPending
                        && messageField.text.trim().length > 0
                    Layout.alignment: Qt.AlignBottom
                    Accessible.name: root.editMode ? "Save edited message" : "Send message"
                    Accessible.description: root.editMode
                        ? "Save changes to this message"
                        : "Send this message"
                    onClicked: root.submitDraft()
                    ToolTip.visible: hovered
                    ToolTip.text: root.editMode ? "Save message" : "Send message"
                }
            }
        }
    }
}
