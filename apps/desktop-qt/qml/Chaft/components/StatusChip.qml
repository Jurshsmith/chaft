import QtQuick
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

    Layout.preferredWidth: Math.min(maxWidth, Math.max(minWidth, chipText.implicitWidth + horizontalPadding * 2))
    Layout.preferredHeight: 30
    radius: 7
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
        font.pixelSize: 12
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }
}
