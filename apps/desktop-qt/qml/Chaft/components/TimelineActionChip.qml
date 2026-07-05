import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property string label: ""
    property string tooltip: label
    property bool destructive: false
    property int minimumWidth: 44
    readonly property bool actionable: root.enabled && root.visible
    readonly property bool hovered: actionMouse.containsMouse && root.actionable
    signal activated

    width: Math.max(minimumWidth, actionLabel.implicitWidth + 16)
    height: 24
    radius: Tokens.radiusSm
    color: hovered ? (destructive ? Tokens.warningSurface : Tokens.secureSurface) : Tokens.surfaceRaised
    border.color: Tokens.borderSubtle
    activeFocusOnTab: actionable

    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.description: tooltip
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.actionable) {
            root.activated();
        }
    }

    Text {
        id: actionLabel
        anchors.centerIn: parent
        text: root.label
        color: root.destructive ? Tokens.warningText : Tokens.textMuted
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: actionMouse
        anchors.fill: parent
        enabled: root.actionable
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activate()
    }

    ToolTip.visible: actionMouse.containsMouse && actionMouse.enabled
    ToolTip.text: root.tooltip

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activate();
            event.accepted = true;
        }
    }
}
