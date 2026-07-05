import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string channelName: "general"
    property bool editMode: false
    property bool replyMode: false
    property string replyLabel: ""
    signal sendRequested(string text)
    signal attachRequested(string text)
    signal saveEditRequested(string text)
    signal cancelEditRequested()
    signal cancelReplyRequested()
    signal draftChanged(string text)
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
        if (!root.enabled || messageField.text.trim().length === 0) {
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
        if (!root.enabled || root.editMode) {
            return false
        }
        root.attachRequested(messageField.text)
        return true
    }

    function cancelEdit() {
        if (!root.editMode) {
            return false
        }
        root.cancelEditRequested()
        return true
    }

    height: ((root.editMode || root.replyMode) ? 70 : 50) + root.inputHeight
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
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                    ScrollBar.vertical.policy: messageField.contentHeight > root.inputHeight
                        ? ScrollBar.AsNeeded
                        : ScrollBar.AlwaysOff

                    TextArea {
                        id: messageField
                        width: messageScroll.availableWidth
                        height: Math.max(root.inputHeight, contentHeight)
                        placeholderText: root.editMode ? "Edit message" : "Message #" + root.channelName
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
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
                    Layout.alignment: Qt.AlignBottom
                    onClicked: root.cancelEdit()
                }

                Button {
                    visible: !root.editMode
                    text: "File"
                    implicitWidth: 64
                    enabled: root.enabled
                    Layout.alignment: Qt.AlignBottom
                    onClicked: root.attachDraft()
                }

                Button {
                    text: root.editMode ? "Save" : "Send"
                    implicitWidth: 78
                    enabled: root.enabled && messageField.text.trim().length > 0
                    Layout.alignment: Qt.AlignBottom
                    onClicked: root.submitDraft()
                }
            }
        }
    }
}
