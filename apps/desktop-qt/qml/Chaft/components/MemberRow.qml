import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string deviceId: ""
    property string avatarId: ""
    property string workspaceId: ""
    property string displayLabel: ""
    property string initial: "?"
    property string roleLabel: ""
    property string roleValue: ""
    property var roleOptions: []
    property bool owner: false
    property bool localDevice: false
    property bool canRemove: false
    property bool showRemoveAction: false
    property bool showRoleEditor: false
    property bool canChangeRole: false
    property bool canMessage: false
    property string removeUnavailableReason: "Only owners and admins can remove people."
    property string roleUnavailableReason: "Only owners and admins can change roles."
    signal removeRequested(string deviceId, string displayLabel)
    signal copyDeviceRequested(string deviceId)
    signal messageRequested(string deviceId, string displayLabel)
    signal roleChangeRequested(string deviceId, string role)
    readonly property string shortDeviceLabel: {
        var value = String(root.deviceId || "")
        return value.length > 14 ? value.slice(0, 7) + "..." + value.slice(value.length - 4) : value
    }

    function roleOptionIndex(role) {
        var normalized = String(role || "").trim().toLowerCase()
        for (var i = 0; i < root.roleOptions.length; i += 1) {
            if (String(root.roleOptions[i].role || "").trim().toLowerCase() === normalized) {
                return i
            }
        }
        return root.roleOptions.length > 0 ? 0 : -1
    }

    width: parent ? parent.width : 320
    height: 58
    radius: Tokens.radiusSm
    color: localDevice ? Tokens.secureSurface : Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: displayLabel
    Accessible.description: roleLabel + ". " + (localDevice ? "This is you. " : "")
        + "Support code " + shortDeviceLabel

    RowLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8

        AvatarMark {
            Layout.preferredWidth: 30
            Layout.preferredHeight: 30
            avatarId: root.avatarId
            workspaceId: root.workspaceId
            identityId: root.deviceId
            displayName: root.displayLabel
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: 1

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Text {
                    Layout.fillWidth: true
                    text: root.displayLabel
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeSm
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }

                Text {
                    visible: root.localDevice
                    text: "You"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                }
            }

            Text {
                Layout.fillWidth: true
                visible: !root.showRoleEditor
                text: root.roleLabel
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                elide: Text.ElideRight
            }
        }

        ComboBox {
            visible: root.showRoleEditor
            enabled: root.canChangeRole
            Layout.preferredWidth: 104
            Layout.preferredHeight: 30
            model: root.roleOptions
            textRole: "label"
            valueRole: "role"
            currentIndex: root.roleOptionIndex(root.roleValue)
            Accessible.name: "Role for " + root.displayLabel
            Accessible.description: enabled ? root.displayLabel : root.roleUnavailableReason
            onActivated: function (index) {
                var row = root.roleOptions[index] || ({})
                var nextRole = String(row.role || "").trim().toLowerCase()
                if (nextRole.length > 0 && nextRole !== String(root.roleValue || "").trim().toLowerCase()) {
                    root.roleChangeRequested(root.deviceId, nextRole)
                }
            }
            ToolTip.visible: hovered && !enabled
            ToolTip.text: root.roleUnavailableReason
        }

        Button {
            id: actionsButton
            visible: root.canMessage || (!root.localDevice
                && (root.showRemoveAction || root.deviceId.length > 0))
            text: "⋯"
            Layout.preferredWidth: 34
            Layout.preferredHeight: 30
            Accessible.name: "Actions for " + root.displayLabel
            onClicked: memberActions.open()
            ToolTip.visible: hovered
            ToolTip.text: Accessible.name

            Menu {
                id: memberActions
                y: actionsButton.height

                MenuItem {
                    text: root.localDevice ? "Message yourself" : "Message"
                    visible: root.canMessage
                    onTriggered: root.messageRequested(root.deviceId, root.displayLabel)
                }

                MenuItem {
                    text: "Copy support code"
                    onTriggered: root.copyDeviceRequested(root.deviceId)
                }

                MenuSeparator {
                    visible: root.showRemoveAction
                }

                MenuItem {
                    text: "Remove from workspace"
                    visible: root.showRemoveAction
                    enabled: root.canRemove
                    onTriggered: root.removeRequested(root.deviceId, root.displayLabel)
                }
            }
        }
    }
}
