import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

ColumnLayout {
    id: root
    property string title: "Section"
    property string badgeText: ""
    property bool danger: false
    property bool collapsible: true
    property bool defaultExpanded: false
    property bool expanded: root.defaultExpanded
    default property alias contentData: contentColumn.data

    spacing: Tokens.space1

    function toggle() {
        if (root.collapsible) {
            root.expanded = !root.expanded
        }
    }

    onCollapsibleChanged: {
        if (!root.collapsible) {
            root.expanded = true
        }
    }

    Rectangle {
        id: sectionHeader
        Layout.fillWidth: true
        implicitHeight: 30
        radius: Tokens.radiusSm
        color: headerMouse.containsMouse
            ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.08)
            : "transparent"
        border.width: sectionHeader.activeFocus ? 2 : 0
        border.color: Tokens.accent
        activeFocusOnTab: root.collapsible

        Accessible.role: root.collapsible ? Accessible.Button : Accessible.StaticText
        Accessible.name: root.title
            + (root.badgeText.length > 0 ? ", " + root.badgeText : "")
        Accessible.description: root.collapsible
            ? (root.expanded ? "Collapse section" : "Expand section")
            : ""
        Accessible.onPressAction: root.toggle()

        Keys.onPressed: function (event) {
            if (event.key === Qt.Key_Return
                    || event.key === Qt.Key_Enter
                    || event.key === Qt.Key_Space) {
                root.toggle()
                event.accepted = true
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Tokens.space1
            anchors.rightMargin: Tokens.space1
            spacing: Tokens.space2

            Text {
                visible: root.collapsible
                text: root.expanded ? "▾" : "▸"
                color: root.danger ? Tokens.warningText : Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
            }

            Text {
                text: root.title
                color: root.danger ? Tokens.warningText : Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            Text {
                visible: root.badgeText.length > 0
                text: root.badgeText
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
            }

            Item {
                Layout.fillWidth: true
            }
        }

        MouseArea {
            id: headerMouse
            anchors.fill: parent
            scrollGestureEnabled: false
            hoverEnabled: true
            enabled: root.collapsible
            cursorShape: root.collapsible ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: root.toggle()
        }
    }

    ColumnLayout {
        id: contentColumn
        Layout.fillWidth: true
        Layout.leftMargin: Tokens.space1
        Layout.rightMargin: Tokens.space1
        Layout.bottomMargin: Tokens.space1
        visible: !root.collapsible || root.expanded
        spacing: Tokens.space2
    }
}
