import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

ColumnLayout {
    id: root

    property string label: ""
    property string supportText: ""
    property string errorText: ""
    property string cleanText: ""
    property bool requiredField: false
    property alias text: field.text
    property alias placeholderText: field.placeholderText
    property alias echoMode: field.echoMode
    property alias maximumLength: field.maximumLength
    property alias readOnly: field.readOnly
    property alias inputMethodHints: field.inputMethodHints
    property alias fieldActiveFocus: field.activeFocus
    readonly property bool dirty: root.text !== root.cleanText
    signal accepted()
    signal edited(string text)

    spacing: Tokens.space1

    function forceFieldFocus() {
        field.forceActiveFocus()
    }

    function markClean() {
        root.cleanText = root.text
    }

    Text {
        Layout.fillWidth: true
        visible: root.label.length > 0
        text: root.label + (root.requiredField ? " *" : "")
        color: Tokens.textMuted
        font.pixelSize: Tokens.fontSizeXs
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        Accessible.role: Accessible.StaticText
        Accessible.name: root.label
            + (root.requiredField ? ", required" : "")
    }

    TextField {
        id: field
        Layout.fillWidth: true
        Accessible.name: (root.label.length > 0 ? root.label : field.placeholderText)
            + (root.requiredField ? ", required" : "")
        Accessible.description: root.errorText.length > 0
            ? root.errorText
            : root.supportText
        onAccepted: root.accepted()
        onTextEdited: root.edited(field.text)
    }

    Text {
        Layout.fillWidth: true
        visible: root.errorText.length > 0 || root.supportText.length > 0
        text: root.errorText.length > 0 ? root.errorText : root.supportText
        color: root.errorText.length > 0 ? Tokens.warningText : Tokens.textMuted
        font.pixelSize: Tokens.fontSizeXs
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
}
