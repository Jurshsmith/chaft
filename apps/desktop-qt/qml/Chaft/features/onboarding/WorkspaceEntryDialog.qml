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
    property string avatarIdText: ""
    property bool displayNameEditing: false
    property bool receivedApprovalDisplayNamePreserved: false

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
            root.keyKitMode,
            peerEndpointField.text,
            recoveryPassphraseField.text)
        : ({ title: "", message: "", rows: [], canImport: false, warning: false })
    readonly property bool credentialSummaryVisible: !root.createMode
        && String(root.credentialSummary.title || "").length > 0
    readonly property var credentialObject: root.app
        ? root.app.parsedCredentialObject(credentialsArea.text)
        : null
    readonly property string credentialKind: String((root.credentialObject && root.credentialObject.kind) || "")
    readonly property var recoveryBundle: root.app
        ? root.app.credentialRecoveryBundleObject(root.credentialObject)
        : null
    readonly property bool credentialIsRecoveryBundle:
        root.recoveryBundle !== null
    readonly property bool keyKitMode: !root.createMode
        && (root.restoreMode || root.credentialIsRecoveryBundle)
    readonly property bool restoreCredentialSelected: root.credentialSummaryVisible
        && root.credentialIsRecoveryBundle
    readonly property var workspaceInvite: root.credentialKind === "chaft.workspace-invite.v1"
        ? root.credentialObject
        : null
    readonly property var claimableInvite: root.credentialKind === "chaft.workspace-invite.v2"
        ? root.credentialObject
        : null
    readonly property var workspaceInviteResponse:
        root.credentialKind === "chaft.workspace-invite-response.v1"
            ? root.credentialObject
            : null
    readonly property var credentialApprovalContext: root.app
        ? root.app.workspaceCredentialApprovalContext(credentialsArea.text)
        : ({ recognized: false, requestId: "", pendingDisplayName: "" })
    readonly property bool secureInviteClaim: root.claimableInvite !== null
    readonly property int secureInviteMaxClaims: root.secureInviteClaim && root.app
        ? root.app.inviteMaxClaims(root.claimableInvite)
        : 1
    readonly property bool secureInviteExpired: root.secureInviteClaim
        && root.app
        && root.app.inviteExpired(root.claimableInvite.expiresAt)
    readonly property string credentialEmbeddedPeerEndpoint: root.workspaceInvite !== null
        ? String(root.workspaceInvite.peerEndpoint || "").trim()
        : (root.claimableInvite !== null
            ? String(root.claimableInvite.peerEndpoint || "").trim()
            : "")
    readonly property bool approvalInviteNeedsRequest: root.workspaceInvite !== null
        && root.app !== null
        && root.app.inviteApprovalBlocksJoin(root.workspaceInvite.approvalPolicy)
    readonly property bool credentialTextSummaryVisible: root.credentialSummaryVisible
        && (root.restoreCredentialSelected
            || root.workspaceCard !== null
            || root.workspaceInvite !== null
            || root.claimableInvite !== null
            || root.workspaceInviteResponse !== null)
    readonly property var workspaceCard: root.app
        ? root.app.workspaceCardObjectFromCredentials(credentialsArea.text)
        : null
    readonly property bool workspaceCardAllowsRequests: !root.secureInviteExpired
        && (root.workspaceCard === null
            || (root.app && root.app.workspaceAccessPolicyAllowsRequests(root.workspaceCard.accessPolicy)))
    readonly property bool joinRequestHasContext: root.workspaceCard !== null
        || root.approvalInviteNeedsRequest
        || root.secureInviteClaim
    readonly property bool requestAccessContext: !root.restoreMode
        && !root.joinRequestPrepared
        && root.joinRequestHasContext
    readonly property bool receivedApprovalCredential:
        root.credentialApprovalContext.recognized === true
    readonly property bool joinIdentityVisible: !root.createMode
        && !root.credentialIsRecoveryBundle
        && root.credentialSummaryVisible
        && (!root.receivedApprovalCredential
            || !root.receivedApprovalDisplayNamePreserved)
    readonly property bool createIdentityVisible: root.createMode
        && root.app
        && root.app.localDeviceDisplayName().trim().length === 0
    readonly property bool identityVisible: root.joinIdentityVisible
        || root.createIdentityVisible
    readonly property bool displayNameReady:
        displayNameField.text.trim().length > 0
    readonly property bool displayNameEditorVisible: root.identityVisible
        && (root.createIdentityVisible
            || root.displayNameEditing
            || !root.displayNameReady)
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
    property string joinRequestPreparedRequestId: ""
    property string joinRequestDirectSubmitError: ""
    property string joinRequestSaveOperationToken: ""
    property bool joinRequestNoteExpanded: false
    property bool credentialImportPending: false
    property bool createOperationPending: false
    property string createOperationError: ""
    property string credentialImportFailureTitle: ""
    property string credentialImportFailureMessage: ""
    property string credentialImportFailureDetail: ""
    readonly property bool handoffOperationPending: root.credentialImportPending
        || root.createOperationPending
        || root.joinRequestPreparedAction === "sending"

    modal: true
    width: Math.min(root.app.width - 48, 560)
    x: Math.round((root.app.width - width) / 2)
    y: Math.round((root.app.height - height) / 2)
    padding: Tokens.space4
    closePolicy: root.handoffOperationPending
        ? Popup.NoAutoClose
        : Popup.CloseOnEscape

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
        opacity: segment.enabled ? 1 : 0.5

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
            enabled: segment.enabled
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
        if (root.displayNameEditorVisible) {
            displayNameField.forceFieldFocus()
        } else if (root.createMode) {
            createNameField.forceFieldFocus()
        } else {
            credentialsArea.forceActiveFocus()
        }
    }

    function synchronizeCredentialMode() {
        if (!root.visible || root.createMode || !root.credentialSummaryVisible) {
            return
        }
        var nextIntent = root.credentialIsRecoveryBundle ? "restore" : "join"
        if (root.app.workspaceEntryIntent !== nextIntent) {
            root.app.workspaceEntryIntent = nextIntent
        }
    }

    function chooseEntryMode(mode, intent) {
        if (root.handoffOperationPending) {
            return
        }
        if (intent === "join" && root.credentialIsRecoveryBundle) {
            credentialsArea.text = ""
            recoveryPassphraseField.text = ""
        } else if (intent === "restore" && root.credentialSummaryVisible
                   && !root.credentialIsRecoveryBundle) {
            credentialsArea.text = ""
            recoveryPassphraseField.text = ""
        }
        root.joinRequestPrepared = false
        root.joinRequestPreparedRequestId = ""
        root.joinRequestDirectSubmitError = ""
        root.joinRequestSaveOperationToken = ""
        root.joinRequestNoteExpanded = false
        root.credentialImportPending = false
        root.createOperationPending = false
        root.createOperationError = ""
        root.displayNameEditing = false
        root.receivedApprovalDisplayNamePreserved = false
        root.clearCredentialImportFailure()
        root.app.workspaceEntryMode = mode
        root.app.workspaceEntryIntent = intent
    }

    function selectedCredentialTitle() {
        if (root.restoreMode || root.restoreCredentialSelected) {
            return "Decryption key kit selected"
        }
        if (root.workspaceCard !== null) {
            return "Request link selected"
        }
        if (root.approvalInviteNeedsRequest) {
            return "Approval invite selected"
        }
        if (root.secureInviteClaim) {
            return "Secure invite selected"
        }
        if (root.workspaceInviteResponse !== null) {
            return "Secure approval received"
        }
        if (root.workspaceInvite !== null && root.receivedApprovalCredential) {
            return "Approval received"
        }
        return "Invite selected"
    }

    function selectedCredentialReplacementText() {
        if (root.restoreMode || root.restoreCredentialSelected) {
            return "Open another decryption key kit or drop one here to replace it."
        }
        if (root.workspaceCard !== null) {
            return "Open another request link or drop one here to replace it."
        }
        if (root.secureInviteClaim) {
            return "Review this invite, then join from this device."
        }
        if (root.workspaceInviteResponse !== null) {
            return "Review this encrypted approval, then join the workspace."
        }
        if (root.workspaceInvite !== null && root.receivedApprovalCredential) {
            return "Review the invite, then join this workspace when ready."
        }
        return "Open another invite or drop one here to replace it."
    }

    function resetForm() {
        createNameField.text = ""
        createChannelField.text = "general"
        createAccessPolicyBox.currentIndex = 0
        displayNameField.text = ""
        root.avatarIdText = ""
        credentialsArea.text = ""
        recoveryPassphraseField.text = ""
        peerEndpointField.text = chaftController.defaultPeerEndpoint
        joinRequestNoteArea.text = ""
        root.joinRequestPrepared = false
        root.joinRequestPreparedAction = "prepared"
        root.joinRequestPreparedWorkspaceLabel = ""
        root.joinRequestPreparedDisplayName = ""
        root.joinRequestPreparedRequestId = ""
        root.joinRequestDirectSubmitError = ""
        root.joinRequestSaveOperationToken = ""
        root.joinRequestNoteExpanded = false
        root.displayNameEditing = false
        root.receivedApprovalDisplayNamePreserved = false
        root.credentialImportPending = false
        root.createOperationPending = false
        root.createOperationError = ""
        root.clearCredentialImportFailure()
    }

    function beginReceivedApproval(displayName, requestId) {
        // Transition the matching request into its approval step without
        // dropping the identity captured when the request was signed.
        credentialsArea.text = ""
        recoveryPassphraseField.text = ""
        peerEndpointField.text = chaftController.defaultPeerEndpoint
        joinRequestNoteArea.text = ""
        root.joinRequestPrepared = false
        root.joinRequestPreparedAction = "prepared"
        root.joinRequestPreparedWorkspaceLabel = ""
        root.joinRequestPreparedDisplayName = ""
        root.joinRequestPreparedRequestId = String(requestId || "").trim()
        root.joinRequestDirectSubmitError = ""
        root.joinRequestSaveOperationToken = ""
        root.joinRequestNoteExpanded = false
        root.displayNameEditing = false
        root.credentialImportPending = false
        root.clearCredentialImportFailure()
        var preservedName = String(displayName || "").trim()
        if (preservedName.length > 0) {
            displayNameField.text = preservedName
        }
        root.bindCredentialIdentity(displayName, requestId, true)
    }

    function bindCredentialIdentity(displayName, requestId, approvalCredential) {
        var wasPreserved = root.receivedApprovalDisplayNamePreserved
        var preservedName = String(displayName || "").trim()
        var preserve = approvalCredential === true && preservedName.length > 0
        root.receivedApprovalDisplayNamePreserved = preserve
        if (approvalCredential === true) {
            root.joinRequestPreparedRequestId = String(requestId || "").trim()
        } else if (wasPreserved) {
            root.joinRequestPreparedRequestId = ""
        }
        if (preserve) {
            displayNameField.text = preservedName
            root.displayNameEditing = false
            return
        }
        if (wasPreserved || displayNameField.text.trim().length === 0) {
            displayNameField.text = root.app
                ? root.app.localDeviceDisplayName()
                : ""
            root.displayNameEditing = displayNameField.text.trim().length === 0
        }
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
                message: "Check the invite, request link, decryption key kit, or access file and try again.",
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
        if (root.credentialIsRecoveryBundle) {
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
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.workspaceId || "")
        }
        return root.secureInviteClaim
            ? String(root.claimableInvite.workspaceId || "")
            : ""
    }

    function joinRequestWorkspaceName() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.workspaceName || "")
        }
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.workspaceName || "")
        }
        return root.secureInviteClaim
            ? String(root.claimableInvite.workspaceName || "")
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
        if (root.secureInviteClaim && root.app) {
            var claimWorkspaceName = String(root.claimableInvite.workspaceName || "").trim()
            return claimWorkspaceName.length > 0
                ? claimWorkspaceName
                : root.app.shortAccessIdentifier(root.claimableInvite.workspaceId)
        }
        return "Workspace admin"
    }

    function workspaceCardDeliveryDeviceId() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.adminDeviceId || "").trim()
        }
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.inviterDeviceId || "").trim()
        }
        return root.secureInviteClaim
            ? String(root.claimableInvite.inviterDeviceId || "").trim()
            : ""
    }

    function workspaceCardDeliveryDisplayName() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.adminDisplayName || "").trim()
        }
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.inviterDisplayName || "").trim()
        }
        return root.secureInviteClaim
            ? String(root.claimableInvite.inviterDisplayName || "").trim()
            : ""
    }

    function workspaceCardDeliveryPeerEndpoint() {
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.peerEndpoint || "").trim()
        }
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.peerEndpoint || "").trim()
        }
        return root.secureInviteClaim
            ? String(root.claimableInvite.peerEndpoint || "").trim()
            : ""
    }

    function joinRequestSourceType() {
        if (root.secureInviteClaim) {
            return "invite_claim"
        }
        if (root.approvalInviteNeedsRequest) {
            return "approval_invite"
        }
        if (root.workspaceCard !== null) {
            return "workspace_card"
        }
        return ""
    }

    function joinRequestSourceInviteId() {
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.inviteId || "").trim()
        }
        return root.secureInviteClaim
            ? String(root.claimableInvite.inviteId || "").trim()
            : ""
    }

    function joinRequestSourceDisplayName() {
        if (root.secureInviteClaim) {
            return String(root.claimableInvite.inviterDisplayName || "").trim()
        }
        if (root.approvalInviteNeedsRequest) {
            return String(root.workspaceInvite.inviterDisplayName || "").trim()
        }
        if (root.workspaceCard !== null) {
            return String(root.workspaceCard.adminDisplayName || "").trim()
        }
        return ""
    }

    function joinRequestSourceApprovalPolicy() {
        if (root.secureInviteClaim) {
            return "preapproved"
        }
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
            return (root.secureInviteClaim ? "Contacting " : "Sending to ")
                + root.joinRequestDeliveryLabel()
        }
        if (root.joinRequestPreparedAction === "sent") {
            return root.secureInviteClaim
                ? "Wait for encrypted access from " + root.joinRequestDeliveryLabel()
                : "Wait for an invite from " + root.joinRequestDeliveryLabel()
        }
        if (root.joinRequestPreparedAction === "send-failed") {
            return root.secureInviteClaim
                ? "Try again or transfer the join request"
                : "Try again or copy the request"
        }
        if (root.joinRequestPreparedAction === "save-failed") {
            return root.secureInviteClaim
                ? "Try saving again or copy the join request"
                : "Try saving again or copy the request"
        }
        if (root.joinRequestPreparedAction === "copied") {
            return (root.secureInviteClaim
                ? "Send copied join request to "
                : "Send copied link to ") + root.joinRequestDeliveryLabel()
        }
        if (root.joinRequestPreparedAction === "save") {
            return root.secureInviteClaim
                ? "Send the saved join request"
                : "Save or send the request file"
        }
        return (root.secureInviteClaim ? "Send join request to " : "Send request to ")
            + root.joinRequestDeliveryLabel()
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
            return (root.secureInviteClaim ? "Contacting " : "Sending your request to ")
                + target + ". Keep this window open until delivery finishes."
        }
        if (root.joinRequestPreparedAction === "sent") {
            return root.secureInviteClaim
                ? "Request sent to " + target + ". Chaft will check for encrypted access."
                : "Request sent to " + target + ". Wait for their invite, then open it here."
        }
        if (root.joinRequestPreparedAction === "send-failed") {
            return root.secureInviteClaim
                ? "Could not reach " + target + ". Try again or send the join request manually."
                : "Could not reach " + target + ". Copy the request link or save the file, then send it to them."
        }
        if (root.joinRequestPreparedAction === "save-failed") {
            return root.secureInviteClaim
                ? "The join request was not saved. Try again or copy it instead."
                : "The request file was not saved. Try again or copy the request link."
        }
        if (root.joinRequestPreparedAction === "copied") {
            return root.secureInviteClaim
                ? "Join request copied. Send it to " + target + ", then open the encrypted access response here."
                : "Request link copied. Send it to " + target + ", then open the invite here after approval."
        }
        if (root.joinRequestPreparedAction === "save") {
            return root.secureInviteClaim
                ? "Send the saved join request to " + target + ", then open the encrypted access response here."
                : "Send the saved request file to " + target + ", then open the invite here after approval."
        }
        return root.secureInviteClaim
            ? "Send this join request to " + target + ". Chaft keeps it on the start screen while you wait."
            : "Copy the request link or save the file, then send it to " + target + ". Chaft keeps the request on the start screen while you wait."
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
        if (root.joinRequestPreparedAction === "save-failed") {
            return "Not saved"
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
            return root.secureInviteClaim ? "Joining workspace" : "Sending request"
        }
        if (root.joinRequestPreparedAction === "sent") {
            return root.secureInviteClaim ? "Waiting for access" : "Waiting for approval"
        }
        if (root.joinRequestPreparedAction === "send-failed") {
            return root.secureInviteClaim ? "Couldn't contact workspace" : "Request not sent"
        }
        if (root.joinRequestPreparedAction === "save-failed") {
            return root.secureInviteClaim ? "Join request not saved" : "Request file not saved"
        }
        if (root.joinRequestPreparedAction === "copied") {
            return root.secureInviteClaim ? "Join request copied" : "Request link copied"
        }
        if (root.joinRequestPreparedAction === "save") {
            return root.secureInviteClaim ? "Join request ready" : "Request file ready"
        }
        return root.secureInviteClaim ? "Manual transfer needed" : "Request ready"
    }

    function joinRequestPreparedSourceLabel() {
        return root.app ? root.app.joinRequestSourceLabel(root.app.keyTransferObject()) : ""
    }

    function prepareJoinRequest(action) {
        if (!root.workspaceCardAllowsRequests) {
            return false
        }
        if (!root.displayNameReady) {
            root.displayNameEditing = true
            root.joinRequestDirectSubmitError =
                "Enter the name teammates should see."
            displayNameField.forceFieldFocus()
            return false
        }
        var prepared = root.secureInviteClaim
            ? chaftController.prepareWorkspaceInviteClaim(
                JSON.stringify(root.claimableInvite),
                displayNameField.text,
                joinRequestNoteArea.text)
            : chaftController.stageWorkspaceJoinRequest(
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
                    root.joinRequestSourceApprovalPolicy())
        if (!prepared) {
            return false
        }
        root.joinRequestPrepared = true
        root.joinRequestPreparedAction = String(action || "prepared")
        root.joinRequestPreparedWorkspaceLabel = root.joinRequestTargetLabel()
        root.joinRequestPreparedDisplayName = root.joinRequestDisplayNameLabel()
        var preparedRequest = root.app ? root.app.keyTransferObject() : null
        root.joinRequestPreparedRequestId = String(
            (preparedRequest && preparedRequest.requestId) || "").trim()
        root.displayNameEditing = false
        return true
    }

    function joinRequestCanSendDirect() {
        return root.workspaceCardAllowsRequests
            && root.workspaceCardDeliveryPeerEndpoint().length > 0
            && root.app
            && root.app.runtimeAccessReady
    }

    function startWorkspaceAccess() {
        root.joinRequestDirectSubmitError = ""
        if (root.joinRequestCanSendDirect()) {
            root.sendPreparedJoinRequest()
            return
        }
        if (!root.prepareJoinRequest("prepared")) {
            root.joinRequestDirectSubmitError = String(
                chaftController.syncStatus || "Could not prepare the join request.")
        }
    }

    function sendPreparedJoinRequest() {
        if (!root.joinRequestPrepared && !root.prepareJoinRequest("prepared")) {
            return
        }
        if (root.joinRequestPrepared && !root.app.keyTransferIsJoinRequest()) {
            root.joinRequestDirectSubmitError =
                "Encrypted workspace access has arrived. Open it to finish joining."
            return
        }
        if (!root.app.recordPendingAccessRequestFromCurrentJoinRequest("sending")) {
            root.joinRequestDirectSubmitError = root.secureInviteClaim
                ? "Could not save the pending join request."
                : "Could not save the pending access request."
            root.joinRequestPreparedAction = "send-failed"
            return
        }
        if (chaftController.submitWorkspaceJoinRequestDirect(
                root.workspaceCardDeliveryPeerEndpoint(),
                root.joinRequestWorkspaceId(),
                chaftController.keyTransferJson)) {
            root.joinRequestDirectSubmitError = ""
            root.joinRequestPreparedAction = "sending"
        } else {
            root.joinRequestDirectSubmitError = String(chaftController.syncStatus || "")
            root.app.recordPendingAccessRequestFromCurrentJoinRequest("send_failed")
            root.joinRequestPreparedAction = "send-failed"
        }
    }

    function copyPreparedJoinRequest() {
        if (!root.joinRequestPrepared && !root.prepareJoinRequest("prepared")) {
            return
        }
        if (!root.app.keyTransferIsJoinRequest()) {
            return
        }
        if (root.app.copyKeyTransferArtifact(
                root.secureInviteClaim ? "join request" : "access request")) {
            if (!root.app.recordPendingAccessRequestFromCurrentJoinRequest("copied")) {
                root.joinRequestDirectSubmitError = root.secureInviteClaim
                    ? "Join request copied, but Chaft could not save its pending response details. Keep this dialog open and restore disk access."
                    : "Request copied, but Chaft could not save its pending response details. Keep this dialog open and restore disk access."
                return
            }
            root.joinRequestDirectSubmitError = ""
            root.joinRequestPreparedAction = "copied"
        }
    }

    function savePreparedJoinRequest() {
        if (!root.joinRequestPrepared && !root.prepareJoinRequest("prepared")) {
            return
        }
        if (!root.app.keyTransferIsJoinRequest()) {
            return
        }
        root.joinRequestDirectSubmitError = ""
        var operationToken = root.app.nextKeyTransferFileSaveToken()
        root.joinRequestSaveOperationToken = operationToken
        if (!root.app.openSaveKeyTransferDialog(
                root.secureInviteClaim ? "join request" : "access request",
                operationToken,
                true)) {
            root.joinRequestSaveOperationToken = ""
            root.joinRequestDirectSubmitError = root.secureInviteClaim
                ? "Could not prepare the join request file."
                : "Could not prepare the request file."
            root.joinRequestPreparedAction = "save-failed"
        }
    }

    function editJoinRequest() {
        if (!root.joinRequestCanEditDetails()) {
            return
        }
        root.joinRequestPrepared = false
        root.joinRequestPreparedRequestId = ""
        root.joinRequestDirectSubmitError = ""
        root.joinRequestSaveOperationToken = ""
        root.joinRequestNoteExpanded = joinRequestNoteArea.text.trim().length > 0
        joinRequestNoteArea.forceActiveFocus()
    }

    function joinRequestCanEditDetails() {
        return root.joinRequestPrepared
            && (root.joinRequestPreparedAction === "prepared"
                || root.joinRequestPreparedAction === "save-failed")
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
        if (!root.app.recordPendingAccessRequestFromCurrentJoinRequest(status)) {
            root.joinRequestDirectSubmitError =
                "Could not save the pending access handoff. Restore disk access before closing."
            return
        }
        root.close()
    }

    Connections {
        target: chaftController

        function onJoinRequestDirectSubmitCompleted(requestId, success, message) {
            if (!root.visible || root.joinRequestPreparedAction !== "sending") {
                return
            }
            if (String(requestId || "").trim()
                    !== root.joinRequestPreparedRequestId) {
                return
            }
            if (success) {
                if (!root.app.recordPendingAccessRequestFromCurrentJoinRequest("sent")) {
                    root.joinRequestDirectSubmitError =
                        "Request sent, but Chaft could not save its pending response details. Keep this dialog open and restore disk access."
                    root.joinRequestPreparedAction = "send-failed"
                    return
                }
                root.joinRequestPreparedAction = "sent"
                return
            }
            root.joinRequestDirectSubmitError = String(message || "")
            root.app.recordPendingAccessRequestFromCurrentJoinRequest("send_failed")
            root.joinRequestPreparedAction = "send-failed"
        }
    }

    Connections {
        target: root.app

        function onKeyTransferFileSaveFinished(success, label, artifactKind,
                                               operationToken) {
            if (!root.visible || !root.joinRequestPrepared) {
                return
            }
            var expectedLabel = root.secureInviteClaim
                ? "join request"
                : "access request"
            var expectedKind = root.secureInviteClaim
                ? "chaft.workspace-invite-claim.v1"
                : "chaft.workspace-join-request.v1"
            if (String(label || "") !== expectedLabel
                    || String(artifactKind || "") !== expectedKind
                    || String(operationToken || "")
                        !== root.joinRequestSaveOperationToken) {
                return
            }
            root.joinRequestSaveOperationToken = ""
            if (!success) {
                root.joinRequestDirectSubmitError = String(
                    chaftController.syncStatus
                        || (root.secureInviteClaim
                            ? "Could not save the join request file."
                            : "Could not save the request file."))
                root.joinRequestPreparedAction = "save-failed"
                return
            }
            root.joinRequestDirectSubmitError = ""
            root.joinRequestPreparedAction = "save"
        }
    }

    Timer {
        id: credentialIdentityRebindTimer
        interval: 0
        repeat: false
        onTriggered: {
            if (root.visible && root.app) {
                root.app.rebindWorkspaceEntryIdentity(credentialsArea.text)
            }
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
                    text: root.createMode ? "#" : (root.restoreCredentialSelected ? "↺" : "⇄")
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
                        : (root.restoreCredentialSelected || root.restoreMode
                            ? "Import a decryption key kit"
                            : "Join a workspace")
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeXl
                    font.weight: Font.Bold
                    elide: Text.ElideRight
                }

                Text {
                    Layout.fillWidth: true
                        text: root.createMode
                            ? "Start a private workspace for your team here."
                            : (root.restoreCredentialSelected || root.restoreMode
                                ? "Import saved keys to decrypt matching history available to this device."
                                : "Open an invite, request link, or access file.")
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
                    enabled: !root.handoffOperationPending
                    onChosen: root.chooseEntryMode("create", "create")
                }

                EntryModeSegment {
                    label: "Join"
                    active: !root.createMode && !root.keyKitMode
                    enabled: !root.handoffOperationPending
                    onChosen: root.chooseEntryMode("join", "join")
                }

                EntryModeSegment {
                    label: "Key kit"
                    active: root.keyKitMode
                    enabled: !root.handoffOperationPending
                    onChosen: root.chooseEntryMode("join", "restore")
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            visible: root.identityVisible && !root.joinRequestPrepared
            spacing: Tokens.space1

            AvatarPicker {
                Layout.fillWidth: true
                avatarId: root.avatarIdText
                workspaceId: root.app ? root.app.currentWorkspaceId() : ""
                identityId: String(chaftController.deviceId || "")
                displayName: displayNameField.text.trim()
                usedAvatarIds: root.app ? root.app.usedWorkspaceAvatarIds() : []
                editable: !root.handoffOperationPending
                onAvatarChosen: function(nextAvatarId) {
                    root.avatarIdText = nextAvatarId
                }
            }

            LabeledField {
                id: displayNameField
                Layout.fillWidth: true
                visible: root.displayNameEditorVisible
                label: root.createMode ? "Your name" : "How teammates will see you"
                placeholderText: "e.g. Ada Lovelace"
                requiredField: true
                maximumLength: 128
                errorText: text.trim().length > 0
                    ? chaftController.deviceDisplayNameValidationError(text)
                    : ""
            }

            Text {
                Layout.fillWidth: true
                visible: root.displayNameEditorVisible && !root.createMode
                text: "You choose this name. It is not set by the person who invited you."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                visible: root.joinIdentityVisible
                    && root.displayNameReady
                    && !root.displayNameEditorVisible
                spacing: Tokens.space2

                Text {
                    Layout.fillWidth: true
                    text: "Joining as " + displayNameField.text.trim()
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeSm
                    font.weight: Font.Medium
                    elide: Text.ElideRight
                }

                Button {
                    text: "Change"
                    flat: true
                    onClicked: {
                        root.displayNameEditing = true
                        displayNameField.forceFieldFocus()
                    }
                }
            }
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
                            ? "Decryption key kit"
                                : (root.workspaceCard !== null
                                ? "Request link"
                                : "Invite or request link")
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Button {
                        text: root.restoreMode || root.restoreCredentialSelected
                            ? "Open key kit"
                            : "Open invite or access file"
                        enabled: root.app.runtimeAccessReady
                        Accessible.name: text
                        Accessible.description: root.restoreMode
                                || root.restoreCredentialSelected
                            ? "Choose a decryption key kit file"
                            : "Choose an invite, request link, or access file"
                        onClicked: root.app.openWorkspaceCredentialFile()
                    }
                }

                Item {
                    id: credentialsFrame
                    Layout.fillWidth: true
                    visible: !root.credentialTextSummaryVisible
                        || root.credentialSummary.warning
                    Layout.preferredHeight: root.credentialSummaryVisible ? 104 : 132

                    TextArea {
                        id: credentialsArea
                        anchors.fill: parent
                        visible: !root.credentialTextSummaryVisible
                        placeholderText: root.restoreMode
                                || root.restoreCredentialSelected
                            ? "Paste a decryption key kit"
                            : "Paste an invite or request link"
                        Accessible.name: root.restoreMode
                                || root.restoreCredentialSelected
                            ? "Decryption key kit"
                            : "Workspace invite or request link"
                        color: Tokens.textStrong
                        placeholderTextColor: Tokens.textMuted
                        font.family: Tokens.fontMono
                        font.pixelSize: Tokens.fontSizeSm
                        wrapMode: TextEdit.WrapAnywhere
                        onTextChanged: {
                            if (root.visible) {
                                credentialIdentityRebindTimer.restart()
                            }
                        }

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
                                ? "Drop decryption key kit"
                                : "Drop invite, request link, or access file"
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
                        && !root.joinRequestPrepared
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
                    label: "Kit passphrase"
                    placeholderText: root.restoreCredentialSelected
                        ? "Passphrase used when this kit was saved"
                        : "Optional for invites; required for decryption key kits"
                    echoMode: TextInput.Password
                    onAccepted: root.app.submitWorkspaceJoin()
                }

                Text {
                    Layout.fillWidth: true
                    visible: root.restoreCredentialSelected
                    text: "This imports only the saved decryption keys. If this device is not already authorized, it needs an invite before Chaft can show or send workspace content."
                    color: Tokens.warningText
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                    Accessible.role: Accessible.AlertMessage
                    Accessible.name: text
                }

                LabeledField {
                    id: peerEndpointField
                    Layout.fillWidth: true
                    visible: root.peerEndpointInputVisible
                    label: "Teammate address (optional)"
                    placeholderText: "Paste an address from your teammate"
                    onAccepted: root.app.submitWorkspaceJoin()
                }

                Text {
                    Layout.fillWidth: true
                    visible: root.peerEndpointInputVisible
                    text: root.restoreCredentialSelected
                        ? "Matching history must already be on this device or come from a reachable teammate. Use a newer kit if recent content remains locked."
                        : "Invites let you join; Chaft loads history when a teammate is reachable."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: false
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
                            text: "After importing keys"
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Repeater {
                                model: [
                                    {
                                        label: "Keys",
                                        value: "Only keys included when the kit was saved are imported."
                                    },
                                {
                                    label: "History",
                                    value: "Readable when matching encrypted history is available."
                                },
                                {
                                    label: "Membership",
                                    value: "Not restored. An unauthorized device needs an invite before content appears."
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
                            || root.approvalInviteNeedsRequest
                            || root.secureInviteClaim)
                    implicitHeight: joinRequestColumn.implicitHeight + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: root.secureInviteClaim && !root.joinRequestPrepared
                        ? "transparent"
                        : Tokens.surfaceRaised
                    border.width: root.secureInviteClaim && !root.joinRequestPrepared
                        ? 0
                        : 1
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
                                text: "Need an invite?"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                text: "Ask an owner or admin, or open a request link to ask for access."
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
                                visible: !root.secureInviteClaim
                                text: root.secureInviteClaim
                                    ? "Join workspace"
                                    : "Request access"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                visible: !root.secureInviteClaim
                                text: root.secureInviteClaim
                                    ? "Access is encrypted for this device. "
                                        + (root.secureInviteMaxClaims === 1
                                            ? "This invite can add one device."
                                            : "This invite can add up to "
                                                + root.secureInviteMaxClaims
                                                + " devices.")
                                    : (root.workspaceCard !== null || root.approvalInviteNeedsRequest
                                    ? "Send an access request for " + root.joinRequestTargetLabel()
                                        + ". An owner or admin will send back an invite you can open here."
                                    : "Send an access request to a workspace admin. They'll send back an invite you can open here.")
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }

                            Button {
                                Layout.alignment: Qt.AlignLeft
                                text: root.joinRequestNoteExpanded
                                    ? "Hide note"
                                    : "Add a note"
                                flat: true
                                Accessible.description: "Optional context for the workspace admin"
                                onClicked: {
                                    root.joinRequestNoteExpanded = !root.joinRequestNoteExpanded
                                    if (root.joinRequestNoteExpanded) {
                                        joinRequestNoteArea.forceActiveFocus()
                                    }
                                }
                            }

                            TextArea {
                                id: joinRequestNoteArea
                                Layout.fillWidth: true
                                Layout.preferredHeight: 58
                                visible: root.joinRequestNoteExpanded
                                placeholderText: "Team or project (optional)"
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

                            EntryPrimaryButton {
                                Layout.fillWidth: true
                                text: chaftController.joinRequestSubmitInFlight
                                    ? (root.secureInviteClaim ? "Joining..." : "Sending...")
                                    : (root.secureInviteClaim ? "Join workspace" : "Request access")
                                enabled: root.app.runtimeAccessReady
                                    && root.displayNameReady
                                    && !chaftController.keyTransferInFlight
                                    && !chaftController.joinRequestSubmitInFlight
                                onClicked: root.startWorkspaceAccess()
                            }

                            Text {
                                Layout.fillWidth: true
                                visible: !root.joinRequestPrepared
                                    && root.joinRequestDirectSubmitError.length > 0
                                text: root.joinRequestDirectSubmitError
                                color: Tokens.warningText
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
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
                                text: root.secureInviteExpired
                                    ? "Invite expired"
                                    : "Invite required"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                text: root.secureInviteExpired
                                    ? "Ask an owner or admin for a fresh secure invite."
                                    : "This workspace is invite only. Ask an owner or admin for a Chaft invite, then paste or open it here."
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
                                visible: false
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
                                        visible: (root.joinRequestPreparedAction === "send-failed"
                                            || root.joinRequestPreparedAction === "save-failed")
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

                            Menu {
                                id: preparedRequestActionsMenu

                                MenuItem {
                                    text: root.secureInviteClaim
                                        ? "Copy join request"
                                        : "Copy request link"
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    onTriggered: root.copyPreparedJoinRequest()
                                }

                                MenuItem {
                                    text: root.secureInviteClaim
                                        ? "Save join request"
                                        : "Save request file"
                                    enabled: root.app.runtimeAccessReady
                                        && !chaftController.keyTransferInFlight
                                        && !chaftController.joinRequestSubmitInFlight
                                    onTriggered: root.savePreparedJoinRequest()
                                }
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
                        visible: !root.joinRequestPrepared
                            || root.joinRequestCanEditDetails()
                            || root.joinRequestPreparedAction === "send-failed"
                        text: !root.joinRequestPrepared
                            ? "Cancel"
                            : (root.joinRequestPreparedAction === "send-failed"
                                ? "Transfer manually"
                                : "Edit details")
                        enabled: !chaftController.joinRequestSubmitInFlight
                            && !root.credentialImportPending
                        onClicked: {
                            if (!root.joinRequestPrepared) {
                                root.close()
                            } else if (root.joinRequestPreparedAction === "send-failed") {
                                preparedRequestActionsMenu.open()
                            } else {
                                root.editJoinRequest()
                            }
                        }
                    }

                    EntryPrimaryButton {
                        visible: !root.requestAccessContext
                        text: root.credentialImportPending
                            ? (root.keyKitMode
                                ? "Importing keys..."
                                : "Joining...")
                            : (root.joinRequestPrepared
                                ? (root.joinRequestPreparedAction === "sending"
                                    ? (root.secureInviteClaim ? "Joining..." : "Sending...")
                                    : (root.joinRequestPreparedAction === "sent"
                                            || root.joinRequestPreparedAction === "copied"
                                            || root.joinRequestPreparedAction === "save"
                                        ? "Close"
                                        : (root.joinRequestPreparedAction === "save-failed"
                                            ? "Try saving again"
                                            : (root.joinRequestPreparedAction === "send-failed"
                                                ? "Try again"
                                            : (root.joinRequestCanSendDirect()
                                                ? (root.secureInviteClaim
                                                    ? "Join workspace"
                                                    : "Send request")
                                                : "Transfer manually")))))
                                : (root.keyKitMode
                                    ? "Import keys"
                                    : "Join workspace"))
                        enabled: root.joinRequestPrepared
                            ? (root.joinRequestPreparedAction !== "sending"
                                && root.app.runtimeAccessReady
                                && !chaftController.keyTransferInFlight
                                && !chaftController.joinRequestSubmitInFlight)
                            : (root.app.runtimeAccessReady
                                && !root.credentialImportPending
                                && (!root.joinIdentityVisible || root.displayNameReady)
                                && root.app.credentialCanSubmit(
                                    credentialsArea.text,
                                    root.restoreMode,
                                    recoveryPassphraseField.text))
                        onClicked: {
                            if (root.joinRequestPrepared) {
                                if (root.joinRequestPreparedAction === "sent"
                                        || root.joinRequestPreparedAction === "copied"
                                        || root.joinRequestPreparedAction === "save") {
                                    root.finishPreparedJoinRequest()
                                } else if (root.joinRequestPreparedAction === "save-failed") {
                                    root.savePreparedJoinRequest()
                                } else if (root.joinRequestPreparedAction === "send-failed"
                                        || root.joinRequestCanSendDirect()) {
                                    root.sendPreparedJoinRequest()
                                } else {
                                    preparedRequestActionsMenu.open()
                                }
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
                    enabled: !root.createOperationPending
                    onAccepted: root.app.submitWorkspaceCreate()
                }

                LabeledField {
                    id: createChannelField
                    Layout.fillWidth: true
                    label: "First room"
                    placeholderText: "general"
                    enabled: !root.createOperationPending
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
                    enabled: !root.createOperationPending
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
                    text: "You can start chatting now, then invite teammates or save a private decryption key kit."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Text {
                    Layout.fillWidth: true
                    visible: root.createOperationError.length > 0
                    text: root.createOperationError
                    color: Tokens.warningText
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                    Accessible.role: Accessible.AlertMessage
                    Accessible.name: root.createOperationError
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
                        enabled: !root.createOperationPending
                        onClicked: root.close()
                    }

                    EntryPrimaryButton {
                        text: root.createOperationPending
                            ? "Creating..."
                            : "Create workspace"
                        enabled: !root.createOperationPending
                            && root.app.runtimeAccessReady
                            && createNameField.text.trim().length > 0
                            && (!root.createIdentityVisible || root.displayNameReady)
                        onClicked: root.app.submitWorkspaceCreate()
                    }
                }
            }
        }
    }

    onOpened: {
        peerEndpointField.text = chaftController.defaultPeerEndpoint
        if (!AvatarCatalog.isValid(root.avatarIdText)) {
            var savedAvatarId = root.app
                ? root.app.avatarIdForDevice(
                    String(chaftController.deviceId || ""))
                : ""
            if (!AvatarCatalog.isValid(savedAvatarId) && root.app) {
                savedAvatarId = AvatarCatalog.deterministicAvatarId(
                    root.app.currentWorkspaceId(),
                    String(chaftController.deviceId || ""))
            }
            root.avatarIdText = AvatarCatalog.isValid(savedAvatarId)
                ? savedAvatarId
                : AvatarCatalog.shuffledAvatarId(
                    "", root.app ? root.app.usedWorkspaceAvatarIds() : [])
        }
        if (displayNameField.text.trim().length === 0) {
            displayNameField.text = root.app.localDeviceDisplayName()
        }
        root.focusInitialField()
    }

    onCredentialSummaryVisibleChanged: {
        root.synchronizeCredentialMode()
        if (root.credentialSummaryVisible
                && root.joinIdentityVisible
                && !root.displayNameReady) {
            root.displayNameEditing = true
            Qt.callLater(function() {
                if (root.visible && root.displayNameEditorVisible) {
                    displayNameField.forceFieldFocus()
                }
            })
        }
    }
    onCredentialIsRecoveryBundleChanged: root.synchronizeCredentialMode()

    onClosed: root.resetForm()
}
