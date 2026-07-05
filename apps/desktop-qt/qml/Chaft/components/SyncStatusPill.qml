import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property string label: ""
    property color tone: Tokens.textMuted
    property string detail: ""
    property bool expanded: false
    signal toggled()

    implicitHeight: 26
    implicitWidth: pillRow.implicitWidth + 20
    radius: height / 2
    color: Qt.rgba(tone.r, tone.g, tone.b, 0.14)
    border.width: 1
    border.color: Qt.rgba(tone.r, tone.g, tone.b, 0.5)
    activeFocusOnTab: true

    Accessible.role: Accessible.Button
    Accessible.name: "Sync status: " + label
    Accessible.description: (expanded ? "Hide" : "Show") + " network controls. " + detail
    Accessible.onPressAction: root.toggled()

    Row {
        id: pillRow
        anchors.centerIn: parent
        spacing: 6

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 8
            height: 8
            radius: width / 2
            color: root.tone
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.Medium
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.expanded ? "▴" : "▾"
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: parent.radius
        color: "transparent"
        border.color: Tokens.accent
        border.width: root.activeFocus ? 2 : 0
    }

    MouseArea {
        id: pillMouse
        anchors.fill: parent
        scrollGestureEnabled: false
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }

    ToolTip.visible: pillMouse.containsMouse && root.detail.length > 0
    ToolTip.text: root.detail

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.toggled();
            event.accepted = true;
        }
    }
}
