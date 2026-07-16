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
        ? "This device cannot decrypt this message. If it is authorized, import a current decryption key kit and fetch matching history. Otherwise ask an admin for an invite first."
        : "This message is readable but did not use Chaft's normal secure path. This should only appear for development or imported test data."

    function activate() {
        root.activated(root.explainerTitle, root.explainerText)
    }

    implicitWidth: 74
    implicitHeight: 22
    visible: !warningRow && (locked || plaintext)
    activeFocusOnTab: visible
    radius: Tokens.radiusSm
    color: locked ? Tokens.secureSurface : Tokens.warningSurface
    border.color: activeFocus ? Tokens.accent : "transparent"
    border.width: activeFocus ? 2 : 0

    Accessible.role: Accessible.Button
    Accessible.name: locked
        ? "Locked message. If this device is authorized, import a current key kit; otherwise ask an admin for an invite."
        : "Plaintext development message"
    Accessible.onPressAction: root.activate()

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                || event.key === Qt.Key_Space) {
            root.activate()
            event.accepted = true
        }
    }

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
        onClicked: root.activate()
    }

    ToolTip.visible: visible && (badgeMouse.containsMouse || root.activeFocus)
    ToolTip.text: root.explainerText
}
