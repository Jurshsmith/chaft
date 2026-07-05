import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Dialog {
    id: root
    property string message: ""
    property string confirmLabel: "Confirm"
    property bool destructive: false
    property string contextId: ""
    signal confirmed(string contextId)

    modal: true
    anchors.centerIn: parent
    width: Math.min(420, parent ? parent.width - Tokens.space4 * 2 : 420)
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    function ask(title, message, confirmLabel, contextId, destructive) {
        root.title = String(title || "Are you sure?")
        root.message = String(message || "")
        root.confirmLabel = String(confirmLabel || "Confirm")
        root.contextId = String(contextId || "")
        root.destructive = Boolean(destructive)
        root.open()
    }

    ColumnLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Tokens.space3

        Text {
            Layout.fillWidth: true
            text: root.message
            visible: root.message.length > 0
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeSm
            wrapMode: Text.Wrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Tokens.space2

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: "Cancel"
                onClicked: root.close()
            }

            Button {
                id: confirmButton
                text: root.confirmLabel
                onClicked: {
                    root.close()
                    root.confirmed(root.contextId)
                }

                background: Rectangle {
                    radius: Tokens.radiusSm
                    color: root.destructive
                        ? (confirmButton.down ? Tokens.warning : Tokens.warningSurface)
                        : (confirmButton.down
                            ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.4)
                            : Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.24))
                    border.width: 1
                    border.color: root.destructive ? Tokens.warning : Tokens.accent
                }

                contentItem: Text {
                    text: confirmButton.text
                    color: root.destructive ? Tokens.warningText : Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeSm
                    font.weight: Font.Medium
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
            }
        }
    }
}
