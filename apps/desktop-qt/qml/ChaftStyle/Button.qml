import QtQuick
import QtQuick.Templates as T
import Chaft

T.Button {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    padding: 5
    horizontalPadding: 12
    font.pixelSize: Tokens.fontSizeSm
    font.weight: Font.Medium
    hoverEnabled: true

    contentItem: Text {
        text: control.text
        font: control.font
        color: control.enabled ? Tokens.textStrong : Tokens.textMuted
        opacity: control.enabled ? 1 : 0.6
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        implicitWidth: 56
        implicitHeight: 28
        radius: Tokens.radiusSm
        color: control.down
            ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.22)
            : control.checked
                ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.24)
                : control.hovered && control.enabled
                    ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.14)
                    : Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.08)
        border.width: control.visualFocus ? 2 : 1
        border.color: control.visualFocus
            ? Tokens.accent
            : control.checked ? Tokens.accent : Tokens.borderSubtle
    }
}
