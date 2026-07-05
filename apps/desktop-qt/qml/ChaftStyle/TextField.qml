import QtQuick
import QtQuick.Templates as T
import Chaft

T.TextField {
    id: control

    implicitWidth: implicitBackgroundWidth + leftInset + rightInset
                   || Math.max(contentWidth, placeholder.implicitWidth) + leftPadding + rightPadding
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding,
                             placeholder.implicitHeight + topPadding + bottomPadding)

    padding: 6
    leftPadding: 10
    rightPadding: 10

    font.pixelSize: Tokens.fontSizeSm
    color: Tokens.textStrong
    placeholderTextColor: Tokens.textMuted
    selectionColor: Tokens.accent
    selectedTextColor: Tokens.onAccent
    verticalAlignment: TextInput.AlignVCenter

    Text {
        id: placeholder
        x: control.leftPadding
        y: control.topPadding
        width: control.width - control.leftPadding - control.rightPadding
        height: control.height - control.topPadding - control.bottomPadding
        text: control.placeholderText
        font: control.font
        color: control.placeholderTextColor
        verticalAlignment: control.verticalAlignment
        visible: !control.length && !control.preeditText
            && (!control.activeFocus || control.horizontalAlignment !== Qt.AlignHCenter)
        elide: Text.ElideRight
    }

    background: Rectangle {
        implicitWidth: 120
        implicitHeight: 28
        radius: Tokens.radiusSm
        color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? Tokens.accent : Tokens.borderSubtle
    }
}
