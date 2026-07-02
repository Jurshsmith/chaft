import QtQuick
import QtQuick.Controls
import Chaft

Item {
    id: root
    property string label: ""
    property string secondaryLabel: ""
    property bool selected: false
    property int unreadCount: 0
    property bool privateChannel: false
    property bool hasDraft: false
    readonly property bool hasSecondaryLabel: secondaryLabel.length > 0
    height: hasSecondaryLabel ? 48 : 34
    width: parent ? parent.width : 220

    Rectangle {
        anchors.fill: parent
        radius: Tokens.radiusSm
        color: root.selected ? "#313746" : "transparent"
    }

    Column {
        anchors.left: parent.left
        anchors.right: root.unreadCount > 0
            ? unreadBadge.left
            : (root.privateChannel ? privateDot.left : parent.right)
        anchors.leftMargin: 10
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1

        Text {
            width: parent.width
            text: "# " + root.label
            color: root.selected ? "white" : "#c7ccd8"
            font.pixelSize: 14
            font.weight: root.unreadCount > 0 ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }

        Text {
            visible: root.hasSecondaryLabel
            width: parent.width
            text: root.secondaryLabel
            color: root.hasDraft ? Tokens.accent : (root.selected ? "#d8deea" : "#8e96a8")
            font.pixelSize: 11
            font.weight: root.hasDraft ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }
    }

    Rectangle {
        id: privateDot
        visible: root.privateChannel
        anchors.verticalCenter: parent.verticalCenter
        anchors.right: root.unreadCount > 0 ? unreadBadge.left : parent.right
        anchors.rightMargin: 8
        width: 6
        height: 6
        radius: 3
        color: Tokens.secure
    }

    Rectangle {
        id: unreadBadge
        visible: root.unreadCount > 0
        anchors.verticalCenter: parent.verticalCenter
        anchors.right: parent.right
        anchors.rightMargin: 8
        width: 24
        height: 20
        radius: 7
        color: Tokens.accent

        Text {
            anchors.centerIn: parent
            text: String(root.unreadCount)
            color: "white"
            font.pixelSize: 12
            font.weight: Font.DemiBold
        }
    }
}
