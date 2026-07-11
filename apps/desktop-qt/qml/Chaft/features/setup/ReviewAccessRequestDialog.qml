import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Dialog {
    id: root

    property var app
    property var request: ({})
    property var roleOptions: []
    readonly property var expiryDays: [1, 7, 30, 0]
    signal approveRequested(string role, int days)
    signal declineRequested()

    parent: Overlay.overlay
    modal: true
    width: Math.min(480, Math.max(0, (parent ? parent.width : 528) - 32))
    x: parent ? Math.round((parent.width - width) / 2) : 0
    y: parent ? Math.max(16, Math.round((parent.height - height) / 2)) : 0
    padding: Tokens.space4
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    onOpened: {
        roleBox.currentIndex = 0
        expiryBox.currentIndex = 1
    }

    contentItem: ColumnLayout {
        spacing: Tokens.space3

        Text {
            Layout.fillWidth: true
            text: "Review access request"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeXl
            font.weight: Font.Bold
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Tokens.space1

            Text {
                Layout.fillWidth: true
                text: String(root.request.displayName
                    || root.request.requesterDisplayName
                    || "Unnamed person")
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                visible: String(root.request.note || "").trim().length > 0
                text: String(root.request.note || "")
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeSm
                wrapMode: Text.WordWrap
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Tokens.space2

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space1

                Text {
                    text: "Role"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                ComboBox {
                    id: roleBox
                    Layout.fillWidth: true
                    model: root.roleOptions
                    textRole: "label"
                    valueRole: "role"
                    Accessible.name: "Approved role"
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space1

                Text {
                    text: "Invite expires"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                ComboBox {
                    id: expiryBox
                    Layout.fillWidth: true
                    model: ["1 day", "7 days", "30 days", "Never"]
                    currentIndex: 1
                    Accessible.name: "Approved invite expiry"
                }
            }
        }

        Text {
            Layout.fillWidth: true
            text: "Approval creates access for this device and sends an encrypted response when a route is available."
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Tokens.space2

            Button {
                text: "Decline"
                onClicked: root.declineRequested()
            }

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: "Cancel"
                onClicked: root.close()
            }

            Button {
                text: "Approve"
                enabled: !chaftController.keyTransferInFlight
                onClicked: root.approveRequested(
                    String(roleBox.currentValue || "member"),
                    root.expiryDays[expiryBox.currentIndex])
            }
        }
    }
}
