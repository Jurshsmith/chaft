import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string text: ""
    property string description: text
    property bool warning: false
    property bool secure: false
    property int minWidth: 118
    property int maxWidth: 240
    property int horizontalPadding: 10

    implicitWidth: Math.min(maxWidth, Math.max(minWidth, chipText.implicitWidth + horizontalPadding * 2))
    implicitHeight: 30
    Layout.preferredWidth: implicitWidth
    Layout.preferredHeight: implicitHeight
    radius: Tokens.radiusMd
    color: warning ? Tokens.warningSurface : (secure ? Tokens.secureSurface : Tokens.surfaceRaised)
    border.color: warning ? Tokens.warning : Tokens.borderSubtle
    clip: true

    Accessible.role: Accessible.StaticText
    Accessible.name: text
    Accessible.description: description

    Text {
        id: chipText
        anchors.fill: parent
        anchors.leftMargin: root.horizontalPadding
        anchors.rightMargin: root.horizontalPadding
        verticalAlignment: Text.AlignVCenter
        text: root.text
        color: root.warning ? Tokens.warningText : (root.secure ? Tokens.secure : Tokens.textMuted)
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }

    ToolTip.visible: chipMouse.containsMouse
        && root.description.length > 0
        && root.description !== root.text
    ToolTip.text: root.description

    MouseArea {
        id: chipMouse
        anchors.fill: parent
        acceptedButtons: Qt.NoButton
        hoverEnabled: true
    }
}
