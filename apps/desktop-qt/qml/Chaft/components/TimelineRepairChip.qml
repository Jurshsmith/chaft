import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property bool repairEnabled: false
    readonly property string tooltip: repairEnabled ? "Pull missing history from peer" : "Set a peer endpoint to repair history"
    signal repairRequested

    width: 62
    height: 24
    radius: Tokens.radiusSm
    color: repairMouse.containsMouse && repairEnabled ? Tokens.secureSurface : Tokens.surfaceRaised
    border.color: repairEnabled ? Tokens.borderSubtle : Tokens.warning
    opacity: repairEnabled ? 1.0 : 0.72
    activeFocusOnTab: visible && repairEnabled

    Accessible.role: repairEnabled ? Accessible.Button : Accessible.StaticText
    Accessible.name: "Repair"
    Accessible.description: tooltip
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.repairEnabled) {
            root.repairRequested();
        }
    }

    Text {
        anchors.centerIn: parent
        text: "Repair"
        color: root.repairEnabled ? Tokens.textMuted : Tokens.warningText
        font.pixelSize: 12
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: repairMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.repairEnabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activate()
    }

    ToolTip.visible: repairMouse.containsMouse
    ToolTip.text: root.tooltip

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activate();
            event.accepted = true;
        }
    }
}
