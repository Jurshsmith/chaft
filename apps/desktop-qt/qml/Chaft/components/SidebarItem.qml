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
    property bool actionable: true
    readonly property bool hasSecondaryLabel: secondaryLabel.length > 0
    readonly property string channelKindLabel: privateChannel ? "Private channel" : "Channel"
    readonly property string unreadLabel: unreadCount > 0 ? String(unreadCount) + " unread" : "No unread messages"
    height: hasSecondaryLabel ? 48 : 34
    width: parent ? parent.width : 220
    activeFocusOnTab: root.actionable
    opacity: channelMouse.enabled ? 1 : 0.58
    signal activated

    Accessible.role: Accessible.Button
    Accessible.name: "# " + label
    Accessible.description: channelKindLabel + ". " + (selected ? "Current channel" : "Switch channel") + ". " + unreadLabel + (hasDraft ? ". Draft saved" : "")
    Accessible.onPressAction: {
        if (channelMouse.enabled) {
            root.activated();
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: Tokens.radiusSm
        color: root.selected
            ? Tokens.sidebarActive
            : channelMouse.containsMouse && channelMouse.enabled
                ? Qt.rgba(Tokens.sidebarActive.r, Tokens.sidebarActive.g, Tokens.sidebarActive.b, 0.55)
                : "transparent"
        border.color: root.activeFocus ? Tokens.accent : "transparent"
        border.width: root.activeFocus ? 2 : 0
    }

    Column {
        anchors.left: parent.left
        anchors.right: root.unreadCount > 0 ? unreadBadge.left : (root.privateChannel ? privateDot.left : parent.right)
        anchors.leftMargin: 10
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1

        Text {
            width: parent.width
            text: "# " + root.label
            color: root.selected ? Tokens.sidebarTextStrong : Tokens.sidebarText
            font.pixelSize: Tokens.fontSizeMd
            font.weight: root.unreadCount > 0 ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }

        Text {
            visible: root.hasSecondaryLabel
            width: parent.width
            text: (root.hasDraft ? "✎ " : "") + root.secondaryLabel
            color: root.hasDraft
                ? (root.selected ? Tokens.sidebarTextStrong : Tokens.sidebarTextSoft)
                : (root.selected ? Tokens.sidebarTextSoft : Tokens.sidebarTextMuted)
            font.pixelSize: Tokens.fontSizeXs
            font.weight: root.hasDraft ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }
    }

    Text {
        id: privateDot
        visible: root.privateChannel
        anchors.verticalCenter: parent.verticalCenter
        anchors.right: root.unreadCount > 0 ? unreadBadge.left : parent.right
        anchors.rightMargin: 8
        text: "🔒"
        color: Tokens.secure
        font.pixelSize: Tokens.fontSizeXs - 1

        Accessible.role: Accessible.StaticText
        Accessible.name: "Private channel"
    }

    Rectangle {
        id: unreadBadge
        visible: root.unreadCount > 0
        anchors.verticalCenter: parent.verticalCenter
        anchors.right: parent.right
        anchors.rightMargin: 8
        width: 24
        height: 20
        radius: Tokens.radiusMd
        color: Tokens.accent

        Text {
            anchors.centerIn: parent
            text: String(root.unreadCount)
            color: Tokens.onAccent
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.DemiBold
        }
    }

    MouseArea {
        id: channelMouse
        anchors.fill: parent
        scrollGestureEnabled: false
        enabled: root.actionable
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activated()
    }

    ToolTip.visible: channelMouse.containsMouse
    ToolTip.text: root.label

    Keys.onPressed: function (event) {
        if (!root.actionable) {
            return;
        }
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activated();
            event.accepted = true;
        }
    }
}
