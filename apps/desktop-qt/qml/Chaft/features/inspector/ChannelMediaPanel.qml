pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Chaft

ColumnLayout {
    id: root
    property var attachments: []
    property var recentAttachments: []
    property bool runtimeReady: false
    readonly property int attachmentCount: (attachments || []).length
    signal saveRequested(string messageId, var attachment)
    signal copyRequested(var attachment)

    Layout.fillWidth: true
    spacing: 8

    function byteSizeLabel(byteLen) {
        var value = Number(byteLen || 0);
        if (value >= 1024 * 1024) {
            return (value / (1024 * 1024)).toFixed(value >= 10 * 1024 * 1024 ? 0 : 1) + " MB";
        }
        if (value >= 1024) {
            return (value / 1024).toFixed(value >= 10 * 1024 ? 0 : 1) + " KB";
        }
        return String(value) + " B";
    }

    function attachmentSelectorFor(attachment) {
        var attachmentId = String((attachment && attachment.attachmentId) || "");
        if (attachmentId.length > 0) {
            return attachmentId;
        }
        return String((attachment && attachment.blobHash) || "");
    }

    function attachmentDetailLabel(attachment) {
        return String((attachment && attachment.mediaType) || "application/octet-stream") + " | " + root.byteSizeLabel(Number((attachment && attachment.byteLen) || 0)) + ((attachment && attachment.localBlobAvailable === false) ? " | missing locally" : "");
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8

        Text {
            Layout.fillWidth: true
            text: "Media"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeLg
            font.weight: Font.DemiBold
        }

        Text {
            text: String(root.attachmentCount)
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.DemiBold
            Accessible.role: Accessible.StaticText
            Accessible.name: root.attachmentCount === 1 ? "1 channel file" : String(root.attachmentCount) + " channel files"
        }
    }

    Text {
        Layout.fillWidth: true
        visible: root.attachmentCount === 0
        text: "No channel files"
        color: Tokens.textMuted
        font.pixelSize: Tokens.fontSizeSm
        elide: Text.ElideRight
    }

    ListView {
        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(210, contentHeight)
        visible: root.attachmentCount > 0
        clip: true
        interactive: contentHeight > height
        spacing: 6
        model: root.recentAttachments

        delegate: AttachmentRow {
            id: attachmentRow
            required property var modelData

            width: ListView.view.width
            attachment: attachmentRow.modelData
            detailText: root.attachmentDetailLabel(attachmentRow.modelData)
            selector: root.attachmentSelectorFor(attachmentRow.modelData)
            runtimeReady: root.runtimeReady
            onSaveRequested: function (messageId, attachment) {
                root.saveRequested(messageId, attachment);
            }
            onCopyRequested: function (attachment) {
                root.copyRequested(attachment);
            }
        }
    }
}
