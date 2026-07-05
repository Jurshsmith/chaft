import QtQuick
import QtQuick.Templates as T
import Chaft

T.ToolTip {
    id: control

    x: parent ? (parent.width - implicitWidth) / 2 : 0
    y: -implicitHeight - 6

    implicitWidth: Math.min(320, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: implicitContentHeight + topPadding + bottomPadding

    padding: 6
    horizontalPadding: 10
    delay: 400
    timeout: 7000

    contentItem: Text {
        text: control.text
        font.pixelSize: Tokens.fontSizeXs
        color: Tokens.textStrong
        wrapMode: Text.Wrap
    }

    background: Rectangle {
        radius: Tokens.radiusSm
        color: Tokens.surfaceRaised
        border.width: 1
        border.color: Tokens.borderSubtle
    }
}
