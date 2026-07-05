import QtQuick
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string endpoint: ""
    property string statusText: ""
    property string stateLabel: ""
    property color stateColor: Tokens.textMuted

    width: parent ? parent.width : 320
    height: 54
    radius: Tokens.radiusSm
    color: Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: endpoint
    Accessible.description: stateLabel.length > 0 ? stateLabel + ". " + statusText : statusText

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 2

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                Layout.fillWidth: true
                text: root.endpoint
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
                elide: Text.ElideMiddle
            }

            Text {
                text: root.stateLabel
                color: root.stateColor
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
            }
        }

        Text {
            Layout.fillWidth: true
            text: root.statusText
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideRight
        }
    }
}
