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
    readonly property var claimLimitOptions: [
        { label: "1 claim", value: 1 },
        { label: "2 claims", value: 2 },
        { label: "5 claims", value: 5 },
        { label: "10 claims", value: 10 },
        { label: "20 claims", value: 20 }
    ]
    readonly property int selectedMaxClaims:
        Math.max(1, Number(claimLimitBox.currentValue || 1))
    readonly property bool secureRouteReady: root.app
        && root.app.preferredInvitePeerEndpoint().length > 0
    readonly property bool adminInvite:
        String(roleBox.currentValue || "member") === "admin"
    readonly property bool reusableInvite: root.selectedMaxClaims > 1
    readonly property bool inviteNeverExpires: expiryBox.currentIndex === 3
    readonly property bool highRiskInvite: root.adminInvite
        && (root.reusableInvite || root.inviteNeverExpires)

    onHighRiskInviteChanged: {
        if (!root.highRiskInvite) {
            highRiskConfirmation.checked = false
        }
    }

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
        claimLimitBox.currentIndex = 0
        highRiskConfirmation.checked = false
    }

    function claimLimitHelperText() {
        if (root.selectedMaxClaims === 1) {
            return "The first successful claim grants one device the selected role."
        }
        return "Up to " + root.selectedMaxClaims
            + " devices can claim the same invite. Revoke it early if it spreads beyond the intended group."
    }

    function highRiskWarningText() {
        if (root.reusableInvite && root.inviteNeverExpires) {
            return "This invite can grant admin access " + root.selectedMaxClaims
                + " times and never expires. Use a smaller limit and shorter expiry when possible."
        }
        if (root.reusableInvite) {
            return "This invite can grant admin access " + root.selectedMaxClaims
                + " times. Send it only to the intended recipients."
        }
        return "This admin invite never expires. Prefer a short expiry and send it directly to the recipient."
    }

    function highRiskConfirmationText() {
        var text = root.reusableInvite
            ? "I understand this invite can grant admin access "
                + root.selectedMaxClaims + " times"
            : "I understand this invite grants admin access"
        return root.inviteNeverExpires ? text + " and never expires" : text
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
            text: "Set a role, expiry, and claim limit. No message keys are included; send the invite privately."
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
                    onActivated: highRiskConfirmation.checked = false
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
                    onActivated: highRiskConfirmation.checked = false
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Tokens.space1

            Text {
                text: "Claim limit"
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
            }

            ComboBox {
                id: claimLimitBox
                Layout.fillWidth: true
                model: root.claimLimitOptions
                textRole: "label"
                valueRole: "value"
                currentIndex: 0
                Accessible.name: "Invite claim limit"
                onActivated: highRiskConfirmation.checked = false
            }

            Text {
                Layout.fillWidth: true
                text: root.claimLimitHelperText()
                color: root.reusableInvite ? Tokens.warningText : Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                wrapMode: Text.WordWrap
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
                        ? "Automatic delivery"
                        : "Manual exchange"
                    secure: true
                    warning: !root.secureRouteReady
                    minWidth: 132
                    maxWidth: 168
                }

                Text {
                    Layout.fillWidth: true
                    text: root.secureRouteReady
                        ? "Chaft can complete the claim while this peer route is reachable."
                        : "The recipient saves their claim, you open it in Access Requests, then return the encrypted access file."
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
            visible: root.app && root.app.keyTransferIsInviteResponse()
            text: "Return or save the encrypted access response before creating another invite."
            color: Tokens.warningText
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            visible: root.highRiskInvite
            text: root.highRiskWarningText()
            color: Tokens.warningText
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
        }

        CheckBox {
            id: highRiskConfirmation
            Layout.fillWidth: true
            visible: root.highRiskInvite
            text: root.highRiskConfirmationText()
            Accessible.name: text
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Tokens.space2

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: "Cancel"
                enabled: !root.creationPending
                    && !chaftController.keyTransferInFlight
                ToolTip.visible: hovered && !enabled
                ToolTip.text: "Wait for invite creation to finish"
                onClicked: root.close()
            }

            Button {
                text: root.creationPending || chaftController.keyTransferInFlight
                    ? "Creating..."
                    : "Create invite"
                enabled: root.app && root.app.runtimeWorkReady
                    && root.app.canManageWorkspaceAccess()
                    && !root.app.keyTransferIsInviteResponse()
                    && (!root.highRiskInvite || highRiskConfirmation.checked)
                    && !root.creationPending
                    && !chaftController.keyTransferInFlight
                onClicked: {
                    root.creationError = ""
                    var role = String(roleBox.currentValue || "member")
                    var days = root.expiryDays[expiryBox.currentIndex]
                    root.creationPending = true
                    root.creationDispatching = true
                    var accepted = chaftController.prepareClaimableWorkspaceInviteWithMaxClaims(
                            inviteLabelField.text.trim(),
                            role,
                            root.app.preferredInvitePeerEndpoint(),
                            root.app.inviteExpiresAtIso(days),
                            root.selectedMaxClaims)
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
