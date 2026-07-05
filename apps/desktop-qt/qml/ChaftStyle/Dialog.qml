import QtQuick
import QtQuick.Templates as T
import Chaft

T.Dialog {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            contentWidth + leftPadding + rightPadding,
                            implicitHeaderWidth,
                            implicitFooterWidth)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding
                             + (implicitHeaderHeight > 0 ? implicitHeaderHeight + spacing : 0)
                             + (implicitFooterHeight > 0 ? implicitFooterHeight + spacing : 0))

    padding: 16
    spacing: 10

    background: Rectangle {
        radius: Tokens.radiusMd
        color: Tokens.surfaceRaised
        border.width: 1
        border.color: Tokens.borderSubtle
    }

    header: Text {
        text: control.title
        visible: control.title.length > 0
        elide: Text.ElideRight
        font.pixelSize: Tokens.fontSizeLg
        font.weight: Font.Bold
        color: Tokens.textStrong
        padding: 16
        bottomPadding: 0
    }

    T.Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.45)
    }

    T.Overlay.modeless: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.25)
    }
}
