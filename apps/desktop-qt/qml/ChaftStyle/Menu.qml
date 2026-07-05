import QtQuick
import QtQuick.Templates as T
import Chaft

T.Menu {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            contentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding)

    margins: 0
    padding: 4
    overlap: 2

    delegate: MenuItem {}

    contentItem: ListView {
        implicitHeight: contentHeight
        model: control.contentModel
        interactive: Window.window
            ? contentHeight + control.topPadding + control.bottomPadding > Window.window.height
            : false
        currentIndex: control.currentIndex
        clip: true
    }

    background: Rectangle {
        implicitWidth: 180
        radius: Tokens.radiusSm
        color: Tokens.surfaceRaised
        border.width: 1
        border.color: Tokens.borderSubtle
    }

    T.Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.35)
    }
}
