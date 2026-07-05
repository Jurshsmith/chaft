import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

RowLayout {
    id: root
    property string endpoint: ""
    property string statusText: ""
    signal removeRequested(string endpoint)

    Layout.fillWidth: true
    spacing: 6

    Accessible.role: Accessible.ListItem
    Accessible.name: endpoint
    Accessible.description: statusText

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 1

        Text {
            Layout.fillWidth: true
            text: root.endpoint
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            elide: Text.ElideMiddle
        }

        Text {
            Layout.fillWidth: true
            text: root.statusText
            color: Tokens.textMuted
            opacity: 0.72
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideRight
        }
    }

    Button {
        text: "x"
        implicitWidth: 28
        Accessible.name: "Remove backup peer"
        Accessible.description: root.endpoint
        onClicked: root.removeRequested(root.endpoint)
    }
}
