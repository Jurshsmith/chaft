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
    property string inviteMode: "single"
    readonly property bool formEditable: !root.creationPending
        && !chaftController.keyTransferInFlight
    readonly property var expiryDays: [1, 7, 30, 0]
    readonly property var groupClaimLimitOptions: [
        { label: "5", value: 5 },
        { label: "10", value: 10 },
        { label: "25", value: 25 },
        { label: "50", value: 50 },
        { label: "100", value: 100 },
        { label: "Custom", value: 0 }
    ]
    readonly property bool groupInvite: root.inviteMode === "group"
    readonly property bool customClaimLimit: root.groupInvite
        && Number(groupClaimLimitBox.currentValue) === 0
    readonly property bool customClaimLimitValid: !root.customClaimLimit
        || customClaimLimitField.acceptableInput
    readonly property int selectedMaxClaims: {
        if (!root.groupInvite) {
            return 1
        }
        if (root.customClaimLimit) {
            return root.customClaimLimitValid
                ? Number(customClaimLimitField.text)
                : 0
        }
        return Number(groupClaimLimitBox.currentValue || 10)
    }
    readonly property bool secureRouteReady: root.app
        && root.app.preferredInvitePeerEndpoint().length > 0
    readonly property bool adminInvite:
        String(roleBox.currentValue || "member") === "admin"
    readonly property bool reusableInvite: root.groupInvite
    readonly property bool inviteNeverExpires: expiryBox.currentIndex === 3
    readonly property bool reusableNeverExpires: root.reusableInvite
        && root.inviteNeverExpires
    readonly property bool expandedCapacityInvite: root.reusableInvite
        && root.selectedMaxClaims > 20
    readonly property bool highRiskInvite: root.adminInvite
    readonly property bool inviteRiskWarningVisible: root.highRiskInvite
        || root.reusableNeverExpires
        || root.expandedCapacityInvite

    ButtonGroup {
        id: inviteTypeButtonGroup
    }

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
        : Popup.CloseOnEscape

    function resetForm() {
        inviteLabelField.text = ""
        roleBox.currentIndex = 0
        expiryBox.currentIndex = 1
        root.inviteMode = "single"
        groupClaimLimitBox.currentIndex = 1
        customClaimLimitField.text = ""
        highRiskConfirmation.checked = false
    }

    function maximumJoinsHelperText() {
        if (!root.customClaimLimitValid) {
            return "Each device uses one join. Enter a whole number from 2 to 100."
        }
        if (root.inviteNeverExpires) {
            return "Each device uses one join. It stops after all "
                + root.selectedMaxClaims + " joins are used or it is revoked."
        }
        return "Each device uses one join. It stops when it expires, all "
            + root.selectedMaxClaims + " joins are used, or it is revoked."
    }

    function groupInviteAudienceText() {
        return root.customClaimLimitValid
            ? root.selectedMaxClaims + " devices"
            : "multiple devices"
    }

    function inviteRiskWarningText() {
        var warning = ""
        if (root.highRiskInvite) {
            if (root.reusableInvite && root.inviteNeverExpires) {
                warning = "This invite can grant admin access to "
                    + root.groupInviteAudienceText()
                    + " and never expires. Use fewer joins and a shorter expiry when possible."
            } else if (root.reusableInvite) {
                warning = "This invite can grant admin access to "
                    + root.groupInviteAudienceText()
                    + ". Send it only to the intended people."
            } else if (root.inviteNeverExpires) {
                warning = "This admin invite never expires. Prefer a short expiry and send it privately."
            } else {
                warning = "This invite grants admin access. Admins can invite or remove people and manage workspace access."
            }
        } else if (root.reusableNeverExpires) {
            warning = "This group invite never expires. Anyone with it can use a remaining join until you revoke it."
        }
        if (root.expandedCapacityInvite) {
            if (warning.length > 0) {
                warning += " "
            }
            warning += "Invite limits above 20 require every workspace device to be updated first."
        }
        return warning
    }

    function highRiskConfirmationText() {
        var text = root.reusableInvite
            ? "I understand this invite can grant admin access to "
                + root.groupInviteAudienceText()
            : "I understand this invite grants admin access to one device"
        return root.inviteNeverExpires ? text + " and never expires" : text
    }

    onOpened: {
        root.resetForm()
        if (String(chaftController.smokeUiState || "")
                === "setup-invite-dialog") {
            root.inviteMode = "group"
            groupClaimLimitBox.currentIndex = 4
            expiryBox.currentIndex = 3
            for (var index = 0; index < root.roleOptions.length; index += 1) {
                if (String(root.roleOptions[index].role || "") === "admin") {
                    roleBox.currentIndex = index
                    break
                }
            }
        }
        root.creationPending = false
        root.creationDispatching = false
        root.creationError = ""
        if (root.groupInvite) {
            groupInviteButton.forceActiveFocus()
        } else {
            singleUseInviteButton.forceActiveFocus()
        }
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
            text: "Choose the access, expiry, and join limit."
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            wrapMode: Text.WordWrap
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Tokens.space1

            Text {
                text: "Invite type"
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Tokens.space1

                Button {
                    id: singleUseInviteButton
                    Layout.fillWidth: true
                    text: "Single-use"
                    checkable: true
                    checked: !root.groupInvite
                    enabled: root.formEditable
                    ButtonGroup.group: inviteTypeButtonGroup
                    Accessible.role: Accessible.RadioButton
                    Accessible.name: "Single-use invite"
                    Accessible.description: checked
                        ? "Selected"
                        : "Allow one device to join"
                    onClicked: {
                        root.inviteMode = "single"
                        highRiskConfirmation.checked = false
                    }
                    Keys.onRightPressed: {
                        root.inviteMode = "group"
                        highRiskConfirmation.checked = false
                        groupInviteButton.forceActiveFocus()
                    }
                }

                Button {
                    id: groupInviteButton
                    Layout.fillWidth: true
                    text: "Group"
                    checkable: true
                    checked: root.groupInvite
                    enabled: root.formEditable
                    ButtonGroup.group: inviteTypeButtonGroup
                    Accessible.role: Accessible.RadioButton
                    Accessible.name: "Group invite"
                    Accessible.description: checked
                        ? "Selected"
                        : "Allow multiple devices to join"
                    onClicked: {
                        root.inviteMode = "group"
                        highRiskConfirmation.checked = false
                    }
                    Keys.onLeftPressed: {
                        root.inviteMode = "single"
                        highRiskConfirmation.checked = false
                        singleUseInviteButton.forceActiveFocus()
                    }
                }
            }

            Text {
                Layout.fillWidth: true
                text: root.groupInvite
                    ? "Share one invite privately with the intended people."
                    : "Send this invite privately. One device can use it."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
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
                    enabled: root.formEditable
                    Accessible.name: "Invited role"
                    onActivated: highRiskConfirmation.checked = false
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space1

                Text {
                    text: "Expires after"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                ComboBox {
                    id: expiryBox
                    Layout.fillWidth: true
                    model: ["1 day", "7 days", "30 days", "Never"]
                    currentIndex: 1
                    enabled: root.formEditable
                    Accessible.name: "Invite expiry"
                    onActivated: highRiskConfirmation.checked = false
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            visible: root.groupInvite
            spacing: Tokens.space1

            Text {
                text: "Maximum joins"
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                ComboBox {
                    id: groupClaimLimitBox
                    Layout.fillWidth: true
                    model: root.groupClaimLimitOptions
                    textRole: "label"
                    valueRole: "value"
                    currentIndex: 1
                    enabled: root.formEditable
                    Accessible.name: "Maximum joins"
                    onActivated: {
                        highRiskConfirmation.checked = false
                        if (Number(currentValue) === 0) {
                            Qt.callLater(function() {
                                customClaimLimitField.forceActiveFocus()
                            })
                        }
                    }
                }

                TextField {
                    id: customClaimLimitField
                    Layout.preferredWidth: 120
                    visible: root.customClaimLimit
                    placeholderText: "2–100"
                    maximumLength: 3
                    inputMethodHints: Qt.ImhDigitsOnly
                    enabled: root.formEditable
                    validator: IntValidator {
                        bottom: 2
                        top: 100
                    }
                    Accessible.name: "Custom maximum joins"
                    Accessible.description: root.customClaimLimitValid
                        ? "Maximum number of devices that can join"
                        : "Enter a whole number from 2 to 100"
                    onTextEdited: highRiskConfirmation.checked = false
                }
            }

            Text {
                Layout.fillWidth: true
                text: root.maximumJoinsHelperText()
                color: root.customClaimLimitValid
                    ? Tokens.textMuted
                    : Tokens.warningText
                font.pixelSize: Tokens.fontSizeXs
                wrapMode: Text.WordWrap
            }
        }

        LabeledField {
            id: inviteLabelField
            Layout.fillWidth: true
            label: "Invite label (optional)"
            placeholderText: "e.g. Design team"
            supportText: "For your invite list. Each joiner chooses their own name."
            maximumLength: 80
            enabled: root.formEditable
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
                        : "Manual fallback"
                    secure: true
                    warning: !root.secureRouteReady
                    minWidth: 132
                    maxWidth: 168
                }

                Text {
                    Layout.fillWidth: true
                    text: root.secureRouteReady
                        ? "Join requests can be delivered automatically while this device is reachable."
                        : "If automatic delivery is unavailable, the joiner can transfer the request manually."
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
            visible: root.inviteRiskWarningVisible
            text: root.inviteRiskWarningText()
            color: Tokens.warningText
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
        }

        CheckBox {
            id: highRiskConfirmation
            Layout.fillWidth: true
            visible: root.highRiskInvite
            text: root.highRiskConfirmationText()
            enabled: root.formEditable
            wrapText: true
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
                    && root.customClaimLimitValid
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
