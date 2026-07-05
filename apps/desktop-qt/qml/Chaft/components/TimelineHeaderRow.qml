import QtQuick
import QtQuick.Layouts
import Chaft

RowLayout {
    id: root
    property string primaryLabel: ""
    property string channelLabel: ""
    property string timeLabel: ""

    spacing: 8

    Accessible.role: Accessible.StaticText
    Accessible.name: root.accessibleLabel()

    function accessibleLabel() {
        var parts = []
        if (primaryLabel.length > 0) {
            parts.push(primaryLabel)
        }
        if (channelLabel.length > 0) {
            parts.push(channelLabel)
        }
        if (timeLabel.length > 0) {
            parts.push(timeLabel)
        }
        return parts.join(", ")
    }

    Text {
        Layout.fillWidth: true
        text: root.primaryLabel
        color: Tokens.textStrong
        font.pixelSize: Tokens.fontSizeMd
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }

    Text {
        Layout.preferredWidth: Math.min(160, implicitWidth)
        text: root.channelLabel
        visible: text.length > 0
        color: Tokens.textMuted
        font.pixelSize: Tokens.fontSizeXs
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }

    Text {
        text: root.timeLabel
        visible: text.length > 0
        color: Tokens.textMuted
        font.family: Tokens.fontMono
        font.pixelSize: Tokens.fontSizeXs
        font.weight: Font.DemiBold
    }
}
