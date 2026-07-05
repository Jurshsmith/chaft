import QtQuick
import Chaft

Rectangle {
    id: root
    property string label: ""
    property string authorDeviceId: ""
    property bool warning: false
    property bool encrypted: false
    readonly property color markColor: warning
        ? Tokens.warning
        : authorDeviceId.length > 0 && typeof Themes.authorColor === "function"
            ? Themes.authorColor(authorDeviceId, Tokens.activeTheme.dark === true)
            : Tokens.accent
    readonly property color markTextColor: typeof Themes.readableTextColor === "function"
        ? Themes.readableTextColor(markColor)
        : Tokens.onAccent

    implicitWidth: 36
    implicitHeight: 36
    radius: Tokens.radiusMd
    color: markColor

    Accessible.role: Accessible.StaticText
    Accessible.name: warning ? "Timeline warning" : "Message author " + label

    Text {
        anchors.centerIn: parent
        text: root.warning ? "!" : root.label
        color: root.markTextColor
        font.pixelSize: Tokens.fontSizeMd
        font.weight: Font.DemiBold
    }
}
