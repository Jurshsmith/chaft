import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property string messageId: ""
    property string reaction: ""
    property bool actionsEnabled: false
    property bool messageDeleted: false
    property bool warningRow: false
    readonly property bool canAdd: actionsEnabled && reaction.length > 0 && messageId.length > 0 && !messageDeleted && !warningRow
    signal addRequested(string messageId, string reaction)

    width: Math.max(44, quickReactionLabel.implicitWidth + 16)
    height: 24
    radius: Tokens.radiusSm
    visible: canAdd
    color: quickReactionMouse.containsMouse ? Tokens.secureSurface : Tokens.surfaceRaised
    border.color: Tokens.borderSubtle
    activeFocusOnTab: canAdd

    Accessible.role: Accessible.Button
    Accessible.name: reaction
    Accessible.description: "Add reaction"
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.canAdd) {
            root.addRequested(root.messageId, root.reaction);
        }
    }

    Text {
        id: quickReactionLabel
        anchors.centerIn: parent
        text: root.reaction
        color: Tokens.textMuted
        font.pixelSize: 12
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: quickReactionMouse
        anchors.fill: parent
        enabled: root.canAdd
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activate()
    }

    ToolTip.visible: quickReactionMouse.containsMouse && quickReactionMouse.enabled
    ToolTip.text: "Add " + root.reaction

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activate();
            event.accepted = true;
        }
    }
}
