pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Popup {
    id: overlay

    // Shortcut registry rows: { label, keys }. Set by App.qml from the single
    // shortcutRows source shared with the command palette labels.
    property var rows: []

    width: Math.min(440, parent ? parent.width - Tokens.space4 * 2 : 440)
    x: parent ? Math.round((parent.width - width) / 2) : 0
    y: parent ? Math.round((parent.height - height) / 2) : 0
    modal: true
    focus: true
    padding: Tokens.space3
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Rectangle {
        radius: Tokens.radiusMd
        color: Tokens.surfaceRaised
        border.width: 1
        border.color: Tokens.borderSubtle
    }

    contentItem: ColumnLayout {
        spacing: Tokens.space2

        Text {
            Layout.fillWidth: true
            text: "Keyboard shortcuts"
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeLg
            font.weight: Font.Bold
            elide: Text.ElideRight

            Accessible.role: Accessible.Heading
            Accessible.name: "Keyboard shortcuts"
        }

        Repeater {
            model: overlay.rows

            delegate: RowLayout {
                id: shortcutRow
                required property var modelData

                Layout.fillWidth: true
                spacing: Tokens.space2

                Accessible.role: Accessible.StaticText
                Accessible.name: shortcutRow.modelData.label + ", "
                    + shortcutRow.modelData.keys

                Text {
                    Layout.fillWidth: true
                    text: shortcutRow.modelData.label
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeSm
                    elide: Text.ElideRight
                }

                Text {
                    text: shortcutRow.modelData.keys
                    color: Tokens.textMuted
                    font.family: Tokens.fontMono
                    font.pixelSize: Tokens.fontSizeSm
                }
            }
        }

        Text {
            Layout.fillWidth: true
            Layout.topMargin: Tokens.space1
            text: "Esc closes this overlay"
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideRight
        }
    }
}
