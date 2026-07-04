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
    property bool owner: false
    property bool localDevice: false
    property bool canRemove: false
    signal removeRequested(string deviceId)

    width: parent ? parent.width : 320
    height: 50
    radius: Tokens.radiusSm
    color: localDevice ? Tokens.secureSurface : Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: displayLabel
    Accessible.description: roleLabel + ". " + (localDevice ? "Current device. " : "") + deviceId

    RowLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8

        Rectangle {
            Layout.preferredWidth: 30
            Layout.preferredHeight: 30
            radius: 7
            color: root.owner ? Tokens.accent : Tokens.secure

            Text {
                anchors.centerIn: parent
                text: root.initial
                color: "white"
                font.pixelSize: 12
                font.weight: Font.DemiBold
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Text {
                Layout.fillWidth: true
                text: root.displayLabel
                color: Tokens.textStrong
                font.pixelSize: 12
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                text: root.deviceId
                color: Tokens.textMuted
                font.pixelSize: 10
                elide: Text.ElideMiddle
            }
        }

        Text {
            text: root.roleLabel
            color: Tokens.textMuted
            font.pixelSize: 11
            font.weight: Font.DemiBold
        }

        Button {
            text: "x"
            visible: root.canRemove
            Layout.preferredWidth: 28
            Accessible.name: "Remove member"
            Accessible.description: root.deviceId
            onClicked: root.removeRequested(root.deviceId)
            ToolTip.visible: hovered
            ToolTip.text: "Remove member"
        }
    }
}
