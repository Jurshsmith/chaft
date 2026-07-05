import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property string messageId: ""
    property string selector: ""
    property string displayName: "attachment"
    property string mediaType: "application/octet-stream"
    property int byteLen: 0
    property bool available: true
    property bool actionsEnabled: false
    readonly property bool actionable: actionsEnabled && available && selector.length > 0 && messageId.length > 0
    readonly property string detailText: mediaType + " - " + String(byteLen) + " bytes" + (available ? "" : " - missing locally")
    signal saveRequested(string messageId, string selector, string displayName)

    width: Math.min(220, Math.max(92, attachmentLabel.implicitWidth + 18))
    height: 24
    radius: Tokens.radiusSm
    color: available ? Tokens.surfaceRaised : Tokens.warningSurface
    border.color: available ? Tokens.borderSubtle : Tokens.warning
    activeFocusOnTab: actionable

    Accessible.role: actionable ? Accessible.Button : Accessible.StaticText
    Accessible.name: displayName
    Accessible.description: detailText
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.actionable) {
            root.saveRequested(root.messageId, root.selector, root.displayName);
        }
    }

    Text {
        id: attachmentLabel
        anchors.centerIn: parent
        width: parent.width - 14
        text: root.displayName
        color: root.available ? Tokens.textStrong : Tokens.warningText
        font.pixelSize: Tokens.fontSizeSm
        font.weight: Font.DemiBold
        elide: Text.ElideMiddle
    }

    ToolTip.visible: attachmentMouse.containsMouse
    ToolTip.text: root.detailText

    MouseArea {
        id: attachmentMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.actionable ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activate()
    }

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activate();
            event.accepted = true;
        }
    }
}
