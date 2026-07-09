import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string label: ""
    property string detail: ""
    property bool warning: false

    implicitWidth: 148
    implicitHeight: 30
    Layout.preferredWidth: implicitWidth
    Layout.preferredHeight: implicitHeight
    radius: Tokens.radiusMd
    color: warning ? Tokens.warningSurface : Tokens.secureSurface
    border.color: warning ? Tokens.warning : Tokens.borderSubtle
    clip: true

    Accessible.role: Accessible.StaticText
    Accessible.name: label
    Accessible.description: detail

    Text {
        id: routeLabel
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        verticalAlignment: Text.AlignVCenter
        text: root.label
        color: root.warning ? Tokens.warningText : Tokens.secure
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }

    ToolTip.visible: routeMouse.containsMouse
    ToolTip.text: root.detail.length > 0 ? root.detail : root.label

    MouseArea {
        id: routeMouse
        anchors.fill: parent
        scrollGestureEnabled: false
        hoverEnabled: true
        acceptedButtons: Qt.NoButton
    }
}
