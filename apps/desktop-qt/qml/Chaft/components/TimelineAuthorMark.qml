import QtQuick
import Chaft

Rectangle {
    id: root
    property string label: ""
    property bool warning: false
    property bool encrypted: false

    implicitWidth: 36
    implicitHeight: 36
    radius: 7
    color: warning ? Tokens.warning : (encrypted ? Tokens.secure : Tokens.accent)

    Accessible.role: Accessible.StaticText
    Accessible.name: warning ? "Timeline warning" : "Message author " + label

    Text {
        anchors.centerIn: parent
        text: root.warning ? "!" : root.label
        color: "white"
        font.pixelSize: 14
        font.weight: Font.DemiBold
    }
}
