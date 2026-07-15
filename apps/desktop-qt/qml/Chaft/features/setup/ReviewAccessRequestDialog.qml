import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Dialog {
    id: root

    property var app
    property var request: ({})
    property var roleOptions: []
    property string actionError: ""
    readonly property var expiryDays: [1, 7, 30, 0]
    readonly property bool actionPending: chaftController.keyTransferInFlight
    readonly property bool highRiskApproval:
        String(roleBox.currentValue || "member") === "admin"
    readonly property bool approvalNeverExpires: expiryBox.currentIndex === 3
    signal approveRequested(string role, int days)
    signal declineRequested()

    parent: Overlay.overlay
    modal: true
    width: Math.min(480, Math.max(0, (parent ? parent.width : 528) - 32))
    x: parent ? Math.round((parent.width - width) / 2) : 0
    y: parent ? Math.max(16, Math.round((parent.height - height) / 2)) : 0
    padding: Tokens.space4
    closePolicy: root.actionPending ? Popup.NoAutoClose : Popup.CloseOnEscape

    function requesterName() {
        var value = String(root.request.displayName
            || root.request.requesterDisplayName || "").trim()
        return value.length > 0 ? value : "Name not provided"
    }

    function requesterDeviceId() {
        return String(root.request.deviceId
            || root.request.requesterDeviceId || "").trim()
    }

    function requesterDeviceLabel() {
        var deviceId = root.requesterDeviceId()
        if (deviceId.length === 0) {
            return "Device code unavailable"
        }
        return "Device " + (root.app
            ? root.app.shortDeviceId(deviceId)
            : deviceId)
    }

    function requestWorkspaceLabel() {
        return String(root.request.workspaceName || "").trim()
    }

    function requestedRole() {
        var invite = root.request.invite || ({})
        return String(root.request.requestedRole
            || root.request.role || invite.role || "member").trim()
    }

    function roleIndexFor(value) {
        for (var i = 0; i < root.roleOptions.length; i += 1) {
            if (String(root.roleOptions[i].role || "") === value) {
                return i
            }
        }
        for (var fallback = 0; fallback < root.roleOptions.length; fallback += 1) {
            if (String(root.roleOptions[fallback].role || "") === "member") {
                return fallback
            }
        }
        return 0
    }

    onOpened: {
        roleBox.currentIndex = root.roleIndexFor(root.requestedRole())
        expiryBox.currentIndex = 1
        highRiskConfirmation.checked = false
        root.actionError = ""
    }

    contentItem: ColumnLayout {
        spacing: Tokens.space3

        Text {
            Layout.fillWidth: true
            text: "Approve access?"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeXl
            font.weight: Font.Bold
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Tokens.space1

            Text {
                Layout.fillWidth: true
                text: root.requesterName()
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                text: root.requesterDeviceLabel()
                    + (root.requestWorkspaceLabel().length > 0
                        ? " · " + root.requestWorkspaceLabel()
                        : "")
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                text: "The name is provided by the requester; the device code identifies this request."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                wrapMode: Text.WordWrap
            }

            Text {
                Layout.fillWidth: true
                visible: String(root.request.note || "").trim().length > 0
                text: "Note: " + String(root.request.note || "")
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
                    text: "Access role"
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
                    onActivated: highRiskConfirmation.checked = false
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space1

                Text {
                    text: "Approval valid for"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                ComboBox {
                    id: expiryBox
                    Layout.fillWidth: true
                    model: ["1 day", "7 days", "30 days", "Never"]
                    currentIndex: 1
                    Accessible.name: "Approval validity"
                    onActivated: highRiskConfirmation.checked = false
                }
            }
        }

        Text {
            Layout.fillWidth: true
            text: "This controls how long the encrypted approval can be opened. Membership does not expire after joining."
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            visible: root.highRiskApproval
            text: root.approvalNeverExpires
                ? "This grants admin access and the approval never expires. Prefer a shorter validity period."
                : "This grants admin access. Admins can invite or remove people and manage workspace access."
            color: Tokens.warningText
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
        }

        CheckBox {
            id: highRiskConfirmation
            Layout.fillWidth: true
            visible: root.highRiskApproval
            text: root.approvalNeverExpires
                ? "I understand this device will receive admin access from a non-expiring approval"
                : "I understand this device will receive admin access"
            Accessible.name: text
        }

        Text {
            Layout.fillWidth: true
            visible: root.actionError.length > 0
            text: root.actionError
            color: Tokens.warningText
            font.pixelSize: Tokens.fontSizeXs
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Tokens.space2

            Item {
                Layout.fillWidth: true
            }

            Button {
                id: reviewActionsButton
                text: "⋯"
                Layout.preferredWidth: 42
                enabled: !root.actionPending
                Accessible.name: "More request actions"
                onClicked: reviewActionsMenu.open()

                Menu {
                    id: reviewActionsMenu
                    y: reviewActionsButton.height

                    MenuItem {
                        text: "Decline request"
                        onTriggered: root.declineRequested()
                    }
                }
            }

            Button {
                text: "Cancel"
                enabled: !root.actionPending
                onClicked: root.close()
            }

            Button {
                text: root.actionPending ? "Approving..." : "Approve"
                enabled: !root.actionPending
                    && (!root.highRiskApproval || highRiskConfirmation.checked)
                onClicked: {
                    root.actionError = ""
                    root.approveRequested(
                        String(roleBox.currentValue || "member"),
                        root.expiryDays[expiryBox.currentIndex])
                }
            }
        }
    }
}
