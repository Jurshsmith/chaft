import QtQuick
import QtQuick.Controls
import Chaft

Item {
    id: root
    property int maxVisible: 4
    property var entries: []

    implicitWidth: 320
    implicitHeight: toastColumn.implicitHeight

    function show(severity, message, actionLabel, actionId, timeoutMs) {
        var normalizedMessage = String(message || "").slice(0, 400)
        if (normalizedMessage.length === 0) {
            return
        }
        var next = root.entries.slice()
        if (next.length >= root.maxVisible) {
            next.shift()
        }
        next.push({
            toastId: root.nextToastId++,
            severity: String(severity || "info"),
            message: normalizedMessage,
            actionLabel: String(actionLabel || ""),
            actionId: String(actionId || ""),
            timeoutMs: Math.max(1500, Number(timeoutMs) > 0 ? Number(timeoutMs) : 4500)
        })
        root.entries = next
    }

    function dismiss(toastId) {
        var next = []
        for (var i = 0; i < root.entries.length; ++i) {
            if (root.entries[i].toastId !== toastId) {
                next.push(root.entries[i])
            }
        }
        root.entries = next
    }

    property int nextToastId: 1
    signal actionTriggered(string actionId)

    Column {
        id: toastColumn
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Tokens.space2

        Repeater {
            model: root.entries

            delegate: Rectangle {
                id: toastDelegate
                required property var modelData

                readonly property bool warningLike: toastDelegate.modelData.severity === "warning"
                    || toastDelegate.modelData.severity === "error"

                width: toastColumn.width
                implicitHeight: toastRow.implicitHeight + Tokens.space2 * 2
                radius: Tokens.radiusMd
                color: warningLike ? Tokens.warningSurface : Tokens.surfaceRaised
                border.width: 1
                border.color: warningLike ? Tokens.warning : Tokens.borderSubtle

                NumberAnimation on opacity {
                    running: Tokens.motionEnabled
                    from: 0
                    to: 1
                    duration: Tokens.motionQuickMs
                    easing.type: Easing.OutCubic
                }

                Accessible.role: Accessible.AlertMessage
                Accessible.name: toastDelegate.modelData.message

                Timer {
                    interval: toastDelegate.modelData.timeoutMs
                    running: true
                    onTriggered: root.dismiss(toastDelegate.modelData.toastId)
                }

                Row {
                    id: toastRow
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: Tokens.space3
                    anchors.rightMargin: Tokens.space2
                    spacing: Tokens.space2

                    Text {
                        width: parent.width
                            - (toastAction.visible ? toastAction.width + Tokens.space2 : 0)
                            - toastClose.width - Tokens.space2
                        text: toastDelegate.modelData.message
                        color: toastDelegate.warningLike ? Tokens.warningText : Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeSm
                        wrapMode: Text.Wrap
                        maximumLineCount: 3
                        elide: Text.ElideRight
                        anchors.verticalCenter: parent.verticalCenter
                    }

                    Button {
                        id: toastAction
                        visible: toastDelegate.modelData.actionLabel.length > 0
                        text: toastDelegate.modelData.actionLabel
                        anchors.verticalCenter: parent.verticalCenter
                        onClicked: {
                            root.actionTriggered(toastDelegate.modelData.actionId)
                            root.dismiss(toastDelegate.modelData.toastId)
                        }
                    }

                    Button {
                        id: toastClose
                        text: "✕"
                        width: 28
                        anchors.verticalCenter: parent.verticalCenter
                        Accessible.name: "Dismiss notification"
                        onClicked: root.dismiss(toastDelegate.modelData.toastId)
                    }
                }
            }
        }
    }
}
