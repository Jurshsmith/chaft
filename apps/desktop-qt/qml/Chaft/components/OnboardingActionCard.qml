import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Rectangle {
    id: root
    property string title: ""
    property string body: ""
    property string glyph: ""
    property bool primary: false
    property bool actionable: true
    signal activated()

    implicitHeight: cardColumn.implicitHeight + Tokens.space4 * 2
    radius: Tokens.radiusMd
    color: cardMouse.containsMouse && root.actionable
        ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.05)
        : Tokens.surfaceRaised
    border.width: root.activeFocus ? 2 : 1
    border.color: root.activeFocus
        ? Tokens.accent
        : (cardMouse.containsMouse && root.actionable) || root.primary
            ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b,
                      cardMouse.containsMouse && root.actionable ? 0.9 : 0.55)
            : Tokens.borderSubtle
    opacity: root.actionable ? 1 : 0.55
    activeFocusOnTab: root.actionable

    Accessible.role: Accessible.Button
    Accessible.name: root.title
    Accessible.description: root.body
    Accessible.onPressAction: root.trigger()

    function trigger() {
        if (root.actionable) {
            root.activated()
        }
    }

    transform: Translate {
        y: cardMouse.containsMouse && root.actionable && Tokens.motionEnabled ? -2 : 0

        Behavior on y {
            enabled: Tokens.motionEnabled
            NumberAnimation {
                duration: Tokens.motionQuickMs
                easing.type: Easing.OutCubic
            }
        }
    }

    ColumnLayout {
        id: cardColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Tokens.space4
        spacing: Tokens.space2

        Rectangle {
            Layout.preferredWidth: 40
            Layout.preferredHeight: 40
            radius: Tokens.radiusMd
            color: root.primary
                ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.2)
                : Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.08)
            border.width: 1
            border.color: root.primary
                ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.55)
                : Tokens.borderSubtle

            Text {
                anchors.centerIn: parent
                text: root.glyph
                color: root.primary ? Tokens.accent : Tokens.textStrong
                font.pixelSize: Tokens.fontSizeLg
                font.weight: Font.Bold
            }
        }

        Text {
            Layout.fillWidth: true
            Layout.topMargin: Tokens.space1
            text: root.title
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeLg
            font.weight: Font.Bold
            elide: Text.ElideRight
        }

        Text {
            Layout.fillWidth: true
            text: root.body
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
    }

    MouseArea {
        id: cardMouse
        anchors.fill: parent
        enabled: root.actionable
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.trigger()
    }

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.trigger();
            event.accepted = true;
        }
    }
}
