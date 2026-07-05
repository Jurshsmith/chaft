import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

ColumnLayout {
    id: root
    property string label: ""
    property alias text: field.text
    property alias placeholderText: field.placeholderText
    property alias echoMode: field.echoMode
    property alias fieldActiveFocus: field.activeFocus
    signal accepted()

    spacing: Tokens.space1

    function forceFieldFocus() {
        field.forceActiveFocus()
    }

    Text {
        Layout.fillWidth: true
        visible: root.label.length > 0
        text: root.label
        color: Tokens.textMuted
        font.pixelSize: Tokens.fontSizeXs
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }

    TextField {
        id: field
        Layout.fillWidth: true
        Accessible.name: root.label.length > 0 ? root.label : field.placeholderText
        onAccepted: root.accepted()
    }
}
