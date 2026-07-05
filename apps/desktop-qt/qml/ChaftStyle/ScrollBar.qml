import QtQuick
import QtQuick.Templates as T
import Chaft

T.ScrollBar {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    padding: 2
    visible: control.policy !== T.ScrollBar.AlwaysOff
    minimumSize: orientation === Qt.Horizontal ? height / width : width / height

    contentItem: Rectangle {
        implicitWidth: control.interactive ? 8 : 4
        implicitHeight: control.interactive ? 8 : 4
        radius: width / 2
        color: control.pressed
            ? Qt.rgba(Tokens.textMuted.r, Tokens.textMuted.g, Tokens.textMuted.b, 0.65)
            : control.hovered
                ? Qt.rgba(Tokens.textMuted.r, Tokens.textMuted.g, Tokens.textMuted.b, 0.5)
                : Qt.rgba(Tokens.textMuted.r, Tokens.textMuted.g, Tokens.textMuted.b, 0.3)
        opacity: 0.0

        states: State {
            name: "active"
            when: control.policy === T.ScrollBar.AlwaysOn || (control.active && control.size < 1.0)
            PropertyChanges { target: control.contentItem; opacity: 0.9 }
        }

        transitions: Transition {
            from: "active"
            SequentialAnimation {
                PauseAnimation { duration: 450 }
                NumberAnimation { target: control.contentItem; duration: 200; property: "opacity"; to: 0.0 }
            }
        }
    }
}
