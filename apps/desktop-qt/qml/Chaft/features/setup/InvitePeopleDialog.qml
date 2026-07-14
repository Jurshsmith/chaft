import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Dialog {
    id: root

    property var app
    property var roleOptions: []
    property bool creationPending: false
    property bool creationDispatching: false
    property string creationError: ""
    readonly property var expiryDays: [1, 7, 30, 0]
    readonly property bool secureRouteReady: root.app
        && root.app.preferredInvitePeerEndpoint().length > 0

    parent: Overlay.overlay
    modal: true
    width: Math.min(480, Math.max(0, (parent ? parent.width : 528) - 32))
    x: parent ? Math.round((parent.width - width) / 2) : 0
    y: parent ? Math.max(16, Math.round((parent.height - height) / 2)) : 0
    padding: Tokens.space4
    closePolicy: root.creationPending
        ? Popup.NoAutoClose
        : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)

    function resetForm() {
        inviteLabelField.text = ""
        roleBox.currentIndex = 0
        expiryBox.currentIndex = 1
    }

    onOpened: {
        root.resetForm()
        root.creationPending = false
        root.creationDispatching = false
        root.creationError = ""
        inviteLabelField.forceActiveFocus()
    }

    function finishCreationIfReady() {
        if (!root.creationPending || root.creationDispatching
                || chaftController.keyTransferInFlight) {
            return
        }
        var parsed = null
        try {
            parsed = JSON.parse(String(chaftController.keyTransferJson || ""))
        } catch (error) {
            parsed = null
        }
        if (parsed !== null
                && String(parsed.kind || "") === "chaft.workspace-invite.v2") {
            root.creationPending = false
            root.close()
            return
        }
        root.creationPending = false
        root.creationError = String(chaftController.syncStatus
            || "Could not create the invite. Try again.")
    }

    Connections {
        target: chaftController

        function onKeyTransferJsonChanged() {
            root.finishCreationIfReady()
        }

        function onKeyTransferInFlightChanged() {
            if (root.creationPending && !chaftController.keyTransferInFlight) {
                Qt.callLater(root.finishCreationIfReady)
            }
        }
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
            text: "Create a one-time invite. It contains no message keys, but anyone who receives it can claim the selected role once. Send it privately."
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
                    text: root.secureRouteReady
                        ? "Secure delivery ready"
                        : "Delivery unavailable"
                    secure: root.secureRouteReady
                    warning: !secure
                    minWidth: 132
                    maxWidth: 168
                }

                Text {
                    Layout.fillWidth: true
                    text: root.secureRouteReady
                        ? "Chaft can complete the claim while this peer route is reachable."
                        : "Keep Chaft online while it prepares a secure delivery route, then try again."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }
            }
        }

        Text {
            Layout.fillWidth: true
            visible: root.creationError.length > 0
            text: root.creationError
            color: Tokens.warningText
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            visible: String(roleBox.currentValue || "member") === "admin"
                && expiryBox.currentIndex === 3
            text: "Admin access with no expiry is high risk. Prefer a short expiry and send the invite directly to the recipient."
            color: Tokens.warningText
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
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
                text: root.creationPending || chaftController.keyTransferInFlight
                    ? "Creating..."
                    : "Create invite"
                enabled: root.app && root.app.runtimeWorkReady
                    && root.app.canManageWorkspaceAccess()
                    && root.secureRouteReady
                    && !root.creationPending
                    && !chaftController.keyTransferInFlight
                onClicked: {
                    root.creationError = ""
                    var role = String(roleBox.currentValue || "member")
                    var days = root.expiryDays[expiryBox.currentIndex]
                    root.creationPending = true
                    root.creationDispatching = true
                    var accepted = chaftController.prepareClaimableWorkspaceInvite(
                            inviteLabelField.text.trim(),
                            role,
                            root.app.preferredInvitePeerEndpoint(),
                            root.app.inviteExpiresAtIso(days))
                    root.creationDispatching = false
                    if (accepted) {
                        if (!chaftController.keyTransferInFlight) {
                            Qt.callLater(root.finishCreationIfReady)
                        }
                        return
                    } else {
                        root.creationPending = false
                        root.creationError = String(chaftController.syncStatus
                            || "Could not create the invite. Try again.")
                    }
                }
            }
        }
    }
}
