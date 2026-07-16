import QtQuick
import QtQuick.Templates as T
import Chaft

T.Button {
    id: control

    property string variant: "secondary"
    readonly property bool primaryVariant: variant === "primary"
    readonly property bool quietVariant: variant === "quiet" || control.flat
    readonly property bool destructiveVariant: variant === "destructive"

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
        color: control.enabled
            ? (control.primaryVariant
                ? Tokens.onAccent
                : (control.destructiveVariant
                    ? Tokens.warningText
                    : Tokens.textStrong))
            : Tokens.textMuted
        opacity: control.enabled ? 1 : 0.6
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        implicitWidth: 56
        implicitHeight: 28
        radius: Tokens.radiusSm
        color: control.primaryVariant
            ? (control.down
                ? Qt.darker(Tokens.accent, 1.12)
                : (control.hovered && control.enabled
                    ? Qt.lighter(Tokens.accent, 1.06)
                    : Tokens.accent))
            : control.destructiveVariant
                ? (control.down || (control.hovered && control.enabled)
                    ? Qt.rgba(Tokens.warning.r, Tokens.warning.g, Tokens.warning.b, 0.24)
                    : Tokens.warningSurface)
                : control.quietVariant
                    ? (control.down || (control.hovered && control.enabled)
                        ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.10)
                        : "transparent")
                    : control.down
                        ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.22)
                        : control.checked
                            ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.24)
                            : control.hovered && control.enabled
                                ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.14)
                                : Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.08)
        border.width: control.visualFocus ? 2 : (control.quietVariant ? 0 : 1)
        border.color: control.visualFocus
            ? Tokens.accent
            : control.primaryVariant
                ? Tokens.accent
                : control.destructiveVariant
                    ? Tokens.warning
                    : control.checked ? Tokens.accent : Tokens.borderSubtle
    }
}
