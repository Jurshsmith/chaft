import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string deviceId: ""
    property string keyPackageId: ""
    property bool openMls: false
    property bool runtimeReady: false
    property bool canManageAccess: true
    property string accessUnavailableReason: "Only owners and admins can change workspace access."
    property bool privateChannelSelected: false
    property string selectedChannelId: ""
    readonly property bool hasKeyPackage: keyPackageId.length > 0
    readonly property string shortDeviceLabel: {
        var value = String(root.deviceId || "")
        return value.length > 14 ? value.slice(0, 7) + "..." + value.slice(value.length - 4) : value
    }
    readonly property string rowTitle: shortDeviceLabel.length > 0
        ? "Access file for support code " + shortDeviceLabel
        : "Access file"
    readonly property string supportDetailText: keyPackageId.trim().length > 0
        ? "Support detail: " + keyPackageId
        : "No access file detail"
    readonly property string accessUpdateStatusText: openMls
        ? "Access file ready."
        : "Access file unavailable."
    signal workspaceMlsRequested(string keyPackageId)
    signal channelMlsRequested(string channelId, string keyPackageId)

    function accessActionUnavailableReason(scope) {
        if (!root.runtimeReady) {
            return "Open a workspace before changing access."
        }
        if (!root.canManageAccess) {
            return root.accessUnavailableReason
        }
        if (!root.openMls) {
            return "This access file is not ready."
        }
        if (!root.hasKeyPackage) {
            return "This access file is missing."
        }
        if (scope === "channel" && root.selectedChannelId.length === 0) {
            return "Choose a private room first."
        }
        return ""
    }

    width: parent ? parent.width : 360
    height: privateChannelSelected ? 96 : 76
    radius: Tokens.radiusSm
    color: Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: rowTitle
    Accessible.description: accessUpdateStatusText + " " + supportDetailText

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 5

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Text {
                    Layout.fillWidth: true
                    text: root.rowTitle
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeSm
                    font.weight: Font.DemiBold
                    elide: Text.ElideMiddle
                }

                Text {
                    Layout.fillWidth: true
                    text: root.supportDetailText
                    color: Tokens.textMuted
                    font.family: Tokens.fontMono
                    font.pixelSize: Tokens.fontSizeXs
                    elide: Text.ElideMiddle
                }
            }

            Text {
                text: root.openMls ? "Ready" : "Missing"
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
                ToolTip.visible: keyKindMouse.containsMouse
                ToolTip.text: root.openMls ? "Access file ready" : "Access file missing"

                MouseArea {
                    id: keyKindMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.NoButton
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Button {
                Layout.fillWidth: true
                text: "Grant workspace access"
                enabled: root.runtimeReady && root.canManageAccess && root.openMls && root.hasKeyPackage
                Accessible.name: "Grant workspace access"
                Accessible.description: enabled
                    ? "Advanced: add this access file to the workspace for support code "
                        + root.shortDeviceLabel
                    : root.accessActionUnavailableReason("workspace")
                ToolTip.visible: hovered
                ToolTip.text: enabled
                    ? "Advanced: add this access file to the workspace"
                    : root.accessActionUnavailableReason("workspace")
                onClicked: root.workspaceMlsRequested(root.keyPackageId)
            }

            Button {
                Layout.fillWidth: true
                visible: root.privateChannelSelected
                text: "Grant room access"
                enabled: root.runtimeReady && root.canManageAccess && root.openMls
                    && root.selectedChannelId.length > 0 && root.hasKeyPackage
                Accessible.name: "Grant room access"
                Accessible.description: enabled
                    ? "Advanced: add this access file to the room for support code "
                        + root.shortDeviceLabel
                    : root.accessActionUnavailableReason("channel")
                ToolTip.visible: hovered
                ToolTip.text: enabled
                    ? "Advanced: add this access file to the room"
                    : root.accessActionUnavailableReason("channel")
                onClicked: root.channelMlsRequested(root.selectedChannelId, root.keyPackageId)
            }
        }
    }
}
