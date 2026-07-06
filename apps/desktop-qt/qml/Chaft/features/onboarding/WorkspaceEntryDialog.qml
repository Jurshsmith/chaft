import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

// Workspace create/join flow in the First Light card language: glyph-tile
// header, segmented mode switch, labeled fields, accent primary action.
// Field text is exposed through aliases so the App root submit handlers own
// all runtime behavior; this file is presentation only.
Dialog {
    id: root
    property var app
    property alias credentialsText: credentialsArea.text
    property alias recoveryPassphraseText: recoveryPassphraseField.text
    property alias peerEndpointText: peerEndpointField.text
    property alias createNameText: createNameField.text
    property alias createChannelText: createChannelField.text

    readonly property bool createMode: root.app.workspaceEntryMode === "create"

    modal: true
    width: Math.min(root.app.width - 48, 560)
    x: Math.round((root.app.width - width) / 2)
    y: Math.round((root.app.height - height) / 2)
    padding: Tokens.space4
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    component EntryPrimaryButton: Button {
        id: primaryButton

        background: Rectangle {
            radius: Tokens.radiusSm
            color: primaryButton.down
                ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.4)
                : Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b,
                          primaryButton.enabled ? 0.24 : 0.1)
            border.width: primaryButton.visualFocus ? 2 : 1
            border.color: primaryButton.enabled ? Tokens.accent : Tokens.borderSubtle
        }

        contentItem: Text {
            text: primaryButton.text
            color: primaryButton.enabled ? Tokens.textStrong : Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            font.weight: Font.Medium
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }

    component EntryModeSegment: Rectangle {
        id: segment
        property string label: ""
        property bool active: false
        signal chosen()

        Layout.fillWidth: true
        implicitHeight: 30
        radius: Tokens.radiusXs
        color: segment.active
            ? Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.2)
            : segmentMouse.containsMouse
                ? Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                : "transparent"
        border.width: segment.activeFocus ? 2 : 0
        border.color: Tokens.accent
        activeFocusOnTab: true

        Accessible.role: Accessible.RadioButton
        Accessible.name: segment.label
        Accessible.description: segment.active ? "Selected mode" : "Switch mode"
        Accessible.onPressAction: segment.chosen()

        Text {
            anchors.centerIn: parent
            text: segment.label
            color: segment.active ? Tokens.textStrong : Tokens.textMuted
            font.pixelSize: Tokens.fontSizeSm
            font.weight: segment.active ? Font.Medium : Font.Normal
        }

        MouseArea {
            id: segmentMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: segment.chosen()
        }

        Keys.onPressed: function (event) {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                segment.chosen();
                event.accepted = true;
            }
        }
    }

    function focusInitialField() {
        if (root.createMode) {
            createNameField.forceFieldFocus()
        } else {
            credentialsArea.forceActiveFocus()
        }
    }

    function resetForm() {
        createNameField.text = ""
        createChannelField.text = "general"
        credentialsArea.text = ""
        recoveryPassphraseField.text = ""
        peerEndpointField.text = chaftController.defaultPeerEndpoint
    }

    ColumnLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Tokens.space3

        RowLayout {
            Layout.fillWidth: true
            spacing: Tokens.space3

            Rectangle {
                Layout.preferredWidth: 40
                Layout.preferredHeight: 40
                Layout.alignment: Qt.AlignTop
                radius: Tokens.radiusMd
                color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.2)
                border.width: 1
                border.color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.55)

                Text {
                    anchors.centerIn: parent
                    text: root.createMode ? "#" : "⇄"
                    color: Tokens.accent
                    font.pixelSize: Tokens.fontSizeLg
                    font.weight: Font.Bold
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Text {
                    Layout.fillWidth: true
                    text: root.createMode ? "Create a workspace" : "Join a workspace"
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeXl
                    font.weight: Font.Bold
                    elide: Text.ElideRight
                }

                Text {
                    Layout.fillWidth: true
                    text: root.createMode
                        ? "A signed, encrypted workspace, created instantly on this device."
                        : "Bring credentials from a teammate or another of your devices."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeSm
                    wrapMode: Text.WordWrap
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: modeRow.implicitHeight + 8
            radius: Tokens.radiusSm
            color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.04)
            border.width: 1
            border.color: Tokens.borderSubtle

            RowLayout {
                id: modeRow
                anchors.fill: parent
                anchors.margins: 4
                spacing: 4

                EntryModeSegment {
                    label: "Create new"
                    active: root.createMode
                    onChosen: root.app.workspaceEntryMode = "create"
                }

                EntryModeSegment {
                    label: "Join existing"
                    active: !root.createMode
                    onChosen: root.app.workspaceEntryMode = "join"
                }
            }
        }

        StackLayout {
            Layout.fillWidth: true
            currentIndex: root.createMode ? 1 : 0

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                Text {
                    Layout.fillWidth: true
                    text: "Credentials"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                TextArea {
                    id: credentialsArea
                    Layout.fillWidth: true
                    Layout.preferredHeight: 132
                    placeholderText: "Paste an invite, workspace key, or recovery bundle JSON"
                    Accessible.name: "Workspace credentials JSON"
                    color: Tokens.textStrong
                    placeholderTextColor: Tokens.textMuted
                    font.family: Tokens.fontMono
                    font.pixelSize: Tokens.fontSizeSm
                    wrapMode: TextEdit.WrapAnywhere

                    background: Rectangle {
                        radius: Tokens.radiusSm
                        color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                        border.width: credentialsArea.activeFocus ? 2 : 1
                        border.color: credentialsArea.activeFocus ? Tokens.accent : Tokens.borderSubtle
                    }
                }

                LabeledField {
                    id: recoveryPassphraseField
                    Layout.fillWidth: true
                    label: "Recovery passphrase — only for recovery bundles"
                    placeholderText: "Leave empty for invites and workspace keys"
                    echoMode: TextInput.Password
                    onAccepted: root.app.submitWorkspaceJoin()
                }

                LabeledField {
                    id: peerEndpointField
                    Layout.fillWidth: true
                    label: "Peer endpoint — optional, you can pull history later"
                    placeholderText: "direct+tcp://host:port or iroh://…"
                    onAccepted: root.app.submitWorkspaceJoin()
                }

                Text {
                    Layout.fillWidth: true
                    text: "Keys unlock history. Writing needs a signed invite from the workspace owner."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: Tokens.space1
                    spacing: Tokens.space2

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: "Cancel"
                        onClicked: root.close()
                    }

                    EntryPrimaryButton {
                        text: "Join workspace"
                        enabled: root.app.runtimeAccessReady
                            && credentialsArea.text.trim().length > 0
                        onClicked: root.app.submitWorkspaceJoin()
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                LabeledField {
                    id: createNameField
                    Layout.fillWidth: true
                    label: "Workspace name"
                    placeholderText: "e.g. Skunkworks"
                    onAccepted: root.app.submitWorkspaceCreate()
                }

                LabeledField {
                    id: createChannelField
                    Layout.fillWidth: true
                    label: "First channel"
                    placeholderText: "general"
                    onAccepted: root.app.submitWorkspaceCreate()
                }

                Text {
                    Layout.fillWidth: true
                    text: "Nothing leaves this machine until you host, back up, or invite."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: Tokens.space1
                    spacing: Tokens.space2

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: "Cancel"
                        onClicked: root.close()
                    }

                    EntryPrimaryButton {
                        text: "Create workspace"
                        enabled: root.app.runtimeAccessReady
                            && createNameField.text.trim().length > 0
                        onClicked: root.app.submitWorkspaceCreate()
                    }
                }
            }
        }
    }

    onOpened: {
        peerEndpointField.text = chaftController.defaultPeerEndpoint
        root.focusInitialField()
    }

    onClosed: root.resetForm()
}
