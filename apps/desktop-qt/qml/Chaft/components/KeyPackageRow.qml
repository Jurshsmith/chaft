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
    property bool privateChannelSelected: false
    property string selectedChannelId: ""
    readonly property bool hasKeyPackage: keyPackageId.length > 0
    signal workspaceMlsRequested(string keyPackageId)
    signal channelMlsRequested(string channelId, string keyPackageId)

    width: parent ? parent.width : 360
    height: privateChannelSelected ? 96 : 76
    radius: Tokens.radiusSm
    color: Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: deviceId
    Accessible.description: (openMls ? "OpenMLS key package. " : "Key package. ") + keyPackageId

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
                    text: root.deviceId
                    color: Tokens.textStrong
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                    elide: Text.ElideMiddle
                }

                Text {
                    Layout.fillWidth: true
                    text: root.keyPackageId
                    color: Tokens.textMuted
                    font.pixelSize: 10
                    elide: Text.ElideMiddle
                }
            }

            Text {
                text: root.openMls ? "OpenMLS" : "Key"
                color: Tokens.textMuted
                font.pixelSize: 11
                font.weight: Font.DemiBold
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Button {
                Layout.fillWidth: true
                text: "Workspace MLS"
                enabled: root.runtimeReady && root.openMls && root.hasKeyPackage
                Accessible.name: "Add to workspace MLS"
                Accessible.description: enabled ? root.keyPackageId : "Workspace MLS add is unavailable"
                onClicked: root.workspaceMlsRequested(root.keyPackageId)
            }

            Button {
                Layout.fillWidth: true
                visible: root.privateChannelSelected
                text: "Channel MLS"
                enabled: root.runtimeReady && root.openMls && root.selectedChannelId.length > 0 && root.hasKeyPackage
                Accessible.name: "Add to channel MLS"
                Accessible.description: enabled ? root.keyPackageId : "Channel MLS add is unavailable"
                onClicked: root.channelMlsRequested(root.selectedChannelId, root.keyPackageId)
            }
        }
    }
}
