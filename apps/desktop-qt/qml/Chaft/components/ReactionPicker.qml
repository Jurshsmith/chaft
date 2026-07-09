pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Popup {
    id: root

    property var anchorItem: null
    property var choices: []
    property var myReactions: []
    property string messageId: ""
    property bool actionsEnabled: false
    property bool messageDeleted: false
    property bool warningRow: false
    readonly property bool canReact: actionsEnabled
        && messageId.length > 0
        && !messageDeleted
        && !warningRow

    signal reactionRequested(string messageId, string reaction)
    signal reactionRemoveRequested(string messageId, string reaction)

    parent: Overlay.overlay
    width: 320
    implicitHeight: pickerColumn.implicitHeight + padding * 2
    padding: Tokens.space3
    modal: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    function normalized(value) {
        return String(value || "").trim().toLowerCase()
    }

    function hasReaction(reaction) {
        var value = String(reaction || "")
        var source = root.myReactions || []
        for (var i = 0; i < source.length; i += 1) {
            if (String(source[i] || "") === value) {
                return true
            }
        }
        return false
    }

    function filteredChoices(query) {
        var needle = root.normalized(query)
        var entries = []
        var source = root.choices || []
        for (var i = 0; i < source.length; i += 1) {
            var choice = source[i] || {}
            var emoji = String(choice.emoji || "").trim()
            if (emoji.length === 0) {
                continue
            }
            var label = root.normalized(choice.label)
            var keywords = root.normalized(choice.keywords)
            if (needle.length === 0
                    || emoji.indexOf(needle) !== -1
                    || label.indexOf(needle) !== -1
                    || keywords.indexOf(needle) !== -1) {
                entries.push(choice)
            }
        }
        return entries
    }

    function reposition() {
        if (root.anchorItem === null || Overlay.overlay === null) {
            return
        }

        var below = root.anchorItem.mapToItem(
            Overlay.overlay, 0, root.anchorItem.height + Tokens.space1)
        var above = root.anchorItem.mapToItem(
            Overlay.overlay, 0, -root.height - Tokens.space1)
        var minX = Tokens.space2
        var maxX = Math.max(minX, Overlay.overlay.width - root.width - Tokens.space2)
        var minY = Tokens.space2
        var maxY = Math.max(minY, Overlay.overlay.height - root.height - Tokens.space2)
        root.x = Math.max(minX, Math.min(maxX, below.x + root.anchorItem.width - root.width))
        root.y = below.y + root.height <= Overlay.overlay.height - Tokens.space2
            ? Math.max(minY, below.y)
            : Math.max(minY, Math.min(maxY, above.y))
    }

    function currentChoice() {
        var entries = root.filteredChoices(searchField.text)
        if (entries.length === 0) {
            return null
        }
        var index = emojiGrid.currentIndex >= 0 ? emojiGrid.currentIndex : 0
        return entries[Math.min(index, entries.length - 1)]
    }

    function activateChoice(choice) {
        if (!root.canReact || choice === null) {
            return
        }
        var reaction = String(choice.emoji || "").trim()
        if (reaction.length === 0) {
            return
        }
        root.close()
        if (root.hasReaction(reaction)) {
            root.reactionRemoveRequested(root.messageId, reaction)
        } else {
            root.reactionRequested(root.messageId, reaction)
        }
    }

    function activateCurrent() {
        root.activateChoice(root.currentChoice())
    }

    onAboutToShow: {
        searchField.text = ""
        emojiGrid.currentIndex = 0
        root.reposition()
    }

    onOpened: searchField.forceActiveFocus()
    onAnchorItemChanged: root.reposition()

    background: Rectangle {
        radius: Tokens.radiusMd
        color: Tokens.surfaceRaised
        border.width: 1
        border.color: Tokens.borderSubtle
    }

    contentItem: ColumnLayout {
        id: pickerColumn

        spacing: Tokens.space2

        TextField {
            id: searchField

            Layout.fillWidth: true
            enabled: root.canReact
            placeholderText: "Search reactions"
            selectByMouse: true
            onTextChanged: emojiGrid.currentIndex = 0
            onAccepted: root.activateCurrent()

            Keys.onPressed: function (event) {
                if (event.key === Qt.Key_Down) {
                    emojiGrid.forceActiveFocus()
                    event.accepted = true
                }
            }
        }

        GridView {
            id: emojiGrid

            Layout.fillWidth: true
            Layout.preferredHeight: 188
            activeFocusOnTab: root.canReact
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            cellWidth: 48
            cellHeight: 44
            model: root.filteredChoices(searchField.text)

            onCountChanged: {
                if (count === 0) {
                    currentIndex = -1
                } else if (currentIndex < 0 || currentIndex >= count) {
                    currentIndex = 0
                }
            }

            Keys.onPressed: function (event) {
                if (event.key === Qt.Key_Return
                        || event.key === Qt.Key_Enter
                        || event.key === Qt.Key_Space) {
                    root.activateCurrent()
                    event.accepted = true
                } else if (event.key === Qt.Key_Up && emojiGrid.currentIndex < 6) {
                    searchField.forceActiveFocus()
                    event.accepted = true
                }
            }

            delegate: Rectangle {
                id: emojiCell

                required property int index
                required property var modelData

                readonly property string emoji: String(emojiCell.modelData.emoji || "")
                readonly property string label: String(emojiCell.modelData.label || emojiCell.emoji)
                readonly property bool mine: root.hasReaction(emojiCell.emoji)
                readonly property bool selected: emojiGrid.currentIndex === emojiCell.index

                width: emojiGrid.cellWidth - 6
                height: emojiGrid.cellHeight - 6
                radius: Tokens.radiusSm
                color: emojiCell.selected
                    ? Tokens.secureSurface
                    : emojiMouse.containsMouse
                        ? Tokens.surfaceBase
                        : Tokens.surfaceRaised
                border.width: emojiCell.selected || emojiCell.mine ? 2 : 1
                border.color: emojiCell.mine
                    ? Tokens.secure
                    : emojiCell.selected
                        ? Tokens.accent
                        : Tokens.borderSubtle

                Accessible.role: Accessible.Button
                Accessible.name: emojiCell.label
                Accessible.description: emojiCell.mine ? "Remove reaction" : "Add reaction"
                Accessible.onPressAction: root.activateChoice(emojiCell.modelData)

                Text {
                    anchors.centerIn: parent
                    text: emojiCell.emoji
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeLg
                }

                MouseArea {
                    id: emojiMouse

                    anchors.fill: parent
                    scrollGestureEnabled: false
                    enabled: root.canReact
                    hoverEnabled: true
                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: {
                        emojiGrid.currentIndex = emojiCell.index
                        root.activateChoice(emojiCell.modelData)
                    }
                }

                ToolTip.visible: emojiMouse.containsMouse && emojiMouse.enabled
                ToolTip.text: (emojiCell.mine ? "Remove " : "Add ") + emojiCell.label
            }
        }

        Text {
            Layout.fillWidth: true
            visible: emojiGrid.count === 0
            text: "No matches"
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
            horizontalAlignment: Text.AlignHCenter
        }
    }
}
