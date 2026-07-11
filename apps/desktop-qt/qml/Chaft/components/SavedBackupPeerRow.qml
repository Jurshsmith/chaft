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
    signal copyRequested(string endpoint)
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
            text: "Backup destination"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }

        Text {
            Layout.fillWidth: true
            visible: false
            text: root.supportDetailText
            color: Tokens.textMuted
            font.family: Tokens.fontMono
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideMiddle
        }

        Text {
            Layout.fillWidth: true
            text: root.statusText.length > 0
                ? root.statusText.charAt(0).toUpperCase() + root.statusText.slice(1)
                : ""
            color: Tokens.textMuted
            opacity: 0.72
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideRight
        }
    }

    Button {
        text: "⋯"
        implicitWidth: 42
        Accessible.name: "More backup destination actions"
        onClicked: backupActionsMenu.open()

        Menu {
            id: backupActionsMenu
            y: parent.height

            MenuItem {
                text: "Copy support address"
                onTriggered: root.copyRequested(root.endpoint)
            }

            MenuItem {
                text: "Remove destination"
                onTriggered: root.removeRequested(root.endpoint)
            }
        }
    }
}
