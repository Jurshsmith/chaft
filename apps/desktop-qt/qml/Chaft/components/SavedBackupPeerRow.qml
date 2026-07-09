import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

RowLayout {
    id: root
    property string endpoint: ""
    property string statusText: ""
    readonly property string supportDetailText: endpoint.trim().length > 0
        ? "Support detail: " + endpoint
        : "No backup address"
    signal removeRequested(string endpoint)

    Layout.fillWidth: true
    spacing: 6

    Accessible.role: Accessible.ListItem
    Accessible.name: "Backup address"
    Accessible.description: statusText + ". " + supportDetailText

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 1

        Text {
            Layout.fillWidth: true
            text: "Backup address"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }

        Text {
            Layout.fillWidth: true
            text: root.supportDetailText
            color: Tokens.textMuted
            font.family: Tokens.fontMono
            font.pixelSize: Tokens.fontSizeXs
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
        text: "Remove"
        implicitWidth: 74
        Accessible.name: "Remove backup address"
        Accessible.description: "Remove this saved backup address"
        onClicked: root.removeRequested(root.endpoint)
    }
}
