import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property bool repairEnabled: false
    property bool addressAvailable: false
    property bool busy: false
    readonly property string actionLabel: busy
        ? "Fetching..."
        : (addressAvailable ? "Fetch history" : "Add address")
    readonly property string tooltip: busy
        ? "Chaft is already fetching history"
        : (addressAvailable
            ? (repairEnabled
                ? "Fetch missing history from a saved teammate"
                : "History fetch is unavailable right now")
            : "Add a teammate address, then fetch history")
    signal repairRequested

    width: 108
    height: 24
    radius: Tokens.radiusSm
    color: repairMouse.containsMouse && repairEnabled ? Tokens.secureSurface : Tokens.surfaceRaised
    border.color: repairEnabled ? Tokens.borderSubtle : Tokens.warning
    opacity: repairEnabled ? 1.0 : 0.72
    activeFocusOnTab: visible && repairEnabled

    Accessible.role: repairEnabled ? Accessible.Button : Accessible.StaticText
    Accessible.name: actionLabel
    Accessible.description: tooltip
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.repairEnabled) {
            root.repairRequested();
        }
    }

    Text {
        anchors.centerIn: parent
        text: root.actionLabel
        color: root.repairEnabled ? Tokens.textMuted : Tokens.warningText
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: repairMouse
        anchors.fill: parent
        scrollGestureEnabled: false
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
