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
    property string createAccessPolicyText: createAccessPolicyBox.currentValue || "invite_only"
    property alias displayNameText: displayNameField.text

    readonly property bool createMode: root.app.workspaceEntryMode === "create"
    readonly property var createAccessPolicyOptions: [
        {
            label: "Invite only",
            value: "invite_only",
            description: "Only people with an invite can join."
        },
        {
            label: "People can request access",
            value: "request_access",
            description: "New people can send a request for an admin to approve."
        }
    ]
    readonly property bool restoreMode: !root.createMode
        && root.app.workspaceEntryIntent === "restore"
    readonly property var credentialSummary: root.app
        ? root.app.credentialImportSummary(
            credentialsArea.text,
            root.restoreMode,
            peerEndpointField.text,
            recoveryPassphraseField.text)
        : ({ title: "", message: "", rows: [], canImport: false, warning: false })
    readonly property bool credentialSummaryVisible: !root.createMode
        && String(root.credentialSummary.title || "").length > 0
    readonly property var credentialObject: root.app
        ? root.app.parsedCredentialObject(credentialsArea.text)
        : null
    readonly property string credentialKind: String((root.credentialObject && root.credentialObject.kind) || "")
    readonly property bool restoreCredentialSelected: root.credentialSummaryVisible
        && (root.restoreMode
            || root.credentialKind === "chaft.recovery-bundle.v1"
            || String(root.credentialSummary.title || "") === "Recovery kit found")
    readonly property var workspaceInvite: root.credentialKind === "chaft.workspace-invite.v1"
        ? root.credentialObject
        : null
    readonly property string credentialEmbeddedPeerEndpoint: root.workspaceInvite !== null
        ? String(root.workspaceInvite.peerEndpoint || "").trim()
        : ""
    readonly property bool approvalInviteNeedsRequest: root.workspaceInvite !== null
        && root.app !== null
        && root.app.inviteApprovalBlocksJoin(root.workspaceInvite.approvalPolicy)
    readonly property bool credentialTextSummaryVisible: root.credentialSummaryVisible
        && (root.restoreCredentialSelected
            || root.workspaceCard !== null
            || root.workspaceInvite !== null)
    readonly property var workspaceCard: root.app
        ? root.app.workspaceCardObjectFromCredentials(credentialsArea.text)
        : null
    readonly property bool workspaceCardAllowsRequests: root.workspaceCard === null
        || (root.app && root.app.workspaceAccessPolicyAllowsRequests(root.workspaceCard.accessPolicy))
    readonly property bool joinRequestHasContext: root.workspaceCard !== null
        || root.approvalInviteNeedsRequest
    readonly property bool requestAccessContext: !root.restoreMode
        && !root.joinRequestPrepared
        && root.joinRequestHasContext
    readonly property bool requestOnlyCredential: root.credentialKind === "chaft.workspace-card.v1"
        || root.credentialKind === "chaft.workspace-join-request.v1"
    readonly property bool peerEndpointInputVisible: !root.createMode
        && root.credentialSummaryVisible
        && !root.requestOnlyCredential
        && !root.approvalInviteNeedsRequest
        && (root.restoreCredentialSelected
            || root.credentialEmbeddedPeerEndpoint.length === 0)
    property bool joinRequestPrepared: false
    property string joinRequestPreparedAction: "prepared"
    property string joinRequestPreparedWorkspaceLabel: ""
    property string joinRequestPreparedDisplayName: ""
    property string joinRequestDirectSubmitError: ""
    property bool credentialImportPending: false
    property string credentialImportFailureTitle: ""
    property string credentialImportFailureMessage: ""
    property string credentialImportFailureDetail: ""

    modal: true
    width: Math.min(root.app.width - 48, 560)
    x: Math.round((root.app.width - width) / 2)
    y: Math.round((root.app.height - height) / 2)
    padding: Tokens.space4
    closePolicy: root.credentialImportPending
        ? Popup.NoAutoClose
        : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)

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

    function chooseEntryMode(mode, intent) {
        root.joinRequestPrepared = false
        root.credentialImportPending = false
        root.clearCredentialImportFailure()
        root.app.workspaceEntryMode = mode
        root.app.workspaceEntryIntent = intent
    }

    function selectedCredentialTitle() {
        if (root.restoreMode || root.restoreCredentialSelected) {
            return "Recovery kit selected"
        }
        if (root.workspaceCard !== null) {
            return "Request link selected"
        }
        if (root.approvalInviteNeedsRequest) {
            return "Approval invite selected"
        }
        return "Invite selected"
    }

    function selectedCredentialReplacementText() {
        if (root.restoreMode || root.restoreCredentialSelected) {
            return "Open another recovery kit or drop one here to replace it."
        }
        if (root.workspaceCard !== null) {
            return "Open another request link or drop one here to replace it."
        }
        return "Open another invite or drop one here to replace it."
    }

    function resetForm() {
        createNameField.text = ""
        createChannelField.text = "general"
        createAccessPolicyBox.currentIndex = 0
        displayNameField.text = ""
        credentialsArea.text = ""
        recoveryPassphraseField.text = ""
        peerEndpointField.text = chaftController.defaultPeerEndpoint
        joinRequestNoteArea.text = ""
        root.joinRequestPrepared = false
        root.joinRequestPreparedAction = "prepared"
        root.joinRequestPreparedWorkspaceLabel = ""
        root.joinRequestPreparedDisplayName = ""
        root.credentialImportPending = false
        root.clearCredentialImportFailure()
    }

    function clearCredentialImportFailure() {
        root.credentialImportFailureTitle = ""
        root.credentialImportFailureMessage = ""
        root.credentialImportFailureDetail = ""
    }

    function showCredentialImportFailure(source, status) {
        var summary = root.app
            ? root.app.credentialImportFailureSummary(source, status)
            : ({
                title: "Couldn't open this item",
                message: "Check the invite, request link, or recovery kit and try again.",
                detail: String(status || "")
            })
        root.credentialImportFailureTitle = String(summary.title || "")
        root.credentialImportFailureMessage = String(summary.message || "")
        root.credentialImportFailureDetail = String(summary.detail || "")
    }

    function credentialImportSupportText() {
        var title = String(root.credentialImportFailureTitle || "").trim()
        var detail = String(root.credentialImportFailureDetail || "").trim()
        if (title.length === 0 && detail.length === 0) {
            return ""
        }
        if (detail.length === 0) {
            return title
        }
        return title + ": " + detail
    }

    function retryCredentialImportInput() {
        root.clearCredentialImportFailure()
        if (root.restoreMode) {
            recoveryPassphraseField.text = ""
            recoveryPassphraseField.forceFieldFocus()
            return
        }
        credentialsArea.forceActiveFocus()
    }

    function openReplacementCredentialFile() {
        root.clearCredentialImportFailure()
        root.app.openWorkspaceCredentialFile()
    }

    function joinRequestWorkspaceId() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.workspaceId || "")
        }
        return root.approvalInviteNeedsRequest
            ? String(root.workspaceInvite.workspaceId || "")
            : ""
    }

    function joinRequestWorkspaceName() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.workspaceName || "")
        }
        return root.approvalInviteNeedsRequest
            ? String(root.workspaceInvite.workspaceName || "")
            : ""
    }

    function joinRequestTargetLabel() {
        if (root.workspaceCard !== null && root.app) {
            return root.app.workspaceCardLabel(root.workspaceCard)
        }
        if (root.approvalInviteNeedsRequest && root.app) {
            var workspaceName = String(root.workspaceInvite.workspaceName || "").trim()
            if (workspaceName.length > 0) {
                return workspaceName
            }
            var workspaceId = String(root.workspaceInvite.workspaceId || "").trim()
            if (workspaceId.length > 0) {
                return root.app.shortAccessIdentifier(workspaceId)
            }
        }
        return "Workspace admin"
    }

    function workspaceCardDeliveryDeviceId() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.adminDeviceId || "").trim()
        }
        return root.approvalInviteNeedsRequest
            ? String(root.workspaceInvite.inviterDeviceId || "").trim()
            : ""
    }

    function workspaceCardDeliveryDisplayName() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.adminDisplayName || "").trim()
        }
        return root.approvalInviteNeedsRequest
            ? String(root.workspaceInvite.inviterDisplayName || "").trim()
            : ""
    }

    function workspaceCardDeliveryPeerEndpoint() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.peerEndpoint || "").trim()
        }
        return root.approvalInviteNeedsRequest
            ? String(root.workspaceInvite.peerEndpoint || "").trim()
            : ""
    }

    function joinRequestSourceType() {
        if (root.approvalInviteNeedsRequest) {
            return "approval_invite"
        }
        if (root.workspaceCard !== null) {
            return "workspace_card"
        }
        return ""
    }

    function joinRequestSourceInviteId() {
        return root.approvalInviteNeedsRequest
            ? String(root.workspaceInvite.inviteId || "").trim()
            : ""
    }

    function joinRequestSourceDisplayName() {
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.inviterDisplayName || "").trim()
        }
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.adminDisplayName || "").trim()
        }
        return ""
    }

    function joinRequestSourceApprovalPolicy() {
        return root.approvalInviteNeedsRequest
            ? String(root.workspaceInvite.approvalPolicy || "").trim()
            : ""
    }

    function joinRequestDeliveryLabel() {
        var displayName = root.workspaceCardDeliveryDisplayName()
        if (displayName.length > 0) {
            return displayName
        }
        var deviceId = root.workspaceCardDeliveryDeviceId()
        if (deviceId.length > 0) {
            return "the workspace admin"
        }
        return "an owner or admin"
    }

    function joinRequestNextStepLabel() {
        if (root.joinRequestPreparedAction === "sending") {
            return "Sending to " + root.joinRequestDeliveryLabel()
        }
        if (root.joinRequestPreparedAction === "sent") {
            return "Wait for an invite from " + root.joinRequestDeliveryLabel()
        }
        if (root.joinRequestPreparedAction === "send-failed") {
            return "Try again or copy the request"
        }
        if (root.joinRequestPreparedAction === "copied") {
            return "Send copied link to " + root.joinRequestDeliveryLabel()
        }
        if (root.joinRequestPreparedAction === "save") {
            return "Save or send the request file"
        }
        return "Send request to " + root.joinRequestDeliveryLabel()
    }

    function joinRequestDisplayNameLabel() {
        var displayName = displayNameField.text.trim()
        if (displayName.length > 0) {
            return displayName
        }
        var savedName = root.app ? root.app.localDeviceDisplayName().trim() : ""
        return savedName.length > 0 ? savedName : "You"
    }

    function joinRequestPreparedMessage() {
        var target = root.joinRequestDeliveryLabel()
        if (root.joinRequestPreparedAction === "sending") {
            return "Sending your request to " + target + ". Keep this window open until delivery finishes."
        }
        if (root.joinRequestPreparedAction === "sent") {
            return "Request sent to " + target + ". Wait for their invite, then open it here."
        }
        if (root.joinRequestPreparedAction === "send-failed") {
            return "Could not reach " + target + ". Copy the request link or save the file, then send it to them."
        }
        if (root.joinRequestPreparedAction === "copied") {
            return "Request link copied. Send it to " + target + ", then open the invite here after approval."
        }
        if (root.joinRequestPreparedAction === "save") {
            return "Send the saved request file to " + target + ", then open the invite here after approval."
        }
        return "Copy the request link or save the file, then send it to " + target + ". Chaft keeps the request on the start screen while you wait."
    }

    function joinRequestPreparedBadgeLabel() {
        if (root.joinRequestPreparedAction === "sending") {
            return "Sending"
        }
        if (root.joinRequestPreparedAction === "sent") {
            return "Waiting"
        }
        if (root.joinRequestPreparedAction === "send-failed") {
            return "Not sent"
        }
        if (root.joinRequestPreparedAction === "copied") {
            return "Copied"
        }
        if (root.joinRequestPreparedAction === "save") {
            return "File ready"
        }
        return "Ready"
    }

    function joinRequestPreparedTitle() {
        if (root.joinRequestPreparedAction === "sending") {
            return "Sending request"
        }
        if (root.joinRequestPreparedAction === "sent") {
            return "Waiting for approval"
        }
        if (root.joinRequestPreparedAction === "send-failed") {
            return "Request not sent"
        }
        if (root.joinRequestPreparedAction === "copied") {
            return "Request link copied"
        }
        if (root.joinRequestPreparedAction === "save") {
            return "Request file ready"
        }
        return "Request ready"
    }

    function joinRequestPreparedSourceLabel() {
        return root.app ? root.app.joinRequestSourceLabel(root.app.keyTransferObject()) : ""
    }

    function prepareJoinRequest(action) {
        if (!root.workspaceCardAllowsRequests) {
            return false
        }
        if (!chaftController.stageWorkspaceJoinRequest(
                    displayNameField.text,
                    joinRequestNoteArea.text,
                    root.joinRequestWorkspaceId(),
                    root.joinRequestWorkspaceName(),
                    root.workspaceCardDeliveryDeviceId(),
                    root.workspaceCardDeliveryDisplayName(),
                    root.workspaceCardDeliveryPeerEndpoint(),
                    root.joinRequestSourceType(),
                    root.joinRequestSourceInviteId(),
                    root.joinRequestSourceDisplayName(),
                    root.joinRequestSourceApprovalPolicy())) {
            return false
        }
        root.joinRequestPrepared = true
        root.joinRequestPreparedAction = String(action || "prepared")
        root.joinRequestPreparedWorkspaceLabel = root.joinRequestTargetLabel()
        root.joinRequestPreparedDisplayName = root.joinRequestDisplayNameLabel()
        return true
    }

    function joinRequestCanSendDirect() {
        return root.workspaceCardAllowsRequests
            && root.workspaceCardDeliveryPeerEndpoint().length > 0
            && root.app
            && root.app.runtimeAccessReady
    }

    function sendPreparedJoinRequest() {
        if (root.prepareJoinRequest("prepared")
                && chaftController.submitWorkspaceJoinRequestDirect(
                    root.workspaceCardDeliveryPeerEndpoint(),
                    root.joinRequestWorkspaceId(),
                    chaftController.keyTransferJson)) {
            root.joinRequestDirectSubmitError = ""
            root.joinRequestPreparedAction = "sending"
        } else if (root.joinRequestPrepared) {
            root.joinRequestDirectSubmitError = String(chaftController.syncStatus || "")
            root.joinRequestPreparedAction = "send-failed"
        }
    }

    function copyPreparedJoinRequest() {
        if (root.prepareJoinRequest("prepared")
                && root.app.copyKeyTransferArtifact("access request")) {
            root.app.recordPendingAccessRequestFromCurrentJoinRequest("copied")
            root.joinRequestPreparedAction = "copied"
        }
    }

    function savePreparedJoinRequest() {
        if (root.prepareJoinRequest("prepared")
                && root.app.openSaveKeyTransferDialog("access request")) {
            root.app.recordPendingAccessRequestFromCurrentJoinRequest("file_ready")
            root.joinRequestPreparedAction = "save"
        }
    }

    function editJoinRequest() {
        root.joinRequestPrepared = false
        joinRequestNoteArea.forceActiveFocus()
    }

    function finishPreparedJoinRequest() {
        var status = "ready_to_send"
        if (root.joinRequestPreparedAction === "sent") {
            status = "sent"
        } else if (root.joinRequestPreparedAction === "send-failed") {
            status = "send_failed"
        } else if (root.joinRequestPreparedAction === "copied") {
            status = "copied"
        } else if (root.joinRequestPreparedAction === "save") {
            status = "file_ready"
        }
        root.app.recordPendingAccessRequestFromCurrentJoinRequest(status)
        root.close()
    }

    Connections {
        target: chaftController

        function onJoinRequestDirectSubmitFinished(success, message) {
            if (!root.visible || root.joinRequestPreparedAction !== "sending") {
                return
            }
            if (success) {
                root.app.recordPendingAccessRequestFromCurrentJoinRequest("sent")
                root.joinRequestPreparedAction = "sent"
                return
            }
            root.joinRequestDirectSubmitError = String(message || "")
            root.joinRequestPreparedAction = "send-failed"
        }
    }

    function prepareJoinRequestForSmoke() {
        displayNameField.text = "Sam Rivera"
        joinRequestNoteArea.text = "Design team access"
        root.prepareJoinRequest("sent")
    }

    function prepareRecoveryFailureForSmoke() {
        displayNameField.text = "Sam Rivera"
        recoveryPassphraseField.text = "wrong passphrase"
        root.showCredentialImportFailure(
            "recovery",
            "runtime_import_recovery_bundle_failed: crypto open failed")
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
                    text: root.createMode ? "#" : (root.restoreMode ? "↺" : "⇄")
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
                    text: root.createMode
                        ? "Create a workspace"
                        : (root.restoreMode ? "Restore a workspace" : "Join a workspace")
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeXl
                    font.weight: Font.Bold
                    elide: Text.ElideRight
                }

                Text {
                    Layout.fillWidth: true
                        text: root.createMode
                            ? "Start a private workspace for your team here."
                            : (root.restoreMode
                                ? "Restore access with a saved recovery kit."
                                : "Open an invite, request link, or recovery kit.")
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
                    label: "Create"
                    active: root.createMode
                    onChosen: root.chooseEntryMode("create", "create")
                }

                EntryModeSegment {
                    label: "Join"
                    active: !root.createMode && !root.restoreMode
                    onChosen: root.chooseEntryMode("join", "join")
                }

                EntryModeSegment {
                    label: "Restore"
                    active: root.restoreMode
                    onChosen: root.chooseEntryMode("join", "restore")
                }
            }
        }

        LabeledField {
            id: displayNameField
            Layout.fillWidth: true
            label: "Your name"
            placeholderText: "e.g. Ada Lovelace"
        }

        StackLayout {
            Layout.fillWidth: true
            currentIndex: root.createMode ? 1 : 0

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: root.restoreMode
                            ? "Recovery kit"
                            : (root.workspaceCard !== null
                                ? "Request link"
                                : "Invite, request link, or recovery kit")
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Button {
                        text: root.restoreMode ? "Open recovery kit" : "Open invite or request"
                        enabled: root.app.runtimeAccessReady
                        Accessible.name: text
                        Accessible.description: root.restoreMode
                            ? "Choose a recovery kit file"
                            : "Choose an invite, request link, or recovery kit file"
                        onClicked: root.app.openWorkspaceCredentialFile()
                    }
                }

                Item {
                    id: credentialsFrame
                    Layout.fillWidth: true
                    Layout.preferredHeight: root.credentialSummaryVisible ? 104 : 132

                    TextArea {
                        id: credentialsArea
                        anchors.fill: parent
                        visible: !root.credentialTextSummaryVisible
                        placeholderText: root.restoreMode
                            ? "Paste a recovery kit"
                            : "Paste an invite, request link, or recovery kit"
                        Accessible.name: "Workspace invite, request link, or recovery kit"
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

                    Rectangle {
                        anchors.fill: parent
                        visible: root.credentialTextSummaryVisible
                        radius: Tokens.radiusSm
                        color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.08)
                        border.width: 1
                        border.color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.44)

                        ColumnLayout {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.margins: Tokens.space3
                            spacing: Tokens.space1

                            Text {
                                Layout.fillWidth: true
                                text: root.selectedCredentialTitle()
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                text: root.selectedCredentialReplacementText()
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }
                        }
                    }

                    DropArea {
                        id: credentialDropArea
                        anchors.fill: parent
                        keys: ["text/uri-list", "text/plain"]
                        onDropped: function (drop) {
                            if (drop.hasUrls && drop.urls.length > 0) {
                                if (root.app.loadWorkspaceCredentialUrl(drop.urls[0])) {
                                    drop.acceptProposedAction()
                                }
                                return
                            }
                            if (drop.hasText
                                    && root.app.loadWorkspaceCredentialText(drop.text)) {
                                drop.acceptProposedAction()
                            }
                        }
                    }

                    Rectangle {
                        anchors.fill: parent
                        visible: credentialDropArea.containsDrag
                        radius: Tokens.radiusSm
                        color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.14)
                        border.width: 2
                        border.color: Tokens.accent

                        Text {
                            anchors.centerIn: parent
                            width: parent.width - Tokens.space4 * 2
                            text: root.restoreMode
                                ? "Drop recovery kit"
                                : "Drop invite, request link, or recovery kit"
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.DemiBold
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                        }
                    }
                }

                Rectangle {
                    id: credentialSummaryCard
                    Layout.fillWidth: true
                    visible: root.credentialSummaryVisible
                    implicitHeight: credentialSummaryColumn.implicitHeight + Tokens.space2 * 2
                    radius: Tokens.radiusSm
                    color: root.credentialSummary.warning
                        ? Tokens.warningSurface
                        : Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.1)
                    border.width: 1
                    border.color: root.credentialSummary.warning
                        ? Tokens.warning
                        : Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.44)

                    ColumnLayout {
                        id: credentialSummaryColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space2
                        spacing: Tokens.space1

                        Text {
                            Layout.fillWidth: true
                            text: root.credentialSummary.title
                            color: root.credentialSummary.warning ? Tokens.warningText : Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            text: root.credentialSummary.message
                            color: root.credentialSummary.warning ? Tokens.warningText : Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Repeater {
                            model: root.credentialSummary.rows

                            delegate: RowLayout {
                                width: credentialSummaryColumn.width
                                spacing: Tokens.space2

                                Text {
                                    Layout.preferredWidth: 92
                                    text: modelData.label
                                    color: root.credentialSummary.warning ? Tokens.warningText : Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: modelData.value
                                    color: root.credentialSummary.warning ? Tokens.warningText : Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.Medium
                                    elide: Text.ElideRight
                                }
                            }
                        }
                    }
                }

                LabeledField {
                    id: recoveryPassphraseField
                    Layout.fillWidth: true
                    visible: root.restoreCredentialSelected
                    label: "Recovery passphrase"
                    placeholderText: root.restoreCredentialSelected
                        ? "Passphrase used when this kit was saved"
                        : "Optional for invites; required for recovery kits"
                    echoMode: TextInput.Password
                    onAccepted: root.app.submitWorkspaceJoin()
                }

                LabeledField {
                    id: peerEndpointField
                    Layout.fillWidth: true
                    visible: root.peerEndpointInputVisible
                    label: "Teammate address — optional, you can fetch history later"
                    placeholderText: "Paste an address from your teammate"
                    onAccepted: root.app.submitWorkspaceJoin()
                }

                Text {
                    Layout.fillWidth: true
                    visible: root.peerEndpointInputVisible
                    text: root.restoreCredentialSelected
                        ? "History loads from the recovery kit when included; otherwise Chaft waits for a reachable teammate."
                        : "Invites let you join; Chaft loads history when a teammate is reachable."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: root.restoreMode
                        && root.credentialImportFailureTitle.length === 0
                    implicitHeight: restoreOutcomeColumn.implicitHeight + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: Tokens.surfaceRaised
                    border.width: 1
                    border.color: Tokens.borderSubtle

                    ColumnLayout {
                        id: restoreOutcomeColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space2

                        Text {
                            Layout.fillWidth: true
                            text: "After restore"
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Repeater {
                                model: [
                                    {
                                        label: "Access",
                                        value: "Workspace access is restored here."
                                    },
                                {
                                    label: "History",
                                    value: "Loads locally, then fetches from a reachable teammate."
                                },
                                {
                                    label: "Private rooms",
                                    value: "Only rooms included in the kit unlock here."
                                }
                            ]

                            delegate: RowLayout {
                                width: restoreOutcomeColumn.width
                                spacing: Tokens.space2

                                Text {
                                    Layout.preferredWidth: 92
                                    text: modelData.label
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: modelData.value
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeXs
                                    wrapMode: Text.WordWrap
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: root.credentialImportFailureTitle.length > 0
                    implicitHeight: credentialImportFailureColumn.implicitHeight + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: Tokens.warningSurface
                    border.width: 1
                    border.color: Tokens.warning

                    ColumnLayout {
                        id: credentialImportFailureColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space1

                        Text {
                            Layout.fillWidth: true
                            text: root.credentialImportFailureTitle
                            color: Tokens.warningText
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            text: root.credentialImportFailureMessage
                            color: Tokens.warningText
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Button {
                                visible: root.restoreMode
                                text: "Re-enter passphrase"
                                onClicked: root.retryCredentialImportInput()
                            }

                            Button {
                                text: root.restoreMode ? "Open another kit" : "Open another file"
                                enabled: root.app.runtimeAccessReady
                                onClicked: root.openReplacementCredentialFile()
                            }

                            Item {
                                Layout.fillWidth: true
                            }

                            Button {
                                visible: root.credentialImportFailureDetail.length > 0
                                text: "Copy support detail"
                                onClicked: root.app.copyTextToClipboard(
                                    root.credentialImportSupportText(),
                                    "support detail")
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: !root.restoreMode
                        && (!root.credentialSummaryVisible
                            || root.workspaceCard !== null
                            || root.approvalInviteNeedsRequest)
                    implicitHeight: joinRequestColumn.implicitHeight + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: Tokens.surfaceRaised
                    border.width: 1
                    border.color: Tokens.borderSubtle

                    ColumnLayout {
                        id: joinRequestColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space2

                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: !root.joinRequestPrepared
                                && !root.joinRequestHasContext
                                && !root.credentialSummaryVisible
                            spacing: Tokens.space2

                            Text {
                                Layout.fillWidth: true
                                text: "No invite yet?"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                text: "Ask an owner or admin for a Chaft invite. If they shared a request link, open it here to request access."
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: !root.joinRequestPrepared
                                && root.joinRequestHasContext
                                && root.workspaceCardAllowsRequests
                            spacing: Tokens.space2

                            Text {
                                Layout.fillWidth: true
                                text: "Need access?"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                text: root.workspaceCard !== null || root.approvalInviteNeedsRequest
                                    ? "Send an access request for " + root.joinRequestTargetLabel()
                                        + ". An owner or admin will send back an invite you can open here."
                                    : "Send an access request to a workspace admin. They'll send back an invite you can open here."
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }

                            Text {
                                Layout.fillWidth: true
                                text: "Note for admin"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            TextArea {
                                id: joinRequestNoteArea
                                Layout.fillWidth: true
                                Layout.preferredHeight: 58
                                placeholderText: "Optional, e.g. team or project"
                                Accessible.name: "Access request note"
                                color: Tokens.textStrong
                                placeholderTextColor: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeSm
                                wrapMode: TextEdit.WordWrap
                                background: Rectangle {
                                    radius: Tokens.radiusSm
                                    color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                                    border.width: joinRequestNoteArea.activeFocus ? 2 : 1
                                    border.color: joinRequestNoteArea.activeFocus
                                        ? Tokens.accent
                                        : Tokens.borderSubtle
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Button {
                                    Layout.fillWidth: true
                                    visible: root.joinRequestCanSendDirect()
                                    text: chaftController.joinRequestSubmitInFlight
                                        ? "Sending..."
                                        : "Send request"
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    onClicked: root.sendPreparedJoinRequest()
                                }

                                Button {
                                    Layout.fillWidth: true
                                    text: "Copy link"
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    Accessible.name: "Copy request link"
                                    onClicked: root.copyPreparedJoinRequest()
                                }

                                Button {
                                    Layout.fillWidth: true
                                    text: "Save file"
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    Accessible.name: "Save request file"
                                    onClicked: root.savePreparedJoinRequest()
                                }
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: !root.joinRequestPrepared
                                && root.joinRequestHasContext
                                && !root.workspaceCardAllowsRequests
                            spacing: Tokens.space2

                            Text {
                                Layout.fillWidth: true
                                text: "Invite required"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                text: "This workspace is invite only. Ask an owner or admin for a Chaft invite, then paste or open it here."
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: root.joinRequestPrepared
                            spacing: Tokens.space2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Rectangle {
                                    Layout.preferredWidth: 64
                                    Layout.preferredHeight: 26
                                    radius: Tokens.radiusSm
                                    color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.18)
                                    border.width: 1
                                    border.color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.5)

                                    Text {
                                        anchors.centerIn: parent
                                        text: root.joinRequestPreparedBadgeLabel()
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.DemiBold
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1

                                    Text {
                                        Layout.fillWidth: true
                                        text: root.joinRequestPreparedTitle()
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeSm
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideRight
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: root.joinRequestPreparedMessage()
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: requestReadyRows.implicitHeight + Tokens.space2 * 2
                                radius: Tokens.radiusSm
                                color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.04)
                                border.width: 1
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    id: requestReadyRows
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    anchors.margins: Tokens.space2
                                    spacing: Tokens.space1

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        Text {
                                            Layout.preferredWidth: 88
                                            text: "Workspace"
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            elide: Text.ElideRight
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.joinRequestPreparedWorkspaceLabel
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        visible: root.joinRequestPreparedSourceLabel().length > 0
                                        spacing: Tokens.space2

                                        Text {
                                            Layout.preferredWidth: 88
                                            text: "Started with"
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            elide: Text.ElideRight
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.joinRequestPreparedSourceLabel()
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        Text {
                                            Layout.preferredWidth: 88
                                            text: "Name"
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            elide: Text.ElideRight
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.joinRequestPreparedDisplayName
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        Text {
                                            Layout.preferredWidth: 88
                                            text: "Next step"
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            elide: Text.ElideRight
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.joinRequestNextStepLabel()
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        visible: root.joinRequestPreparedAction === "send-failed"
                                            && root.joinRequestDirectSubmitError.length > 0
                                        spacing: Tokens.space2

                                        Text {
                                            Layout.preferredWidth: 88
                                            text: "Reason"
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            elide: Text.ElideRight
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: root.joinRequestDirectSubmitError
                                            color: Tokens.textStrong
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                        }
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Button {
                                    Layout.fillWidth: true
                                    visible: root.joinRequestCanSendDirect()
                                    text: root.joinRequestPreparedAction === "sending"
                                        ? "Sending..."
                                        : (root.joinRequestPreparedAction === "sent"
                                            ? "Resend"
                                            : (root.joinRequestPreparedAction === "send-failed"
                                                ? "Try again"
                                                : "Send request"))
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    onClicked: root.sendPreparedJoinRequest()
                                }

                                Button {
                                    Layout.fillWidth: true
                                    text: "Copy link"
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    Accessible.name: "Copy request link"
                                    onClicked: root.copyPreparedJoinRequest()
                                }

                                Button {
                                    Layout.fillWidth: true
                                    text: "Save file"
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    Accessible.name: "Save request file"
                                    onClicked: root.savePreparedJoinRequest()
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                text: root.joinRequestPreparedAction === "sent"
                                    ? "Done keeps this request on the start screen while you wait for the invite."
                                    : "Done keeps this request on the start screen so you can send it later."
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: Tokens.space1
                    spacing: Tokens.space2

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: root.joinRequestPrepared ? "Edit request" : "Cancel"
                        enabled: !chaftController.joinRequestSubmitInFlight
                            && !root.credentialImportPending
                        onClicked: {
                            if (root.joinRequestPrepared) {
                                root.editJoinRequest()
                            } else {
                                root.close()
                            }
                        }
                    }

                    EntryPrimaryButton {
                        visible: !root.requestAccessContext
                        text: root.credentialImportPending
                            ? (root.restoreMode ? "Restoring..." : "Joining...")
                            : (root.joinRequestPrepared
                                ? "Done"
                                : (root.restoreMode ? "Restore workspace" : "Join workspace"))
                        enabled: root.joinRequestPrepared
                            ? !chaftController.joinRequestSubmitInFlight
                            : (root.app.runtimeAccessReady
                                && !root.credentialImportPending
                                && root.app.credentialCanSubmit(
                                    credentialsArea.text,
                                    root.restoreMode,
                                    recoveryPassphraseField.text))
                        onClicked: {
                            if (root.joinRequestPrepared) {
                                root.finishPreparedJoinRequest()
                            } else {
                                root.app.submitWorkspaceJoin()
                            }
                        }
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
                    label: "First room"
                    placeholderText: "general"
                    onAccepted: root.app.submitWorkspaceCreate()
                }

                Text {
                    Layout.fillWidth: true
                    text: "Who can join"
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }

                ComboBox {
                    id: createAccessPolicyBox

                    Layout.fillWidth: true
                    model: root.createAccessPolicyOptions
                    textRole: "label"
                    valueRole: "value"
                    Accessible.name: "Who can join"
                }

                Text {
                    Layout.fillWidth: true
                    text: String(root.createAccessPolicyOptions[
                        Math.max(0, createAccessPolicyBox.currentIndex)
                    ].description || "")
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Text {
                    Layout.fillWidth: true
                    text: "You can start chatting now, then invite teammates or save a recovery kit."
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
        displayNameField.text = root.app.localDeviceDisplayName()
        root.focusInitialField()
    }

    onClosed: root.resetForm()
}
