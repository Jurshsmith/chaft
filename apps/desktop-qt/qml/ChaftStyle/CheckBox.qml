import QtQuick
import QtQuick.Templates as T
import Chaft

T.CheckBox {
    id: control

    property bool wrapText: false

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding)

    padding: 4
    spacing: 6
    font.pixelSize: Tokens.fontSizeSm
    hoverEnabled: true

    indicator: Rectangle {
        implicitWidth: 16
        implicitHeight: 16
        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        radius: Tokens.radiusXs
        color: control.checked ? Tokens.accent : "transparent"
        border.width: control.visualFocus ? 2 : 1
        border.color: control.visualFocus
            ? Tokens.accent
            : control.checked ? Tokens.accent : Tokens.borderSubtle

        Text {
            anchors.centerIn: parent
            visible: control.checked
            text: "✓"
            color: Tokens.onAccent
            font.pixelSize: Tokens.fontSizeXs
            font.weight: Font.Bold
        }
    }

    contentItem: Text {
        leftPadding: control.indicator.width + control.spacing
        text: control.text
        font: control.font
        color: control.enabled ? Tokens.textStrong : Tokens.textMuted
        verticalAlignment: Text.AlignVCenter
        wrapMode: control.wrapText ? Text.WordWrap : Text.NoWrap
        elide: control.wrapText ? Text.ElideNone : Text.ElideRight
    }
}
