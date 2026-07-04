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
    signal activated(string workspaceId)

    width: 40
    height: 40
    radius: Tokens.radiusMd
    color: selected ? Tokens.accent : Tokens.railElevated
    opacity: railMouse.enabled ? 1 : 0.58

    Accessible.role: Accessible.Button
    Accessible.name: workspaceName
    Accessible.description: selected ? "Current workspace" : "Switch workspace"
    Accessible.onPressAction: {
        if (railMouse.enabled) {
            root.activated(root.workspaceId)
        }
    }

    Text {
        anchors.centerIn: parent
        text: root.initial
        color: "white"
        font.pixelSize: 15
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: railMouse
        anchors.fill: parent
        enabled: root.actionable && root.workspaceId.length > 0
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activated(root.workspaceId)
    }

    ToolTip.visible: railMouse.containsMouse
    ToolTip.text: root.workspaceName
}
