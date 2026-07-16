import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Chaft

ScrollView {
    id: setupScroll

    // The App root window. Set by App.qml at the single instantiation site.
    property var app
    property string category: "profile"
    signal categoryRequested(string categoryId)

    readonly property var darkThemes: Themes.catalog.filter(function (theme) {
        return theme.dark === true
    })
    readonly property var lightThemes: Themes.catalog.filter(function (theme) {
        return theme.dark !== true
    })
    readonly property var inviteExpiryDayOptions: [1, 7, 30, 0]
    readonly property var primaryWorkspaceAccessPolicyOptions: [
        { label: "Invite only", value: "invite_only" },
        { label: "People can request access", value: "request_access" }
    ]
    property string loadedJoinRequestText: ""
    property string accessRequestImportError: ""
    property string pendingJoinRequestFileText: ""
    property string pendingReplacementInviteId: ""
    property string pendingReplacementInviteDeviceId: ""
    property string pendingReplacementInviteRole: "member"
    property string pendingReplacementInviteDisplayName: ""
    property string pendingReplacementInviteRequestId: ""
    property string pendingReplacementInviteApprovalPolicy: "preapproved"
    property int pendingReplacementInviteMaxClaims: 1
    property bool pendingReplacementInviteClaimable: false
    property bool pendingReplacementInviteWaitingForClose: false
    property bool updatingInviteDeviceField: false
    property bool approvalInviteModeEnabled: false
    property string peopleAccessTab: "members"
    property bool inviteComposerVisible: false
    property bool inviteReadyDismissed: false
    property bool secureAccessReadyDismissed: false
    property bool manualInviteClaimPending: false
    property string manualInviteClaimError: ""
    property bool secureResponseDiscardPending: false
    property var reviewAccessRequest: ({})
    property bool showRequestHistory: false
    property bool showInviteHistory: false
    property bool advancedAccessToolsOpen: false
    property bool addAnotherDeviceGuideVisible: false
    property bool addAnotherDeviceOpenSaveWhenReady: false
    property string autoLoadedKeyTransferText: ""
    property string profileSaveError: ""
    property string profileDraftWorkspaceId: ""
    readonly property string standardAccessUpdateProtocol: "openmls/key-package"

    onAdvancedAccessToolsOpenChanged: {
        if (!setupScroll.advancedAccessToolsOpen) {
            setupScroll.clearRecoveryPassphraseDrafts()
        }
    }

    onAddAnotherDeviceGuideVisibleChanged: {
        if (!setupScroll.addAnotherDeviceGuideVisible) {
            addDeviceRecoveryPassphraseField.text = ""
            addDeviceRecoveryPassphraseConfirmationField.text = ""
        }
    }

    onCategoryChanged: Qt.callLater(function() {
        if (setupScroll.contentItem !== null) {
            setupScroll.contentItem.contentY = 0
        }
    })

    function requestCategory(categoryId) {
        if (setupScroll.category !== categoryId) {
            setupScroll.categoryRequested(categoryId)
        }
    }

    function startAddAnotherDeviceFlow() {
        if (!chaftController.hasRuntimeWorkspace) {
            return false
        }
        setupScroll.requestCategory("devices")
        setupScroll.addAnotherDeviceGuideVisible = true
        peopleAccessSection.expanded = false
        advancedAccessSection.expanded = true
        Qt.callLater(function() {
            if (setupScroll.contentItem !== null) {
                setupScroll.contentItem.contentY = Math.max(
                    0, advancedAccessSection.y + addAnotherDeviceRecoveryPanel.y
                        - Tokens.space2)
            }
            addDeviceRecoveryPassphraseField.forceFieldFocus()
        })
        return true
    }

    function createAddAnotherDeviceRecoveryKit() {
        if (!setupScroll.app || !setupScroll.app.runtimeWorkReady
                || setupScroll.hasUnreturnedInviteResponse()
                || chaftController.keyTransferInFlight) {
            return false
        }
        if (chaftController.keyTransferJson.length > 0
                && setupScroll.keyTransferIsRecoveryBundle()) {
            setupScroll.app.openSaveKeyTransferDialog("recovery kit")
            return true
        }
        if (addDeviceRecoveryPassphraseField.text.trim().length === 0) {
            addDeviceRecoveryPassphraseField.forceFieldFocus()
            return false
        }
        if (addDeviceRecoveryPassphraseConfirmationField.text
                !== addDeviceRecoveryPassphraseField.text) {
            addDeviceRecoveryPassphraseConfirmationField.forceFieldFocus()
            return false
        }
        setupScroll.addAnotherDeviceOpenSaveWhenReady =
            chaftController.exportRecoveryBundle(addDeviceRecoveryPassphraseField.text)
        if (setupScroll.addAnotherDeviceOpenSaveWhenReady) {
            addDeviceRecoveryPassphraseField.text = ""
            addDeviceRecoveryPassphraseConfirmationField.text = ""
        }
        return setupScroll.addAnotherDeviceOpenSaveWhenReady
    }

    function clearRecoveryPassphraseDrafts() {
        recoveryPassphraseField.text = ""
        recoveryPassphraseConfirmationField.text = ""
        addDeviceRecoveryPassphraseField.text = ""
        addDeviceRecoveryPassphraseConfirmationField.text = ""
    }

    function localPersonDeviceLink() {
        if (!setupScroll.app) {
            return ({})
        }
        var rows = setupScroll.app.workspaceSnapshot.personDeviceLinks || []
        var localDeviceId = String(chaftController.deviceId || "").trim()
        for (var i = 0; i < rows.length; i += 1) {
            var row = rows[i] || ({})
            if (String(row.deviceId || "").trim() === localDeviceId) {
                return row
            }
        }
        return ({})
    }

    function localProfileName() {
        var link = setupScroll.localPersonDeviceLink()
        var personName = String(link.personDisplayName || "").trim()
        if (personName.length > 0) {
            return personName
        }
        return setupScroll.app ? setupScroll.app.localDeviceDisplayName().trim() : ""
    }

    function localProfileStatusText() {
        var name = setupScroll.localProfileName()
        if (name.length > 0) {
            return name + " is saved for this device."
        }
        return "Set your name so teammates can recognize you."
    }

    function syncDisplayNameDraft(force) {
        if (!setupScroll.app) {
            return false
        }
        var workspaceId = setupScroll.app.currentWorkspaceId()
        var workspaceChanged = workspaceId !== setupScroll.profileDraftWorkspaceId
        if (!force && !workspaceChanged && displayNameField.dirty) {
            return false
        }
        displayNameField.text = setupScroll.app.localDeviceDisplayName()
        displayNameField.markClean()
        setupScroll.profileDraftWorkspaceId = workspaceId
        setupScroll.profileSaveError = ""
        return true
    }

    function saveDisplayNameDraft() {
        if (!setupScroll.app) {
            setupScroll.profileSaveError = "Workspace profile is unavailable."
            return false
        }
        var workspaceId = setupScroll.app.currentWorkspaceId()
        if (workspaceId !== setupScroll.profileDraftWorkspaceId) {
            setupScroll.syncDisplayNameDraft(true)
            setupScroll.profileSaveError =
                "The workspace changed. Review the name for this workspace before saving."
            return false
        }
        var displayName = displayNameField.text.trim()
        if (displayName.length === 0) {
            setupScroll.profileSaveError = "Enter the name teammates should see."
            displayNameField.forceFieldFocus()
            return false
        }
        if (!setupScroll.app.runtimeWorkReady) {
            setupScroll.profileSaveError = "Unlock this workspace before saving your name."
            return false
        }
        if (!chaftController.updateDeviceProfile(displayName)) {
            setupScroll.profileSaveError = String(chaftController.syncStatus
                || "Could not save your name. Try again.")
            return false
        }
        displayNameField.text = displayName
        displayNameField.markClean()
        setupScroll.profileDraftWorkspaceId = workspaceId
        setupScroll.profileSaveError = ""
        return true
    }

    function localDeviceLinkStatusText() {
        var link = setupScroll.localPersonDeviceLink()
        if (String(link.personId || "").trim().length > 0) {
            return "This device has a saved profile. Other devices still show separately until Chaft can safely link them."
        }
        return "Other devices show separately until Chaft can safely link them."
    }

    function workspaceAccessPolicyIndex(policy) {
        var options = setupScroll.workspaceAccessPolicyOptionsForPolicy(policy)
        var normalized = setupScroll.app
            ? setupScroll.app.normalizedWorkspaceAccessPolicy(policy)
            : "invite_only"
        for (var i = 0; i < options.length; i += 1) {
            if (options[i].value === normalized) {
                return i
            }
        }
        return 0
    }

    function workspaceAccessPolicyOptionsForPolicy(policy) {
        var options = setupScroll.primaryWorkspaceAccessPolicyOptions.slice()
        var normalized = setupScroll.app
            ? setupScroll.app.normalizedWorkspaceAccessPolicy(policy)
            : "invite_only"
        if (normalized === "discoverable") {
            options.push({
                label: "Discoverable not live",
                value: "discoverable"
            })
        }
        return options
    }

    function updateWorkspaceAccessPolicy(policy) {
        if (!setupScroll.app || !setupScroll.app.canManageWorkspaceAccess()) {
            return false
        }
        var normalized = setupScroll.app.normalizedWorkspaceAccessPolicy(policy)
        if (normalized === setupScroll.app.accessPolicy) {
            return true
        }
        return chaftController.updateWorkspaceAccessPolicy(normalized)
    }

    function workspaceAccessPolicy() {
        return setupScroll.app
            ? setupScroll.app.normalizedWorkspaceAccessPolicy(setupScroll.app.accessPolicy)
            : "invite_only"
    }

    function workspacePolicyEncouragesRequests() {
        var policy = setupScroll.workspaceAccessPolicy()
        return policy === "request_access" || policy === "discoverable"
    }

    function canShareWorkspaceCard() {
        return setupScroll.app
            && setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && setupScroll.workspacePolicyEncouragesRequests()
            && !setupScroll.hasUnreturnedInviteResponse()
            && !chaftController.keyTransferInFlight
    }

    function workspaceCardUnavailableReason() {
        if (!setupScroll.app) {
            return "Workspace access unavailable."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before sharing a request link."
        }
        if (!setupScroll.workspacePolicyEncouragesRequests()) {
            return "Set Who can join to People can request access before sharing a request link."
        }
        if (setupScroll.hasUnreturnedInviteResponse()) {
            return setupScroll.unreturnedInviteResponseReason()
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before sharing a request link."
        }
        return "Request link unavailable."
    }

    function copyRequestOnlyInvite() {
        if (!setupScroll.canShareWorkspaceCard()) {
            return false
        }
        if (chaftController.stageWorkspaceAccessCard()) {
            return setupScroll.app.copyKeyTransferArtifact("request link")
        }
        return false
    }

    function saveRequestOnlyInvite() {
        if (!setupScroll.canShareWorkspaceCard()) {
            return false
        }
        if (chaftController.stageWorkspaceAccessCard()) {
            return setupScroll.app.openSaveKeyTransferDialog("request card")
        }
        return false
    }

    function peopleAccessPolicyHintText() {
        switch (setupScroll.workspaceAccessPolicy()) {
        case "request_access":
            return "People can ask to join. Have them send an access request, then approve it here."
        case "discoverable":
            return "Discovery is not live yet. Use access requests or direct invites."
        case "invite_only":
        default:
            return "Invite only is on. Create an invite and choose how many devices can use it."
        }
    }

    function inviteDeviceFieldLabel() {
        return "Invite label or access request"
    }

    function inviteDeviceFieldPlaceholder() {
        return "Optional name, or paste an access request"
    }

    function createInviteActionLabel() {
        if (setupScroll.loadedJoinRequestObject() !== null) {
            return "Approve request"
        }
        return "Create secure invite"
    }

    function inviteRoleOptions() {
        var options = [
            { label: "Member", role: "member" },
            { label: "Guest", role: "guest" }
        ]
        if (setupScroll.app && setupScroll.app.canManageWorkspaceOwners()) {
            options.splice(1, 0, { label: "Admin", role: "admin" })
        }
        return options
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

    function keyTransferObject() {
        var json = String(chaftController.keyTransferJson || "").trim()
        if (json.length === 0) {
            return null
        }
        try {
            return JSON.parse(json)
        } catch (error) {
            return null
        }
    }

    function keyTransferIsInvitePackage() {
        var parsed = setupScroll.keyTransferObject()
        return parsed !== null
            && (String(parsed.kind || "") === "chaft.workspace-invite.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite.v2")
    }

    function invitePackageObject() {
        return setupScroll.keyTransferIsInvitePackage()
            ? setupScroll.keyTransferObject()
            : null
    }

    function invitePackageIsClaimable() {
        var invite = setupScroll.invitePackageObject()
        return invite !== null
            && String(invite.kind || "") === "chaft.workspace-invite.v2"
    }

    function workspaceInviteUsesClaims(invite) {
        return invite !== null && invite !== undefined
            && (String(invite.kind || "") === "chaft.workspace-invite.v2"
                || Boolean(invite.claimable)
                || String(invite.capabilityPublicKey || "").trim().length > 0
                || Number(invite.maxClaims || 1) > 1)
    }

    function workspaceInviteMaxClaims(invite) {
        return setupScroll.app
            ? setupScroll.app.inviteMaxClaims(invite)
            : 1
    }

    function workspaceInviteClaimAvailabilityLabel(invite) {
        if (!setupScroll.app) {
            return "1 device remaining"
        }
        var maximum = setupScroll.workspaceInviteMaxClaims(invite)
        if (setupScroll.workspaceInviteStatus(invite) === "invited") {
            var remaining = setupScroll.app.inviteRemainingClaims(invite)
            if (remaining === 0) {
                return "No uses remaining"
            }
            return maximum === 1
                ? "1 device remaining"
                : remaining + " of " + maximum + " devices remaining"
        }
        var used = setupScroll.app.inviteClaimCount(invite)
        if (used === 0) {
            return "Not used"
        }
        return maximum === 1
            ? "Used by 1 device"
            : "Used by " + used + " of " + maximum + " devices"
    }

    function invitePackageClaimLimitLabel() {
        var invite = setupScroll.invitePackageObject()
        var maximum = setupScroll.workspaceInviteMaxClaims(invite)
        return maximum === 1 ? "1 device" : maximum + " devices"
    }

    function invitePackageId() {
        var invite = setupScroll.invitePackageObject()
        return invite === null ? "" : String(invite.inviteId || "").trim()
    }

    function invitePackageRequestId() {
        var invite = setupScroll.invitePackageObject()
        return invite === null ? "" : String(invite.requestId || "").trim()
    }

    function invitePackageRecipientLabel() {
        var invite = setupScroll.invitePackageObject()
        if (invite === null) {
            return "your teammate"
        }
        if (String(invite.kind || "") === "chaft.workspace-invite.v2") {
            return "the people you share it with"
        }
        var displayName = String(invite.inviteeDisplayName || "").trim()
        if (displayName.length === 0) {
            displayName = String(invite.displayName || "").trim()
        }
        if (displayName.length > 0) {
            return displayName
        }
        var deviceId = String(invite.inviteeDeviceId || "").trim()
        return deviceId.length > 0
            ? setupScroll.app.unnamedPersonLabel(deviceId)
            : "your teammate"
    }

    function invitePackageRoleLabel() {
        var invite = setupScroll.invitePackageObject()
        return setupScroll.app.roleLabel(invite ? invite.role : "member")
    }

    function inviteExpiryDays() {
        var index = inviteExpiryBox.currentIndex
        return index >= 0 && index < setupScroll.inviteExpiryDayOptions.length
            ? setupScroll.inviteExpiryDayOptions[index]
            : 7
    }

    function inviteExpiryChoiceLabel() {
        var days = setupScroll.inviteExpiryDays()
        if (days <= 0) {
            return "never expires"
        }
        return days === 1 ? "expires in 1 day" : "expires in " + days + " days"
    }

    function invitePackageExpiryLabel() {
        var invite = setupScroll.invitePackageObject()
        return setupScroll.app.inviteExpiryLabel(invite ? invite.expiresAt : "")
    }

    function invitePackageApprovalLabel() {
        var invite = setupScroll.invitePackageObject()
        if (setupScroll.invitePackageIsClaimable()) {
            return setupScroll.workspaceInviteMaxClaims(invite) === 1
                ? "Approval required"
                : "Approval required per device"
        }
        return setupScroll.app.inviteApprovalLabel(invite ? invite.approvalPolicy : "preapproved")
    }

    function invitePackageRequiresApproval() {
        var invite = setupScroll.invitePackageObject()
        return invite !== null && setupScroll.app.inviteApprovalBlocksJoin(invite.approvalPolicy)
    }

    function invitePackageReadySummaryText() {
        var invite = setupScroll.invitePackageObject()
        if (invite !== null
                && String(invite.kind || "") === "chaft.workspace-invite.v2") {
            var maximum = setupScroll.workspaceInviteMaxClaims(invite)
            if (maximum === 1) {
                return "Send this invite privately. One device can use it to join with "
                    + setupScroll.invitePackageRoleLabel() + " access."
            }
            return "Share this invite privately. Up to " + maximum
                + " devices can use it to join with "
                + setupScroll.invitePackageRoleLabel() + " access."
        }
        if (setupScroll.invitePackageRequiresApproval()) {
            return "Send this approval invite to "
                + setupScroll.invitePackageRecipientLabel()
                + ". They'll request " + setupScroll.invitePackageRoleLabel()
                + " access before messages load."
        }
        return "Send this invite to " + setupScroll.invitePackageRecipientLabel()
            + ". They'll join as " + setupScroll.invitePackageRoleLabel() + "."
    }

    function inviteAccessModeText() {
        if (setupScroll.loadedJoinRequestObject() !== null) {
            return "Approves this request and creates an invite only this person should receive."
        }
        if (setupScroll.approvalInviteMode()) {
            return "Records the invite but shares no message access. They send a request before messages load."
        }
        if (setupScroll.workspacePolicyEncouragesRequests()) {
            return "For people you already know. They can join as soon as they import it."
        }
        return "They can join as soon as they import it."
    }

    function invitePackagePeerEndpoint() {
        var invite = setupScroll.invitePackageObject()
        return invite === null ? "" : String(invite.peerEndpoint || "").trim()
    }

    function invitePackageHasSyncSource() {
        return setupScroll.invitePackagePeerEndpoint().length > 0
    }

    function invitePackageSyncExpectationLabel() {
        var invite = setupScroll.invitePackageObject()
        return setupScroll.app
            ? setupScroll.app.inviteSyncExpectationLabel(invite, "")
            : ""
    }

    function invitePackageSyncExpectationMessage() {
        var invite = setupScroll.invitePackageObject()
        if (!setupScroll.app) {
            return ""
        }
        if (setupScroll.invitePackageRequiresApproval()) {
            return "They will send a request before messages load. Approve it from Access Requests."
        }
        if (setupScroll.invitePackageHasSyncSource()) {
            return setupScroll.app.inviteSyncExpectationMessage(invite, "")
                + " Keep Chaft open until they finish joining."
        }
        return chaftController.peerHosting
            ? "History can be shared now. Add it to the invite before sharing."
            : setupScroll.app.inviteSyncExpectationMessage(invite, "")
    }

    function keyTransferIsJoinRequest() {
        var parsed = setupScroll.keyTransferObject()
        return parsed !== null
            && (String(parsed.kind || "") === "chaft.workspace-join-request.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite-claim.v1")
            && parsed.deviceId !== undefined
    }

    function keyTransferIsInviteResponse() {
        var parsed = setupScroll.keyTransferObject()
        return parsed !== null
            && String(parsed.kind || "") === "chaft.workspace-invite-response.v1"
    }

    function hasUnreturnedInviteResponse() {
        return setupScroll.keyTransferIsInviteResponse()
    }

    function unreturnedInviteResponseReason() {
        return "Return or save the encrypted access response before starting another access handoff."
    }

    function discardSecureInviteResponse() {
        if (!setupScroll.hasUnreturnedInviteResponse()) {
            return false
        }
        if (chaftController.keyTransferFromJoinResponseInbox) {
            if (setupScroll.secureResponseDiscardPending) {
                return false
            }
            if (!chaftController.acknowledgeCurrentJoinResponseInboxEntry()) {
                setupScroll.manualInviteClaimError =
                    "Could not clear the received response. Try again."
                return false
            }
            setupScroll.manualInviteClaimError = ""
            setupScroll.secureResponseDiscardPending = true
            return true
        }
        setupScroll.manualInviteClaimError = ""
        setupScroll.secureAccessReadyDismissed = true
        chaftController.clearKeyTransferJson()
        return true
    }

    function confirmDiscardSecureInviteResponse() {
        if (!setupScroll.hasUnreturnedInviteResponse()) {
            return false
        }
        setupScroll.app.confirmSetupAction(
            "Clear encrypted access",
            "Clear this encrypted access response? Do this only after you copied, saved, or returned it. Otherwise the recipient cannot finish joining.",
            "Clear response",
            "discard-secure-invite-response")
        return true
    }

    function keyTransferIsWorkspaceCard() {
        var parsed = setupScroll.keyTransferObject()
        return parsed !== null
            && String(parsed.kind || "") === "chaft.workspace-card.v1"
            && parsed.workspaceId !== undefined
    }

    function keyTransferIsWorkspaceKey() {
        var parsed = setupScroll.keyTransferObject()
        return parsed !== null
            && (parsed.aes256GcmSivKey !== undefined
                || parsed.aes_256_gcm_siv_key !== undefined)
    }

    function keyTransferIsAccessFile() {
        var parsed = setupScroll.keyTransferObject()
        return setupScroll.keyTransferIsWorkspaceKey()
            || (parsed !== null
                && parsed.channelId !== undefined
                && (parsed.aes256GcmSivKey !== undefined
                    || parsed.aes_256_gcm_siv_key !== undefined))
    }

    function keyTransferIsRecoveryBundle() {
        var parsed = setupScroll.keyTransferObject()
        return parsed !== null
            && parsed.schemaVersion !== undefined
            && parsed.workspaceId !== undefined
            && parsed.exporterDeviceId !== undefined
            && parsed.kdf !== undefined
            && parsed.sealedPayload !== undefined
    }

    function keyTransferCopyLabel() {
        if (setupScroll.keyTransferIsInvitePackage()) {
            return "invite link"
        }
        if (setupScroll.keyTransferIsInviteResponse()) {
            return "secure access"
        }
        if (setupScroll.keyTransferIsJoinRequest()) {
            return setupScroll.joinRequestIsSecureClaim(
                setupScroll.keyTransferObject())
                ? "join request"
                : "access request"
        }
        if (setupScroll.keyTransferIsWorkspaceCard()) {
            return "request link"
        }
        if (setupScroll.keyTransferIsRecoveryBundle()) {
            return "recovery kit"
        }
        if (setupScroll.keyTransferIsAccessFile()) {
            return "access file"
        }
        return "support detail"
    }

    function joinRequestObjectFromText(text) {
        var value = String(text || "").trim()
        if (value.length === 0) {
            return null
        }
        var parsed = setupScroll.app.parsedCredentialObject(value)
        return parsed !== null
            && (String(parsed.kind || "") === "chaft.workspace-join-request.v1"
                || String(parsed.kind || "") === "chaft.workspace-invite-claim.v1")
            ? parsed
            : null
    }

    function joinRequestIsSecureClaim(request) {
        return request !== null
            && String(request.kind || "") === "chaft.workspace-invite-claim.v1"
    }

    function loadedJoinRequestIsSecureClaim() {
        return setupScroll.joinRequestIsSecureClaim(
            setupScroll.loadedJoinRequestObject())
    }

    function secureClaimTargetsThisDevice(request) {
        if (!setupScroll.joinRequestIsSecureClaim(request)) {
            return true
        }
        var deliveryDeviceId = String(
            (request && request.deliveryDeviceId) || "").trim()
        return deliveryDeviceId.length > 0
            && deliveryDeviceId === String(chaftController.deviceId || "").trim()
    }

    function secureClaimCanBeAccepted(request) {
        return !setupScroll.joinRequestIsSecureClaim(request)
            || (setupScroll.secureClaimTargetsThisDevice(request)
                && !setupScroll.secureClaimIsKnownExpired(request))
    }

    function secureClaimInvite(request) {
        if (!setupScroll.joinRequestIsSecureClaim(request)) {
            return null
        }
        var inviteId = String(
            (request && (request.inviteId || request.sourceInviteId)) || "").trim()
        var invite = setupScroll.workspaceInviteById(inviteId)
        if (invite !== null) {
            return invite
        }
        var persisted = chaftController.workspaceInviteArtifactSummary(inviteId)
        return String((persisted && persisted.inviteId) || "").trim().length > 0
            ? persisted
            : null
    }

    function secureClaimAccessLabel(request) {
        var invite = setupScroll.secureClaimInvite(request)
        return invite === null
            ? ""
            : setupScroll.workspaceInviteAccessLabel(invite)
    }

    function secureClaimExpiryLabel(request) {
        var invite = setupScroll.secureClaimInvite(request)
        return invite === null
            ? ""
            : setupScroll.workspaceInviteExpirySummary(invite)
    }

    function secureClaimIsKnownExpired(request) {
        var invite = setupScroll.secureClaimInvite(request)
        return invite !== null
            && setupScroll.app
            && setupScroll.app.inviteExpired(invite.expiresAt)
    }

    function secureClaimIssuerLabel(request) {
        if (!setupScroll.joinRequestIsSecureClaim(request)
                || !setupScroll.secureClaimTargetsThisDevice(request)) {
            return ""
        }
        var name = setupScroll.app
            ? setupScroll.app.localDeviceDisplayName().trim()
            : ""
        if (name.length > 0) {
            return name
        }
        var deviceId = String(chaftController.deviceId || "").trim()
        return deviceId.length > 0 && setupScroll.app
            ? setupScroll.app.unnamedPersonLabel(deviceId)
            : "this device"
    }

    function loadedJoinRequestObject() {
        var loaded = setupScroll.joinRequestObjectFromText(setupScroll.loadedJoinRequestText)
        return loaded !== null
            ? loaded
            : setupScroll.joinRequestObjectFromText(inviteDeviceField.text)
    }

    function loadedJoinRequestLabel() {
        var request = setupScroll.loadedJoinRequestObject()
        if (request === null) {
            return "Unknown person"
        }
        var displayName = String(request.displayName || "").trim()
        if (displayName.length > 0) {
            return displayName
        }
        var deviceId = String(request.deviceId || "").trim()
        return deviceId.length > 0
            ? setupScroll.app.unnamedPersonLabel(deviceId)
            : "Unknown person"
    }

    function loadedJoinRequestDeviceLabel() {
        var request = setupScroll.loadedJoinRequestObject()
        if (request === null) {
            return ""
        }
        var deviceId = String(request.deviceId || "").trim()
        return deviceId.length > 0
            ? "Support code " + setupScroll.app.shortDeviceId(deviceId)
            : ""
    }

    function loadedJoinRequestNote() {
        var request = setupScroll.loadedJoinRequestObject()
        return request === null ? "" : String(request.note || "").trim()
    }

    function loadedJoinRequestId() {
        var request = setupScroll.loadedJoinRequestObject()
        return request === null ? "" : setupScroll.joinRequestId(request)
    }

    function joinRequestId(request) {
        return String((request && (request.requestId || request.request_id)) || "").trim()
    }

    function joinRequestDeviceId(request) {
        return String((request && (request.deviceId || request.requesterDeviceId)) || "").trim()
    }

    function shortDeviceCode(deviceId) {
        var value = String(deviceId || "").trim()
        if (value.length <= 16) {
            return value
        }
        return value.slice(0, 8) + "..." + value.slice(value.length - 6)
    }

    function joinRequestDisplayName(request) {
        return String((request && (request.displayName || request.requesterDisplayName)) || "").trim()
    }

    function joinRequestNoteText(request) {
        return String((request && request.note) || "").trim()
    }

    function joinRequestCompactContextLabel(request) {
        var sourceType = String((request && request.sourceType) || "").trim()
        if (sourceType === "invite_claim") {
            return "Invite join request"
        }
        if (sourceType === "approval_invite") {
            return "Approval invite"
        }
        if (sourceType === "workspace_card") {
            return "Request link"
        }
        var workspace = setupScroll.joinRequestWorkspaceLabel(request)
        return workspace.length > 0 ? "Requesting " + workspace : "Requesting access"
    }

    function joinRequestNoteSummary(request) {
        var note = setupScroll.joinRequestNoteText(request)
        return note.length > 0 ? "Note: " + note : ""
    }

    function joinRequestWorkspaceId(request) {
        return String((request && request.workspaceId) || "").trim()
    }

    function joinRequestWorkspaceName(request) {
        return String((request && request.workspaceName) || "").trim()
    }

    function joinRequestWorkspaceLabel(request) {
        var name = setupScroll.joinRequestWorkspaceName(request)
        if (name.length > 0) {
            return name
        }
        var workspaceId = setupScroll.joinRequestWorkspaceId(request)
        return workspaceId.length > 0 && setupScroll.app
            ? setupScroll.app.shortAccessIdentifier(workspaceId)
            : ""
    }

    function joinRequestSourceLabel(request) {
        return setupScroll.app
            ? setupScroll.app.joinRequestSourceLabel(request)
            : ""
    }

    function joinRequestTargetsCurrentWorkspace(request) {
        var workspaceId = setupScroll.joinRequestWorkspaceId(request)
        return workspaceId.length === 0
            || (setupScroll.app && workspaceId === setupScroll.app.currentWorkspaceId())
    }

    function joinRequestLabel(request) {
        var name = setupScroll.joinRequestDisplayName(request)
        if (name.length > 0) {
            return name
        }
        var deviceId = setupScroll.joinRequestDeviceId(request)
        return deviceId.length > 0 ? setupScroll.shortDeviceCode(deviceId) : "Teammate"
    }

    function joinRequestStatusLabel(status) {
        var normalized = String(status || "").trim().toLowerCase()
        if (normalized === "approved") {
            return "Approved"
        }
        if (normalized === "declined") {
            return "Declined"
        }
        if (normalized === "revoked") {
            return "Revoked"
        }
        return "Waiting"
    }

    function joinRequestResolvedByLabel(request) {
        var displayName = String((request && request.resolvedByDisplayName) || "").trim()
        if (displayName.length > 0) {
            return displayName
        }
        var deviceId = String((request && request.resolvedByDeviceId) || "").trim()
        if (deviceId.length > 0) {
            return setupScroll.app.unnamedPersonLabel(deviceId)
        }
        return "an admin"
    }

    function joinRequestStatusDetail(request) {
        var normalized = String((request && request.status) || "").trim().toLowerCase()
        if (normalized === "approved") {
            if (setupScroll.canShareCurrentInviteForJoinRequest(request)) {
                return "Approved by " + setupScroll.joinRequestResolvedByLabel(request)
                    + ". The invite is ready to send from this row."
            }
            if (setupScroll.hasLostInviteForJoinRequest(request)) {
                return "Approved by " + setupScroll.joinRequestResolvedByLabel(request)
                    + ". The original invite link or file is not available here; replace it to create a fresh one."
            }
            if (setupScroll.canCreateReplacementInviteForJoinRequest(request)) {
                return "Approved by " + setupScroll.joinRequestResolvedByLabel(request)
                    + ". The previous invite is closed; create a new invite from this row."
            }
            return "Approved by " + setupScroll.joinRequestResolvedByLabel(request)
                + ". No invite link or file is ready here. Use the matching invite row to replace it or create a new one."
        }
        if (normalized === "declined") {
            return "Declined by " + setupScroll.joinRequestResolvedByLabel(request)
                + ". They will need a new request before joining."
        }
        if (normalized === "revoked") {
            return "Revoked by " + setupScroll.joinRequestResolvedByLabel(request)
                + ". This request is closed."
        }
        return "Waiting for review."
    }

    function joinRequestNextActionText(request) {
        var normalized = String((request && request.status) || "").trim().toLowerCase()
        var label = setupScroll.joinRequestLabel(request)
        if (normalized === "approved") {
            if (setupScroll.canShareCurrentInviteForJoinRequest(request)) {
                return "Send the invite link or file to " + label + "."
            }
            if (setupScroll.hasLostInviteForJoinRequest(request)) {
                return "Replace the missing invite, then send the new link or file to " + label + "."
            }
            if (setupScroll.canCreateReplacementInviteForJoinRequest(request)) {
                return "Create a new invite, then send it to " + label + "."
            }
            return "Use the matching invite row to replace it or create a new one."
        }
        if (normalized === "waiting" && setupScroll.app.canManageWorkspaceAccess()) {
            return "Approve to create an invite, decline if they should not join, or close a stale duplicate."
        }
        if (normalized === "declined") {
            return "Wait for a new request before inviting this person."
        }
        if (normalized === "revoked") {
            return "No action unless they send a fresh request."
        }
        return ""
    }

    function joinRequestReceiptLabel(request) {
        var normalized = String((request && request.status) || "").trim().toLowerCase()
        var requestedAt = setupScroll.app
            ? setupScroll.app.physicalMsTimeLabel((request && request.requestedPhysicalMs) || 0)
            : ""
        var resolvedAt = setupScroll.app
            ? setupScroll.app.physicalMsTimeLabel((request && request.resolvedPhysicalMs) || 0)
            : ""
        if (normalized === "waiting" && requestedAt.length > 0) {
            return "Request received " + requestedAt
        }
        if (resolvedAt.length > 0) {
            return "Request reviewed " + resolvedAt
        }
        if (requestedAt.length > 0) {
            return "Request received " + requestedAt
        }
        return ""
    }

    function workspaceInviteLabel(invite) {
        if (setupScroll.workspaceInviteUsesClaims(invite)) {
            var inviteLabel = setupScroll.workspaceInviteTrackingLabel(invite)
            if (inviteLabel.length > 0) {
                return inviteLabel
            }
            var maximum = setupScroll.workspaceInviteMaxClaims(invite)
            return maximum === 1
                ? "Single-device invite"
                : maximum + "-device invite"
        }
        var name = String((invite
            && (invite.inviteeDisplayName || invite.displayName)) || "").trim()
        if (name.length > 0) {
            return name
        }
        var deviceId = String((invite && invite.inviteeDeviceId) || "").trim()
        return deviceId.length > 0
            ? setupScroll.app.unnamedPersonLabel(deviceId)
            : "Teammate"
    }

    function workspaceInviteTrackingLabel(invite) {
        if (!setupScroll.workspaceInviteUsesClaims(invite)) {
            return ""
        }
        // Older v2 snapshots exposed the tracking label as displayName. Keep
        // that compatibility path without treating it as recipient identity.
        return String((invite && (invite.inviteLabel || invite.displayName))
            || "").trim()
    }

    function workspaceInviteAccessLabel(invite) {
        var label = setupScroll.app.roleLabel(
            invite && invite.role ? invite.role : "member") + " access"
        if (setupScroll.workspaceInviteUsesClaims(invite)) {
            label += " · " + setupScroll.workspaceInviteClaimAvailabilityLabel(invite)
        }
        return label
    }

    function workspaceInviteExpirySummary(invite) {
        var label = setupScroll.app.inviteExpiryLabel(invite ? invite.expiresAt : "")
        return label.length > 0
            ? label.charAt(0).toUpperCase() + label.slice(1)
            : ""
    }

    function workspaceInviteStatus(invite) {
        var normalized = String((invite && invite.status) || "").trim().toLowerCase()
        if (normalized === "invited" && setupScroll.app.inviteExpired(invite.expiresAt)) {
            return "expired"
        }
        return normalized
    }

    function workspaceInviteRequiresApproval(invite) {
        return String((invite && invite.approvalPolicy) || "preapproved").trim()
            === "admin_required"
    }

    function workspaceInviteStatusLabel(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        if (status === "accepted") {
            if (setupScroll.workspaceInviteUsesClaims(invite)) {
                return setupScroll.workspaceInviteMaxClaims(invite) === 1
                    ? "Used"
                    : "Fully used"
            }
            return "Accepted"
        }
        if (status === "expired") {
            return "Expired"
        }
        if (status === "revoked") {
            return "Revoked"
        }
        if (setupScroll.workspaceInviteRequiresApproval(invite)) {
            return "Approval first"
        }
        return "Invited"
    }

    function workspaceInviteStatusDetail(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        if (status === "accepted") {
            if (setupScroll.workspaceInviteUsesClaims(invite)) {
                var maximum = setupScroll.workspaceInviteMaxClaims(invite)
                return maximum === 1
                    ? "This invite has been used and cannot be used again."
                    : "All " + maximum
                        + " uses are complete. This invite cannot be used again."
            }
            return "They have opened Chaft and received updates at least once."
        }
        if (status === "expired") {
            return "This invite expired. Create a new invite if they still need access."
        }
        if (status === "revoked") {
            return "This invite was revoked. Create a new invite if they still need access."
        }
        if (setupScroll.canShareCurrentWorkspaceInvite(invite)) {
            if (setupScroll.workspaceInviteRequiresApproval(invite)) {
                return "This approval invite is ready to send. They will send a request before messages load."
            }
            return "This invite is ready to send. Copy the link or save the file from this row."
        }
        if (status === "invited") {
            if (setupScroll.workspaceInviteRequiresApproval(invite)) {
                return "This approval invite is still open, but the original link or file is not available here. Replace it to create a fresh one."
            }
            return "This invite is still open, but the original link or file is not available here. Replace it to create a fresh one."
        }
        return setupScroll.app.inviteSyncExpectationMessage(invite, "")
            + " If the invite link or file was lost, create a fresh invite from this row when it is available."
    }

    function workspaceInviteNextActionText(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        var label = setupScroll.workspaceInviteLabel(invite)
        if (status === "accepted") {
            return "No action needed."
        }
        if (setupScroll.canShareCurrentWorkspaceInvite(invite)) {
            return setupScroll.workspaceInviteUsesClaims(invite)
                ? "Send the invite privately."
                : "Send the invite link or file to " + label + "."
        }
        if (status === "invited") {
            return setupScroll.workspaceInviteUsesClaims(invite)
                ? "Replace the missing invite, then share the new one privately."
                : "Replace the missing invite, then send the new link or file to " + label + "."
        }
        if (status === "expired" || status === "revoked") {
            return setupScroll.workspaceInviteUsesClaims(invite)
                ? "Create a new invite if access is still needed."
                : "Create a new invite if " + label + " still needs access."
        }
        return ""
    }

    function joinRequestInviteRecoveryLabel(request) {
        if (setupScroll.hasLostInviteForJoinRequest(request)) {
            return "Missing invite"
        }
        return "Send another invite"
    }

    function workspaceInviteRecoveryLabel(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        if (status === "invited" && !setupScroll.canShareCurrentWorkspaceInvite(invite)) {
            return "Missing invite"
        }
        return "Send another invite"
    }

    function canShareCurrentWorkspaceInvite(invite) {
        var inviteId = String((invite && invite.inviteId) || "").trim()
        var currentlyStaged = setupScroll.invitePackageId().length > 0
            && setupScroll.invitePackageId() === inviteId
            && setupScroll.keyTransferIsInvitePackage()
        return setupScroll.workspaceInviteStatus(invite) === "invited"
            && inviteId.length > 0
            && (currentlyStaged
                || chaftController.hasWorkspaceInviteArtifact(inviteId))
    }

    function stageWorkspaceInviteForSharing(invite) {
        if (setupScroll.hasUnreturnedInviteResponse()) {
            return false
        }
        var inviteId = String((invite && invite.inviteId) || "").trim()
        if (setupScroll.invitePackageId() === inviteId
                && setupScroll.keyTransferIsInvitePackage()) {
            return true
        }
        return chaftController.stageWorkspaceInviteArtifact(inviteId)
    }

    function copyCurrentWorkspaceInvite(invite) {
        if (!setupScroll.canShareCurrentWorkspaceInvite(invite)
                || !setupScroll.stageWorkspaceInviteForSharing(invite)) {
            return false
        }
        return setupScroll.app.copyKeyTransferArtifact("invite link")
    }

    function saveCurrentWorkspaceInvite(invite) {
        if (!setupScroll.canShareCurrentWorkspaceInvite(invite)
                || !setupScroll.stageWorkspaceInviteForSharing(invite)) {
            return false
        }
        return setupScroll.app.openSaveKeyTransferDialog("invite")
    }

    function workspaceInviteById(inviteId) {
        var normalizedInviteId = String(inviteId || "").trim()
        if (!setupScroll.app || normalizedInviteId.length === 0) {
            return null
        }
        var rows = setupScroll.app.invites || []
        for (var i = 0; i < rows.length; i += 1) {
            var invite = rows[i]
            if (String(invite.inviteId || "").trim() === normalizedInviteId) {
                return invite
            }
        }
        return null
    }

    function canShareCurrentInviteForJoinRequest(request) {
        return String((request && request.status) || "").trim().toLowerCase() === "approved"
            && setupScroll.invitePackageRequestId().length > 0
            && setupScroll.invitePackageRequestId() === setupScroll.joinRequestId(request)
            && setupScroll.keyTransferIsInvitePackage()
    }

    function copyCurrentInviteForJoinRequest(request) {
        if (!setupScroll.canShareCurrentInviteForJoinRequest(request)) {
            return false
        }
        return setupScroll.app.copyKeyTransferArtifact("invite link")
    }

    function saveCurrentInviteForJoinRequest(request) {
        if (!setupScroll.canShareCurrentInviteForJoinRequest(request)) {
            return false
        }
        return setupScroll.app.openSaveKeyTransferDialog("invite")
    }

    function workspaceInviteForJoinRequest(request) {
        var requestId = setupScroll.joinRequestId(request)
        if (!setupScroll.app || requestId.length === 0) {
            return null
        }
        var rows = setupScroll.app.invites || []
        var closedInvite = null
        for (var i = 0; i < rows.length; i += 1) {
            var invite = rows[i]
            if (String(invite.requestId || "").trim() !== requestId) {
                continue
            }
            var status = setupScroll.workspaceInviteStatus(invite)
            if (status === "invited" || status === "accepted") {
                return invite
            }
            if (closedInvite === null) {
                closedInvite = invite
            }
        }
        return closedInvite
    }

    function canCreateReplacementInviteForJoinRequest(request) {
        var normalized = String((request && request.status) || "").trim().toLowerCase()
        if (normalized !== "approved"
                || setupScroll.canShareCurrentInviteForJoinRequest(request)) {
            return false
        }
        var invite = setupScroll.workspaceInviteForJoinRequest(request)
        var inviteStatus = invite === null ? "" : setupScroll.workspaceInviteStatus(invite)
        return setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && setupScroll.joinRequestDeviceId(request).length > 0
            && (invite === null || inviteStatus === "expired" || inviteStatus === "revoked")
            && !setupScroll.hasUnreturnedInviteResponse()
            && !chaftController.keyTransferInFlight
    }

    function replacementInviteForJoinRequestUnavailableReason(request) {
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before creating invites."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (String((request && request.status) || "").trim().toLowerCase() !== "approved") {
            return "Approve the request before creating an invite."
        }
        if (setupScroll.joinRequestDeviceId(request).length === 0) {
            return "This request is missing support details."
        }
        if (setupScroll.canShareCurrentInviteForJoinRequest(request)) {
            return "The current invite is ready to send."
        }
        var invite = setupScroll.workspaceInviteForJoinRequest(request)
        var inviteStatus = invite === null ? "" : setupScroll.workspaceInviteStatus(invite)
        if (inviteStatus === "invited" || inviteStatus === "accepted") {
            return "The existing invite is still open."
        }
        if (setupScroll.hasUnreturnedInviteResponse()) {
            return setupScroll.unreturnedInviteResponseReason()
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before creating another one."
        }
        return ""
    }

    function hasLostInviteForJoinRequest(request) {
        var normalized = String((request && request.status) || "").trim().toLowerCase()
        if (normalized !== "approved"
                || setupScroll.canShareCurrentInviteForJoinRequest(request)) {
            return false
        }
        var invite = setupScroll.workspaceInviteForJoinRequest(request)
        return invite !== null && setupScroll.workspaceInviteStatus(invite) === "invited"
    }

    function canReplaceLostInviteForJoinRequest(request) {
        if (!setupScroll.hasLostInviteForJoinRequest(request)) {
            return false
        }
        var invite = setupScroll.workspaceInviteForJoinRequest(request)
        return invite !== null && setupScroll.canReplaceLostWorkspaceInvite(invite)
    }

    function replaceLostInviteForJoinRequestUnavailableReason(request) {
        if (setupScroll.canShareCurrentInviteForJoinRequest(request)) {
            return "The current invite is ready to send."
        }
        var invite = setupScroll.workspaceInviteForJoinRequest(request)
        if (invite === null) {
            return "No previous invite was found for this request."
        }
        return setupScroll.replaceLostWorkspaceInviteUnavailableReason(invite)
    }

    function confirmReplaceLostInviteForJoinRequest(request) {
        var invite = setupScroll.workspaceInviteForJoinRequest(request)
        if (invite === null) {
            return false
        }
        return setupScroll.confirmReplaceLostWorkspaceInvite(invite)
    }

    function createReplacementInviteForJoinRequest(request) {
        if (!setupScroll.canCreateReplacementInviteForJoinRequest(request)) {
            return false
        }
        return chaftController.prepareWorkspaceInvitePackage(
            setupScroll.joinRequestDeviceId(request),
            setupScroll.inviteRoleValue(),
            setupScroll.app.preferredInvitePeerEndpoint(),
            setupScroll.joinRequestDisplayName(request),
            setupScroll.app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()),
            "preapproved",
            setupScroll.joinRequestId(request))
    }

    function canCreateReplacementWorkspaceInvite(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        var hasRecipient = setupScroll.workspaceInviteUsesClaims(invite)
            || String((invite && invite.inviteeDeviceId) || "").trim().length > 0
        return setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && hasRecipient
            && (status === "expired" || status === "revoked")
            && !setupScroll.hasUnreturnedInviteResponse()
            && !chaftController.keyTransferInFlight
    }

    function replacementWorkspaceInviteUnavailableReason(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before creating invites."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (!setupScroll.workspaceInviteUsesClaims(invite)
                && String((invite && invite.inviteeDeviceId) || "").trim().length === 0) {
            return "This invite is missing support details."
        }
        if (status !== "expired" && status !== "revoked") {
            return "This invite is still open."
        }
        if (setupScroll.hasUnreturnedInviteResponse()) {
            return setupScroll.unreturnedInviteResponseReason()
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before creating another one."
        }
        return ""
    }

    function createWorkspaceInviteWithPolicy(deviceId, role, displayName, expiresAt,
                                             approvalPolicy, requestId) {
        if (String(approvalPolicy || "preapproved").trim() === "admin_required") {
            return chaftController.prepareApprovalInvitePackage(
                deviceId,
                role,
                setupScroll.app.preferredInvitePeerEndpoint(),
                displayName,
                expiresAt)
        }
        return chaftController.prepareWorkspaceInvitePackage(
            deviceId,
            role,
            setupScroll.app.preferredInvitePeerEndpoint(),
            displayName,
            expiresAt,
            "preapproved",
            requestId)
    }

    function createReplacementWorkspaceInvite(invite) {
        if (!setupScroll.canCreateReplacementWorkspaceInvite(invite)) {
            return false
        }
        if (setupScroll.workspaceInviteUsesClaims(invite)) {
            return chaftController.prepareClaimableWorkspaceInviteWithMaxClaims(
                setupScroll.workspaceInviteTrackingLabel(invite),
                String(invite.role || "member").trim(),
                setupScroll.app.preferredInvitePeerEndpoint(),
                setupScroll.app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()),
                setupScroll.workspaceInviteMaxClaims(invite))
        }
        return setupScroll.createWorkspaceInviteWithPolicy(
            String(invite.inviteeDeviceId || "").trim(),
            String(invite.role || "member").trim(),
            setupScroll.workspaceInviteLabel(invite),
            setupScroll.app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()),
            String(invite.approvalPolicy || "preapproved").trim(),
            String(invite.requestId || "").trim())
    }

    function canReplaceLostWorkspaceInvite(invite) {
        var hasRecipient = setupScroll.workspaceInviteUsesClaims(invite)
            || String((invite && invite.inviteeDeviceId) || "").trim().length > 0
        return setupScroll.workspaceInviteStatus(invite) === "invited"
            && !setupScroll.canShareCurrentWorkspaceInvite(invite)
            && setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && String((invite && invite.inviteId) || "").trim().length > 0
            && hasRecipient
            && !setupScroll.hasUnreturnedInviteResponse()
            && !chaftController.keyTransferInFlight
            && !setupScroll.pendingReplacementInviteWaitingForClose
    }

    function replaceLostWorkspaceInviteUnavailableReason(invite) {
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before replacing invites."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (setupScroll.canShareCurrentWorkspaceInvite(invite)) {
            return "The current invite is ready to send."
        }
        if (setupScroll.workspaceInviteStatus(invite) !== "invited") {
            return "Only open invites can be replaced from this row."
        }
        if (String((invite && invite.inviteId) || "").trim().length === 0) {
            return "This invite is missing support details."
        }
        if (!setupScroll.workspaceInviteUsesClaims(invite)
                && String((invite && invite.inviteeDeviceId) || "").trim().length === 0) {
            return "This invite is missing support details."
        }
        if (setupScroll.pendingReplacementInviteWaitingForClose) {
            return "Another invite replacement is already running."
        }
        if (setupScroll.hasUnreturnedInviteResponse()) {
            return setupScroll.unreturnedInviteResponseReason()
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before replacing another one."
        }
        return ""
    }

    function clearPendingWorkspaceInviteReplacement() {
        setupScroll.pendingReplacementInviteId = ""
        setupScroll.pendingReplacementInviteDeviceId = ""
        setupScroll.pendingReplacementInviteRole = "member"
        setupScroll.pendingReplacementInviteDisplayName = ""
        setupScroll.pendingReplacementInviteRequestId = ""
        setupScroll.pendingReplacementInviteApprovalPolicy = "preapproved"
        setupScroll.pendingReplacementInviteMaxClaims = 1
        setupScroll.pendingReplacementInviteClaimable = false
        setupScroll.pendingReplacementInviteWaitingForClose = false
    }

    function rememberPendingWorkspaceInviteReplacement(invite) {
        setupScroll.pendingReplacementInviteId = String((invite && invite.inviteId) || "").trim()
        setupScroll.pendingReplacementInviteDeviceId = String((invite && invite.inviteeDeviceId) || "").trim()
        setupScroll.pendingReplacementInviteRole = String((invite && invite.role) || "member").trim()
        setupScroll.pendingReplacementInviteDisplayName =
            setupScroll.workspaceInviteUsesClaims(invite)
                ? setupScroll.workspaceInviteTrackingLabel(invite)
                : setupScroll.workspaceInviteLabel(invite)
        setupScroll.pendingReplacementInviteRequestId = String((invite && invite.requestId) || "").trim()
        setupScroll.pendingReplacementInviteApprovalPolicy =
            String((invite && invite.approvalPolicy) || "preapproved").trim()
        // Replacing a lost artifact must preserve the old invite's remaining
        // authority, not reset its original capacity after partial use.
        setupScroll.pendingReplacementInviteMaxClaims = Math.max(
            1,
            setupScroll.app
                ? setupScroll.app.inviteRemainingClaims(invite)
                : 1)
        setupScroll.pendingReplacementInviteClaimable =
            setupScroll.workspaceInviteUsesClaims(invite)
    }

    function confirmReplaceLostWorkspaceInvite(invite) {
        if (!setupScroll.canReplaceLostWorkspaceInvite(invite)) {
            return false
        }
        setupScroll.rememberPendingWorkspaceInviteReplacement(invite)
        var label = setupScroll.workspaceInviteLabel(invite)
        var reusableMessage = setupScroll.workspaceInviteTrackingLabel(invite).length > 0
            ? "Close the \"" + label + "\" invite and create a fresh link or file? "
            : "Close this invite and create a fresh link or file? "
        return setupScroll.app.confirmSetupAction(
            "Replace invite",
            (setupScroll.workspaceInviteUsesClaims(invite)
                ? reusableMessage
                : "Close the old invite for " + label + " and create a fresh invite link or file? ")
                + "This does not remove access from anyone who already joined.",
            "Replace invite",
            "replace-workspace-invite:" + setupScroll.pendingReplacementInviteId)
    }

    function startPendingWorkspaceInviteReplacement(inviteId) {
        var normalizedInviteId = String(inviteId || "").trim()
        if (normalizedInviteId.length === 0) {
            return false
        }
        if (setupScroll.pendingReplacementInviteId !== normalizedInviteId) {
            var invite = setupScroll.workspaceInviteById(normalizedInviteId)
            if (invite === null || !setupScroll.canReplaceLostWorkspaceInvite(invite)) {
                setupScroll.clearPendingWorkspaceInviteReplacement()
                return false
            }
            setupScroll.rememberPendingWorkspaceInviteReplacement(invite)
        }
        setupScroll.pendingReplacementInviteWaitingForClose = true
        if (!chaftController.resolveWorkspaceInvite(normalizedInviteId, "revoked")) {
            setupScroll.clearPendingWorkspaceInviteReplacement()
            return false
        }
        return true
    }

    function maybeCompletePendingWorkspaceInviteReplacement() {
        if (!setupScroll.pendingReplacementInviteWaitingForClose) {
            return
        }
        var invite = setupScroll.workspaceInviteById(setupScroll.pendingReplacementInviteId)
        if (invite !== null) {
            var status = setupScroll.workspaceInviteStatus(invite)
            if (status === "invited") {
                return
            }
            if (status === "accepted") {
                setupScroll.clearPendingWorkspaceInviteReplacement()
                return
            }
            if (setupScroll.pendingReplacementInviteClaimable) {
                var remainingClaims = setupScroll.app
                    ? setupScroll.app.inviteRemainingClaims(invite)
                    : 0
                if (remainingClaims < 1) {
                    setupScroll.clearPendingWorkspaceInviteReplacement()
                    return
                }
                // Use the post-revocation snapshot so a claim accepted while
                // the close was in flight cannot expand the total budget.
                setupScroll.pendingReplacementInviteMaxClaims = remainingClaims
            }
        }
        if (!setupScroll.app.runtimeWorkReady
                || !setupScroll.app.canManageWorkspaceAccess()
                || setupScroll.hasUnreturnedInviteResponse()
                || chaftController.keyTransferInFlight) {
            return
        }
        var ok = setupScroll.pendingReplacementInviteClaimable
            ? chaftController.prepareClaimableWorkspaceInviteWithMaxClaims(
                setupScroll.pendingReplacementInviteDisplayName,
                setupScroll.pendingReplacementInviteRole,
                setupScroll.app.preferredInvitePeerEndpoint(),
                setupScroll.app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()),
                setupScroll.pendingReplacementInviteMaxClaims)
            : setupScroll.createWorkspaceInviteWithPolicy(
                setupScroll.pendingReplacementInviteDeviceId,
                setupScroll.pendingReplacementInviteRole,
                setupScroll.pendingReplacementInviteDisplayName,
                setupScroll.app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()),
                setupScroll.pendingReplacementInviteApprovalPolicy,
                setupScroll.pendingReplacementInviteRequestId)
        if (ok) {
            setupScroll.clearPendingWorkspaceInviteReplacement()
        }
    }

    function canRevokeWorkspaceInvite(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        return setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && String((invite && invite.inviteId) || "").trim().length > 0
            && (status === "invited" || status === "expired")
            && !chaftController.keyTransferInFlight
    }

    function revokeWorkspaceInviteUnavailableReason(invite) {
        var status = setupScroll.workspaceInviteStatus(invite)
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before changing invites."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (String((invite && invite.inviteId) || "").trim().length === 0) {
            return "This invite is missing support details."
        }
        if (status !== "invited" && status !== "expired") {
            return "This invite is already closed."
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before changing another one."
        }
        return ""
    }

    function confirmRevokeWorkspaceInvite(invite) {
        var inviteId = String((invite && invite.inviteId) || "").trim()
        if (inviteId.length === 0) {
            return false
        }
        var label = setupScroll.workspaceInviteLabel(invite)
        var lostOpenInvite = setupScroll.workspaceInviteStatus(invite) === "invited"
            && !setupScroll.canShareCurrentWorkspaceInvite(invite)
        var title = lostOpenInvite ? "Close invite" : "Revoke invite"
        var confirmLabel = lostOpenInvite ? "Close invite" : "Revoke invite"
        var message = lostOpenInvite
            ? "Close the old invite for " + label + " without creating a replacement? This does not remove workspace access from anyone who already joined."
            : "Close the invite for " + label + "? This does not remove workspace access from anyone who already joined."
        return setupScroll.app.confirmSetupAction(
            title,
            message,
            confirmLabel,
            "revoke-workspace-invite:" + inviteId)
    }

    function joinRequestAlreadyTracked(request) {
        if (!app) {
            return false
        }
        var requestId = setupScroll.joinRequestId(request)
        var deviceId = setupScroll.joinRequestDeviceId(request)
        var rows = app.joinRequests || []
        for (var i = 0; i < rows.length; i += 1) {
            var row = rows[i]
            if (requestId.length > 0 && String(row.requestId || "") === requestId) {
                return true
            }
            if (requestId.length === 0
                    && deviceId.length > 0
                    && String(row.requesterDeviceId || "") === deviceId
                    && String(row.status || "") === "waiting") {
                return true
            }
        }
        return false
    }

    function trackLoadedJoinRequest() {
        var request = setupScroll.loadedJoinRequestObject()
        if (request === null || setupScroll.joinRequestIsSecureClaim(request)) {
            return false
        }
        if (!setupScroll.joinRequestTargetsCurrentWorkspace(request)) {
            return false
        }
        return chaftController.recordWorkspaceJoinRequest(
            setupScroll.joinRequestId(request),
            setupScroll.joinRequestDeviceId(request),
            setupScroll.joinRequestDisplayName(request),
            setupScroll.joinRequestNoteText(request),
            String(request.sourceType || "").trim(),
            String(request.sourceInviteId || "").trim(),
            String(request.sourceDisplayName || "").trim(),
            String(request.sourceApprovalPolicy || "").trim(),
            String(request.responsePeerEndpoint || "").trim())
    }

    function acceptLoadedWorkspaceInviteClaim() {
        var request = setupScroll.loadedJoinRequestObject()
        if (!setupScroll.joinRequestIsSecureClaim(request)) {
            return false
        }
        if (String(chaftController.keyTransferJson || "").trim().length > 0) {
            setupScroll.manualInviteClaimError = setupScroll.hasUnreturnedInviteResponse()
                ? setupScroll.unreturnedInviteResponseReason()
                : "Finish the current access handoff before approving this join request."
            return false
        }
        if (!setupScroll.secureClaimTargetsThisDevice(request)) {
            setupScroll.manualInviteClaimError =
                "Open this join request on the device that created the invite."
            return false
        }
        if (setupScroll.secureClaimIsKnownExpired(request)) {
            setupScroll.manualInviteClaimError =
                "This invite has expired. Create and send a new invite."
            return false
        }
        if (!setupScroll.joinRequestTargetsCurrentWorkspace(request)) {
            setupScroll.manualInviteClaimError = "Switch to "
                + setupScroll.joinRequestWorkspaceLabel(request)
                + " before approving this join request."
            return false
        }
        setupScroll.manualInviteClaimError = ""
        setupScroll.manualInviteClaimPending = true
        setupScroll.secureAccessReadyDismissed = false
        if (chaftController.acceptWorkspaceInviteClaim(
                    JSON.stringify(request))) {
            return true
        }
        setupScroll.manualInviteClaimPending = false
        setupScroll.manualInviteClaimError = String(
            chaftController.syncStatus || "Could not approve this join request.")
        return false
    }

    function joinRequestPayloadObject(request) {
        var requestId = setupScroll.joinRequestId(request)
        var deviceId = setupScroll.joinRequestDeviceId(request)
        if (requestId.length === 0 || deviceId.length === 0) {
            return null
        }
        return {
            kind: "chaft.workspace-join-request.v1",
            schemaVersion: 1,
            requestId: requestId,
            deviceId: deviceId,
            displayName: setupScroll.joinRequestDisplayName(request),
            note: setupScroll.joinRequestNoteText(request),
            workspaceId: setupScroll.joinRequestWorkspaceId(request),
            workspaceName: setupScroll.joinRequestWorkspaceName(request),
            sourceType: String((request && request.sourceType) || "").trim(),
            sourceInviteId: String((request && request.sourceInviteId) || "").trim(),
            sourceDisplayName: String((request && request.sourceDisplayName) || "").trim(),
            sourceApprovalPolicy: String((request && request.sourceApprovalPolicy) || "").trim()
        }
    }

    function joinRequestArtifactObject(request) {
        var payload = setupScroll.joinRequestPayloadObject(request)
        if (payload === null) {
            return null
        }
        return {
            kind: "chaft.join-request-file.v1",
            schemaVersion: 1,
            workspaceId: payload.workspaceId,
            workspaceName: payload.workspaceName,
            displayName: payload.displayName,
            deviceId: payload.deviceId,
            note: payload.note,
            sourceType: payload.sourceType,
            sourceInviteId: payload.sourceInviteId,
            sourceDisplayName: payload.sourceDisplayName,
            sourceApprovalPolicy: payload.sourceApprovalPolicy,
            request: payload
        }
    }

    function joinRequestArtifactText(request) {
        var payload = setupScroll.joinRequestPayloadObject(request)
        return payload === null ? "" : JSON.stringify(payload, null, 2)
    }

    function canShareJoinRequest(request) {
        return setupScroll.joinRequestArtifactObject(request) !== null
    }

    function shareJoinRequestUnavailableReason(request) {
        if (setupScroll.joinRequestId(request).length === 0) {
            return "This request is missing its request ID."
        }
        if (setupScroll.joinRequestDeviceId(request).length === 0) {
            return "This request is missing support details."
        }
        return "This request cannot be rebuilt here."
    }

    function copyJoinRequest(request) {
        var artifact = setupScroll.joinRequestArtifactObject(request)
        if (artifact === null) {
            return false
        }
        var link = setupScroll.app.artifactLink("chaft-request:", artifact)
        return setupScroll.app.copyTextToClipboard(link, "access request")
    }

    function saveJoinRequest(request) {
        var artifact = setupScroll.joinRequestArtifactObject(request)
        if (artifact === null) {
            return false
        }
        setupScroll.pendingJoinRequestFileText = JSON.stringify(artifact, null, 2)
        saveTrackedJoinRequestDialog.selectedFile =
            setupScroll.app.credentialSuggestedFileUrl(
                setupScroll.pendingJoinRequestFileText,
                "access request")
        saveTrackedJoinRequestDialog.open()
        return true
    }

    function focusInviteForm() {
        setupScroll.requestCategory("people")
        setupScroll.peopleAccessTab = "invites"
        peopleAccessSection.expanded = true
        Qt.callLater(function() {
            if (setupScroll.contentItem !== null) {
                setupScroll.contentItem.contentY = Math.max(
                    0, peopleAccessSection.y - Tokens.space2)
            }
            invitePeopleDialog.open()
        })
    }

    function openPeopleAccessSection() {
        setupScroll.requestCategory("people")
        setupScroll.peopleAccessTab = "members"
        peopleAccessSection.expanded = true
        Qt.callLater(function() {
            if (setupScroll.contentItem !== null) {
                setupScroll.contentItem.contentY = Math.max(
                    0, peopleAccessSection.y - Tokens.space2)
            }
        })
    }

    function openAccessRequestsSection() {
        setupScroll.requestCategory("people")
        setupScroll.peopleAccessTab = "requests"
        peopleAccessSection.expanded = true
        Qt.callLater(function() {
            if (setupScroll.contentItem !== null) {
                setupScroll.contentItem.contentY = Math.max(
                    0, peopleAccessSection.y - Tokens.space2)
            }
        })
    }

    function openInvitationsSection() {
        setupScroll.requestCategory("people")
        setupScroll.peopleAccessTab = "invites"
        peopleAccessSection.expanded = true
    }

    function loadJoinRequestForInvite(request) {
        var text = setupScroll.joinRequestArtifactText(request)
        if (text.length === 0) {
            return false
        }
        setupScroll.applyJoinRequestText(text)
        setupScroll.focusInviteForm()
        return true
    }

    function canApproveJoinRequest(request) {
        return setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && String((request && request.status) || "") === "waiting"
            && setupScroll.joinRequestId(request).length > 0
            && setupScroll.joinRequestDeviceId(request).length > 0
            && setupScroll.joinRequestTargetsCurrentWorkspace(request)
            && !setupScroll.hasUnreturnedInviteResponse()
            && !chaftController.keyTransferInFlight
    }

    function approveJoinRequestUnavailableReason(request) {
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before approving requests."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (String((request && request.status) || "") !== "waiting") {
            return "This request is already resolved."
        }
        if (setupScroll.joinRequestId(request).length === 0) {
            return "This request is missing its request ID."
        }
        if (setupScroll.joinRequestDeviceId(request).length === 0) {
            return "This request is missing support details."
        }
        if (!setupScroll.joinRequestTargetsCurrentWorkspace(request)) {
            return "This request is for " + setupScroll.joinRequestWorkspaceLabel(request) + "."
        }
        if (setupScroll.hasUnreturnedInviteResponse()) {
            return setupScroll.unreturnedInviteResponseReason()
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before approving another request."
        }
        return ""
    }

    function openJoinRequestReview(request) {
        if (!setupScroll.canApproveJoinRequest(request)) {
            return false
        }
        setupScroll.reviewAccessRequest = request || ({})
        reviewAccessRequestDialog.open()
        return true
    }

    function openFirstWaitingJoinRequestReview() {
        var requests = setupScroll.waitingJoinRequests()
        if (requests.length === 0) {
            return false
        }
        setupScroll.peopleAccessTab = "requests"
        return setupScroll.openJoinRequestReview(requests[0])
    }

    function approveJoinRequestWithOptions(request, role, expiryDays) {
        if (!setupScroll.canApproveJoinRequest(request)) {
            return false
        }
        if (chaftController.prepareWorkspaceInvitePackage(
                    setupScroll.joinRequestDeviceId(request),
                    String(role || "member"),
                    setupScroll.app.preferredInvitePeerEndpoint(),
                    setupScroll.joinRequestDisplayName(request),
                    setupScroll.app.inviteExpiresAtIso(expiryDays),
                    "preapproved",
                    setupScroll.joinRequestId(request),
                    String(request.responsePeerEndpoint || "").trim())) {
            setupScroll.loadedJoinRequestText = ""
            inviteDeviceField.text = ""
            reviewAccessRequestDialog.close()
            return true
        }
        return false
    }

    function waitingJoinRequests() {
        return (setupScroll.app ? setupScroll.app.joinRequests : []).filter(function(request) {
            return String((request && request.status) || "") === "waiting"
        })
    }

    function resolvedJoinRequests() {
        return (setupScroll.app ? setupScroll.app.joinRequests : []).filter(function(request) {
            return String((request && request.status) || "") !== "waiting"
        })
    }

    function activeWorkspaceInvites() {
        return (setupScroll.app ? setupScroll.app.invites : []).filter(function(invite) {
            return setupScroll.workspaceInviteStatus(invite) === "invited"
        })
    }

    function resolvedWorkspaceInvites() {
        return (setupScroll.app ? setupScroll.app.invites : []).filter(function(invite) {
            return setupScroll.workspaceInviteStatus(invite) !== "invited"
        })
    }

    function canResolveJoinRequest(request) {
        return setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && String((request && request.status) || "") === "waiting"
            && setupScroll.joinRequestId(request).length > 0
            && !chaftController.keyTransferInFlight
    }

    function resolveJoinRequestUnavailableReason(request) {
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before resolving requests."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (String((request && request.status) || "") !== "waiting") {
            return "This request is already resolved."
        }
        if (setupScroll.joinRequestId(request).length === 0) {
            return "This request is missing support details."
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before resolving another request."
        }
        return ""
    }

    function confirmDeclineJoinRequest(request) {
        if (!setupScroll.canResolveJoinRequest(request)) {
            return false
        }
        var requestId = setupScroll.joinRequestId(request)
        var responsePeerEndpoint = String((request && request.responsePeerEndpoint) || "").trim()
        var label = setupScroll.joinRequestLabel(request)
        return setupScroll.app.confirmSetupAction(
            "Decline request",
            "Decline " + label + "'s access request? They will need to send a new request before joining.",
            "Decline",
            "decline-join-request:" + encodeURIComponent(requestId)
                + "::" + encodeURIComponent(responsePeerEndpoint))
    }

    function confirmRevokeJoinRequest(request) {
        if (!setupScroll.canResolveJoinRequest(request)) {
            return false
        }
        var requestId = setupScroll.joinRequestId(request)
        var responsePeerEndpoint = String((request && request.responsePeerEndpoint) || "").trim()
        var label = setupScroll.joinRequestLabel(request)
        return setupScroll.app.confirmSetupAction(
            "Close request",
            "Close " + label + "'s access request without approving it? Use this for duplicate or stale requests. They will need to send a new request before joining.",
            "Close request",
            "revoke-join-request:" + encodeURIComponent(requestId)
                + "::" + encodeURIComponent(responsePeerEndpoint))
    }

    function loadedJoinRequestFieldLabel(request) {
        var displayName = String((request && request.displayName) || "").trim()
        if (displayName.length > 0) {
            return "Request from " + displayName
        }
        var deviceId = String((request && request.deviceId) || "").trim()
        return deviceId.length > 0
            ? "Request from " + setupScroll.app.unnamedPersonLabel(deviceId)
            : "Request loaded"
    }

    function applyJoinRequestText(text) {
        var value = String(text || "").trim()
        var request = setupScroll.joinRequestObjectFromText(value)
        if (request === null) {
            setupScroll.loadedJoinRequestText = ""
            return false
        }
        setupScroll.accessRequestImportError = ""
        setupScroll.manualInviteClaimError = ""
        setupScroll.loadedJoinRequestText = value
        setupScroll.updatingInviteDeviceField = true
        inviteDeviceField.text = setupScroll.loadedJoinRequestFieldLabel(request)
        setupScroll.updatingInviteDeviceField = false
        return true
    }

    function applyJoinRequestFileText(text) {
        var value = String(text || "").trim()
        if (value.length === 0 || !setupScroll.applyJoinRequestText(value)) {
            setupScroll.loadedJoinRequestText = ""
            setupScroll.updatingInviteDeviceField = true
            inviteDeviceField.text = ""
            setupScroll.updatingInviteDeviceField = false
            setupScroll.accessRequestImportError =
                "Chaft could not read that access request. Ask them to send a new request link or .chaftrequest file."
            return false
        }
        return true
    }

    function inviteDeviceCodeFromText(text) {
        var request = setupScroll.loadedJoinRequestObject()
        if (request === null) {
            request = setupScroll.joinRequestObjectFromText(text)
        }
        if (request !== null) {
            return String(request.deviceId || "").trim()
        }
        return String(text || "").trim()
    }

    function inviteDisplayNameFromText(text) {
        var request = setupScroll.loadedJoinRequestObject()
        if (request === null) {
            request = setupScroll.joinRequestObjectFromText(text)
        }
        return request !== null ? String(request.displayName || "").trim() : ""
    }

    function inviteRoleValue() {
        return String(inviteRoleBox.currentValue || "member")
    }

    function approvalInviteModeAvailable() {
        return false
    }

    function approvalInviteMode() {
        return setupScroll.approvalInviteModeAvailable()
            && setupScroll.approvalInviteModeEnabled
    }

    function canCreateWorkspaceInvite() {
        var request = setupScroll.loadedJoinRequestObject()
        var role = setupScroll.inviteRoleValue()
        return setupScroll.app.runtimeWorkReady
            && setupScroll.app.canManageWorkspaceAccess()
            && (role !== "admin" && role !== "owner"
                || setupScroll.app.canManageWorkspaceOwners())
            && (request === null || setupScroll.joinRequestTargetsCurrentWorkspace(request))
            && !setupScroll.hasUnreturnedInviteResponse()
            && !chaftController.keyTransferInFlight
    }

    function inviteCreateUnavailableReason() {
        if (!setupScroll.app.runtimeWorkReady) {
            return "Open a workspace before creating an invite."
        }
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        if (setupScroll.app.keyTransferIsInviteResponse()) {
            return "Return or save the encrypted access response before creating another invite."
        }
        var role = setupScroll.inviteRoleValue()
        if ((role === "admin" || role === "owner")
                && !setupScroll.app.canManageWorkspaceOwners()) {
            return "Only an owner can invite admins or owners."
        }
        var request = setupScroll.loadedJoinRequestObject()
        if (request !== null && !setupScroll.joinRequestTargetsCurrentWorkspace(request)) {
            return "This request is for " + setupScroll.joinRequestWorkspaceLabel(request) + "."
        }
        if (chaftController.keyTransferInFlight) {
            return "Finish the current invite before creating another one."
        }
        return ""
    }

    function inviteFormHelperText() {
        if (!setupScroll.app.canManageWorkspaceAccess()) {
            return setupScroll.app.workspaceAccessUnavailableReason()
        }
        var request = setupScroll.loadedJoinRequestObject()
        var displayName = request !== null
            ? setupScroll.inviteDisplayNameFromText(inviteDeviceField.text)
            : inviteDeviceField.text.trim()
        if (displayName.length > 0) {
            return setupScroll.createInviteActionLabel() + " for " + displayName
                + ". The invite " + setupScroll.inviteExpiryChoiceLabel() + "."
        }
        return "Create a secure invite, then send the link or file privately. No workspace key is included."
    }

    function loadedJoinRequestStatusText() {
        var label = setupScroll.loadedJoinRequestLabel()
        var request = setupScroll.loadedJoinRequestObject()
        if (request !== null && !setupScroll.joinRequestTargetsCurrentWorkspace(request)) {
            return label + " is requesting " + setupScroll.joinRequestWorkspaceLabel(request)
                + ". Switch to that workspace before approving."
        }
        if (setupScroll.joinRequestIsSecureClaim(request)) {
            if (!setupScroll.secureClaimTargetsThisDevice(request)) {
                return "This join request belongs on the device that created the invite. Open Requests there to approve it."
            }
            if (setupScroll.secureClaimIsKnownExpired(request)) {
                return label
                    + " used an expired invite. Create and send a new invite."
            }
            var access = setupScroll.secureClaimAccessLabel(request)
            var expiry = setupScroll.secureClaimExpiryLabel(request)
            var context = access.length > 0
                ? " for " + access
                : " after Chaft verifies the original access and expiry"
            if (expiry.length > 0) {
                context += ", " + expiry.toLowerCase()
            }
            return "Approve " + label
                + "'s signed join request" + context
                + ", then return the encrypted access file to them."
        }
        if (setupScroll.app.canManageWorkspaceAccess()) {
            return label + " is waiting for an invite. Choose access and expiry, then create an approved invite."
        }
        return label + " is waiting for an invite from an owner or admin."
    }

    function smokeAccessRequestTargetY(rowIndex) {
        if (!accessRequestsPanel.visible) {
            return peopleAccessSection.y - Tokens.space2
        }
        var targetY = peopleAccessSection.y + accessRequestsPanel.y - Tokens.space2
        var rowCount = accessRequestsRepeater.count || 0
        if (rowCount <= 0 || rowIndex <= 0) {
            return targetY
        }
        var clampedIndex = Math.max(0, Math.min(rowIndex, rowCount - 1))
        var rowItem = accessRequestsRepeater.itemAt(clampedIndex)
        if (rowItem !== null) {
            targetY = peopleAccessSection.y + accessRequestsPanel.y
                + accessRequestsColumn.y + rowItem.y - Tokens.space2
        }
        return targetY
    }

    clip: true
    contentWidth: availableWidth
    contentHeight: setupColumn.implicitHeight
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ScrollBar.vertical.policy: ScrollBar.AsNeeded

    Timer {
        id: smokeSetupInviteScrollTimer
        interval: 350
        repeat: false
        onTriggered: {
            var state = String(chaftController.smokeUiState || "")
            var handledState = state === "setup-invite"
                || state === "setup-approval-invite"
                || state === "setup-invite-lost"
                || state === "setup-identity"
                || state === "setup-add-device"
                || state === "setup-access-updates"
                || state === "setup-request"
                || state === "setup-request-approved"
                || state === "setup-request-lost"
                || state === "setup-request-reinvite"
                || state === "setup-security"
                || state === "setup-backup"
            if (!handledState || setupScroll.contentItem === null) {
                return
            }
            if (state === "setup-add-device") {
                addDeviceRecoveryPassphraseField.text = "visual smoke recovery kit"
                addDeviceRecoveryPassphraseConfirmationField.text =
                    addDeviceRecoveryPassphraseField.text
                setupScroll.startAddAnotherDeviceFlow()
                return
            }
            if (state === "setup-access-updates") {
                advancedAccessSection.expanded = true
                setupScroll.advancedAccessToolsOpen = true
            } else if (state === "setup-security") {
                securityToolsSection.expanded = true
            } else if (state === "setup-backup") {
                hostingBackupSection.expanded = true
                chaftController.addBackupPeerEndpoint("127.0.0.1:44944")
            } else {
                peopleAccessSection.expanded = true
            }
            if (state === "setup-request") {
                setupScroll.applyJoinRequestText(JSON.stringify({
                    kind: "chaft.workspace-join-request.v1",
                    schemaVersion: 1,
                    deviceId: "dev_visual_smoke_joiner",
                    displayName: "Sam Rivera",
                    note: "Joining the product team workspace",
                    sourceType: "workspace_card",
                    sourceDisplayName: "Mira Chen",
                    createdAt: new Date().toISOString()
                }, null, 2))
            }
            Qt.callLater(function() {
                var targetY = state === "setup-security"
                    ? securityToolsSection.y - Tokens.space2
                    : peopleAccessSection.y - Tokens.space2
                if (state === "setup-access-updates") {
                    targetY = advancedAccessSection.y - Tokens.space2
                }
                if (state === "setup-backup") {
                    targetY = hostingBackupSection.y - Tokens.space2
                }
                if (state === "setup-identity") {
                    targetY = identityProfileSection.y - Tokens.space2
                } else if (state === "setup-approval-invite" && inviteReadyPanel.visible) {
                    targetY = peopleAccessSection.y + inviteReadyPanel.y - Tokens.space2
                } else if ((state === "setup-invite" || state === "setup-invite-lost")
                        && inviteHistoryPanel.visible) {
                    targetY = peopleAccessSection.y + inviteHistoryPanel.y - Tokens.space2
                } else if (state === "setup-invite" && inviteReadyPanel.visible) {
                    targetY = peopleAccessSection.y + inviteReadyPanel.y - Tokens.space2
                } else if ((state === "setup-request-lost"
                        || state === "setup-request-reinvite")
                        && accessRequestsPanel.visible) {
                    targetY = setupScroll.smokeAccessRequestTargetY(1)
                } else if ((state === "setup-request" || state === "setup-request-approved")
                        && accessRequestsPanel.visible) {
                    targetY = setupScroll.smokeAccessRequestTargetY(0)
                }
                setupScroll.contentItem.contentY = Math.max(0, targetY)
            })
        }
    }

    Component.onCompleted: {
        var state = String(chaftController.smokeUiState || "")
        if (state === "setup-request"
                || state === "setup-identity"
                || state === "setup-add-device"
                || state === "setup-request-approved"
                || state === "setup-request-lost"
                || state === "setup-request-reinvite"
                || state === "setup-approval-invite"
                || state === "setup-invite-lost"
                || state === "setup-access-updates"
                || state === "setup-security"
                || state === "setup-backup") {
            smokeSetupInviteScrollTimer.restart()
        }
    }

    Connections {
        target: chaftController
        function onRuntimeUnlockChanged() {
            if (!chaftController.runtimeUnlocked) {
                workspaceKeyField.text = ""
                setupScroll.autoLoadedKeyTransferText = ""
                recoveryPassphraseField.text = ""
                recoveryPassphraseConfirmationField.text = ""
                addDeviceRecoveryPassphraseField.text = ""
                addDeviceRecoveryPassphraseConfirmationField.text = ""
                setupScroll.addAnotherDeviceOpenSaveWhenReady = false
                setupScroll.manualInviteClaimPending = false
                setupScroll.manualInviteClaimError = ""
                setupScroll.secureResponseDiscardPending = false
            }
        }
        function onKeyTransferJsonChanged() {
            var transferText = String(chaftController.keyTransferJson || "")
            if (transferText.length > 0) {
                workspaceKeyField.text = transferText
                setupScroll.autoLoadedKeyTransferText = transferText
            } else {
                if (String(workspaceKeyField.text || "").trim()
                        === setupScroll.autoLoadedKeyTransferText.trim()) {
                    workspaceKeyField.text = ""
                }
                setupScroll.autoLoadedKeyTransferText = ""
            }
            if (setupScroll.addAnotherDeviceOpenSaveWhenReady
                    && setupScroll.keyTransferIsRecoveryBundle()) {
                setupScroll.addAnotherDeviceOpenSaveWhenReady = false
                setupScroll.app.openSaveKeyTransferDialog("recovery kit")
            }
            if (setupScroll.keyTransferIsInvitePackage()) {
                setupScroll.peopleAccessTab = "invites"
                setupScroll.inviteReadyDismissed = false
                smokeSetupInviteScrollTimer.restart()
            } else if (setupScroll.keyTransferIsInviteResponse()) {
                setupScroll.peopleAccessTab = "requests"
                setupScroll.secureAccessReadyDismissed = false
            }
        }
        function onWorkspaceInviteClaimFinished(success, message) {
            setupScroll.manualInviteClaimPending = false
            if (!success) {
                setupScroll.manualInviteClaimError = String(
                    message || "Could not approve this join request.")
                return
            }
            setupScroll.manualInviteClaimError = ""
            setupScroll.loadedJoinRequestText = ""
            setupScroll.updatingInviteDeviceField = true
            inviteDeviceField.text = ""
            setupScroll.updatingInviteDeviceField = false
            setupScroll.peopleAccessTab = "requests"
            setupScroll.secureAccessReadyDismissed = false
        }
        function onJoinResponseInboxEntryAcknowledged(success, message) {
            if (!setupScroll.secureResponseDiscardPending) {
                return
            }
            setupScroll.secureResponseDiscardPending = false
            if (!success) {
                setupScroll.manualInviteClaimError = String(
                    message || "Could not clear the received response. Try again.")
                return
            }
            setupScroll.manualInviteClaimError = ""
            setupScroll.secureAccessReadyDismissed = true
        }
        function onWorkspaceSnapshotChanged() {
            setupScroll.syncDisplayNameDraft(false)
            setupScroll.maybeCompletePendingWorkspaceInviteReplacement()
            var state = String(chaftController.smokeUiState || "")
            if (state === "setup-invite"
                    || state === "setup-approval-invite"
                    || state === "setup-invite-lost") {
                Qt.callLater(function() {
                    if (setupScroll.app && setupScroll.app.invites.length > 0) {
                        smokeSetupInviteScrollTimer.restart()
                    }
                })
            }
        }
        function onDeviceIdChanged() {
            setupScroll.syncDisplayNameDraft(false)
        }
    }

    FileDialog {
        id: keyPackageDialog
        property string pendingProtocol: "openmls/key-package"
        title: "Share access file"
        fileMode: FileDialog.OpenFile
        onAccepted: {
            var filePath = app.localPathFromUrl(selectedFile)
            chaftController.publishDeviceKeyPackage(pendingProtocol, filePath)
        }
    }

    InvitePeopleDialog {
        id: invitePeopleDialog
        app: setupScroll.app
        roleOptions: setupScroll.inviteRoleOptions()
    }

    ReviewAccessRequestDialog {
        id: reviewAccessRequestDialog
        app: setupScroll.app
        request: setupScroll.reviewAccessRequest
        roleOptions: setupScroll.inviteRoleOptions()
        onApproveRequested: function(role, days) {
            if (!setupScroll.approveJoinRequestWithOptions(
                        setupScroll.reviewAccessRequest, role, days)) {
                reviewAccessRequestDialog.actionError = String(
                    chaftController.syncStatus
                        || "Could not approve this request. Try again.")
            }
        }
        onDeclineRequested: {
            reviewAccessRequestDialog.close()
            setupScroll.confirmDeclineJoinRequest(setupScroll.reviewAccessRequest)
        }
    }

    FileDialog {
        id: joinRequestDialog
        title: "Open join request"
        fileMode: FileDialog.OpenFile
        nameFilters: [
            "Chaft access requests (*.chaftrequest)",
            "Older support files (*.json)",
            "All files (*)"
        ]
        onAccepted: {
            var text = chaftController.readCredentialFile(
                setupScroll.app.localPathFromUrl(selectedFile))
            if (setupScroll.applyJoinRequestFileText(text)) {
                setupScroll.peopleAccessTab = "requests"
            }
        }
    }

    Dialog {
        id: pasteJoinRequestDialog
        parent: Overlay.overlay
        modal: true
        width: Math.min(560, Math.max(0, (parent ? parent.width : 608) - 32))
        x: parent ? Math.round((parent.width - width) / 2) : 0
        y: parent ? Math.max(16, Math.round((parent.height - height) / 2)) : 0
        padding: Tokens.space4
        closePolicy: Popup.CloseOnEscape

        onOpened: {
            pastedJoinRequestArea.text = ""
            pastedJoinRequestArea.forceActiveFocus()
        }
        onClosed: pastedJoinRequestArea.text = ""

        contentItem: ColumnLayout {
            spacing: Tokens.space3

            Text {
                Layout.fillWidth: true
                text: "Paste access handoff"
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeLg
                font.weight: Font.Bold
            }

            Text {
                Layout.fillWidth: true
                text: "Paste the join request exactly as the recipient sent it."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeSm
                wrapMode: Text.WordWrap
            }

            TextArea {
                id: pastedJoinRequestArea
                Layout.fillWidth: true
                Layout.preferredHeight: 180
                placeholderText: "chaft-request:… or join-request JSON"
                Accessible.name: "Join request"
                wrapMode: TextEdit.WrapAnywhere
                selectByMouse: true
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Tokens.space2

                Item { Layout.fillWidth: true }

                Button {
                    text: "Cancel"
                    onClicked: pasteJoinRequestDialog.close()
                }

                Button {
                    text: "Open handoff"
                    enabled: pastedJoinRequestArea.text.trim().length > 0
                    onClicked: {
                        if (setupScroll.applyJoinRequestFileText(
                                    pastedJoinRequestArea.text)) {
                            setupScroll.peopleAccessTab = "requests"
                            pasteJoinRequestDialog.close()
                        }
                    }
                }
            }
        }
    }

    FileDialog {
        id: saveTrackedJoinRequestDialog
        title: "Save access request"
        fileMode: FileDialog.SaveFile
        nameFilters: [
            "Chaft access requests (*.chaftrequest)",
            "Older support files (*.json)",
            "All files (*)"
        ]
        onAccepted: {
            var outputPath = setupScroll.app.outputPathWithExtension(
                setupScroll.app.localPathFromUrl(selectedFile),
                ".chaftrequest")
            chaftController.saveTextFile(
                outputPath,
                setupScroll.pendingJoinRequestFileText,
                "access request")
            setupScroll.pendingJoinRequestFileText = ""
        }
        onRejected: setupScroll.pendingJoinRequestFileText = ""
    }

    Item {
        width: setupScroll.availableWidth
        height: setupColumn.implicitHeight

        ColumnLayout {
            id: setupColumn
            width: parent.width
            spacing: Tokens.space2

            SetupSection {
                id: identityProfileSection
                Layout.fillWidth: true
                visible: setupScroll.category === "profile"
                title: "Identity & Profile"
                collapsible: false
                defaultExpanded: true

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    spacing: Tokens.space2

                    LabeledField {
                        id: displayNameField
                        Layout.fillWidth: true
                        label: "Your name"
                        placeholderText: "e.g. Ada Lovelace"
                        requiredField: true
                        maximumLength: 80
                        supportText: "This is how teammates see you."
                        errorText: setupScroll.profileSaveError
                        Component.onCompleted: setupScroll.syncDisplayNameDraft(true)
                        onEdited: setupScroll.profileSaveError = ""
                        onAccepted: setupScroll.saveDisplayNameDraft()
                    }

                    Button {
                        Layout.alignment: Qt.AlignVCenter
                        text: "Save"
                        enabled: app.runtimeWorkReady
                            && displayNameField.text.trim().length > 0
                            && displayNameField.dirty
                        onClicked: setupScroll.saveDisplayNameDraft()
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    implicitHeight: profileStatusLayout.implicitHeight + Tokens.space2 * 2
                    radius: Tokens.radiusSm
                    color: Tokens.surfaceBase
                    border.width: 1
                    border.color: Tokens.borderSubtle

                    ColumnLayout {
                        id: profileStatusLayout
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space2
                        spacing: Tokens.space1

                        Text {
                            Layout.fillWidth: true
                            text: setupScroll.localProfileStatusText()
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            text: setupScroll.localDeviceLinkStatusText()
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    visible: chaftController.deviceId.length > 0
                    text: "Support code"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: chaftController.deviceId.length > 0
                    spacing: Tokens.space2

                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 26
                        radius: Tokens.radiusXs
                        color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.06)
                        border.width: 1
                        border.color: Tokens.borderSubtle

                        Text {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.leftMargin: 8
                            anchors.rightMargin: 8
                            text: "Code " + setupScroll.app.shortDeviceId(chaftController.deviceId)
                            color: Tokens.textMuted
                            font.family: Tokens.fontMono
                            font.pixelSize: Tokens.fontSizeXs
                            elide: Text.ElideRight
                        }
                    }

                    Button {
                        text: "Copy"
                        Layout.preferredWidth: 56
                        Accessible.name: "Copy support code"
                        onClicked: setupScroll.app.copyTextToClipboard(chaftController.deviceId, "support code")
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                        && chaftController.runtimeUnlocked
                    text: "Lock workspace"
                    enabled: chaftController.runtimeUnlockClearable
                        && !chaftController.keyTransferInFlight
                        && !chaftController.workspaceOperationInFlight
                    onClicked: chaftController.clearRuntimeUnlock()
                    ToolTip.visible: hovered
                    ToolTip.text: chaftController.runtimeUnlockClearable
                        ? "Forget the saved workspace passphrase"
                        : "Passphrase is provided by environment"
                }

                Button {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                        && chaftController.runtimeLocked
                    text: "Unlock workspace"
                    enabled: !chaftController.keyTransferInFlight
                        && !chaftController.workspaceOperationInFlight
                    onClicked: chaftController.requestRuntimeUnlock()
                    ToolTip.visible: hovered
                    ToolTip.text: "Show the workspace passphrase prompt"
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: chaftController.hasRuntimeWorkspace
                    implicitHeight: addAnotherDeviceLayout.implicitHeight + Tokens.space2 * 2
                    radius: Tokens.radiusSm
                    color: Tokens.surfaceBase
                    border.width: 1
                    border.color: Tokens.borderSubtle

                    ColumnLayout {
                        id: addAnotherDeviceLayout
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space2
                        spacing: Tokens.space1

                        Text {
                            Layout.fillWidth: true
                            text: "Add another device"
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeXs
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            text: "Use a recovery kit to restore this workspace on another device. The new device appears separately until Chaft can safely link it."
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Button {
                            Layout.fillWidth: true
                            text: "Create recovery kit"
                            enabled: app.runtimeWorkReady
                                && !chaftController.keyTransferInFlight
                            Accessible.name: "Create recovery kit for another device"
                            ToolTip.visible: hovered && !enabled
                            ToolTip.text: "Unlock this workspace before creating a recovery kit."
                            onClicked: setupScroll.startAddAnotherDeviceFlow()
                        }
                    }
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: setupScroll.category === "workspace"
                    && chaftController.hasRuntimeWorkspace
                title: "Workspace Access"
                collapsible: false
                defaultExpanded: true

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: "Who can join"
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeXs
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    StatusChip {
                        text: app ? app.workspaceAccessPolicyLabel(app.accessPolicy) : "Invite only"
                        description: "Workspace access policy"
                        warning: app && app.accessPolicy !== "invite_only"
                        secure: app && app.accessPolicy === "invite_only"
                        minWidth: 104
                        maxWidth: 168
                    }
                }

                ComboBox {
                    id: workspaceAccessPolicyBox

                    Layout.fillWidth: true
                    model: setupScroll.workspaceAccessPolicyOptionsForPolicy(
                        app ? app.accessPolicy : "")
                    textRole: "label"
                    valueRole: "value"
                    currentIndex: setupScroll.workspaceAccessPolicyIndex(app ? app.accessPolicy : "")
                    enabled: app && app.runtimeWorkReady && app.canManageWorkspaceAccess()
                    Accessible.name: "Who can join this workspace"
                    ToolTip.visible: hovered && !enabled
                    ToolTip.text: app && app.canManageWorkspaceAccess()
                        ? "Open a workspace before changing access."
                        : (app ? app.workspaceAccessUnavailableReason()
                               : "Workspace access unavailable.")
                    onActivated: setupScroll.updateWorkspaceAccessPolicy(currentValue)
                }

                Text {
                    Layout.fillWidth: true
                    text: app ? app.workspaceAccessPolicyDescription(app.accessPolicy) : ""
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Copy request link"
                        enabled: setupScroll.canShareWorkspaceCard()
                        Accessible.name: "Copy workspace request link"
                        ToolTip.visible: hovered && !enabled
                        ToolTip.text: setupScroll.workspaceCardUnavailableReason()
                        onClicked: {
                            if (chaftController.stageWorkspaceAccessCard()) {
                                app.copyKeyTransferArtifact("request link")
                            }
                        }
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Save request card"
                        enabled: setupScroll.canShareWorkspaceCard()
                        Accessible.name: "Save workspace request card"
                        ToolTip.visible: hovered && !enabled
                        ToolTip.text: setupScroll.workspaceCardUnavailableReason()
                        onClicked: {
                            if (chaftController.stageWorkspaceAccessCard()) {
                                app.openSaveKeyTransferDialog("request card")
                            }
                        }
                    }
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: setupScroll.category === "preferences"
                title: "Notifications"
                collapsible: false
                defaultExpanded: true

                CheckBox {
                    text: "Message notifications"
                    checked: chaftController.notificationsEnabled
                    onToggled: chaftController.notificationsEnabled = checked
                    Accessible.name: "Message notifications"
                    Accessible.description: app ? app.notificationScopeText() : "Show desktop notifications while Chaft is open"
                }

                Button {
                    Layout.fillWidth: true
                    text: "Startup settings"
                    Accessible.name: "Open startup settings"
                    ToolTip.visible: hovered
                    ToolTip.text: "Add Chaft to your system startup apps"
                    onClicked: chaftController.openStartupSettings()
                }

                CheckBox {
                    text: "Sound"
                    enabled: chaftController.notificationsEnabled
                    checked: chaftController.notificationSoundEnabled
                    onToggled: chaftController.notificationSoundEnabled = checked
                    Accessible.name: "Notification sound"
                }

                CheckBox {
                    text: "Preview text"
                    enabled: chaftController.notificationsEnabled
                    checked: chaftController.notificationPreviewEnabled
                    onToggled: chaftController.notificationPreviewEnabled = checked
                    Accessible.name: "Notification preview text"
                }

                Text {
                    Layout.fillWidth: true
                    text: app ? app.notificationScopeText() : ""
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Text {
                    Layout.fillWidth: true
                    visible: chaftController.notificationsEnabled
                    text: app ? app.notificationQuietRoomsText() : ""
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: setupScroll.category === "preferences"
                title: "Links"
                collapsible: false
                defaultExpanded: true

                CheckBox {
                    text: "Ask before opening links"
                    checked: chaftController.externalLinkConfirmationEnabled
                    onToggled: chaftController.externalLinkConfirmationEnabled = checked
                    Accessible.name: "Ask before opening external links"
                }

                Text {
                    Layout.fillWidth: true
                    text: chaftController.externalLinkConfirmationEnabled
                        ? "Chaft confirms web and email links before another app opens."
                        : "Message links open immediately in your default app."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }
            }

            SetupSection {
                Layout.fillWidth: true
                visible: setupScroll.category === "preferences"
                title: "Appearance"
                collapsible: false
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
                id: peopleAccessSection
                Layout.fillWidth: true
                visible: setupScroll.category === "people"
                    && chaftController.hasRuntimeWorkspace
                title: ""
                collapsible: false
                defaultExpanded: setupScroll.app
                    && setupScroll.app.waitingAccessRequestCount > 0

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Button {
                        id: accessPolicyButton
                        text: "Access: " + (app
                            ? app.workspaceAccessPolicyLabel(app.accessPolicy)
                            : "Invite only")
                        enabled: app && app.runtimeWorkReady
                            && app.canManageWorkspaceAccess()
                        Accessible.name: "Workspace access policy"
                        Accessible.description: "Change who can ask to join"
                        ToolTip.visible: hovered && !enabled
                        ToolTip.text: app ? app.workspaceAccessUnavailableReason() : ""
                        onClicked: accessPolicyMenu.open()

                        Menu {
                            id: accessPolicyMenu
                            y: accessPolicyButton.height

                            MenuItem {
                                text: "Invite only"
                                checkable: true
                                checked: setupScroll.workspaceAccessPolicy() === "invite_only"
                                onTriggered: setupScroll.updateWorkspaceAccessPolicy("invite_only")
                            }

                            MenuItem {
                                text: "People can request access"
                                checkable: true
                                checked: setupScroll.workspaceAccessPolicy() === "request_access"
                                onTriggered: setupScroll.updateWorkspaceAccessPolicy("request_access")
                            }
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: "Invite people"
                        enabled: app.runtimeWorkReady && app.canManageWorkspaceAccess()
                            && !app.keyTransferIsInviteResponse()
                            && !chaftController.keyTransferInFlight
                        Accessible.name: "Invite people"
                        ToolTip.visible: hovered && !enabled
                        ToolTip.text: setupScroll.inviteCreateUnavailableReason()
                        onClicked: setupScroll.focusInviteForm()
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space1

                    Button {
                        Layout.fillWidth: true
                        checkable: true
                        checked: setupScroll.peopleAccessTab === "members"
                        text: "Members  " + String(app ? app.memberCount : 0)
                        Accessible.name: "Members"
                        Accessible.description: checked ? "Selected" : "Show workspace members"
                        onClicked: setupScroll.peopleAccessTab = "members"
                    }

                    Button {
                        Layout.fillWidth: true
                        checkable: true
                        checked: setupScroll.peopleAccessTab === "requests"
                        text: "Requests  " + String(app ? app.waitingAccessRequestCount : 0)
                        Accessible.name: "Access requests"
                        Accessible.description: checked ? "Selected" : "Show access requests"
                        onClicked: setupScroll.peopleAccessTab = "requests"
                    }

                    Button {
                        Layout.fillWidth: true
                        checkable: true
                        checked: setupScroll.peopleAccessTab === "invites"
                        text: "Invites  " + String(
                            setupScroll.activeWorkspaceInvites().length)
                        Accessible.name: "Invitations"
                        Accessible.description: checked ? "Selected" : "Show invitations"
                        onClicked: setupScroll.peopleAccessTab = "invites"
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: setupScroll.peopleAccessTab === "members"
                    spacing: Tokens.space2

                    Repeater {
                        model: app ? app.members : []

                        delegate: MemberRow {
                            id: peopleMemberRow
                            required property var modelData

                            Layout.fillWidth: true
                            deviceId: String(peopleMemberRow.modelData.deviceId || "")
                            displayLabel: app.memberLabel(peopleMemberRow.modelData)
                            initial: app.memberInitial(peopleMemberRow.modelData)
                            roleLabel: app.roleLabel(peopleMemberRow.modelData.role)
                            roleValue: app.normalizedRole(peopleMemberRow.modelData.role)
                            roleOptions: app.memberRoleOptions(peopleMemberRow.modelData)
                            owner: peopleMemberRow.modelData.role === "owner"
                            localDevice: deviceId === chaftController.deviceId
                            canMessage: app.runtimeWorkReady
                            showRoleEditor: app.canManageWorkspaceAccess() && !localDevice
                            canChangeRole: app.canChangeMemberRole(peopleMemberRow.modelData)
                            roleUnavailableReason: app.memberRoleUnavailableReason(peopleMemberRow.modelData)
                            showRemoveAction: !localDevice
                            canRemove: app.canRemoveMember(peopleMemberRow.modelData)
                            removeUnavailableReason: app.memberRemovalUnavailableReason(
                                peopleMemberRow.modelData)
                            onCopyDeviceRequested: function(deviceId) {
                                app.copyTextToClipboard(deviceId, "support code")
                            }
                            onMessageRequested: function(deviceId, displayLabel) {
                                app.startDirectMessage(deviceId, displayLabel)
                            }
                            onRoleChangeRequested: function(deviceId, role) {
                                app.confirmMemberRoleChange(
                                    deviceId, peopleMemberRow.displayLabel, role)
                            }
                            onRemoveRequested: function(deviceId, displayLabel) {
                                app.confirmMemberRemoval(deviceId, displayLabel)
                            }
                        }
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app && app.memberCount > app.members.length
                        text: "Load more members"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.loadMoreMembers()
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: false
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: setupScroll.peopleAccessPolicyHintText()
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        wrapMode: Text.WordWrap
                    }

                    LabeledField {
                        id: inviteDeviceField
                        Layout.fillWidth: true
                        label: setupScroll.inviteDeviceFieldLabel()
                        placeholderText: setupScroll.inviteDeviceFieldPlaceholder()
                        onTextChanged: {
                            if (setupScroll.updatingInviteDeviceField) {
                                return
                            }
                            if (setupScroll.joinRequestObjectFromText(text) !== null) {
                                Qt.callLater(function() {
                                    setupScroll.applyJoinRequestText(inviteDeviceField.text)
                                })
                            } else {
                                setupScroll.loadedJoinRequestText = ""
                                setupScroll.accessRequestImportError = ""
                            }
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        visible: setupScroll.accessRequestImportError.length > 0
                        text: setupScroll.accessRequestImportError
                        color: Tokens.warningText
                        font.pixelSize: Tokens.fontSizeXs
                        wrapMode: Text.WordWrap
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Tokens.space2

                        Button {
                            Layout.fillWidth: true
                            text: "Open join request"
                            enabled: app.runtimeWorkReady
                            Accessible.name: "Open join request file"
                            onClicked: joinRequestDialog.open()
                        }

                        ComboBox {
                            id: inviteExpiryBox
                            Layout.fillWidth: true
                            model: ["1 day", "7 days", "30 days", "Never"]
                            currentIndex: 1
                            enabled: app.runtimeWorkReady && app.canManageWorkspaceAccess()
                            Accessible.name: "Invite expiry"
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Tokens.space2

                        ComboBox {
                            id: inviteRoleBox
                            Layout.preferredWidth: 120
                            model: setupScroll.inviteRoleOptions()
                            textRole: "label"
                            valueRole: "role"
                            enabled: app.runtimeWorkReady && app.canManageWorkspaceAccess()
                            Accessible.name: "Invited member role"
                        }

                        Button {
                            id: createInviteButton
                            Layout.fillWidth: true
                            text: setupScroll.createInviteActionLabel()
                            Accessible.name: setupScroll.createInviteActionLabel()
                            enabled: setupScroll.canCreateWorkspaceInvite()
                            ToolTip.visible: hovered && !enabled
                            ToolTip.text: setupScroll.inviteCreateUnavailableReason()
                            onClicked: {
                                var loadedRequest = setupScroll.loadedJoinRequestObject()
                                var loadedRequestId = loadedRequest !== null
                                    ? setupScroll.joinRequestId(loadedRequest)
                                    : ""
                                if (loadedRequest !== null
                                        && !setupScroll.joinRequestAlreadyTracked(loadedRequest)) {
                                    setupScroll.trackLoadedJoinRequest()
                                }
                                var created = false
                                if (loadedRequest !== null) {
                                    created = chaftController.prepareWorkspaceInvitePackage(
                                                setupScroll.inviteDeviceCodeFromText(inviteDeviceField.text),
                                                setupScroll.inviteRoleValue(),
                                                app.preferredInvitePeerEndpoint(),
                                                setupScroll.inviteDisplayNameFromText(inviteDeviceField.text),
                                                app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()),
                                                "preapproved",
                                                loadedRequestId)
                                } else {
                                    created = chaftController.prepareClaimableWorkspaceInvite(
                                                inviteDeviceField.text.trim(),
                                                setupScroll.inviteRoleValue(),
                                                app.preferredInvitePeerEndpoint(),
                                                app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()))
                                }
                                if (created) {
                                    inviteDeviceField.text = ""
                                }
                            }
                        }
                    }

                    CheckBox {
                        id: approvalInviteModeBox
                        Layout.fillWidth: true
                        visible: setupScroll.approvalInviteModeAvailable()
                        enabled: app.runtimeWorkReady
                            && app.canManageWorkspaceAccess()
                            && !chaftController.keyTransferInFlight
                        checked: setupScroll.approvalInviteModeEnabled
                        text: "Require approval before messages load"
                        Accessible.name: "Require approval before messages load"
                        onClicked: setupScroll.approvalInviteModeEnabled = checked
                        onVisibleChanged: {
                            if (!visible) {
                                setupScroll.approvalInviteModeEnabled = false
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: inviteAccessModeColumn.implicitHeight + Tokens.space2 * 2
                        radius: Tokens.radiusSm
                        color: Tokens.surfaceBase
                        border.width: 1
                        border.color: Tokens.borderSubtle

                        ColumnLayout {
                            id: inviteAccessModeColumn
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: Tokens.space2
                            spacing: Tokens.space1

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Text {
                                    Layout.fillWidth: true
                                text: setupScroll.loadedJoinRequestObject() !== null
                                    ? "Request approval"
                                    : "Secure invite"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                StatusChip {
                                    text: "No key inside"
                                    description: "Access is encrypted after the recipient uses the invite"
                                    warning: false
                                    secure: true
                                    minWidth: 108
                                    maxWidth: 130
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                text: setupScroll.inviteAccessModeText()
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        visible: setupScroll.workspacePolicyEncouragesRequests()
                        implicitHeight: requestOnlyInviteColumn.implicitHeight + Tokens.space2 * 2
                        radius: Tokens.radiusSm
                        color: Tokens.surfaceBase
                        border.width: 1
                        border.color: Tokens.borderSubtle

                        ColumnLayout {
                            id: requestOnlyInviteColumn
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: Tokens.space2
                            spacing: Tokens.space1

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Text {
                                    Layout.fillWidth: true
                                    text: "Request link"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }

                                StatusChip {
                                    text: "Approval first"
                                    description: "This link shares no workspace access"
                                    warning: true
                                    minWidth: 106
                                    maxWidth: 130
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                text: "Shares workspace info only. They send a request; you approve it here."
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space1

                                Button {
                                    Layout.fillWidth: true
                                    text: "Copy link"
                                    enabled: setupScroll.canShareWorkspaceCard()
                                    Accessible.name: "Copy request link"
                                    ToolTip.visible: hovered
                                    ToolTip.text: enabled
                                        ? "Copy request link"
                                        : setupScroll.workspaceCardUnavailableReason()
                                    onClicked: setupScroll.copyRequestOnlyInvite()
                                }

                                Button {
                                    Layout.fillWidth: true
                                    text: "Save card"
                                    enabled: setupScroll.canShareWorkspaceCard()
                                    Accessible.name: "Save request card"
                                    ToolTip.visible: hovered
                                    ToolTip.text: enabled
                                        ? "Save request card"
                                        : setupScroll.workspaceCardUnavailableReason()
                                    onClicked: setupScroll.saveRequestOnlyInvite()
                                }
                            }
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        text: app.roleDescription(setupScroll.inviteRoleValue())
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        wrapMode: Text.WordWrap
                    }
                }

                Text {
                    Layout.fillWidth: true
                    visible: false
                    text: setupScroll.inviteFormHelperText()
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: setupScroll.loadedJoinRequestObject() !== null
                        && setupScroll.peopleAccessTab === "requests"
                        && !setupScroll.keyTransferIsInvitePackage()
                        && !setupScroll.joinRequestAlreadyTracked(setupScroll.loadedJoinRequestObject())
                    implicitHeight: requestStatusColumn.implicitHeight + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: Tokens.surfaceRaised
                    border.width: 1
                    border.color: Tokens.borderSubtle

                    ColumnLayout {
                        id: requestStatusColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Text {
                                Layout.fillWidth: true
                                text: setupScroll.loadedJoinRequestIsSecureClaim()
                                    ? "Invite join request"
                                    : "Access request"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            StatusChip {
                                text: setupScroll.loadedJoinRequestIsSecureClaim()
                                    ? "Signed"
                                    : "Waiting"
                                description: setupScroll.loadedJoinRequestIsSecureClaim()
                                    ? "The signed request is verified before access is granted"
                                    : "Waiting for an admin to create and send an invite"
                                warning: !setupScroll.loadedJoinRequestIsSecureClaim()
                                secure: setupScroll.loadedJoinRequestIsSecureClaim()
                                minWidth: 82
                                maxWidth: 104
                            }

                            Button {
                                text: setupScroll.manualInviteClaimPending
                                    ? "Approving..."
                                    : (setupScroll.loadedJoinRequestIsSecureClaim()
                                        ? "Approve"
                                        : "Track")
                                visible: !setupScroll.joinRequestAlreadyTracked(setupScroll.loadedJoinRequestObject())
                                enabled: app.runtimeWorkReady
                                    && app.canManageWorkspaceAccess()
                                    && setupScroll.secureClaimCanBeAccepted(
                                        setupScroll.loadedJoinRequestObject())
                                    && String(chaftController.keyTransferJson || "")
                                        .trim().length === 0
                                    && !setupScroll.hasUnreturnedInviteResponse()
                                    && !setupScroll.manualInviteClaimPending
                                    && !chaftController.keyTransferInFlight
                                Accessible.name: setupScroll.loadedJoinRequestIsSecureClaim()
                                    ? "Approve invite join request"
                                    : "Track access request"
                                ToolTip.visible: hovered && !enabled
                                ToolTip.text: !setupScroll.secureClaimTargetsThisDevice(
                                        setupScroll.loadedJoinRequestObject())
                                    ? "Open this on the device that created the invite"
                                    : (setupScroll.secureClaimIsKnownExpired(
                                            setupScroll.loadedJoinRequestObject())
                                        ? "This invite has expired"
                                    : (String(chaftController.keyTransferJson || "")
                                            .trim().length > 0
                                        && !setupScroll.hasUnreturnedInviteResponse()
                                        ? "Finish the current access handoff first"
                                    : (setupScroll.hasUnreturnedInviteResponse()
                                    ? setupScroll.unreturnedInviteResponseReason()
                                    : (app.canManageWorkspaceAccess()
                                        ? "Open a workspace before tracking requests"
                                        : "Only workspace admins can track access requests"))))
                                onClicked: {
                                    if (setupScroll.loadedJoinRequestIsSecureClaim()) {
                                        setupScroll.acceptLoadedWorkspaceInviteClaim()
                                    } else {
                                        setupScroll.trackLoadedJoinRequest()
                                    }
                                }
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: setupScroll.loadedJoinRequestStatusText()
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.loadedJoinRequestIsSecureClaim()
                                && setupScroll.secureClaimIssuerLabel(
                                    setupScroll.loadedJoinRequestObject()).length > 0
                            text: "Invite created by "
                                + setupScroll.secureClaimIssuerLabel(
                                    setupScroll.loadedJoinRequestObject())
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.loadedJoinRequestNote().length > 0
                            text: "Note: " + setupScroll.loadedJoinRequestNote()
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.manualInviteClaimError.length > 0
                            text: setupScroll.manualInviteClaimError
                            color: Tokens.warningText
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.joinRequestWorkspaceLabel(setupScroll.loadedJoinRequestObject()).length > 0
                            text: "Workspace: " + setupScroll.joinRequestWorkspaceLabel(setupScroll.loadedJoinRequestObject())
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.joinRequestSourceLabel(setupScroll.loadedJoinRequestObject()).length > 0
                            text: "Started from " + setupScroll.joinRequestSourceLabel(setupScroll.loadedJoinRequestObject())
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.loadedJoinRequestDeviceLabel().length > 0
                            text: setupScroll.loadedJoinRequestDeviceLabel()
                            color: Tokens.textMuted
                            font.family: Tokens.fontMono
                            font.pixelSize: Tokens.fontSizeXs
                            elide: Text.ElideMiddle
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    visible: setupScroll.peopleAccessTab === "requests"
                        && setupScroll.keyTransferIsInviteResponse()
                        && !setupScroll.secureAccessReadyDismissed
                    implicitHeight: secureAccessReadyColumn.implicitHeight
                        + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: Qt.rgba(Tokens.accent.r, Tokens.accent.g,
                                   Tokens.accent.b, 0.1)
                    border.width: 1
                    border.color: Qt.rgba(Tokens.accent.r, Tokens.accent.g,
                                          Tokens.accent.b, 0.45)

                    ColumnLayout {
                        id: secureAccessReadyColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Text {
                                Layout.fillWidth: true
                                text: "Encrypted access ready"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                            }

                            StatusChip {
                                text: "Device-bound"
                                description: "Only the requesting device can open it"
                                secure: true
                                minWidth: 104
                                maxWidth: 132
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: "Return this access file to the requester. Copy or save it before clearing this handoff."
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.manualInviteClaimError.length > 0
                            text: setupScroll.manualInviteClaimError
                            color: Tokens.warningText
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Button {
                                Layout.fillWidth: true
                                text: "Copy access"
                                onClicked: app.copyKeyTransferArtifact("secure access")
                            }

                            Button {
                                Layout.fillWidth: true
                                text: "Save access file"
                                onClicked: app.openSaveKeyTransferDialog("secure access")
                            }

                            Button {
                                text: "Clear"
                                ToolTip.visible: hovered
                                ToolTip.text: "Clear this response after it has been returned"
                                onClicked: setupScroll.confirmDiscardSecureInviteResponse()
                            }
                        }
                    }
                }

                Item {
                    id: accessRequestsPanel

                    Layout.fillWidth: true
                    visible: setupScroll.peopleAccessTab === "requests" && app
                    implicitHeight: accessRequestsColumn.implicitHeight

                    ColumnLayout {
                        id: accessRequestsColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        spacing: Tokens.space2

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Item {
                                    Layout.fillWidth: true
                                }

                                Button {
                                    id: openAccessRequestButton
                                    visible: app && app.canManageWorkspaceAccess()
                                    text: "Open"
                                    Accessible.name: "Open join request"
                                    onClicked: openAccessRequestMenu.open()

                                    Menu {
                                        id: openAccessRequestMenu
                                        y: openAccessRequestButton.height

                                        MenuItem {
                                            text: "Paste join request"
                                            onTriggered: pasteJoinRequestDialog.open()
                                        }

                                        MenuItem {
                                            text: "Open join request file"
                                            onTriggered: joinRequestDialog.open()
                                        }
                                    }
                                }

                                Button {
                                    text: setupScroll.showRequestHistory ? "Active" : "History"
                                    enabled: app && app.joinRequestCount > 0
                                    Accessible.name: setupScroll.showRequestHistory
                                        ? "Show active requests"
                                        : "Show request history"
                                    onClicked: setupScroll.showRequestHistory =
                                        !setupScroll.showRequestHistory
                                }

                                Button {
                                    visible: app && app.canManageWorkspaceAccess()
                                    text: chaftController.joinRequestInboxInFlight
                                        ? "Checking..."
                                        : "↻"
                                    Accessible.name: chaftController.joinRequestInboxInFlight
                                        ? "Checking for delivered access requests"
                                        : "Check for delivered access requests"
                                    enabled: app && app.runtimeWorkReady
                                        && app.canManageWorkspaceAccess()
                                        && !chaftController.joinRequestInboxInFlight
                                    onClicked: chaftController.refreshJoinRequestInbox()
                                    ToolTip.visible: hovered
                                    ToolTip.text: app && app.canManageWorkspaceAccess()
                                        ? "Check this device for new access requests"
                                        : setupScroll.app
                                            ? setupScroll.app.workspaceAccessUnavailableReason()
                                            : "Open a workspace before checking access requests"
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                visible: false
                                spacing: Tokens.space1

                                StatusChip {
                                    text: String(app ? app.joinRequestCount : 0)
                                    description: "Tracked access requests"
                                    minWidth: 42
                                    maxWidth: 64
                                }

                                StatusChip {
                                    visible: app && app.waitingAccessRequestCount > 0
                                    text: app ? app.accessRequestCountLabel(app.waitingAccessRequestCount) : ""
                                    description: "Waiting access requests"
                                    warning: true
                                    minWidth: 78
                                    maxWidth: 112
                                }

                                Item {
                                    Layout.fillWidth: true
                                }
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: setupScroll.showRequestHistory
                                ? (setupScroll.resolvedJoinRequests().length > 0
                                    ? "Previous access decisions"
                                    : "No request history")
                                : (setupScroll.waitingJoinRequests().length > 0
                                    ? "Review who is waiting to join"
                                    : "No requests need attention")
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Repeater {
                            id: accessRequestsRepeater

                            model: setupScroll.showRequestHistory
                                ? setupScroll.resolvedJoinRequests()
                                : setupScroll.waitingJoinRequests()

                            delegate: Rectangle {
                                id: accessRequestRow

                                readonly property string requestStatus: String(modelData.status || "")

                                Layout.fillWidth: true
                                implicitHeight: requestRowLayout.implicitHeight + Tokens.space2 * 2
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceRaised
                                border.width: 1
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    id: requestRowLayout
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    anchors.margins: Tokens.space2
                                    spacing: Tokens.space2

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 2

                                            Text {
                                                Layout.fillWidth: true
                                                text: setupScroll.joinRequestLabel(modelData)
                                                color: Tokens.textStrong
                                                font.pixelSize: Tokens.fontSizeXs
                                                font.weight: Font.DemiBold
                                                elide: Text.ElideRight
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: setupScroll.joinRequestCompactContextLabel(modelData)
                                                color: Tokens.textMuted
                                                font.pixelSize: Tokens.fontSizeXs
                                                elide: Text.ElideRight
                                            }
                                        }

                                        StatusChip {
                                            text: setupScroll.joinRequestStatusLabel(accessRequestRow.requestStatus)
                                            description: "Access request status"
                                            warning: accessRequestRow.requestStatus === "waiting"
                                            secure: accessRequestRow.requestStatus === "approved"
                                            minWidth: 82
                                            maxWidth: 104
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: accessRequestRow.requestStatus !== "waiting"
                                        text: setupScroll.joinRequestStatusDetail(modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: accessRequestRow.requestStatus !== "waiting"
                                            && setupScroll.joinRequestNextActionText(modelData).length > 0
                                        text: setupScroll.joinRequestNextActionText(modelData)
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.Medium
                                        wrapMode: Text.WordWrap
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: setupScroll.joinRequestNoteSummary(modelData).length > 0
                                        text: setupScroll.joinRequestNoteSummary(modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: setupScroll.joinRequestReceiptLabel(modelData).length > 0
                                        text: setupScroll.joinRequestReceiptLabel(modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        elide: Text.ElideRight
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: false
                                        text: "Started from " + setupScroll.joinRequestSourceLabel(modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: Tokens.space1

                                        Text {
                                            Layout.fillWidth: true
                                            visible: accessRequestRow.requestStatus === "waiting"
                                                && !setupScroll.canApproveJoinRequest(modelData)
                                            text: setupScroll.approveJoinRequestUnavailableReason(modelData)
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            wrapMode: Text.WordWrap
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            visible: false
                                            text: "Invite to send"
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.DemiBold
                                            elide: Text.ElideRight
                                        }

                                        RowLayout {
                                            Layout.fillWidth: true
                                            visible: setupScroll.canShareCurrentInviteForJoinRequest(modelData)
                                            spacing: Tokens.space1

                                            Button {
                                                Layout.fillWidth: true
                                                text: "Copy link"
                                                Accessible.name: "Copy invite link for "
                                                    + setupScroll.joinRequestLabel(modelData)
                                                ToolTip.visible: hovered
                                                ToolTip.text: "Copy the approved invite link"
                                                onClicked: setupScroll.copyCurrentInviteForJoinRequest(modelData)
                                            }

                                            Button {
                                                Layout.fillWidth: true
                                                text: "Save file"
                                                Accessible.name: "Save invite for "
                                                    + setupScroll.joinRequestLabel(modelData)
                                                ToolTip.visible: hovered
                                                ToolTip.text: "Save the approved invite file"
                                                onClicked: setupScroll.saveCurrentInviteForJoinRequest(modelData)
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            visible: accessRequestRow.requestStatus === "approved"
                                                && !setupScroll.canShareCurrentInviteForJoinRequest(modelData)
                                            text: setupScroll.joinRequestInviteRecoveryLabel(modelData)
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            font.weight: Font.DemiBold
                                            elide: Text.ElideRight
                                        }

                                        Button {
                                            Layout.fillWidth: true
                                            visible: accessRequestRow.requestStatus === "approved"
                                                && !setupScroll.canShareCurrentInviteForJoinRequest(modelData)
                                                && setupScroll.hasLostInviteForJoinRequest(modelData)
                                            text: "Replace invite"
                                            enabled: setupScroll.canReplaceLostInviteForJoinRequest(modelData)
                                            Accessible.name: "Replace invite for "
                                                + setupScroll.joinRequestLabel(modelData)
                                            ToolTip.visible: hovered && !enabled
                                            ToolTip.text: setupScroll.replaceLostInviteForJoinRequestUnavailableReason(modelData)
                                            onClicked: setupScroll.confirmReplaceLostInviteForJoinRequest(modelData)
                                        }

                                        Button {
                                            Layout.fillWidth: true
                                            visible: accessRequestRow.requestStatus === "approved"
                                                && !setupScroll.canShareCurrentInviteForJoinRequest(modelData)
                                                && !setupScroll.hasLostInviteForJoinRequest(modelData)
                                            text: "Create new invite"
                                            enabled: setupScroll.canCreateReplacementInviteForJoinRequest(modelData)
                                            Accessible.name: "Create new invite for "
                                                + setupScroll.joinRequestLabel(modelData)
                                            ToolTip.visible: hovered && !enabled
                                            ToolTip.text: setupScroll.replacementInviteForJoinRequestUnavailableReason(modelData)
                                            onClicked: setupScroll.createReplacementInviteForJoinRequest(modelData)
                                        }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        visible: accessRequestRow.requestStatus === "waiting"
                                        spacing: Tokens.space1

                                            Button {
                                                Layout.fillWidth: true
                                                text: "Approve"
                                                enabled: setupScroll.canApproveJoinRequest(modelData)
                                                Accessible.name: "Approve access request"
                                            ToolTip.visible: hovered && !enabled
                                            ToolTip.text: setupScroll.approveJoinRequestUnavailableReason(modelData)
                                            onClicked: setupScroll.openJoinRequestReview(modelData)
                                        }

                                        Button {
                                            id: requestActionsButton
                                            text: "⋯"
                                            Layout.preferredWidth: 38
                                            Accessible.name: "More actions for "
                                                + setupScroll.joinRequestLabel(modelData)
                                            onClicked: requestActionsMenu.open()

                                            Menu {
                                                id: requestActionsMenu
                                                y: requestActionsButton.height

                                                MenuItem {
                                                    text: "Decline"
                                                    enabled: setupScroll.canResolveJoinRequest(modelData)
                                                    onTriggered: setupScroll.confirmDeclineJoinRequest(modelData)
                                                }

                                                MenuItem {
                                                    text: "Close as duplicate"
                                                    enabled: setupScroll.canResolveJoinRequest(modelData)
                                                    onTriggered: setupScroll.confirmRevokeJoinRequest(modelData)
                                                }

                                                MenuSeparator {}

                                                MenuItem {
                                                    text: "Copy technical request"
                                                    enabled: setupScroll.canShareJoinRequest(modelData)
                                                    onTriggered: setupScroll.copyJoinRequest(modelData)
                                                }
                                            }
                                        }

                                        RowLayout {
                                            Layout.fillWidth: true
                                            visible: false
                                                spacing: Tokens.space1

                                                Button {
                                                    Layout.fillWidth: true
                                                    text: "Decline"
                                                    enabled: setupScroll.canResolveJoinRequest(modelData)
                                                    Accessible.name: "Decline access request"
                                                    ToolTip.visible: hovered && !enabled
                                                    ToolTip.text: setupScroll.resolveJoinRequestUnavailableReason(modelData)
                                                    onClicked: setupScroll.confirmDeclineJoinRequest(modelData)
                                                }

                                                Button {
                                                    Layout.fillWidth: true
                                                    text: "Close request"
                                                    enabled: setupScroll.canResolveJoinRequest(modelData)
                                                    Accessible.name: "Close access request"
                                                    ToolTip.visible: hovered && !enabled
                                                    ToolTip.text: setupScroll.resolveJoinRequestUnavailableReason(modelData)
                                                    onClicked: setupScroll.confirmRevokeJoinRequest(modelData)
                                                }
                                            }
                                        }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        visible: false
                                        spacing: Tokens.space1

                                            Text {
                                                Layout.fillWidth: true
                                                visible: accessRequestRow.requestStatus === "approved"
                                                    && setupScroll.canShareJoinRequest(modelData)
                                                text: "Original request"
                                                color: Tokens.textMuted
                                                font.pixelSize: Tokens.fontSizeXs
                                                font.weight: Font.DemiBold
                                                elide: Text.ElideRight
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: Tokens.space1

                                                Button {
                                                    Layout.fillWidth: true
                                                    text: "Copy link"
                                                    enabled: setupScroll.canShareJoinRequest(modelData)
                                                    Accessible.name: "Copy access request"
                                                    ToolTip.visible: hovered
                                                    ToolTip.text: enabled
                                                        ? "Copy the original access request"
                                                        : setupScroll.shareJoinRequestUnavailableReason(modelData)
                                                    onClicked: setupScroll.copyJoinRequest(modelData)
                                                }

                                                Button {
                                                    Layout.fillWidth: true
                                                    text: "Save file"
                                                    enabled: setupScroll.canShareJoinRequest(modelData)
                                                    Accessible.name: "Save access request"
                                                    ToolTip.visible: hovered
                                                    ToolTip.text: enabled
                                                        ? "Save the original access request file"
                                                        : setupScroll.shareJoinRequestUnavailableReason(modelData)
                                                    onClicked: setupScroll.saveJoinRequest(modelData)
                                                }
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            visible: !setupScroll.canShareJoinRequest(modelData)
                                            text: setupScroll.shareJoinRequestUnavailableReason(modelData)
                                            color: Tokens.textMuted
                                            font.pixelSize: Tokens.fontSizeXs
                                            wrapMode: Text.WordWrap
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Item {
                    id: inviteHistoryPanel

                    Layout.fillWidth: true
                    visible: setupScroll.peopleAccessTab === "invites"
                        && app && app.invites.length > 0
                        && (!setupScroll.keyTransferIsInvitePackage()
                            || setupScroll.inviteReadyDismissed)
                    implicitHeight: inviteHistoryColumn.implicitHeight

                    ColumnLayout {
                        id: inviteHistoryColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        spacing: Tokens.space2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Item {
                                Layout.fillWidth: true
                            }

                            Button {
                                text: setupScroll.showInviteHistory ? "Active" : "History"
                                enabled: app && app.inviteCount > 0
                                Accessible.name: setupScroll.showInviteHistory
                                    ? "Show active invitations"
                                    : "Show invitation history"
                                onClicked: setupScroll.showInviteHistory =
                                    !setupScroll.showInviteHistory
                            }

                            StatusChip {
                                text: String(setupScroll.showInviteHistory
                                    ? setupScroll.resolvedWorkspaceInvites().length
                                    : setupScroll.activeWorkspaceInvites().length)
                                description: "Invitations"
                                minWidth: 42
                                maxWidth: 64
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: setupScroll.showInviteHistory
                                ? "Expired, revoked, and completed invitations"
                                : (setupScroll.activeWorkspaceInvites().length > 0
                                    ? "Invitations that can still be used"
                                    : "No active invitations")
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Repeater {
                            model: setupScroll.showInviteHistory
                                ? setupScroll.resolvedWorkspaceInvites()
                                : setupScroll.activeWorkspaceInvites()

                            delegate: Rectangle {
                                id: inviteHistoryRow

                                readonly property string inviteStatus: setupScroll.workspaceInviteStatus(modelData)

                                Layout.fillWidth: true
                                implicitHeight: inviteHistoryRowLayout.implicitHeight + Tokens.space2 * 2
                                radius: Tokens.radiusSm
                                color: Tokens.surfaceRaised
                                border.width: 1
                                border.color: Tokens.borderSubtle

                                ColumnLayout {
                                    id: inviteHistoryRowLayout
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    anchors.margins: Tokens.space2
                                    spacing: Tokens.space2

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: Tokens.space2

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 2

                                            Text {
                                                Layout.fillWidth: true
                                                text: setupScroll.workspaceInviteLabel(modelData)
                                                color: Tokens.textStrong
                                                font.pixelSize: Tokens.fontSizeXs
                                                font.weight: Font.DemiBold
                                                elide: Text.ElideRight
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: setupScroll.workspaceInviteAccessLabel(modelData)
                                                color: Tokens.textMuted
                                                font.pixelSize: Tokens.fontSizeXs
                                                elide: Text.ElideRight
                                            }
                                        }

                                        StatusChip {
                                            text: setupScroll.workspaceInviteStatusLabel(modelData)
                                            description: "Invite status"
                                            warning: inviteHistoryRow.inviteStatus === "invited"
                                                || inviteHistoryRow.inviteStatus === "expired"
                                            secure: inviteHistoryRow.inviteStatus === "accepted"
                                            minWidth: 82
                                            maxWidth: 126
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: setupScroll.workspaceInviteExpirySummary(modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: false
                                        text: setupScroll.workspaceInviteStatusDetail(modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: setupScroll.workspaceInviteNextActionText(modelData).length > 0
                                        text: setupScroll.workspaceInviteNextActionText(modelData)
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.Medium
                                        wrapMode: Text.WordWrap
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: false
                                        text: "Invite to send"
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideRight
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        visible: setupScroll.canShareCurrentWorkspaceInvite(modelData)
                                        spacing: Tokens.space1

                                        Button {
                                            Layout.fillWidth: true
                                            text: "Copy invite"
                                            Accessible.name: "Copy invite link for "
                                                + setupScroll.workspaceInviteLabel(modelData)
                                            ToolTip.visible: hovered
                                            ToolTip.text: "Copy the current invite link"
                                            onClicked: setupScroll.copyCurrentWorkspaceInvite(modelData)
                                        }

                                        Button {
                                            id: inviteActionsButton
                                            text: "⋯"
                                            Layout.preferredWidth: 38
                                            Accessible.name: "More actions for "
                                                + setupScroll.workspaceInviteLabel(modelData)
                                            onClicked: inviteActionsMenu.open()

                                            Menu {
                                                id: inviteActionsMenu
                                                y: inviteActionsButton.height

                                                MenuItem {
                                                    text: "Save invite file"
                                                    onTriggered: setupScroll.saveCurrentWorkspaceInvite(modelData)
                                                }

                                                MenuItem {
                                                    text: "Revoke invite"
                                                    enabled: setupScroll.canRevokeWorkspaceInvite(modelData)
                                                    onTriggered: setupScroll.confirmRevokeWorkspaceInvite(modelData)
                                                }
                                            }
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        visible: inviteHistoryRow.inviteStatus === "expired"
                                            || inviteHistoryRow.inviteStatus === "revoked"
                                            || (inviteHistoryRow.inviteStatus === "invited"
                                                && !setupScroll.canShareCurrentWorkspaceInvite(modelData))
                                        text: setupScroll.workspaceInviteRecoveryLabel(modelData)
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideRight
                                    }

                                    Button {
                                        Layout.fillWidth: true
                                        visible: inviteHistoryRow.inviteStatus === "expired"
                                            || inviteHistoryRow.inviteStatus === "revoked"
                                        text: "Create new invite"
                                        enabled: setupScroll.canCreateReplacementWorkspaceInvite(modelData)
                                        Accessible.name: "Create new invite for "
                                            + setupScroll.workspaceInviteLabel(modelData)
                                        ToolTip.visible: hovered && !enabled
                                        ToolTip.text: setupScroll.replacementWorkspaceInviteUnavailableReason(modelData)
                                        onClicked: setupScroll.createReplacementWorkspaceInvite(modelData)
                                    }

                                    Button {
                                        Layout.fillWidth: true
                                        visible: inviteHistoryRow.inviteStatus === "invited"
                                            && !setupScroll.canShareCurrentWorkspaceInvite(modelData)
                                        text: "Replace invite"
                                        enabled: setupScroll.canReplaceLostWorkspaceInvite(modelData)
                                        Accessible.name: "Replace invite for "
                                            + setupScroll.workspaceInviteLabel(modelData)
                                        ToolTip.visible: hovered && !enabled
                                        ToolTip.text: setupScroll.replaceLostWorkspaceInviteUnavailableReason(modelData)
                                        onClicked: setupScroll.confirmReplaceLostWorkspaceInvite(modelData)
                                    }

                                    Button {
                                        Layout.fillWidth: true
                                        visible: false
                                        text: inviteHistoryRow.inviteStatus === "invited"
                                            && !setupScroll.canShareCurrentWorkspaceInvite(modelData)
                                            ? "Close invite"
                                            : "Revoke invite"
                                        enabled: setupScroll.canRevokeWorkspaceInvite(modelData)
                                        Accessible.name: text + " for "
                                            + setupScroll.workspaceInviteLabel(modelData)
                                        ToolTip.visible: hovered && !enabled
                                        ToolTip.text: setupScroll.revokeWorkspaceInviteUnavailableReason(modelData)
                                        onClicked: setupScroll.confirmRevokeWorkspaceInvite(modelData)
                                    }
                                }
                            }
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    visible: setupScroll.peopleAccessTab === "invites"
                        && app && app.invites.length === 0
                        && setupScroll.activeWorkspaceInvites().length === 0
                        && !setupScroll.showInviteHistory
                        && !setupScroll.keyTransferIsInvitePackage()
                    text: "No active invitations"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeSm
                    horizontalAlignment: Text.AlignHCenter
                }

                Rectangle {
                    id: inviteReadyPanel

                    Layout.fillWidth: true
                    visible: setupScroll.peopleAccessTab === "invites"
                        && setupScroll.keyTransferIsInvitePackage()
                        && !setupScroll.inviteReadyDismissed
                    implicitHeight: inviteReadyColumn.implicitHeight + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.1)
                    border.width: 1
                    border.color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.45)

                    ColumnLayout {
                        id: inviteReadyColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Text {
                                Layout.fillWidth: true
                                text: "Ready to share"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeSm
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            StatusChip {
                                visible: setupScroll.invitePackageRequiresApproval()
                                text: "Approval required"
                                description: "The recipient requests approval before joining"
                                warning: true
                                minWidth: 118
                                maxWidth: 130
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: setupScroll.invitePackageReadySummaryText()
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            text: setupScroll.invitePackageExpiryLabel()
                                + " · " + setupScroll.invitePackageApprovalLabel()
                                + (setupScroll.invitePackageIsClaimable()
                                    ? " · " + setupScroll.invitePackageClaimLimitLabel()
                                    : "")
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: setupScroll.invitePackageSyncExpectationMessage().length > 0
                            text: setupScroll.invitePackageSyncExpectationLabel()
                                + ". " + setupScroll.invitePackageSyncExpectationMessage()
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Button {
                                Layout.fillWidth: true
                                text: "Copy invite"
                                onClicked: app.copyKeyTransferArtifact("invite link")
                            }

                            Button {
                                id: readyInviteActionsButton
                                text: "⋯"
                                Layout.preferredWidth: 42
                                Accessible.name: "More invite actions"
                                onClicked: readyInviteActionsMenu.open()

                                Menu {
                                    id: readyInviteActionsMenu
                                    y: readyInviteActionsButton.height

                                    MenuItem {
                                        text: "Save invite file"
                                        onTriggered: app.openSaveKeyTransferDialog("invite")
                                    }

                                    MenuSeparator {}

                                    MenuItem {
                                        text: "Close"
                                        onTriggered: {
                                            chaftController.clearKeyTransferJson()
                                            setupScroll.inviteReadyDismissed = true
                                        }
                                    }
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Button {
                                Layout.fillWidth: true
                                visible: !setupScroll.invitePackageHasSyncSource()
                                    && !chaftController.peerHosting
                                    && !setupScroll.invitePackageIsClaimable()
                                text: "Make history available"
                                enabled: app.runtimeWorkReady
                                    && !chaftController.peerHostingInFlight
                                onClicked: chaftController.startLocalIrohPeer()
                            }

                            Button {
                                Layout.fillWidth: true
                                visible: !setupScroll.invitePackageHasSyncSource()
                                    && chaftController.peerHosting
                                    && !setupScroll.invitePackageIsClaimable()
                                text: "Add history to invite"
                                enabled: chaftController.hostedPeerEndpoint.length > 0
                                onClicked: chaftController.updateWorkspaceInvitePeerEndpoint(
                                    chaftController.hostedPeerEndpoint)
                            }

                            Button {
                                Layout.fillWidth: true
                                visible: setupScroll.invitePackageHasSyncSource()
                                text: "Copy history detail"
                                onClicked: app.copyTextToClipboard(
                                    setupScroll.invitePackagePeerEndpoint(),
                                    "history detail")
                            }
                        }
                    }
                }
            }

            SetupSection {
                id: advancedAccessSection
                Layout.fillWidth: true
                visible: setupScroll.category === "devices"
                title: setupScroll.addAnotherDeviceGuideVisible
                    ? "Access & Recovery"
                    : ""
                collapsible: false

                Rectangle {
                    id: addAnotherDeviceRecoveryPanel
                    Layout.fillWidth: true
                    visible: setupScroll.addAnotherDeviceGuideVisible
                    implicitHeight: addAnotherDeviceRecoveryLayout.implicitHeight
                        + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: Tokens.surfaceRaised
                    border.width: 1
                    border.color: Tokens.borderSubtle

                    Accessible.role: Accessible.Pane
                    Accessible.name: "Add another device"

                    ColumnLayout {
                        id: addAnotherDeviceRecoveryLayout
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space2

                        Text {
                            Layout.fillWidth: true
                            text: "Add another device"
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            text: "Create a recovery kit here, then open it in Chaft on the new device."
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            text: "Store the kit privately and keep its passphrase separate."
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            visible: false
                            implicitHeight: addAnotherDeviceSteps.implicitHeight + Tokens.space2 * 2
                            radius: Tokens.radiusSm
                            color: Qt.rgba(Tokens.textStrong.r, Tokens.textStrong.g, Tokens.textStrong.b, 0.04)
                            border.width: 1
                            border.color: Tokens.borderSubtle

                            ColumnLayout {
                                id: addAnotherDeviceSteps
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: Tokens.space2
                                spacing: Tokens.space1

                                Repeater {
                                    model: [
                                        "1. Save the recovery kit file.",
                                        "2. Open the kit in Chaft on the new device.",
                                        "3. Keep the kit private and never share it as an invite."
                                    ]

                                    delegate: Text {
                                        required property string modelData

                                        Layout.fillWidth: true
                                        text: modelData
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }
                                }
                            }
                        }

                        LabeledField {
                            id: addDeviceRecoveryPassphraseField
                            Layout.fillWidth: true
                            visible: !(chaftController.keyTransferJson.length > 0
                                && setupScroll.keyTransferIsRecoveryBundle())
                            label: "Kit passphrase"
                            placeholderText: "Choose a passphrase"
                            echoMode: TextInput.Password
                            requiredField: true
                            maximumLength: 1024
                            supportText: "Keep this passphrase separate from the recovery kit."
                            onAccepted: setupScroll.createAddAnotherDeviceRecoveryKit()
                        }

                        LabeledField {
                            id: addDeviceRecoveryPassphraseConfirmationField
                            Layout.fillWidth: true
                            visible: addDeviceRecoveryPassphraseField.visible
                            label: "Confirm passphrase"
                            placeholderText: "Enter the passphrase again"
                            echoMode: TextInput.Password
                            requiredField: true
                            maximumLength: 1024
                            errorText: text.length > 0
                                && text !== addDeviceRecoveryPassphraseField.text
                                    ? "Passphrases do not match."
                                    : ""
                            supportText: "Enter the same passphrase again."
                            onAccepted: setupScroll.createAddAnotherDeviceRecoveryKit()
                        }

                        Text {
                            Layout.fillWidth: true
                            text: chaftController.keyTransferInFlight
                                ? "Creating recovery kit..."
                                : (chaftController.keyTransferJson.length > 0
                                    && setupScroll.keyTransferIsRecoveryBundle()
                                    ? "Recovery kit ready. Save it somewhere only you can access."
                                    : "Choose a passphrase only you know.")
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space2

                            Button {
                                Layout.fillWidth: true
                                text: "Save recovery kit"
                                enabled: app && app.runtimeWorkReady
                                    && !setupScroll.hasUnreturnedInviteResponse()
                                    && !chaftController.keyTransferInFlight
                                    && ((chaftController.keyTransferJson.length > 0
                                            && setupScroll.keyTransferIsRecoveryBundle())
                                        || (addDeviceRecoveryPassphraseField.text.trim().length > 0
                                            && addDeviceRecoveryPassphraseConfirmationField.text
                                                === addDeviceRecoveryPassphraseField.text))
                                onClicked: setupScroll.createAddAnotherDeviceRecoveryKit()
                            }

                            Button {
                                Layout.preferredWidth: 72
                                text: "Cancel"
                                onClicked: {
                                    addDeviceRecoveryPassphraseField.text = ""
                                    addDeviceRecoveryPassphraseConfirmationField.text = ""
                                    setupScroll.addAnotherDeviceGuideVisible = false
                                }
                            }
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && chaftController.hasRuntimeWorkspace
                    spacing: Tokens.space2

                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: setupScroll.advancedAccessToolsOpen
                        spacing: Tokens.space1

                        Text {
                            Layout.fillWidth: true
                            text: "Access file type"
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 42
                            radius: Tokens.radiusMd
                            color: Tokens.sidebarInput
                            border.color: Tokens.borderSubtle

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: Tokens.space2
                                anchors.rightMargin: Tokens.space2
                                spacing: Tokens.space2

                                Text {
                                    Layout.fillWidth: true
                                    text: "Standard access file"
                                    color: Tokens.textStrong
                                    font.pixelSize: Tokens.fontSizeSm
                                    elide: Text.ElideRight
                                }

                                Text {
                                    text: "Support"
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    font.weight: Font.DemiBold
                                }
                            }
                        }
                    }

                    Button {
                        Layout.fillWidth: true
                        text: setupScroll.advancedAccessToolsOpen
                            ? "Hide advanced access tools"
                            : "Advanced access tools"
                        onClicked: setupScroll.advancedAccessToolsOpen =
                            !setupScroll.advancedAccessToolsOpen
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: setupScroll.advancedAccessToolsOpen
                        text: "Share access file"
                        enabled: app.runtimeWorkReady
                        onClicked: {
                            keyPackageDialog.pendingProtocol =
                                setupScroll.standardAccessUpdateProtocol
                            keyPackageDialog.open()
                        }
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: share an access file"
                    }
                }

                GridLayout {
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && chaftController.hasRuntimeWorkspace
                        && setupScroll.advancedAccessToolsOpen
                    columns: 1
                    columnSpacing: Tokens.space2
                    rowSpacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Share my access details"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.publishOpenMlsDeviceKeyPackage()
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: share this device's access details"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Set up workspace access"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.createOpenMlsWorkspaceGroup()
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: set up workspace access"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Accept workspace access"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.joinOpenMlsWorkspaceGroup("")
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: accept workspace access"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Apply workspace access"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.applyOpenMlsWorkspaceGroupCommits("")
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: apply pending workspace access changes"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Refresh workspace access"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.updateOpenMlsWorkspaceGroup()
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: update workspace access"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Refresh all access"
                        enabled: app.runtimeWorkReady
                        onClicked: chaftController.updateWorkspaceOpenMlsGroups()
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: update workspace and room access"
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "Set up room access"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.createOpenMlsChannelGroup(app.selectedChannelKey)
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: set up this private room's access"
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "Accept room access"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.joinOpenMlsChannelGroup(app.selectedChannelKey, "")
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: accept this private room's access"
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "Apply room access"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.applyOpenMlsChannelGroupCommits(app.selectedChannelKey, "")
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: apply pending room access changes"
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: app.selectedChannelPrivate
                        text: "Refresh room access"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                        onClicked: chaftController.updateOpenMlsChannelGroup(app.selectedChannelKey)
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: update this private room's access"
                    }
                }

                Text {
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                    text: "Paste support item"
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }

                TextArea {
                    id: workspaceKeyField
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                    Layout.preferredHeight: 72
                    placeholderText: "Invite, recovery kit, or access file"
                    Accessible.name: "Paste support item"
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
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                        && chaftController.keyTransferJson.length > 0
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Copy " + setupScroll.keyTransferCopyLabel()
                        onClicked: app.copyKeyTransferArtifact(
                            setupScroll.keyTransferCopyLabel())
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Save " + setupScroll.keyTransferCopyLabel()
                        onClicked: app.openSaveKeyTransferDialog(
                            setupScroll.keyTransferCopyLabel())
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: setupScroll.keyTransferIsWorkspaceKey()
                        text: "Prepare invite"
                        enabled: app.runtimeWorkReady
                            && app.canManageWorkspaceAccess()
                            && setupScroll.inviteDeviceCodeFromText(inviteDeviceField.text).length > 0
                        ToolTip.visible: hovered && !enabled
                        ToolTip.text: setupScroll.inviteCreateUnavailableReason()
                        onClicked: chaftController.stageWorkspaceInvitePackage(
                            setupScroll.inviteDeviceCodeFromText(inviteDeviceField.text),
                            setupScroll.inviteRoleValue(),
                            app.preferredInvitePeerEndpoint(),
                            setupScroll.inviteDisplayNameFromText(inviteDeviceField.text),
                            app.inviteExpiresAtIso(setupScroll.inviteExpiryDays()),
                            "preapproved")
                    }
                }

                LabeledField {
                    id: recoveryPassphraseField
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                    label: "Recovery passphrase"
                    echoMode: TextInput.Password
                    maximumLength: 1024
                    supportText: "Used to save or restore a recovery kit."
                }

                LabeledField {
                    id: recoveryPassphraseConfirmationField
                    Layout.fillWidth: true
                    visible: recoveryPassphraseField.visible
                        && chaftController.hasRuntimeWorkspace
                    label: "Confirm passphrase to save"
                    echoMode: TextInput.Password
                    maximumLength: 1024
                    errorText: text.length > 0
                        && text !== recoveryPassphraseField.text
                            ? "Passphrases do not match."
                            : ""
                    supportText: "Required only when creating a recovery kit."
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        visible: chaftController.hasRuntimeWorkspace
                        text: "Save access file"
                        enabled: app.runtimeWorkReady
                            && !setupScroll.hasUnreturnedInviteResponse()
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.exportWorkspaceKey()
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Apply access file"
                        enabled: workspaceKeyField.text.trim().length > 0
                            && app.runtimeAccessReady
                            && !chaftController.rawEventStoreMode
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.importWorkspaceKey(workspaceKeyField.text)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        visible: chaftController.hasRuntimeWorkspace
                        text: "Save recovery kit"
                        enabled: recoveryPassphraseField.text.trim().length > 0
                            && recoveryPassphraseConfirmationField.text
                                === recoveryPassphraseField.text
                            && app.runtimeWorkReady
                            && !setupScroll.hasUnreturnedInviteResponse()
                            && !chaftController.keyTransferInFlight
                        onClicked: {
                            if (chaftController.exportRecoveryBundle(
                                        recoveryPassphraseField.text)) {
                                recoveryPassphraseField.text = ""
                                recoveryPassphraseConfirmationField.text = ""
                            }
                        }
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Restore recovery kit"
                        enabled: workspaceKeyField.text.trim().length > 0
                            && recoveryPassphraseField.text.trim().length > 0
                            && app.runtimeAccessReady
                            && !chaftController.rawEventStoreMode
                            && !chaftController.keyTransferInFlight
                        onClicked: {
                            if (chaftController.importRecoveryBundle(
                                        workspaceKeyField.text,
                                        recoveryPassphraseField.text)) {
                                recoveryPassphraseField.text = ""
                                recoveryPassphraseConfirmationField.text = ""
                            }
                        }
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                        && chaftController.hasRuntimeWorkspace
                    text: "Export access record"
                    enabled: app.runtimeWorkReady
                        && !setupScroll.hasUnreturnedInviteResponse()
                        && !chaftController.keyTransferInFlight
                    onClicked: chaftController.exportTrustSnapshot()
                    ToolTip.visible: hovered
                    ToolTip.text: "Advanced: export the workspace access record"
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: !setupScroll.addAnotherDeviceGuideVisible
                        && setupScroll.advancedAccessToolsOpen
                        && chaftController.hasRuntimeWorkspace
                        && app.selectedChannelPrivate
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Save room access"
                        enabled: app.runtimeWorkReady
                            && app.selectedChannelKey.length > 0
                            && !setupScroll.hasUnreturnedInviteResponse()
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.exportChannelKey(app.selectedChannelKey)
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: export this private room's access"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Apply room access"
                        enabled: workspaceKeyField.text.trim().length > 0
                            && app.runtimeWorkReady
                            && !chaftController.rawEventStoreMode
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.importChannelKey(workspaceKeyField.text)
                        ToolTip.visible: hovered
                        ToolTip.text: "Advanced: import this private room's access"
                    }
                }
            }

            SetupSection {
                id: hostingBackupSection
                Layout.fillWidth: true
                visible: setupScroll.category === "backup"
                    && chaftController.hasRuntimeWorkspace
                title: "Backup destinations"
                collapsible: false
                badgeText: (chaftController.backupPeerEndpoints || []).length > 0
                    ? String((chaftController.backupPeerEndpoints || []).length)
                    : ""

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    LabeledField {
                        id: backupPeerField
                        Layout.fillWidth: true
                        label: "Teammate address"
                        placeholderText: "Paste teammate address"
                        onAccepted: {
                            if (chaftController.addBackupPeerEndpoint(text)) {
                                text = ""
                            }
                        }
                    }

                    Button {
                        Layout.alignment: Qt.AlignBottom
                        text: "Add"
                        Accessible.name: "Save backup address"
                        enabled: backupPeerField.text.trim().length > 0
                        onClicked: {
                            if (chaftController.addBackupPeerEndpoint(backupPeerField.text)) {
                                backupPeerField.text = ""
                            }
                        }
                    }

                    CheckBox {
                        Layout.alignment: Qt.AlignBottom
                        text: "Automatic"
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
                            onCopyRequested: function (endpoint) {
                                app.copyTextToClipboard(endpoint, "backup address")
                            }
                            onRemoveRequested: function (endpoint) {
                                chaftController.removeBackupPeerEndpoint(endpoint)
                            }
                        }
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: false
                    text: "Refresh search"
                    enabled: app.runtimeWorkReady
                        && !chaftController.keyTransferInFlight
                    onClicked: chaftController.reindexWorkspaceSearch()
                    ToolTip.visible: hovered
                    ToolTip.text: "Refresh local message search"
                }
            }

            SetupSection {
                id: securityToolsSection
                Layout.fillWidth: true
                visible: setupScroll.category === "advanced"
                    && chaftController.hasRuntimeWorkspace
                title: "Safety tools"
                collapsible: false
                danger: true

                Text {
                    Layout.fillWidth: true
                    text: "Use these tools only when access looks wrong or someone should no longer read future messages."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: securitySummaryLayout.implicitHeight + Tokens.space3 * 2
                    radius: Tokens.radiusSm
                    color: app && (app.storageHealthHasIssue || app.backupDeviceReviewCount() > 0)
                        ? Tokens.warningSurface
                        : Tokens.secureSurface
                    border.width: 1
                    border.color: app && (app.storageHealthHasIssue || app.backupDeviceReviewCount() > 0)
                        ? Tokens.warning
                        : Tokens.secure

                    Accessible.role: Accessible.StaticText
                    Accessible.name: app
                        ? app.safetyChatStatusText() + " " + app.safetyNextActionText()
                        : "Safety status unavailable."

                    ColumnLayout {
                        id: securitySummaryLayout
                        anchors.fill: parent
                        anchors.margins: Tokens.space3
                        spacing: Tokens.space1

                        Text {
                            Layout.fillWidth: true
                            text: "Can I keep chatting?"
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeXs
                            font.weight: Font.DemiBold
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            text: app ? app.safetyChatStatusText() : "Safety status unavailable."
                            color: app && (app.storageHealthHasIssue || app.backupDeviceReviewCount() > 0)
                                ? Tokens.warningText
                                : Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            text: app ? app.safetyNextActionText() : ""
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }

                        Text {
                            Layout.fillWidth: true
                            text: app ? app.safetySupportDetailText() : ""
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                            maximumLineCount: 3
                            elide: Text.ElideRight
                        }

                        Button {
                            Layout.alignment: Qt.AlignRight
                            text: "Copy detail"
                            enabled: app && app.runtimeAccessReady
                            Accessible.name: "Copy safety support detail"
                            onClicked: app.copyTextToClipboard(
                                app.safetySupportCopyText(),
                                "support detail")
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Button {
                        Layout.fillWidth: true
                        text: "Check access"
                        enabled: app.runtimeWorkReady
                            && !chaftController.keyTransferInFlight
                        onClicked: chaftController.detectCompromise()
                        ToolTip.visible: hovered
                        ToolTip.text: "Check for suspicious access changes"
                    }

                    Button {
                        Layout.fillWidth: true
                        text: "Protect future messages"
                        enabled: app.runtimeWorkReady
                            && !chaftController.keyTransferInFlight
                        onClicked: app.confirmSetupAction(
                            "Protect future messages",
                            "Protect future messages with fresh access now? People "
                                + "who miss the change may need help receiving updates before they can read new messages.",
                            "Refresh access",
                            "rotate-keys")
                        ToolTip.visible: hovered
                        ToolTip.text: "Protect future messages with fresh access"
                    }
                }

                Button {
                    Layout.fillWidth: true
                    visible: app.selectedChannelPrivate
                    text: "Protect this room"
                    enabled: app.runtimeWorkReady
                        && app.selectedChannelKey.length > 0
                        && !chaftController.keyTransferInFlight
                    onClicked: app.confirmSetupAction(
                        "Protect this room",
                        "Refresh access for this private room? People without the "
                            + "update cannot read new room messages.",
                        "Refresh access",
                        "rotate-channel-key:" + app.selectedChannelKey)
                    ToolTip.visible: hovered
                    ToolTip.text: "Protect future messages in this private room"
                }
            }

            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: Tokens.space4 * 6
            }
        }
    }
}
