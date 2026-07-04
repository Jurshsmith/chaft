import QtQuick
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string label: ""

    implicitHeight: 28
    visible: label.length > 0
    radius: Tokens.radiusSm
    color: Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.StaticText
    Accessible.name: label

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        spacing: 6

        Rectangle {
            Layout.preferredWidth: 3
            Layout.fillHeight: true
            Layout.topMargin: 6
            Layout.bottomMargin: 6
            radius: 2
            color: Tokens.secure
        }

        Text {
            Layout.fillWidth: true
            text: root.label
            color: Tokens.textMuted
            font.pixelSize: 12
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }
    }
}
