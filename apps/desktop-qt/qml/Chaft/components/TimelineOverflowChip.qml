import QtQuick
import Chaft

Rectangle {
    id: root
    property string label: ""
    property int minimumWidth: 78
    property int maximumWidth: 160

    visible: label.length > 0
    width: Math.min(maximumWidth, Math.max(minimumWidth, overflowText.implicitWidth + 18))
    height: 24
    radius: Tokens.radiusSm
    color: Tokens.surfaceRaised
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.StaticText
    Accessible.name: label

    Text {
        id: overflowText
        anchors.centerIn: parent
        width: parent.width - 14
        text: root.label
        color: Tokens.textMuted
        font.pixelSize: 12
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }
}
