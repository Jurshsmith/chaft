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
    property bool directMessage: false
    property string avatarId: ""
    property string avatarWorkspaceId: ""
    property string avatarIdentityId: ""
    property bool hasDraft: false
    property bool muted: false
    property bool archived: false
    property bool actionable: true
    readonly property bool hasSecondaryLabel: secondaryLabel.length > 0
    readonly property bool showPrivateAffordance: privateChannel && !directMessage
    readonly property string itemPrefix: directMessage ? "@ " : "# "
    readonly property string channelKindLabel: archived
        ? "Archived room"
        : (directMessage ? "Direct message" : (privateChannel ? "Private room" : "Room"))
    readonly property string selectedLabel: selected
        ? (directMessage ? "Current direct message" : "Current room")
        : (directMessage ? "Open direct message" : "Open room")
    readonly property string unreadLabel: unreadCount > 0 ? String(unreadCount) + " unread" : "No unread messages"
    readonly property string mutedLabel: muted ? ". Muted" : ""
    height: hasSecondaryLabel ? 48 : 34
    width: parent ? parent.width : 220
    activeFocusOnTab: root.actionable
    opacity: channelMouse.enabled ? (root.muted || root.archived ? 0.78 : 1) : 0.58
    signal activated

    Accessible.role: Accessible.Button
    Accessible.name: itemPrefix + label
    Accessible.description: channelKindLabel + ". " + selectedLabel + ". " + unreadLabel + mutedLabel + (hasDraft ? ". Draft saved" : "")
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

    AvatarMark {
        id: directAvatar
        visible: root.directMessage
        anchors.left: parent.left
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 26
        height: 26
        avatarId: root.avatarId
        workspaceId: root.avatarWorkspaceId
        identityId: root.avatarIdentityId
        displayName: root.label
    }

    Column {
        anchors.left: parent.left
        anchors.right: root.unreadCount > 0 ? unreadBadge.left : (root.showPrivateAffordance ? privateDot.left : parent.right)
        anchors.leftMargin: root.directMessage ? 42 : 10
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1

        Text {
            width: parent.width
            text: root.itemPrefix + root.label
            color: root.selected
                ? Tokens.sidebarTextStrong
                : (root.muted || root.archived ? Tokens.sidebarTextMuted : Tokens.sidebarText)
            font.pixelSize: Tokens.fontSizeMd
            font.weight: root.unreadCount > 0 && !root.muted && !root.archived ? Font.DemiBold : Font.Normal
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
        visible: root.showPrivateAffordance
        anchors.verticalCenter: parent.verticalCenter
        anchors.right: root.unreadCount > 0 ? unreadBadge.left : parent.right
        anchors.rightMargin: 8
        text: "🔒"
        color: Tokens.secure
        font.pixelSize: Tokens.fontSizeXs - 1

        Accessible.role: Accessible.StaticText
        Accessible.name: "Private room"
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
        color: root.muted ? Tokens.surfaceRaised : Tokens.accent
        border.width: root.muted ? 1 : 0
        border.color: root.muted ? Tokens.borderSubtle : "transparent"

        Text {
            anchors.centerIn: parent
            text: String(root.unreadCount)
            color: root.muted ? Tokens.sidebarTextMuted : Tokens.onAccent
            font.pixelSize: Tokens.fontSizeSm
            font.weight: root.muted ? Font.Normal : Font.DemiBold
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
        + (root.archived ? " (archived)" : "")
        + (root.muted ? " (muted)" : "")

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
