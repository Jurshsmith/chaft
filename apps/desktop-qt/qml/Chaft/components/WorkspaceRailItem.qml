import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property string workspaceId: ""
    property string workspaceName: "Workspace"
    property string initial: "C"
    property bool selected: false
    property bool actionable: true
    property int unreadCount: 0
    readonly property string unreadLabel: unreadCount > 99 ? "99+" : String(unreadCount)
    signal activated(string workspaceId)

    width: 40
    height: 40
    radius: Tokens.radiusMd
    color: selected ? Tokens.accent : Tokens.railElevated
    opacity: railMouse.enabled ? 1 : 0.58

    Accessible.role: Accessible.Button
    Accessible.name: root.unreadCount > 0
        ? workspaceName + ", " + root.unreadLabel + " unread"
        : workspaceName
    Accessible.description: selected ? "Current workspace" : "Switch workspace"
    Accessible.onPressAction: {
        if (railMouse.enabled) {
            root.activated(root.workspaceId)
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: parent.radius
        visible: !root.selected && railMouse.enabled && railMouse.containsMouse
        color: Qt.rgba(Tokens.railText.r, Tokens.railText.g, Tokens.railText.b, 0.12)
    }

    Text {
        anchors.centerIn: parent
        text: root.initial
        color: root.selected ? Tokens.onAccent : Tokens.railText
        font.pixelSize: Tokens.fontSizeLg
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: railMouse
        anchors.fill: parent
        scrollGestureEnabled: false
        enabled: root.actionable && root.workspaceId.length > 0
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activated(root.workspaceId)
    }

    Rectangle {
        id: unreadBadge
        visible: root.unreadCount > 0
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.topMargin: -4
        anchors.rightMargin: -4
        width: Math.max(16, unreadBadgeText.implicitWidth + 8)
        height: 16
        radius: Tokens.radiusMd
        color: Tokens.accent
        border.width: 1
        border.color: Tokens.rail

        Text {
            id: unreadBadgeText
            anchors.centerIn: parent
            text: root.unreadLabel
            color: Tokens.onAccent
            font.pixelSize: Tokens.fontSizeXs
            font.weight: Font.DemiBold
        }
    }

    ToolTip.visible: railMouse.containsMouse
    ToolTip.text: root.unreadCount > 0
        ? root.workspaceName + " — " + root.unreadLabel + " unread"
        : root.workspaceName
}
