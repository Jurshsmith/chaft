pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Popup {
    id: palette

    // The App root window and its action registry. Both are set by App.qml.
    property var app
    property var actions: []
    property int maxChannelResults: 12

    readonly property string trimmedQuery: queryField.text.trim()
    property int selectedIndex: 0

    width: Math.min(560, parent ? parent.width - Tokens.space4 * 2 : 560)
    x: parent ? Math.round((parent.width - width) / 2) : 0
    y: parent ? Math.round(parent.height * 0.12) : 0
    modal: true
    focus: true
    padding: Tokens.space2
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Rectangle {
        radius: Tokens.radiusMd
        color: Tokens.surfaceRaised
        border.width: 1
        border.color: Tokens.borderSubtle
    }

    onOpened: {
        queryField.text = ""
        palette.selectedIndex = 0
        queryField.forceActiveFocus()
    }

    onClosed: {
        // Restore the sidebar's channel-search worker state in case the
        // palette dispatched its own query while open.
        if (app && app.runtimeWorkReady) {
            chaftController.searchWorkspaceChannels(
                app.searchHasTerms ? app.trimmedSearchQuery : "")
        }
    }

    Timer {
        id: channelSearchDebounce
        interval: 120
        repeat: false
        onTriggered: {
            if (palette.app
                    && palette.app.runtimeWorkReady
                    && chaftController.searchQueryHasTerms(palette.trimmedQuery)) {
                chaftController.searchWorkspaceChannels(palette.trimmedQuery)
            }
        }
    }

    // Simple case-insensitive subsequence scorer. Higher is better; -1 means
    // the query is not a subsequence of the candidate.
    function fuzzyScore(query, candidate) {
        var q = String(query || "").toLowerCase()
        var c = String(candidate || "").toLowerCase()
        if (q.length === 0) {
            return 0
        }
        var score = 0
        var searchFrom = 0
        var streak = 0
        for (var qi = 0; qi < q.length; qi++) {
            var found = c.indexOf(q.charAt(qi), searchFrom)
            if (found < 0) {
                return -1
            }
            streak = found === searchFrom && qi > 0 ? streak + 1 : 1
            score += streak
            if (found === 0) {
                score += 3
            } else {
                var previous = c.charAt(found - 1)
                if (previous === " " || previous === "-" || previous === "_" || previous === "#") {
                    score += 2
                }
            }
            searchFrom = found + 1
        }
        return score
    }

    function channelCandidates() {
        var rows = []
        var seen = ({})
        var loaded = (palette.app && palette.app.channels) || []
        for (var i = 0; i < loaded.length; i++) {
            var channelId = String(loaded[i].channelId || "")
            if (channelId.length === 0 || seen[channelId]) {
                continue
            }
            seen[channelId] = true
            rows.push(loaded[i])
        }
        if (palette.trimmedQuery.length > 0
                && chaftController.channelSearchQuery === palette.trimmedQuery) {
            var results = chaftController.channelSearchResults || []
            for (var j = 0; j < results.length; j++) {
                var resultChannelId = String(results[j].channelId || "")
                if (resultChannelId.length === 0 || seen[resultChannelId]) {
                    continue
                }
                seen[resultChannelId] = true
                rows.push(results[j])
            }
        }
        return rows
    }

    function channelRow(channel) {
        var directMessage = palette.app && palette.app.channelIsDirectMessage(channel)
        var archived = palette.app && palette.app.channelArchived(channel)
        var displayName = palette.app
            ? palette.app.channelDisplayName(channel)
            : String(channel.name || "")
        return {
            kind: "channel",
            rowId: "channel:" + String(channel.channelId || ""),
            label: (directMessage ? "@ " : "# ") + displayName,
            detail: directMessage
                ? "Direct message"
                : (archived ? "Archived room" : (Boolean(channel.isPrivate) ? "Private room" : "Room")),
            keys: "",
            enabledNow: true,
            channelId: String(channel.channelId || ""),
            action: null
        }
    }

    function actionRow(action) {
        return {
            kind: "action",
            rowId: "action:" + String(action.id || ""),
            label: String(action.label || ""),
            detail: "Action",
            keys: String(action.shortcut || ""),
            enabledNow: Boolean(action.enabled === undefined || action.enabled()),
            channelId: "",
            action: action
        }
    }

    function scoredRows(candidates, labelFor) {
        var scored = []
        for (var i = 0; i < candidates.length; i++) {
            var score = palette.fuzzyScore(palette.trimmedQuery, labelFor(candidates[i]))
            if (score >= 0) {
                scored.push({ entry: candidates[i], score: score, order: i })
            }
        }
        scored.sort(function (left, right) {
            return right.score !== left.score
                ? right.score - left.score
                : left.order - right.order
        })
        return scored
    }

    readonly property var results: {
        var rows = []
        var channels = palette.channelCandidates()
        var actionList = palette.actions || []
        var i
        if (palette.trimmedQuery.length === 0) {
            for (i = 0; i < actionList.length; i++) {
                rows.push(palette.actionRow(actionList[i]))
            }
            for (i = 0; i < channels.length && i < palette.maxChannelResults; i++) {
                rows.push(palette.channelRow(channels[i]))
            }
            return rows
        }

        // Conversations rank first on non-empty queries, preserving the old
        // Ctrl/Cmd+K jump muscle memory.
        var scoredChannels = palette.scoredRows(channels, function (channel) {
            var displayName = palette.app
                ? palette.app.channelDisplayName(channel)
                : String(channel.name || "")
            return displayName + " " + String(channel.name || "") + " "
                + String(channel.channelId || "")
        })
        for (i = 0; i < scoredChannels.length && i < palette.maxChannelResults; i++) {
            rows.push(palette.channelRow(scoredChannels[i].entry))
        }
        var scoredActions = palette.scoredRows(actionList, function (action) {
            return String(action.label || "")
        })
        for (i = 0; i < scoredActions.length; i++) {
            rows.push(palette.actionRow(scoredActions[i].entry))
        }
        return rows
    }

    onResultsChanged: {
        if (palette.selectedIndex >= palette.results.length) {
            palette.selectedIndex = Math.max(0, palette.results.length - 1)
        }
    }

    function moveSelection(step) {
        if (palette.results.length === 0) {
            return
        }
        var next = palette.selectedIndex + step
        if (next < 0) {
            next = palette.results.length - 1
        } else if (next >= palette.results.length) {
            next = 0
        }
        palette.selectedIndex = next
        resultsList.positionViewAtIndex(next, ListView.Contain)
    }

    function activateRow(row) {
        if (!row || !row.enabledNow) {
            return
        }
        palette.close()
        if (row.kind === "channel") {
            palette.app.selectChannelId(row.channelId, true)
        } else if (row.action && row.action.run) {
            row.action.run()
        }
    }

    function activateSelected() {
        if (palette.selectedIndex >= 0 && palette.selectedIndex < palette.results.length) {
            palette.activateRow(palette.results[palette.selectedIndex])
        }
    }

    contentItem: ColumnLayout {
        spacing: Tokens.space2

        TextField {
            id: queryField
            Layout.fillWidth: true
            placeholderText: "Search actions, rooms, or DMs"
            Accessible.name: "Command palette"
            Accessible.description: "Search actions, rooms, and DMs; Up and Down move, Enter runs"
            onTextChanged: {
                palette.selectedIndex = 0
                channelSearchDebounce.restart()
            }
            Keys.onPressed: function (event) {
                if (event.key === Qt.Key_Down) {
                    palette.moveSelection(1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Up) {
                    palette.moveSelection(-1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    palette.activateSelected()
                    event.accepted = true
                }
            }
        }

        Text {
            Layout.fillWidth: true
            visible: palette.results.length === 0
            text: "No matching actions, rooms, or DMs"
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            elide: Text.ElideRight
        }

        ListView {
            id: resultsList
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 336)
            visible: palette.results.length > 0
            clip: true
            interactive: contentHeight > height
            model: palette.results
            spacing: 1

            delegate: Rectangle {
                id: resultDelegate
                required property var modelData
                required property int index

                readonly property bool current: resultDelegate.index === palette.selectedIndex

                width: ListView.view.width
                implicitHeight: 34
                radius: Tokens.radiusSm
                color: resultDelegate.current
                    ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.18)
                    : (resultRowMouse.containsMouse
                        ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                        : "transparent")
                opacity: resultDelegate.modelData.enabledNow ? 1 : 0.45

                Accessible.role: Accessible.Button
                Accessible.name: resultDelegate.modelData.label
                Accessible.description: resultDelegate.modelData.detail
                    + (resultDelegate.modelData.enabledNow ? "" : ", unavailable")
                Accessible.onPressAction: palette.activateRow(resultDelegate.modelData)

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Tokens.space2
                    anchors.rightMargin: Tokens.space2
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: resultDelegate.modelData.label
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeSm
                        font.weight: resultDelegate.current ? Font.Medium : Font.Normal
                        elide: Text.ElideRight
                    }

                    Text {
                        text: resultDelegate.modelData.detail
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                    }

                    Text {
                        visible: resultDelegate.modelData.keys.length > 0
                        text: resultDelegate.modelData.keys
                        color: Tokens.textMuted
                        font.family: Tokens.fontMono
                        font.pixelSize: Tokens.fontSizeXs
                    }
                }

                MouseArea {
                    id: resultRowMouse
                    anchors.fill: parent
                    scrollGestureEnabled: false
                    hoverEnabled: true
                    cursorShape: resultDelegate.modelData.enabledNow
                        ? Qt.PointingHandCursor
                        : Qt.ArrowCursor
                    onEntered: palette.selectedIndex = resultDelegate.index
                    onClicked: palette.activateRow(resultDelegate.modelData)
                }
            }
        }

        Text {
            Layout.fillWidth: true
            text: "↑↓ select · Enter run · Esc close"
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideRight
        }
    }
}
