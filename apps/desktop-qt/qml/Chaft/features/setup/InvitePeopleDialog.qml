import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Dialog {
    id: root

    property var app
    property var roleOptions: []
    readonly property var expiryDays: [1, 7, 30, 0]

    parent: Overlay.overlay
    modal: true
    width: Math.min(480, Math.max(0, (parent ? parent.width : 528) - 32))
    x: parent ? Math.round((parent.width - width) / 2) : 0
    y: parent ? Math.max(16, Math.round((parent.height - height) / 2)) : 0
    padding: Tokens.space4
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    function resetForm() {
        inviteLabelField.text = ""
        roleBox.currentIndex = 0
        expiryBox.currentIndex = 1
    }

    onOpened: {
        root.resetForm()
        inviteLabelField.forceActiveFocus()
    }

    contentItem: ColumnLayout {
        spacing: Tokens.space3

        Text {
            Layout.fillWidth: true
            text: "Invite people"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeXl
            font.weight: Font.Bold
        }

        Text {
            Layout.fillWidth: true
            text: "Create a one-time invite. It contains no workspace secret; access is encrypted to the recipient after they claim it."
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            wrapMode: Text.WordWrap
        }

        LabeledField {
            id: inviteLabelField
            Layout.fillWidth: true
            label: "Name or label (optional)"
            placeholderText: "e.g. Sam Rivera"
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
                    Accessible.name: "Invited role"
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space1

                Text {
                    text: "Expires"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                ComboBox {
                    id: expiryBox
                    Layout.fillWidth: true
                    model: ["1 day", "7 days", "30 days", "Never"]
                    currentIndex: 1
                    Accessible.name: "Invite expiry"
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: routeRow.implicitHeight + Tokens.space2 * 2
            radius: Tokens.radiusSm
            color: Tokens.surfaceRaised
            border.width: 1
            border.color: Tokens.borderSubtle

            RowLayout {
                id: routeRow
                anchors.fill: parent
                anchors.margins: Tokens.space2
                spacing: Tokens.space2

                StatusChip {
                    text: root.app && root.app.preferredInvitePeerEndpoint().length > 0
                        ? "Direct delivery ready"
                        : "Manual exchange"
                    secure: root.app && root.app.preferredInvitePeerEndpoint().length > 0
                    warning: !secure
                    minWidth: 132
                    maxWidth: 168
                }

                Text {
                    Layout.fillWidth: true
                    text: root.app && root.app.preferredInvitePeerEndpoint().length > 0
                        ? "Chaft can complete the claim while this route is reachable."
                        : "Send the invite privately. Chaft packages any follow-up as a secure file when no direct route is available."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
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
                text: "Cancel"
                onClicked: root.close()
            }

            Button {
                text: chaftController.keyTransferInFlight
                    ? "Creating..."
                    : "Create invite"
                enabled: root.app && root.app.runtimeWorkReady
                    && root.app.canManageWorkspaceAccess()
                    && !chaftController.keyTransferInFlight
                onClicked: {
                    var role = String(roleBox.currentValue || "member")
                    var days = root.expiryDays[expiryBox.currentIndex]
                    if (chaftController.prepareClaimableWorkspaceInvite(
                            inviteLabelField.text.trim(),
                            role,
                            root.app.preferredInvitePeerEndpoint(),
                            root.app.inviteExpiresAtIso(days))) {
                        root.close()
                    }
                }
            }
        }
    }
}
