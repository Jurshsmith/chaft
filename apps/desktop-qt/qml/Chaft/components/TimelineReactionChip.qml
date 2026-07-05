import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property string messageId: ""
    property string reaction: ""
    property int count: 0
    property bool mine: false
    property bool actionsEnabled: false
    property bool messageDeleted: false
    property bool warningRow: false
    readonly property bool canRemove: actionsEnabled && mine && reaction.length > 0 && messageId.length > 0 && !messageDeleted && !warningRow
    signal removeRequested(string messageId, string reaction)

    width: Math.max(44, reactionLabel.implicitWidth + countPill.width + 20)
    height: 24
    radius: Tokens.radiusSm
    color: mine ? (reactionMouse.containsMouse && canRemove ? Tokens.surfaceRaised : Tokens.secureSurface) : Tokens.surfaceRaised
    border.color: mine && reactionMouse.containsMouse && canRemove ? Tokens.secure : (mine ? Tokens.secure : Tokens.borderSubtle)
    activeFocusOnTab: canRemove

    Accessible.role: canRemove ? Accessible.Button : Accessible.StaticText
    Accessible.name: reaction + " " + String(count)
    Accessible.description: canRemove ? "Remove reaction" : "Reaction count"
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.canRemove) {
            root.removeRequested(root.messageId, root.reaction);
        }
    }

    Text {
        id: reactionLabel
        anchors.left: parent.left
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        text: root.reaction
        color: root.mine ? Tokens.secure : Tokens.textMuted
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
    }

    Rectangle {
        id: countPill
        anchors.right: parent.right
        anchors.rightMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        width: countLabel.implicitWidth + 8
        height: 16
        radius: Tokens.radiusXs
        color: root.mine
            ? Qt.rgba(Tokens.secure.r, Tokens.secure.g, Tokens.secure.b, 0.18)
            : Qt.rgba(Tokens.textMuted.r, Tokens.textMuted.g, Tokens.textMuted.b, 0.14)

        Text {
            id: countLabel
            anchors.centerIn: parent
            text: String(root.count)
            color: root.mine ? Tokens.secure : Tokens.textMuted
            font.family: Tokens.fontMono
            font.pixelSize: Tokens.fontSizeXs
            font.weight: Font.DemiBold
        }
    }

    MouseArea {
        id: reactionMouse
        anchors.fill: parent
        enabled: root.canRemove
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activate()
    }

    ToolTip.visible: reactionMouse.containsMouse && reactionMouse.enabled
    ToolTip.text: "Remove " + root.reaction

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activate();
            event.accepted = true;
        }
    }
}
