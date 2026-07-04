import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property string label: ""
    property string latestReplyLabel: ""
    readonly property string tooltip: latestReplyLabel.length > 0 ? "Latest: " + latestReplyLabel : "Thread replies"
    signal openRequested

    width: Math.max(70, threadReplyText.implicitWidth + 18)
    height: 24
    radius: Tokens.radiusSm
    color: Tokens.secureSurface
    border.color: Tokens.borderSubtle
    activeFocusOnTab: visible

    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.description: tooltip
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.visible) {
            root.openRequested();
        }
    }

    Text {
        id: threadReplyText
        anchors.centerIn: parent
        text: root.label
        color: Tokens.secure
        font.pixelSize: 12
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: threadReplyMouse
        anchors.fill: parent
        enabled: root.visible
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activate()
    }

    ToolTip.visible: threadReplyMouse.containsMouse && threadReplyMouse.enabled
    ToolTip.text: root.tooltip

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activate();
            event.accepted = true;
        }
    }
}
