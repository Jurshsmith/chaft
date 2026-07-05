import QtQuick
import QtQuick.Controls
import Chaft

Rectangle {
    id: root
    property var themeData: null
    property bool active: false
    readonly property string themeName: themeData ? themeData.name : ""
    readonly property string themeTagline: themeData ? themeData.tagline : ""
    signal chosen(string themeId)

    implicitWidth: 120
    implicitHeight: 88
    radius: Tokens.radiusMd
    color: themeData ? themeData.surfaceBase : "transparent"
    border.width: active ? 2 : 1
    border.color: active ? Tokens.accent : Tokens.borderSubtle
    activeFocusOnTab: true

    Accessible.role: Accessible.Button
    Accessible.name: "Theme " + themeName
    Accessible.description: (active ? "Current theme. " : "Apply theme. ") + themeTagline
    Accessible.onPressAction: root.choose()

    function choose() {
        if (root.themeData) {
            root.chosen(root.themeData.id)
        }
    }

    Row {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: nameBar.top
        anchors.margins: 6
        anchors.bottomMargin: 4
        spacing: 4

        Rectangle {
            width: 12
            height: parent.height
            radius: 3
            color: root.themeData ? root.themeData.rail : "transparent"

            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                y: 4
                width: 7
                height: 7
                radius: 2
                color: root.themeData ? root.themeData.accent : "transparent"
            }
        }

        Rectangle {
            width: 26
            height: parent.height
            radius: 3
            color: root.themeData ? root.themeData.sidebar : "transparent"

            Column {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 4
                spacing: 3

                Rectangle {
                    width: parent.width
                    height: 5
                    radius: 2
                    color: root.themeData ? root.themeData.sidebarActive : "transparent"
                }

                Rectangle {
                    width: parent.width * 0.8
                    height: 3
                    radius: 1
                    color: root.themeData ? root.themeData.sidebarText : "transparent"
                }

                Rectangle {
                    width: parent.width * 0.6
                    height: 3
                    radius: 1
                    color: root.themeData ? root.themeData.sidebarTextMuted : "transparent"
                }
            }
        }

        Column {
            width: parent.width - 12 - 26 - 8
            spacing: 3

            Rectangle {
                width: parent.width
                height: 14
                radius: 3
                color: root.themeData ? root.themeData.surfaceRaised : "transparent"

                Rectangle {
                    x: 3
                    anchors.verticalCenter: parent.verticalCenter
                    width: 8
                    height: 8
                    radius: 2
                    color: root.themeData ? root.themeData.accent : "transparent"
                }

                Rectangle {
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 18
                    height: 3
                    radius: 1
                    color: root.themeData ? root.themeData.textStrong : "transparent"
                }
            }

            Rectangle {
                width: parent.width * 0.85
                height: 3
                radius: 1
                color: root.themeData ? root.themeData.textMuted : "transparent"
            }

            Rectangle {
                width: parent.width * 0.6
                height: 3
                radius: 1
                color: root.themeData ? root.themeData.textMuted : "transparent"
            }

            Rectangle {
                width: 22
                height: 7
                radius: 2
                color: root.themeData ? root.themeData.secureSurface : "transparent"

                Rectangle {
                    anchors.centerIn: parent
                    width: 12
                    height: 3
                    radius: 1
                    color: root.themeData ? root.themeData.secure : "transparent"
                }
            }
        }
    }

    Rectangle {
        id: nameBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: root.border.width
        height: 20
        radius: Tokens.radiusMd - 2
        color: root.themeData ? root.themeData.sidebar : "transparent"

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 7
            anchors.right: activeDot.visible ? activeDot.left : parent.right
            anchors.rightMargin: 6
            anchors.verticalCenter: parent.verticalCenter
            text: root.themeName
            color: root.themeData ? root.themeData.sidebarTextStrong : "transparent"
            font.pixelSize: Tokens.fontSizeXs
            font.weight: root.active ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }

        Rectangle {
            id: activeDot
            visible: root.active
            anchors.right: parent.right
            anchors.rightMargin: 7
            anchors.verticalCenter: parent.verticalCenter
            width: 7
            height: 7
            radius: 3
            color: root.themeData ? root.themeData.accent : "transparent"
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: root.radius
        color: "transparent"
        border.color: Tokens.accent
        border.width: root.activeFocus ? 2 : 0
    }

    MouseArea {
        id: swatchMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.choose()
    }

    ToolTip.visible: swatchMouse.containsMouse
    ToolTip.text: root.themeName + (root.themeTagline.length > 0 ? " — " + root.themeTagline : "")

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.choose();
            event.accepted = true;
        }
    }
}
