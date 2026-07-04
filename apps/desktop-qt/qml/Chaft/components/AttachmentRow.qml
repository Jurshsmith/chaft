import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property var attachment: ({})
    property string detailText: ""
    property string messageId: String((attachment && attachment.messageId) || "")
    property string selector: ""
    property bool runtimeReady: false
    readonly property bool missingLocalBlob: Boolean(attachment && attachment.localBlobAvailable === false)
    signal saveRequested(string messageId, var attachment)
    signal copyRequested(var attachment)

    width: parent ? parent.width : 320
    height: 58
    radius: Tokens.radiusSm
    color: Tokens.surfaceBase
    border.color: missingLocalBlob ? Tokens.warning : Tokens.borderSubtle

    RowLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Text {
                Layout.fillWidth: true
                text: String((root.attachment && root.attachment.displayName) || "attachment")
                color: root.missingLocalBlob ? Tokens.warningText : Tokens.textStrong
                font.pixelSize: 12
                font.weight: Font.DemiBold
                elide: Text.ElideMiddle
            }

            Text {
                Layout.fillWidth: true
                text: root.detailText
                color: root.missingLocalBlob ? Tokens.warningText : Tokens.textMuted
                font.pixelSize: 11
                elide: Text.ElideRight
            }
        }

        Button {
            text: "Save"
            Layout.preferredWidth: 56
            enabled: root.runtimeReady && !root.missingLocalBlob && root.selector.length > 0 && root.messageId.length > 0
            onClicked: root.saveRequested(root.messageId, root.attachment)
        }

        Button {
            text: String((root.attachment && root.attachment.attachmentId) || "").length > 0 ? "Copy ID" : "Copy hash"
            Layout.preferredWidth: 82
            enabled: root.selector.length > 0
            onClicked: root.copyRequested(root.attachment)
        }
    }
}
