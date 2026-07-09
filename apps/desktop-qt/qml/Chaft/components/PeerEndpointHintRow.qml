import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string endpoint: ""
    property string kindLabel: ""
    property string detailLabel: ""
    property bool backupPeer: false
    property bool expired: false
    property bool runtimeReady: false
    property bool syncInFlight: false
    property bool savedAsBackup: false
    readonly property bool hasEndpoint: endpoint.trim().length > 0
    readonly property string rowTitle: kindLabel.length > 0 ? kindLabel : "Teammate address"
    readonly property string supportDetailText: hasEndpoint
        ? "Support detail: " + endpoint
        : "No teammate address"
    signal useRequested(string endpoint)
    signal syncRequested(string endpoint)
    signal saveRequested(string endpoint)

    width: parent ? parent.width : 360
    height: 98
    radius: Tokens.radiusSm
    color: backupPeer ? Tokens.secureSurface : Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: rowTitle
    Accessible.description: (detailLabel.length > 0 ? detailLabel + ". " : "")
        + supportDetailText

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 4

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                Layout.fillWidth: true
                text: root.rowTitle
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
                elide: Text.ElideMiddle
            }
        }

        Text {
            Layout.fillWidth: true
            text: root.supportDetailText
            color: Tokens.textMuted
            font.family: Tokens.fontMono
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideMiddle
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Text {
                Layout.fillWidth: true
                text: root.detailLabel
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                elide: Text.ElideRight
            }

            Button {
                text: "Use"
                Layout.preferredWidth: 48
                enabled: !root.expired && root.hasEndpoint
                Accessible.name: "Use teammate address"
                Accessible.description: enabled
                    ? "Use this address when fetching history later"
                    : "Teammate address is expired or empty"
                onClicked: root.useRequested(root.endpoint)
            }

            Button {
                text: "Fetch"
                Layout.preferredWidth: 54
                enabled: root.runtimeReady && !root.syncInFlight && !root.expired && root.hasEndpoint
                Accessible.name: "Fetch from teammate address"
                Accessible.description: enabled
                    ? "Load history from this teammate"
                    : "History fetch is unavailable for this teammate address"
                onClicked: root.syncRequested(root.endpoint)
            }

            Button {
                text: root.savedAsBackup ? "Saved" : "Save"
                Layout.preferredWidth: 60
                enabled: !root.savedAsBackup && !root.expired && root.hasEndpoint
                Accessible.name: root.savedAsBackup ? "Backup address saved" : "Save as backup address"
                Accessible.description: enabled
                    ? "Save this teammate address as a backup"
                    : "Backup address is already saved, expired, or empty"
                onClicked: root.saveRequested(root.endpoint)
            }
        }
    }
}
