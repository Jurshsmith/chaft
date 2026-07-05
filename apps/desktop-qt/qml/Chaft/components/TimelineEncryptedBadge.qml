import QtQuick
import Chaft

Rectangle {
    id: root
    property bool encrypted: false
    property string kind: ""
    property bool warningRow: false

    // Encryption is the norm: badge only the exceptions. Locked rows are
    // sealed ciphertext without a local content key; plaintext rows are
    // development-only events.
    readonly property bool locked: kind === "encrypted_message"
    readonly property bool plaintext: kind === "message" && !encrypted

    implicitWidth: 74
    implicitHeight: 22
    visible: !warningRow && (locked || plaintext)
    radius: Tokens.radiusSm
    color: locked ? Tokens.secureSurface : Tokens.warningSurface

    Accessible.role: Accessible.StaticText
    Accessible.name: locked
        ? "Locked message. Import the channel or workspace key to read it."
        : "Plaintext development message"

    Text {
        anchors.centerIn: parent
        text: root.locked ? "Locked" : "Plaintext"
        color: root.locked ? Tokens.secure : Tokens.warningText
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
    }
}
