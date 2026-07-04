import QtQuick
import Chaft

Rectangle {
    id: root
    property bool encrypted: false

    implicitWidth: 74
    implicitHeight: 22
    visible: encrypted
    radius: Tokens.radiusSm
    color: Tokens.secureSurface

    Accessible.role: Accessible.StaticText
    Accessible.name: "Encrypted message"

    Text {
        anchors.centerIn: parent
        text: "Encrypted"
        color: Tokens.secure
        font.pixelSize: 12
        font.weight: Font.DemiBold
    }
}
