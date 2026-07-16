pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

ColumnLayout {
    id: root
    property int replyCount: 0
    property var replyPreviews: []
    property bool runtimeReady: false
    property string workspaceId: ""
    property string messageId: ""
    property bool messageDeleted: false
    readonly property bool hasReplies: replyCount > 0
    signal replyRequested

    Layout.fillWidth: true
    visible: hasReplies
    spacing: 8

    Accessible.name: "Thread"
    Accessible.description: root.replyCountLabel(root.replyCount)

    function authorLabel(authorDisplayName, authorDeviceId) {
        var displayName = String(authorDisplayName || "").trim();
        if (displayName.length > 0) {
            return displayName;
        }
        var deviceId = String(authorDeviceId || "");
        return deviceId.length > 0 ? "Unnamed person " + root.shortDeviceId(deviceId) : "";
    }

    function shortDeviceId(deviceId) {
        var value = String(deviceId || "");
        return value.length > 14 ? value.slice(0, 7) + "..." + value.slice(value.length - 4) : value;
    }

    function compactInlineText(value, fallback) {
        var text = String(value || "").replace(/\s+/g, " ").trim();
        if (text.length === 0) {
            return fallback || "";
        }
        return text.length > 96 ? text.slice(0, 93) + "..." : text;
    }

    function replyCountLabel(count) {
        var value = Number(count || 0);
        return value === 1 ? "1 reply" : String(value) + " replies";
    }

    function overflowLabel(count, previews) {
        var hidden = Number(count || 0) - ((previews || []).length);
        if (hidden <= 0) {
            return "";
        }
        return String(hidden) + (hidden === 1 ? " older reply" : " older replies");
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8

        Text {
            Layout.fillWidth: true
            text: "Thread"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeLg
            font.weight: Font.DemiBold
        }

        Rectangle {
            Layout.preferredWidth: Math.max(70, threadCountText.implicitWidth + 16)
            Layout.preferredHeight: 22
            radius: Tokens.radiusSm
            color: Tokens.secureSurface
            border.color: Tokens.borderSubtle

            Text {
                id: threadCountText
                anchors.centerIn: parent
                text: root.replyCountLabel(root.replyCount)
                color: Tokens.secure
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }
    }

    ListView {
        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(230, contentHeight)
        clip: true
        interactive: contentHeight > height
        spacing: 6
        model: root.replyPreviews

        delegate: Rectangle {
            id: replyRow
            required property var modelData
            readonly property string authorText: root.authorLabel(modelData.authorDisplayName, modelData.authorDeviceId)
            readonly property string bodyText: root.compactInlineText(modelData.body, "message")

            width: ListView.view.width
            height: 58
            radius: Tokens.radiusSm
            color: Tokens.surfaceBase
            border.color: Tokens.borderSubtle

            Accessible.role: Accessible.ListItem
            Accessible.name: authorText + ": " + bodyText

            RowLayout {
                anchors.fill: parent
                anchors.margins: 8
                spacing: 8

                AvatarMark {
                    Layout.preferredWidth: 32
                    Layout.preferredHeight: 32
                    avatarId: String(replyRow.modelData.authorAvatarId || "")
                    workspaceId: root.workspaceId
                    identityId: String(replyRow.modelData.authorDeviceId || "")
                    displayName: replyRow.authorText
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    Text {
                        Layout.fillWidth: true
                        text: replyRow.authorText
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeSm
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.fillWidth: true
                        text: replyRow.bodyText
                        color: replyRow.modelData.deleted ? Tokens.textMuted : Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeSm
                        elide: Text.ElideRight
                    }
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8

        Text {
            Layout.fillWidth: true
            text: root.overflowLabel(root.replyCount, root.replyPreviews)
            visible: text.length > 0
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideRight
        }

        Button {
            text: "Reply"
            Layout.preferredWidth: 70
            enabled: root.runtimeReady && root.messageId.length > 0 && !root.messageDeleted
            Accessible.name: "Reply to thread"
            Accessible.description: enabled ? "Reply to selected message" : "Thread reply unavailable"
            onClicked: root.replyRequested()
        }
    }
}
