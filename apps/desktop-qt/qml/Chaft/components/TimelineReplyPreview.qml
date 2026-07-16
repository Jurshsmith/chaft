import QtQuick
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string label: ""
    property string avatarId: ""
    property string workspaceId: ""
    property string authorDeviceId: ""
    property string authorDisplayName: ""

    implicitHeight: 28
    visible: label.length > 0
    radius: Tokens.radiusSm
    color: Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.StaticText
    Accessible.name: label

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        spacing: 6

        Rectangle {
            Layout.preferredWidth: 3
            Layout.fillHeight: true
            Layout.topMargin: 6
            Layout.bottomMargin: 6
            radius: 2
            color: Tokens.secure
        }

        AvatarMark {
            visible: root.authorDeviceId.length > 0
                || root.avatarId.length > 0
                || root.authorDisplayName.length > 0
            Layout.preferredWidth: 18
            Layout.preferredHeight: 18
            avatarId: root.avatarId
            workspaceId: root.workspaceId
            identityId: root.authorDeviceId
            displayName: root.authorDisplayName
        }

        Text {
            Layout.fillWidth: true
            text: root.label
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }
    }
}
