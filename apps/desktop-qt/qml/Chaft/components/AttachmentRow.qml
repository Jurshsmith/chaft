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
    readonly property string copyActionLabel: "Copy file support ID"
    readonly property bool missingLocalBlob: Boolean(attachment && attachment.localBlobAvailable === false)
    readonly property string availabilityLabel: missingLocalBlob ? "File is missing on this device" : "File is available on this device"
    readonly property string saveBlockedReason: {
        if (!runtimeReady)
            return "Open a workspace before saving files";
        if (missingLocalBlob)
            return "File is missing on this device";
        if (selector.length <= 0 || messageId.length <= 0)
            return "File cannot be saved yet";
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
            text: "Copy support"
            Layout.preferredWidth: 92
            enabled: root.selector.length > 0
            Accessible.name: root.copyActionLabel + " for " + root.displayName
            Accessible.description: enabled ? root.copyActionLabel : "File has no support ID"
            onClicked: root.copyRequested(root.attachment)

            ToolTip.visible: hovered
            ToolTip.text: enabled ? root.copyActionLabel : "No file support ID"
        }
    }
}
