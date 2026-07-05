import QtQuick
import QtQuick.Templates as T
import Chaft

T.ComboBox {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding)

    padding: 5
    leftPadding: 10
    rightPadding: 26
    font.pixelSize: Tokens.fontSizeSm
    hoverEnabled: true

    delegate: T.ItemDelegate {
        required property var model
        required property int index

        width: ListView.view.width
        padding: 6
        leftPadding: 10
        highlighted: control.highlightedIndex === index
        hoverEnabled: true

        contentItem: Text {
            text: model[control.textRole.length > 0 ? control.textRole : "modelData"]
            font.pixelSize: Tokens.fontSizeSm
            color: Tokens.textStrong
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }

        background: Rectangle {
            radius: Tokens.radiusXs
            color: parent.highlighted
                ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.22)
                : "transparent"
        }
    }

    indicator: Text {
        x: control.width - width - 8
        y: control.topPadding + (control.availableHeight - height) / 2
        text: "▾"
        color: Tokens.textMuted
        font.pixelSize: Tokens.fontSizeSm
    }

    contentItem: Text {
        text: control.displayText
        font: control.font
        color: control.enabled ? Tokens.textStrong : Tokens.textMuted
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        implicitWidth: 100
        implicitHeight: 28
        radius: Tokens.radiusSm
        color: control.down || control.popup.visible
            ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.16)
            : control.hovered
                ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.12)
                : Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.08)
        border.width: control.visualFocus ? 2 : 1
        border.color: control.visualFocus ? Tokens.accent : Tokens.borderSubtle
    }

    popup: T.Popup {
        y: control.height + 2
        width: control.width
        implicitHeight: contentItem.implicitHeight + topPadding + bottomPadding
        padding: 4

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.popup.visible ? control.delegateModel : null
            currentIndex: control.highlightedIndex
        }

        background: Rectangle {
            radius: Tokens.radiusSm
            color: Tokens.surfaceRaised
            border.width: 1
            border.color: Tokens.borderSubtle
        }
    }
}
