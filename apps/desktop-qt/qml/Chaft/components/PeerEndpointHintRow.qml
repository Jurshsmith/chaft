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
    signal useRequested(string endpoint)
    signal syncRequested(string endpoint)
    signal saveRequested(string endpoint)

    width: parent ? parent.width : 360
    height: 82
    radius: Tokens.radiusSm
    color: backupPeer ? Tokens.secureSurface : Tokens.surfaceBase
    border.color: Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: endpoint
    Accessible.description: kindLabel + ". " + detailLabel

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 4

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                Layout.fillWidth: true
                text: root.endpoint
                color: Tokens.textStrong
                font.pixelSize: 12
                font.weight: Font.DemiBold
                elide: Text.ElideMiddle
            }

            Text {
                text: root.kindLabel
                color: root.backupPeer ? Tokens.secure : Tokens.textMuted
                font.pixelSize: 11
                font.weight: Font.DemiBold
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Text {
                Layout.fillWidth: true
                text: root.detailLabel
                color: Tokens.textMuted
                font.pixelSize: 11
                elide: Text.ElideRight
            }

            Button {
                text: "Use"
                Layout.preferredWidth: 48
                enabled: !root.expired && root.hasEndpoint
                Accessible.name: "Use peer endpoint"
                Accessible.description: enabled ? root.endpoint : "Peer endpoint is expired or empty"
                onClicked: root.useRequested(root.endpoint)
            }

            Button {
                text: "Sync"
                Layout.preferredWidth: 54
                enabled: root.runtimeReady && !root.syncInFlight && !root.expired && root.hasEndpoint
                Accessible.name: "Sync peer endpoint"
                Accessible.description: enabled ? root.endpoint : "Sync is unavailable for this peer endpoint"
                onClicked: root.syncRequested(root.endpoint)
            }

            Button {
                text: root.savedAsBackup ? "Saved" : "Save"
                Layout.preferredWidth: 60
                enabled: !root.savedAsBackup && !root.expired && root.hasEndpoint
                Accessible.name: root.savedAsBackup ? "Backup peer saved" : "Save backup peer"
                Accessible.description: enabled ? root.endpoint : "Backup peer is already saved, expired, or empty"
                onClicked: root.saveRequested(root.endpoint)
            }
        }
    }
}
