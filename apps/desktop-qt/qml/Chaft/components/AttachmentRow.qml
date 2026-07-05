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
    readonly property string displayName: String((attachment && attachment.displayName) || "attachment")
    readonly property string attachmentId: String((attachment && attachment.attachmentId) || "")
    readonly property string copyActionLabel: attachmentId.length > 0 ? "Copy attachment ID" : "Copy blob hash"
    readonly property bool missingLocalBlob: Boolean(attachment && attachment.localBlobAvailable === false)
    readonly property string availabilityLabel: missingLocalBlob ? "Blob is missing locally" : "Blob is available locally"
    readonly property string saveBlockedReason: {
        if (!runtimeReady)
            return "Runtime is not ready";
        if (missingLocalBlob)
            return "Blob is missing locally";
        if (selector.length <= 0 || messageId.length <= 0)
            return "Attachment is missing a save selector";
        return "";
    }
    signal saveRequested(string messageId, var attachment)
    signal copyRequested(var attachment)

    width: parent ? parent.width : 320
    height: 58
    radius: Tokens.radiusSm
    color: Tokens.surfaceBase
    border.color: missingLocalBlob ? Tokens.warning : Tokens.borderSubtle

    Accessible.role: Accessible.ListItem
    Accessible.name: displayName
    Accessible.description: detailText.length > 0 ? detailText + ". " + availabilityLabel : availabilityLabel

    RowLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Text {
                Layout.fillWidth: true
                text: root.displayName
                color: root.missingLocalBlob ? Tokens.warningText : Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
                elide: Text.ElideMiddle
            }

            Text {
                Layout.fillWidth: true
                text: root.detailText
                color: root.missingLocalBlob ? Tokens.warningText : Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                elide: Text.ElideRight
            }
        }

        Button {
            text: "Save"
            Layout.preferredWidth: 56
            enabled: root.runtimeReady && !root.missingLocalBlob && root.selector.length > 0 && root.messageId.length > 0
            Accessible.name: "Save " + root.displayName
            Accessible.description: enabled ? "Save attachment to disk" : root.saveBlockedReason
            onClicked: root.saveRequested(root.messageId, root.attachment)

            ToolTip.visible: hovered
            ToolTip.text: enabled ? "Save attachment" : root.saveBlockedReason
        }

        Button {
            text: root.attachmentId.length > 0 ? "Copy ID" : "Copy hash"
            Layout.preferredWidth: 82
            enabled: root.selector.length > 0
            Accessible.name: root.copyActionLabel + " for " + root.displayName
            Accessible.description: enabled ? root.copyActionLabel : "Attachment has no copyable identifier"
            onClicked: root.copyRequested(root.attachment)

            ToolTip.visible: hovered
            ToolTip.text: enabled ? root.copyActionLabel : "No copyable identifier"
        }
    }
}
