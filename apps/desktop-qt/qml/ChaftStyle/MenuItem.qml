import QtQuick
import QtQuick.Templates as T
import Chaft

T.MenuItem {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    padding: 6
    leftPadding: 10
    rightPadding: 10
    spacing: 6
    hoverEnabled: true

    contentItem: Text {
        readonly property bool destructive: String(control.text).toLowerCase().indexOf("delete") === 0
            || String(control.text).toLowerCase().indexOf("remove") === 0

        text: control.text
        font.pixelSize: Tokens.fontSizeSm
        color: !control.enabled
            ? Tokens.textMuted
            : destructive ? Tokens.warningText : Tokens.textStrong
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        implicitWidth: 160
        implicitHeight: 28
        radius: Tokens.radiusXs
        color: control.highlighted || (control.hovered && control.enabled)
            ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.2)
            : "transparent"
    }
}
