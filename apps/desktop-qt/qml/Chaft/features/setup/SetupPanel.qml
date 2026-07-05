import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Chaft

ScrollView {
    id: setupScroll

    // The App root window. Set by App.qml at the single instantiation site.
    property var app

    readonly property var darkThemes: Themes.catalog.filter(function (theme) {
        return theme.dark === true
    })
    readonly property var lightThemes: Themes.catalog.filter(function (theme) {
        return theme.dark !== true
    })

    function clearChannelMemberField() {
        channelMemberDeviceField.text = ""
    }

    // Preserves the manual-vs-system slot routing: in system mode a pick
    // updates the matching dark/light slot, otherwise the single manual theme.
    function applyThemeChoice(themeId) {
        if (app.systemThemeMode) {
            if (Themes.themeById(themeId).dark) {
                chaftController.darkThemeId = themeId
            } else {
                chaftController.lightThemeId = themeId
            }
        } else {
            chaftController.themeId = themeId
        }
    }

    clip: true
    contentWidth: availableWidth
    contentHeight: setupColumn.implicitHeight
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ScrollBar.vertical.policy: ScrollBar.AsNeeded

    Connections {
        target: chaftController
        function onRuntimeUnlockChanged() {
            if (!chaftController.runtimeUnlocked) {
                workspaceKeyField.text = ""
                recoveryPassphraseField.text = ""
            }
        }
        function onKeyTransferJsonChanged() {
            if (chaftController.keyTransferJson.length > 0) {
                workspaceKeyField.text = chaftController.keyTransferJson
            }
        }
        function onWorkspaceSnapshotChanged() {
            if (!displayNameField.fieldActiveFocus) {
                displayNameField.text = setupScroll.app.localDeviceDisplayName()
            }
        }
        function onDeviceIdChanged() {
            if (!displayNameField.fieldActiveFocus) {
                displayNameField.text = setupScroll.app.localDeviceDisplayName()
            }
        }
    }

    FileDialog {
        id: keyPackageDialog
        property string pendingProtocol: "openmls/key-package"
        title: "Publish key package"
        fileMode: FileDialog.OpenFile
        onAccepted: {
            var filePath = app.localPathFromUrl(selectedFile)
            chaftController.publishDeviceKeyPackage(pendingProtocol, filePath)
        }
    }

    Item {
        width: setupScroll.availableWidth
        height: setupColumn.implicitHeight

        ColumnLayout {
            id: setupColumn
            width: parent.width
            spacing: Tokens.space2

            SetupSection {
                Layout.fillWidth: true
                title: "Identity & Profile"
                defaultExpanded: true

                Text {
                    Layout.fillWidth: true
                    visible: chaftController.deviceId.length > 0
                    text: "Device " + chaftController.deviceId
                    color: Tokens.textMuted
                    font.family: Tokens.fontMono
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WrapAnywhere
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    spacing: Tokens.space2

                    LabeledField {
                        id: displayNameField
                        Layout.fillWidth: true
                        label: "Display name"
                        placeholderText: "e.g. Ada Lovelace"
                        Component.onCompleted: text = setupScroll.app.localDeviceDisplayName()
                        onAccepted: {
                            if (app.runtimeWorkReady
                                    && chaftController.updateDeviceProfile(text)) {
                                text = app.localDeviceDisplayName()
                            }
                        }
                    }

                    Button {
                        Layout.alignment: Qt.AlignBottom
                        text: "Set"
                        enabled: app.runtimeWorkReady
                            && displayNameField.text.trim().length > 0
                        onClicked: {
                            if (chaftController.updateDeviceProfile(displayNameField.text)) {
                                displayNameField.text = app.localDeviceDisplayName()
                            }
                        }
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                        && chaftController.runtimeUnlocked
                    text: "Lock runtime"
                    enabled: chaftController.runtimeUnlockClearable
                        && !chaftController.keyTransferInFlight
                        && !chaftController.syncInFlight
                    onClicked: chaftController.clearRuntimeUnlock()
                    ToolTip.visible: hovered
                    ToolTip.text: chaftController.runtimeUnlockClearable
                        ? "Clear cached runtime passphrase"
                        : "Passphrase is provided by environment"
                }

                Button {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                        && chaftController.runtimeLocked
                    text: "Unlock runtime"
                    enabled: !chaftController.keyTransferInFlight
                        && !chaftController.syncInFlight
                    onClicked: chaftController.requestRuntimeUnlock()
                    ToolTip.visible: hovered
                    ToolTip.text: "Show the runtime passphrase prompt"
                }
            }

            SetupSection {
                Layout.fillWidth: true
                title: "Appearance"
                defaultExpanded: true

                CheckBox {
                    text: "Follow system"
                    checked: app.systemThemeMode
                    onToggled: chaftController.themeMode = checked ? "system" : "manual"
                    Accessible.name: "Follow system theme"
                }

                CheckBox {
                    text: "Reduced motion"
                    checked: chaftController.reducedMotionEnabled
                    onToggled: chaftController.reducedMotionEnabled = checked
                    Accessible.name: "Reduced motion"
                    Accessible.description: "Disable interface animations"
                }

                Text {
                    Layout.fillWidth: true
                    text: app.systemThemeMode
                        ? "System " + (app.systemPrefersDark ? "dark" : "light") + " active. Dark → "
                            + Themes.themeById(app.resolvedDarkThemeId).name + ", light → "
                            + Themes.themeById(app.resolvedLightThemeId).name
                            + ". Picking a theme updates its slot."
                        : Tokens.activeTheme.name + " — " + Tokens.activeTheme.tagline
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Text {
                    Layout.fillWidth: true
                    text: "Dark themes"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Repeater {
                        model: setupScroll.darkThemes
                        delegate: ThemeSwatch {
                            id: darkThemeSwatchDelegate
                            required property var modelData

                            themeData: darkThemeSwatchDelegate.modelData
                            active: darkThemeSwatchDelegate.modelData.id === Tokens.activeThemeId
                            onChosen: function (themeId) {
                                setupScroll.applyThemeChoice(themeId)
                            }
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    text: "Light themes"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Repeater {
                        model: setupScroll.lightThemes
                        delegate: ThemeSwatch {
                            id: lightThemeSwatchDelegate
                            required property var modelData

                            themeData: lightThemeSwatchDelegate.modelData
                            active: lightThemeSwatchDelegate.modelData.id === Tokens.activeThemeId
                            onChosen: function (themeId) {
                                setupScroll.applyThemeChoice(themeId)
                            }
                        }
                    }
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: chaftController.hasRuntimeWorkspace
                title: "Members & Access"
                badgeText: setupScroll.app ? String(setupScroll.app.memberCount) : ""

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    LabeledField {
                        id: inviteDeviceField
                        Layout.fillWidth: true
                        label: "Invite device ID"
                        placeholderText: "e.g. dev_4f9a…"
                    }

                    ComboBox {
                        id: inviteRoleBox
                        Layout.alignment: Qt.AlignBottom
                        Layout.preferredWidth: 82
                        model: ["member", "admin", "guest"]
                        Accessible.name: "Invited member role"
                    }

                    Button {
                        Layout.alignment: Qt.AlignBottom
                        text: "Add"
                        enabled: app.runtimeWorkReady
                            && inviteDeviceField.text.trim().length > 0
                        onClicked: {
                            if (chaftController.inviteMember(inviteDeviceField.text, inviteRoleBox.currentText)) {
                                inviteDeviceField.text = ""
                            }
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    text: "The member roster lives in the details panel on the right."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: chaftController.hasRuntimeWorkspace && app.selectedChannelPrivate
                title: "Channels & Privacy"

                Text {
                    Layout.fillWidth: true
                    text: "Grant or revoke access to the selected private channel."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    LabeledField {
                        id: channelMemberDeviceField
                        Layout.fillWidth: true
                        label: "Device ID"
                        placeholderText: "e.g. dev_4f9a…"
                        onAccepted: {
                            if (app.runtimeWorkReady
                                    && chaftController.addChannelMember(app.selectedChannelKey, text)) {
                                text = ""
                            }
                        }
                    }

                    Button {
                        Layout.alignment: Qt.AlignBottom
                        text: "Grant"
                        enabled: app.runtimeWorkReady
                            && channelMemberDeviceField.text.trim().length > 0
                            && app.selectedChannelKey.length > 0
                        onClicked: {
                            if (chaftController.addChannelMember(app.selectedChannelKey, channelMemberDeviceField.text)) {
                                channelMemberDeviceField.text = ""
                            }
                        }
                    }

                    Button {
                        Layout.alignment: Qt.AlignBottom
                        text: "Revoke"
                        enabled: app.runtimeWorkReady
                            && channelMemberDeviceField.text.trim().length > 0
                            && app.selectedChannelKey.length > 0
                        onClicked: app.confirmSetupAction(
                            "Revoke channel access",
                            "Remove device " + channelMemberDeviceField.text.trim()
                                + " from this private channel? The channel key rotates so the "
                                + "device cannot read new channel messages.",
                            "Revoke access",
                            "revoke-channel-member:" + channelMemberDeviceField.text.trim())
                    }
                }
            }

            SetupSection {
                Layout.fillWidth: true
                title: "Keys & Recovery"

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    spacing: Tokens.space2

                    LabeledField {
                        id: keyPackageProtocolField
                        Layout.fillWidth: true
                        label: "Key package protocol"
                        text: "openmls/key-package"
                        placeholderText: "e.g. openmls/key-package"
                    }

                    Button {
                        Layout.alignment: Qt.AlignBottom
                        text: "Publish"
                        enabled: app.runtimeWorkReady
                            && keyPackageProtocolField.text.trim().length > 0
                        onClicked: {
                            keyPackageDialog.pendingProtocol = keyPackageProtocolField.text.trim()
                            keyPackageDialog.open()
                        }
                    }
                }

                GridLayout {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    columns: 2
                    columnSpacing: Tokens.space2
                    rowSpacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "MLS key"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.publishOpenMlsDeviceKeyPackage()
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "MLS workspace"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.createOpenMlsWorkspaceGroup()
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Join workspace"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.joinOpenMlsWorkspaceGroup("")
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Apply workspace"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.applyOpenMlsWorkspaceGroupCommits("")
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Update workspace"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.updateOpenMlsWorkspaceGroup()
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Update all MLS"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.updateWorkspaceOpenMlsGroups()
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "MLS channel"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.createOpenMlsChannelGroup(app.selectedChannelKey)
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "Join channel"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.joinOpenMlsChannelGroup(app.selectedChannelKey, "")
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "Apply channel"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.applyOpenMlsChannelGroupCommits(app.selectedChannelKey, "")
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "Update channel"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.updateOpenMlsChannelGroup(app.selectedChannelKey)
                    }
                }

                Text {
                    Layout.fillWidth: true
                    text: "Key, recovery, trust, or rotation JSON"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }

                TextArea {
                    id: workspaceKeyField
                    Layout.fillWidth: true
                    Layout.preferredHeight: 72
                    placeholderText: "Paste exported JSON"
                    Accessible.name: "Key, recovery, trust, or rotation JSON"
                    color: Tokens.textStrong
                    placeholderTextColor: Tokens.textMuted
                    wrapMode: TextEdit.WrapAnywhere
                    background: Rectangle {
                        radius: Tokens.radiusMd
                        color: Tokens.sidebarInput
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.keyTransferJson.length > 0
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Copy JSON"
                        onClicked: app.copyTextToClipboard(
                            chaftController.keyTransferJson,
                            "credentials JSON")
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Save JSON"
                        onClicked: app.openSaveKeyTransferDialog()
                    }
                }

                LabeledField {
                    id: recoveryPassphraseField
                    Layout.fillWidth: true
                    label: "Recovery passphrase"
                    echoMode: TextInput.Password
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        visible: chaftController.hasRuntimeWorkspace
                        text: "Export workspace"
                        enabled: app.runtimeWorkReady
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.exportWorkspaceKey()
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Import workspace"
                        enabled: workspaceKeyField.text.trim().length > 0
                            && app.runtimeAccessReady
                            && !chaftController.rawEventStoreMode
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.importWorkspaceKey(workspaceKeyField.text)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        visible: chaftController.hasRuntimeWorkspace
                        text: "Export recovery"
                        enabled: recoveryPassphraseField.text.trim().length > 0
                            && app.runtimeWorkReady
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.exportRecoveryBundle(
                            recoveryPassphraseField.text)
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Import recovery"
                        enabled: workspaceKeyField.text.trim().length > 0
                            && recoveryPassphraseField.text.trim().length > 0
                            && app.runtimeAccessReady
                            && !chaftController.rawEventStoreMode
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.importRecoveryBundle(
                            workspaceKeyField.text,
                            recoveryPassphraseField.text)
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    text: "Export trust"
                    enabled: app.runtimeWorkReady
                        && !chaftController.keyTransferInFlight
                    onClicked: chaftController.exportTrustSnapshot()
                    ToolTip.visible: hovered
                    ToolTip.text: "Export the workspace trust snapshot"
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace && app.selectedChannelPrivate
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Export channel"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.exportChannelKey(app.selectedChannelKey)
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Import channel"
                        enabled: workspaceKeyField.text.trim().length > 0
                            && app.runtimeWorkReady
                            && !chaftController.rawEventStoreMode
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.importChannelKey(workspaceKeyField.text)
                    }
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: chaftController.hasRuntimeWorkspace
                title: "Hosting & Backup"
                badgeText: (chaftController.backupPeerEndpoints || []).length > 0
                    ? String((chaftController.backupPeerEndpoints || []).length)
                    : ""

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    LabeledField {
                        id: backupPeerField
                        Layout.fillWidth: true
                        label: "Backup peer endpoint"
                        placeholderText: "e.g. 127.0.0.1:7411"
                        onAccepted: {
                            if (chaftController.addBackupPeerEndpoint(text)) {
                                text = ""
                            }
                        }
                    }

                    Button {
                        Layout.alignment: Qt.AlignBottom
                        text: "Save"
                        enabled: backupPeerField.text.trim().length > 0
                        onClicked: {
                            if (chaftController.addBackupPeerEndpoint(backupPeerField.text)) {
                                backupPeerField.text = ""
                            }
                        }
                    }

                    CheckBox {
                        Layout.alignment: Qt.AlignBottom
                        text: "Auto"
                        checked: app.autoBackupEnabled
                        enabled: app.hasAutoBackupTargets
                        onToggled: app.autoBackupEnabled = checked
                        Accessible.name: "Automatic backup"
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space1
                    visible: (chaftController.backupPeerEndpoints || []).length > 0

                    Repeater {
                        model: chaftController.backupPeerEndpoints

                        delegate: SavedBackupPeerRow {
                            id: savedBackupPeerDelegate
                            required property string modelData

                            endpoint: savedBackupPeerDelegate.modelData
                            statusText: app.backupPeerStatusText(savedBackupPeerDelegate.modelData)
                            onRemoveRequested: function (endpoint) {
                                chaftController.removeBackupPeerEndpoint(endpoint)
                            }
                        }
                    }
                }

                Button {
                    Layout.fillWidth: true
                    text: "Reindex search"
                    enabled: app.runtimeWorkReady
                        && !chaftController.keyTransferInFlight
                    onClicked: chaftController.reindexWorkspaceSearch()
                    ToolTip.visible: hovered
                    ToolTip.text: "Rebuild local message search"
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: chaftController.hasRuntimeWorkspace
                title: "Danger Zone"
                danger: true

                Text {
                    Layout.fillWidth: true
                    text: "Rotations cut off devices that miss the new keys until they resync."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Review"
                        enabled: app.runtimeWorkReady
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.detectCompromise()
                        ToolTip.visible: hovered
                        ToolTip.text: "Review compromise signals"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Rotate keys"
                        enabled: app.runtimeWorkReady
                            && !chaftController.keyTransferInFlight
                        onClicked: app.confirmSetupAction(
                            "Rotate workspace keys",
                            "Rotate the OpenMLS and manual workspace keys now? Devices "
                                + "that miss the rotation cannot read new messages until "
                                + "they resync.",
                            "Rotate keys",
                            "rotate-keys")
                        ToolTip.visible: hovered
                        ToolTip.text: "Rotate OpenMLS and manual keys"
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: app.selectedChannelPrivate
                    text: "Rotate channel key"
                    enabled: app.runtimeWorkReady
                        && app.selectedChannelKey.length > 0
                        && !chaftController.keyTransferInFlight
                    onClicked: app.confirmSetupAction(
                        "Rotate channel key",
                        "Rotate the key for this private channel? Devices without the "
                            + "new key cannot read new channel messages.",
                        "Rotate key",
                        "rotate-channel-key:" + app.selectedChannelKey)
                    ToolTip.visible: hovered
                    ToolTip.text: "Rotate this private channel key"
                }
            }
        }
    }
}
