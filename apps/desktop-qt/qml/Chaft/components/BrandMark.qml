import QtQuick
import Chaft

Item {
    id: root

    property color canvasColor: Tokens.brandCanvas
    property bool showCanvas: true

    implicitWidth: 48
    implicitHeight: 48

    Accessible.role: Accessible.Graphic
    Accessible.name: "Chaft"

    Rectangle {
        visible: root.showCanvas
        anchors.centerIn: parent
        width: parent.width * 0.64
        height: parent.height * 0.64
        radius: width * 0.07
        color: root.canvasColor
    }

    Image {
        anchors.fill: parent
        source: "qrc:/branding/chaft-mark.png"
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true
    }
}
