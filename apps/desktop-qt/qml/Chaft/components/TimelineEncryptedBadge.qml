import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property bool encrypted: false
    property string kind: ""
    property bool warningRow: false
    property bool bodyDecrypted: false
    property bool messageDeleted: false
    signal activated(string title, string message)

    // Encryption is the norm: badge only the exceptions. Locked rows are
    // sealed ciphertext this device could not decrypt (no content key);
    // decrypted sealed rows and tombstones carry no badge. Plaintext rows
    // are development-only events.
    readonly property bool locked: kind === "encrypted_message" && !bodyDecrypted && !messageDeleted
    readonly property bool plaintext: kind === "message" && !encrypted && !messageDeleted
    readonly property string explainerTitle: locked ? "Message locked here" : "Development message"
    readonly property string explainerText: locked
        ? "This message is locked here. Ask an admin to add you again, import a recovery kit, or fetch history from a teammate who can read it."
        : "This message is readable but did not use Chaft's normal secure path. This should only appear for development or imported test data."

    implicitWidth: 74
    implicitHeight: 22
    visible: !warningRow && (locked || plaintext)
    radius: Tokens.radiusSm
    color: locked ? Tokens.secureSurface : Tokens.warningSurface

    Accessible.role: Accessible.Button
    Accessible.name: locked
        ? "Locked message. Ask an admin for access or restore from a recovery kit."
        : "Plaintext development message"
    Accessible.onPressAction: root.activated(root.explainerTitle, root.explainerText)

    Text {
        anchors.centerIn: parent
        text: root.locked ? "Locked" : "Plaintext"
        color: root.locked ? Tokens.secure : Tokens.warningText
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
    }

    MouseArea {
        id: badgeMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated(root.explainerTitle, root.explainerText)
    }

    ToolTip.visible: visible && badgeMouse.containsMouse
    ToolTip.text: root.explainerText
}
