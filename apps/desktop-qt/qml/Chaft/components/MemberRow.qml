import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string deviceId: ""
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
    height: canMessage || showRemoveAction ? 112 : (showRoleEditor ? 84 : 64)
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

        Rectangle {
            Layout.preferredWidth: 30
            Layout.preferredHeight: 30
            radius: Tokens.radiusMd
            color: root.owner ? Tokens.accent : Tokens.secure

            Text {
                anchors.centerIn: parent
                text: root.initial
                color: Tokens.onAccent
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
            }
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
                    visible: !root.showRoleEditor
                    text: root.roleLabel
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                ComboBox {
                    visible: root.showRoleEditor
                    enabled: root.canChangeRole
                    Layout.preferredWidth: 94
                    Layout.preferredHeight: 28
                    model: root.roleOptions
                    textRole: "label"
                    valueRole: "role"
                    currentIndex: root.roleOptionIndex(root.roleValue)
                    Accessible.name: "Member role"
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
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 4

                Rectangle {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 96
                    Layout.preferredWidth: 132
                    Layout.preferredHeight: 22
                    radius: Tokens.radiusXs
                    color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                    border.width: 1
                    border.color: Tokens.borderSubtle

                    Text {
                        id: deviceChipLabel
                        anchors.fill: parent
                        anchors.leftMargin: 6
                        anchors.rightMargin: 6
                        verticalAlignment: Text.AlignVCenter
                        text: "Code " + root.shortDeviceLabel
                        color: Tokens.textMuted
                        font.family: Tokens.fontMono
                        font.pixelSize: Tokens.fontSizeXs
                        elide: Text.ElideMiddle
                    }
                }

                Button {
                    text: "Copy"
                    Layout.preferredWidth: 64
                    Layout.preferredHeight: 24
                    Accessible.name: "Copy support code"
                    Accessible.description: root.shortDeviceLabel
                    onClicked: root.copyDeviceRequested(root.deviceId)
                    ToolTip.visible: hovered
                    ToolTip.text: "Copy support code"
                }

            }

            RowLayout {
                Layout.fillWidth: true
                visible: root.canMessage || root.showRemoveAction
                spacing: 6

                Item {
                    Layout.fillWidth: true
                }

                Button {
                    text: "Message"
                    visible: root.canMessage
                    Layout.preferredWidth: 82
                    Layout.preferredHeight: 24
                    Accessible.name: "Message " + root.displayLabel
                    Accessible.description: "Start direct message with " + root.displayLabel
                    onClicked: root.messageRequested(root.deviceId, root.displayLabel)
                    ToolTip.visible: hovered
                    ToolTip.text: "Start direct message"
                }

                Button {
                    text: "Remove"
                    visible: root.showRemoveAction
                    enabled: root.canRemove
                    Layout.preferredWidth: 82
                    Layout.preferredHeight: 24
                    Accessible.name: "Remove member"
                    Accessible.description: enabled ? root.displayLabel : root.removeUnavailableReason
                    onClicked: root.removeRequested(root.deviceId, root.displayLabel)
                    ToolTip.visible: hovered
                    ToolTip.text: enabled
                        ? "Remove " + root.displayLabel + " from this workspace"
                        : root.removeUnavailableReason
                }
            }
        }
    }
}
