import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Item {
    id: root

    property string avatarId: ""
    property string workspaceId: ""
    property string identityId: ""
    property string displayName: ""
    property var usedAvatarIds: []
    property bool editable: true
    property bool showLabel: true
    readonly property string effectiveAvatarId: AvatarCatalog.isValid(avatarId)
        ? AvatarCatalog.parse(avatarId).id
        : AvatarCatalog.resolvedAvatarId("", workspaceId, identityId)
    readonly property var effectiveParts: AvatarCatalog.parse(effectiveAvatarId)
    property int selectedPaletteIndex: effectiveParts === null ? 0 : effectiveParts.palette
    property bool browseAll: false
    property var visibleChoices: []
    signal avatarChosen(string avatarId)

    implicitWidth: 320
    implicitHeight: previewRow.implicitHeight

    function refreshChoices() {
        root.visibleChoices = AvatarCatalog.choices(
            root.selectedPaletteIndex,
            root.usedAvatarIds,
            root.effectiveAvatarId,
            root.browseAll ? 0 : 24)
    }

    function currentChoiceIndex() {
        for (var i = 0; i < root.visibleChoices.length; i += 1) {
            if (String(root.visibleChoices[i].avatarId || "")
                    === root.effectiveAvatarId) {
                return i
            }
        }
        return root.visibleChoices.length > 0 ? 0 : -1
    }

    function activateChoiceAt(index) {
        if (index < 0 || index >= root.visibleChoices.length) {
            return
        }
        root.selectAvatar(String(
            root.visibleChoices[index].avatarId || ""))
    }

    function selectAvatar(nextAvatarId) {
        var parsed = AvatarCatalog.parse(nextAvatarId)
        if (parsed === null) {
            return
        }
        root.selectedPaletteIndex = parsed.palette
        root.avatarChosen(parsed.id)
        root.refreshChoices()
    }

    function shuffle() {
        root.selectAvatar(AvatarCatalog.shuffledAvatarId(
            root.effectiveAvatarId, root.usedAvatarIds))
    }

    function openPicker() {
        if (!root.editable) {
            return
        }
        root.browseAll = false
        root.refreshChoices()
        pickerPopup.open()
    }

    function closePicker() {
        pickerPopup.close()
    }

    RowLayout {
        id: previewRow
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Tokens.space2

        AvatarMark {
            Layout.preferredWidth: 52
            Layout.preferredHeight: 52
            avatarId: root.effectiveAvatarId
            workspaceId: root.workspaceId
            identityId: root.identityId
            displayName: root.displayName
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Text {
                visible: root.showLabel
                Layout.fillWidth: true
                text: "Your avatar"
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeSm
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                text: AvatarCatalog.description(root.effectiveAvatarId)
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                elide: Text.ElideRight
            }
        }

        Button {
            visible: root.editable
            text: "Change"
            Accessible.name: "Change avatar"
            onClicked: root.openPicker()
        }
    }

    Popup {
        id: pickerPopup
        parent: Overlay.overlay
        width: Math.min(460, parent.width - 40)
        height: Math.min(570, parent.height - 56)
        x: Math.round((parent.width - width) / 2)
        y: Math.round((parent.height - height) / 2)
        modal: true
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        padding: Tokens.space3

        onOpened: {
            root.refreshChoices()
            paletteGrid.currentIndex = root.selectedPaletteIndex
            avatarGrid.currentIndex = root.currentChoiceIndex()
            if (avatarGrid.currentIndex >= 0) {
                avatarGrid.positionViewAtIndex(
                    avatarGrid.currentIndex, GridView.Contain)
            }
            avatarGrid.forceActiveFocus()
        }

        background: Rectangle {
            color: Tokens.surfaceRaised
            border.color: Tokens.borderSubtle
            border.width: 1
            radius: Tokens.radiusMd
        }

        contentItem: ColumnLayout {
            spacing: Tokens.space2

            RowLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 1

                    Text {
                        Layout.fillWidth: true
                        text: "Choose an avatar"
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeLg
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.fillWidth: true
                        text: root.browseAll
                            ? "128 routes in this palette"
                            : "Unused avatars appear first"
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        elide: Text.ElideRight
                    }
                }

                Button {
                    text: "Shuffle"
                    Accessible.name: "Choose a random unused avatar"
                    onClicked: root.shuffle()
                }

                Button {
                    text: "×"
                    implicitWidth: 34
                    Accessible.name: "Close avatar picker"
                    onClicked: pickerPopup.close()
                }
            }

            Text {
                Layout.fillWidth: true
                text: "Palette"
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
            }

            GridView {
                id: paletteGrid
                Layout.fillWidth: true
                Layout.preferredHeight: 74
                cellWidth: Math.max(38, width / 6)
                cellHeight: 37
                interactive: false
                model: AvatarCatalog.palettes
                keyNavigationEnabled: true
                keyNavigationWraps: true
                Accessible.role: Accessible.List
                Accessible.name: "Avatar palettes"

                delegate: Button {
                    id: paletteButton
                    required property var modelData
                    required property int index

                    width: paletteGrid.cellWidth - 5
                    height: paletteGrid.cellHeight - 5
                    checkable: true
                    activeFocusOnTab: true
                    checked: root.selectedPaletteIndex === paletteButton.index
                    Accessible.name: String(paletteButton.modelData.name || "")
                    Accessible.description: checked ? "Selected palette" : "Choose palette"
                    onClicked: {
                        paletteGrid.currentIndex = paletteButton.index
                        root.selectedPaletteIndex = paletteButton.index
                        var recolored = AvatarCatalog.withPalette(
                            root.effectiveAvatarId, paletteButton.index)
                        root.selectAvatar(recolored)
                    }

                    background: Rectangle {
                        radius: Tokens.radiusSm
                        color: paletteButton.modelData.background
                        border.width: paletteButton.checked || paletteButton.visualFocus ? 2 : 1
                        border.color: paletteButton.checked
                            ? Tokens.accent
                            : paletteButton.modelData.route
                    }

                    contentItem: Rectangle {
                        anchors.centerIn: parent
                        width: 11
                        height: 11
                        radius: width / 2
                        color: paletteButton.modelData.node
                    }
                }
            }

            GridView {
                id: avatarGrid
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                cellWidth: 66
                cellHeight: 66
                model: root.visibleChoices
                keyNavigationEnabled: true
                keyNavigationWraps: true
                boundsBehavior: Flickable.StopAtBounds
                Accessible.role: Accessible.List
                Accessible.name: root.browseAll
                    ? "All avatars in palette"
                    : "Suggested avatars"
                Keys.onReturnPressed: root.activateChoiceAt(currentIndex)
                Keys.onEnterPressed: root.activateChoiceAt(currentIndex)
                Keys.onSpacePressed: root.activateChoiceAt(currentIndex)

                ScrollBar.vertical: ScrollBar {}

                delegate: Button {
                    id: avatarButton
                    required property var modelData
                    required property int index

                    width: avatarGrid.cellWidth - 8
                    height: avatarGrid.cellHeight - 8
                    checkable: true
                    activeFocusOnTab: true
                    checked: String(avatarButton.modelData.avatarId || "")
                        === root.effectiveAvatarId
                    Accessible.name: String(avatarButton.modelData.label || "")
                    Accessible.description: avatarButton.modelData.used
                        ? "Already used in this workspace"
                        : "Available"
                    onClicked: {
                        avatarGrid.currentIndex = avatarButton.index
                        root.selectAvatar(
                            String(avatarButton.modelData.avatarId || ""))
                    }

                    background: Rectangle {
                        radius: Tokens.radiusMd
                        color: avatarButton.checked
                            ? Tokens.secureSurface
                            : "transparent"
                        border.width: avatarButton.checked
                            || avatarButton.visualFocus
                            || avatarGrid.currentIndex === avatarButton.index
                            ? 2 : 1
                        border.color: avatarButton.checked
                            || avatarGrid.currentIndex === avatarButton.index
                            ? Tokens.accent
                            : (avatarButton.modelData.used
                                ? Tokens.textMuted
                                : Tokens.borderSubtle)
                    }

                    contentItem: Item {
                        AvatarMark {
                            anchors.centerIn: parent
                            width: 44
                            height: 44
                            avatarId: String(avatarButton.modelData.avatarId || "")
                        }

                        Rectangle {
                            visible: avatarButton.modelData.used
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.margins: 2
                            width: 15
                            height: 15
                            radius: width / 2
                            color: Tokens.surfaceRaised
                            border.color: Tokens.borderSubtle

                            Text {
                                anchors.centerIn: parent
                                text: "✓"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.Bold
                            }
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Text {
                    Layout.fillWidth: true
                    text: String(root.visibleChoices.length)
                        + (root.visibleChoices.length === 1 ? " avatar" : " avatars")
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                }

                Button {
                    text: root.browseAll ? "Show suggestions" : "Browse all"
                    onClicked: {
                        root.browseAll = !root.browseAll
                        root.refreshChoices()
                        avatarGrid.positionViewAtBeginning()
                        avatarGrid.forceActiveFocus()
                    }
                }

                Button {
                    text: "Done"
                    onClicked: pickerPopup.close()
                }
            }
        }
    }

    onEffectiveAvatarIdChanged: {
        var parsed = AvatarCatalog.parse(root.effectiveAvatarId)
        if (parsed !== null) {
            root.selectedPaletteIndex = parsed.palette
        }
        root.refreshChoices()
    }
    onUsedAvatarIdsChanged: root.refreshChoices()
    Component.onCompleted: root.refreshChoices()
}
