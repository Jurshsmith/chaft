#!/usr/bin/env python3
"""Regression contracts for Chaft's invite and workspace-entry forms.

The desktop app does not currently ship a QML unit-test target. These checks
therefore protect the cross-file UI contracts that are otherwise easy to
break while the Rust integration tests cover the cryptographic behavior.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
APP_QML = REPO_ROOT / "apps/desktop-qt/qml/Chaft/App.qml"
MAIN_CPP = REPO_ROOT / "apps/desktop-qt/src/main.cpp"
TYPES_RS = REPO_ROOT / "crates/chaft-types/src/lib.rs"
ENTRY_QML = (
    REPO_ROOT
    / "apps/desktop-qt/qml/Chaft/features/onboarding/WorkspaceEntryDialog.qml"
)
INVITE_QML = (
    REPO_ROOT / "apps/desktop-qt/qml/Chaft/features/setup/InvitePeopleDialog.qml"
)
REVIEW_QML = (
    REPO_ROOT
    / "apps/desktop-qt/qml/Chaft/features/setup/ReviewAccessRequestDialog.qml"
)
SETUP_QML = REPO_ROOT / "apps/desktop-qt/qml/Chaft/features/setup/SetupPanel.qml"
FIELD_QML = (
    REPO_ROOT / "apps/desktop-qt/qml/Chaft/components/controls/LabeledField.qml"
)
CHECKBOX_QML = REPO_ROOT / "apps/desktop-qt/qml/ChaftStyle/CheckBox.qml"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def balanced_block(source: str, opening_brace: int) -> str:
    """Return one JS/QML brace-delimited block, skipping strings/comments."""
    if opening_brace >= len(source) or source[opening_brace] != "{":
        raise AssertionError("balanced_block must start at an opening brace")

    depth = 0
    index = opening_brace
    quote: str | None = None
    escaped = False
    line_comment = False
    block_comment = False
    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""

        if line_comment:
            if char == "\n":
                line_comment = False
        elif block_comment:
            if char == "*" and next_char == "/":
                block_comment = False
                index += 1
        elif quote is not None:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
        elif char == "/" and next_char == "/":
            line_comment = True
            index += 1
        elif char == "/" and next_char == "*":
            block_comment = True
            index += 1
        elif char in {'"', "'", "`"}:
            quote = char
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[opening_brace : index + 1]
        index += 1

    raise AssertionError("unterminated QML/JavaScript block")


def function_body(source: str, name: str) -> str:
    match = re.search(rf"\bfunction\s+{re.escape(name)}\s*\([^)]*\)\s*\{{", source)
    if match is None:
        raise AssertionError(f"missing function {name}()")
    opening_brace = source.find("{", match.start())
    return balanced_block(source, opening_brace)


def cpp_void_function_body(source: str, name: str) -> str:
    match = re.search(rf"\bvoid\s+{re.escape(name)}\s*\([^)]*\)\s*\{{", source)
    if match is None:
        raise AssertionError(f"missing C++ function {name}()")
    opening_brace = source.find("{", match.start())
    return balanced_block(source, opening_brace)


def cpp_bool_function_body(source: str, name: str) -> str:
    match = re.search(rf"\bbool\s+{re.escape(name)}\s*\([^)]*\)\s*\{{", source)
    if match is None:
        raise AssertionError(f"missing C++ function {name}()")
    opening_brace = source.find("{", match.start())
    return balanced_block(source, opening_brace)


def object_block(source: str, object_type: str, object_id: str) -> str:
    for match in re.finditer(rf"\b{re.escape(object_type)}\s*\{{", source):
        opening_brace = source.find("{", match.start())
        block = balanced_block(source, opening_brace)
        if re.search(rf"\bid\s*:\s*{re.escape(object_id)}\b", block):
            return block
    raise AssertionError(f"missing {object_type} with id {object_id}")


class InviteFormContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = read(APP_QML)
        cls.main_cpp = read(MAIN_CPP)
        cls.types_rs = read(TYPES_RS)
        cls.entry = read(ENTRY_QML)
        cls.invite = read(INVITE_QML)
        cls.review = read(REVIEW_QML)
        cls.setup = read(SETUP_QML)
        cls.field = read(FIELD_QML)
        cls.checkbox = read(CHECKBOX_QML)

    def test_inviter_enters_an_invite_label_not_the_joiners_name(self) -> None:
        self.assertIn('label: "Invite label (optional)"', self.invite)
        self.assertIn('placeholderText: "e.g. Design team"', self.invite)
        self.assertIn("Each joiner chooses their own name.", self.invite)
        self.assertNotIn('label: "Name or label (optional)"', self.invite)
        self.assertNotIn('placeholderText: "e.g. Sam Rivera"', self.invite)

    def test_desktop_and_runtime_share_the_100_join_invite_limit(self) -> None:
        rust_limit = re.search(
            r"\bWORKSPACE_INVITE_MAX_CLAIMS\s*:\s*u32\s*=\s*(\d+)",
            self.types_rs,
        )
        cpp_limit = re.search(
            r"\bkMaxWorkspaceInviteClaims\s*=\s*(\d+)", self.main_cpp
        )
        self.assertIsNotNone(rust_limit)
        self.assertIsNotNone(cpp_limit)
        self.assertEqual(int(rust_limit.group(1)), 100)
        self.assertEqual(
            int(cpp_limit.group(1)),
            int(rust_limit.group(1)),
            "desktop validation must not drift from the signed invite limit",
        )
        self.assertIn(
            "an invite can allow between 1 and 100 joins",
            self.main_cpp,
        )

    def test_invite_type_control_supports_bounded_group_reuse(self) -> None:
        for visible_copy in (
            'text: "Single-use"',
            'text: "Group"',
            'text: "Maximum joins"',
            "Each device uses one join.",
        ):
            self.assertIn(visible_copy, self.invite)

        self.assertIn('property string inviteMode: "single"', self.invite)
        self.assertIn(
            'readonly property bool groupInvite: root.inviteMode === "group"',
            self.invite,
        )
        self.assertIn('root.inviteMode = "single"', self.invite)
        self.assertIn('root.inviteMode = "group"', self.invite)
        self.assertIn("id: inviteTypeButtonGroup", self.invite)
        self.assertEqual(
            self.invite.count("ButtonGroup.group: inviteTypeButtonGroup"),
            2,
        )
        self.assertIn("Keys.onRightPressed", self.invite)
        self.assertIn("Keys.onLeftPressed", self.invite)
        self.assertRegex(
            self.invite,
            r"Keys\.onRightPressed\s*:\s*\{"
            r"(?=[\s\S]{0,180}root\.inviteMode\s*=\s*\"group\")"
            r"(?=[\s\S]{0,180}highRiskConfirmation\.checked\s*=\s*false)",
        )
        self.assertRegex(
            self.invite,
            r"Keys\.onLeftPressed\s*:\s*\{"
            r"(?=[\s\S]{0,180}root\.inviteMode\s*=\s*\"single\")"
            r"(?=[\s\S]{0,180}highRiskConfirmation\.checked\s*=\s*false)",
        )

        for preset in (5, 10, 20, 25):
            self.assertIn(
                f'{{ label: "{preset}", value: {preset} }}',
                self.invite,
            )
        self.assertIn('{ label: "Custom", value: 0 }', self.invite)

        group_limits = object_block(
            self.invite, "RowLayout", "groupClaimLimitChoices"
        )
        self.assertIn("model: root.groupClaimLimitOptions", group_limits)
        self.assertIn("delegate: Button", group_limits)
        self.assertIn("checked: root.selectedGroupClaimLimit", group_limits)
        self.assertIn("root.selectGroupClaimLimit(", group_limits)

        custom_limit = object_block(
            self.invite, "TextField", "customClaimLimitField"
        )
        self.assertIn("visible: root.customClaimLimit", custom_limit)
        self.assertIn("validator: IntValidator", custom_limit)
        self.assertIn("bottom: 2", custom_limit)
        self.assertIn("top: 100", custom_limit)
        self.assertIn("root.customClaimLimitValid", custom_limit)

        selected_limit_match = re.search(
            r"readonly\s+property\s+int\s+selectedMaxClaims\s*:\s*\{",
            self.invite,
        )
        self.assertIsNotNone(selected_limit_match)
        selected_limit = balanced_block(
            self.invite,
            self.invite.find("{", selected_limit_match.start()),
        )
        self.assertIn("if (!root.groupInvite)", selected_limit)
        self.assertRegex(selected_limit, r"\breturn\s+1\b")
        self.assertIn("root.customClaimLimitValid", selected_limit)
        self.assertIn("Number(customClaimLimitField.text)", selected_limit)
        self.assertIn("root.selectedGroupClaimLimit", selected_limit)

        self.assertRegex(
            self.invite,
            r"ColumnLayout\s*\{"
            r"(?=[\s\S]{0,240}visible\s*:\s*root\.groupInvite)"
            r"(?=[\s\S]{0,400}text\s*:\s*\"Maximum joins\")",
        )
        self.assertIn("&& root.customClaimLimitValid", self.invite)
        self.assertIn("readonly property bool formEditable:", self.invite)
        self.assertGreaterEqual(
            self.invite.count("enabled: root.formEditable"),
            7,
            "all invite inputs and the risk acknowledgement must freeze during creation",
        )
        self.assertIn("readonly property bool reusableNeverExpires:", self.invite)
        self.assertIn("readonly property bool expandedCapacityInvite:", self.invite)
        self.assertIn(
            "Anyone with it can use a remaining join until you revoke it.",
            self.invite,
        )
        self.assertIn(
            "Invite limits above 20 require every workspace device to be updated first.",
            self.invite,
        )

    def test_admin_invite_risk_acknowledgement_wraps_without_eliding(self) -> None:
        acknowledgement = object_block(
            self.invite, "CheckBox", "highRiskConfirmation"
        )
        self.assertIn("wrapText: true", acknowledgement)
        self.assertIn("property bool wrapText: false", self.checkbox)
        self.assertIn(
            "wrapMode: control.wrapText ? Text.WordWrap : Text.NoWrap",
            self.checkbox,
        )
        self.assertIn(
            "elide: control.wrapText ? Text.ElideNone : Text.ElideRight",
            self.checkbox,
        )

    def test_joiner_identity_is_contextual(self) -> None:
        identity_contract = re.compile(
            r"readonly\s+property\s+bool\s+joinIdentityVisible\s*:"
            r"(?=[\s\S]{0,400}!root\.createMode)"
            r"(?=[\s\S]{0,400}!root\.restoreMode)"
            r"(?=[\s\S]{0,400}root\.credentialSummaryVisible)"
            r"(?=[\s\S]{0,400}!root\.receivedApprovalCredential)",
        )
        self.assertRegex(
            self.entry,
            identity_contract,
            "joiner identity must only appear after a recognized credential and must "
            "stay hidden for Create, Restore, and received approvals",
        )

        field = object_block(self.entry, "LabeledField", "displayNameField")
        self.assertIn("visible: root.displayNameEditorVisible", field)
        self.assertIn('text: "Joining as " + displayNameField.text.trim()', self.entry)
        self.assertIn('text: "Change"', self.entry)

    def test_received_approval_preserves_the_name_from_its_request(self) -> None:
        open_approval = function_body(self.app, "openReceivedApprovalInvite")
        self.assertIn("pendingAccessRequestRowByRequestId(responseRequestId)", open_approval)
        self.assertRegex(
            open_approval,
            r"beginReceivedApproval\s*\(\s*pendingDisplayName\s*,\s*responseRequestId\s*\)",
        )
        self.assertNotIn(
            "resetForm(",
            open_approval,
            "receiving an approval must not clear the identity captured in the request",
        )

        begin_approval = function_body(self.entry, "beginReceivedApproval")
        self.assertIn("displayNameField.text = preservedName", begin_approval)
        self.assertIn("root.joinRequestPreparedRequestId", begin_approval)

        submit_join = function_body(self.app, "submitWorkspaceJoin")
        self.assertIn("pendingAccessRequestRowByRequestId(", submit_join)
        self.assertIn("entryDisplayName = requestedDisplayName", submit_join)
        self.assertIn("pendingWorkspaceImportRequestId = responseRequestId", submit_join)

    def test_pending_requests_and_callbacks_are_correlated_by_request_id(self) -> None:
        record = function_body(self.app, "recordPendingAccessRequestFromArtifact")
        self.assertRegex(
            record,
            r"var\s+key\s*=\s*requestId\.length\s*>\s*0\s*\?\s*requestId\s*:\s*workspaceId",
            "a second request for one workspace must not overwrite the first request",
        )

        complete_import = function_body(
            self.app, "handleWorkspaceCredentialImportFinished"
        )
        self.assertIn("clearPendingAccessRequestForRequestId(requestId)", complete_import)

        direct_submit_callback = function_body(
            self.entry, "onJoinRequestDirectSubmitCompleted"
        )
        self.assertIn("joinRequestPreparedRequestId", direct_submit_callback)
        self.assertRegex(
            direct_submit_callback,
            r"String\(requestId\s*\|\|\s*\"\"\)\.trim\(\)",
        )

    def test_inbox_acknowledgement_is_correlated_to_the_imported_artifact(self) -> None:
        submit_join = function_body(self.app, "submitWorkspaceJoin")
        self.assertIn("pendingWorkspaceImportInboxArtifact", submit_join)
        self.assertRegex(
            submit_join,
            r"stagedInboxArtifact\s*===\s*credentials",
            "only a credential copied from the staged inbox may arm acknowledgement",
        )

        complete_import = function_body(
            self.app, "handleWorkspaceCredentialImportFinished"
        )
        for guard in (
            "inboxArtifact.length > 0",
            "chaftController.keyTransferFromJoinResponseInbox",
            "=== inboxArtifact",
        ):
            self.assertIn(guard, complete_import)
        self.assertIn(
            "chaftController.acknowledgeCurrentJoinResponseInboxEntry()",
            complete_import,
        )

    def test_create_identity_waits_for_the_created_workspace_id(self) -> None:
        submit_create = function_body(self.app, "submitWorkspaceCreate")
        self.assertIn("pendingWorkspaceCreateDisplayName", submit_create)
        self.assertNotIn("pendingEntryDisplayNameWorkspaceId", submit_create)

        complete_create = function_body(self.app, "handleWorkspaceCreateFinished")
        self.assertIn(
            "pendingEntryDisplayNameWorkspaceId = createdWorkspaceId",
            complete_create,
        )
        self.assertIn(
            "pendingPostCreateWorkspaceId = createdWorkspaceId",
            complete_create,
        )
        self.assertIn("selected === true", complete_create)

    def test_joiner_name_waits_for_membership_and_profile_confirmation(self) -> None:
        apply_name = function_body(self.app, "applyPendingEntryDisplayName")
        update_call = apply_name.index(
            "chaftController.updateDeviceProfile(displayName)"
        )
        self.assertLess(apply_name.index("localDeviceMembershipReady()"), update_call)
        self.assertLess(
            apply_name.index("pendingEntryDisplayNameUpdateInFlight"), update_call
        )
        self.assertIn("pendingEntryDisplayNameConfirmed()", apply_name)
        self.assertNotIn(
            'pendingEntryDisplayName = ""',
            apply_name[update_call:],
            "accepting the async write must not clear the pending joiner name",
        )

        completion = function_body(
            self.app, "handleDeviceProfileUpdateFinished"
        )
        confirmation = completion.index("pendingEntryDisplayNameConfirmed()")
        clear = completion.index("clearPendingEntryDisplayName()", confirmation)
        self.assertLess(confirmation, clear)
        self.assertIn("schedulePendingEntryDisplayNameRetry()", completion)
        self.assertIn("function onDeviceProfileUpdateFinished(", self.app)

        confirmation = function_body(self.app, "pendingEntryDisplayNameConfirmed")
        self.assertIn("localDeviceDisplayName()", confirmation)
        self.assertIn("localLinkedPersonDisplayName()", confirmation)

    def test_joiner_name_lifecycle_is_durable_and_never_rewrites_after_success(
        self,
    ) -> None:
        restore = function_body(
            self.app, "pendingEntryDisplayNameRequestForCurrentWorkspace"
        )
        self.assertIn('status !== "profile_pending"', restore)
        self.assertIn('status !== "profile_written"', restore)
        self.assertIn('writeSucceeded: status === "profile_written"', restore)

        persist = function_body(self.app, "persistPendingEntryDisplayNameState")
        self.assertIn('row.sourceType = "profile_finalization"', persist)
        self.assertIn('kind: "chaft.pending-profile.v1"', persist)
        self.assertNotIn("workspaceKey", persist)
        self.assertNotIn("keyTransferJson", persist)

        complete_import = function_body(
            self.app, "handleWorkspaceCredentialImportFinished"
        )
        self.assertIn(
            'persistPendingEntryDisplayNameState("profile_pending")',
            complete_import,
        )

        completion = function_body(self.app, "handleDeviceProfileUpdateFinished")
        self.assertIn(
            'persistPendingEntryDisplayNameState("profile_written")', completion
        )
        self.assertIn("schedulePendingEntryDisplayNameReconciliation()", completion)
        self.assertNotIn(
            "updateDeviceProfile(",
            completion,
            "a successful append may only reconcile; it must never append again",
        )

        reconcile = function_body(self.app, "reconcilePendingEntryDisplayName")
        self.assertIn("reconcileRuntimeSnapshotIfIdle()", reconcile)
        self.assertNotIn("updateDeviceProfile(", reconcile)

        request_rows = function_body(self.app, "pendingAccessRequestRows")
        for lifecycle_status in ("profile_pending", "profile_written"):
            self.assertIn(f'persistedStatus === "{lifecycle_status}"', request_rows)
        self.assertIn("continue", request_rows)

        signature = "QStringList pendingJoinResponseRequestIds("
        function_start = self.main_cpp.index(signature)
        function_suffix = self.main_cpp.index(") const {", function_start)
        opening_brace = self.main_cpp.index("{", function_suffix)
        response_poll = balanced_block(self.main_cpp, opening_brace)
        for lifecycle_status in ("profile_pending", "profile_written"):
            self.assertIn(
                f'status == QStringLiteral("{lifecycle_status}")', response_poll
            )

    def test_joiner_avatar_tracks_the_durable_profile_lifecycle(self) -> None:
        confirmation = function_body(self.app, "pendingEntryDisplayNameConfirmed")
        self.assertIn("pendingEntryAvatarId", confirmation)
        self.assertIn("localDeviceAvatarId()", confirmation)
        self.assertIn("localLinkedPersonAvatarId()", confirmation)

        persist = function_body(self.app, "persistPendingEntryDisplayNameState")
        self.assertIn("row.avatarId = avatarId", persist)

        restore = function_body(
            self.app, "restorePendingEntryDisplayNameFromRequests"
        )
        self.assertIn("pendingEntryAvatarId = avatarId", restore)

        apply_profile = function_body(self.app, "applyPendingEntryDisplayName")
        self.assertIn("updateDeviceProfileWithAvatar", apply_profile)
        self.assertIn("pendingEntryDisplayNameUpdateAvatarId", apply_profile)

        submit_join = function_body(self.app, "submitWorkspaceJoin")
        self.assertIn("requestedAvatarId", submit_join)
        self.assertIn("pendingEntryAvatarId = entryAvatarId", submit_join)

        pending_request = function_body(
            self.app, "recordPendingAccessRequestFromArtifact"
        )
        self.assertIn("avatarId:", pending_request)

        sanitizer_start = self.main_cpp.index(
            "QVariantMap sanitizedPendingJoinRequests("
        )
        sanitizer_suffix = self.main_cpp.index(") {", sanitizer_start)
        sanitizer = balanced_block(
            self.main_cpp, self.main_cpp.index("{", sanitizer_suffix)
        )
        self.assertIn('QStringLiteral("avatarId")', sanitizer)

    def test_message_refresh_does_not_depend_on_a_receiver_action(self) -> None:
        auto_sync = function_body(self.app, "syncSelectedPeerIfReady")
        reconcile_due = auto_sync.index("hostedRuntimeReconcileDue")
        network_endpoint = auto_sync.index("preferredSyncPeerEndpoint()")
        self.assertLess(reconcile_due, network_endpoint)

        hosted_timer = object_block(self.app, "Timer", "hostedRuntimeReconcileTimer")
        self.assertIn("reconcileHostedRuntimeIfReady()", hosted_timer)
        self.assertNotIn("syncWorkspace", hosted_timer)

        reconcile_start = self.main_cpp.index("void runRuntimeSnapshotReconcile(")
        reconcile_opening = self.main_cpp.index("{", reconcile_start)
        reconcile = balanced_block(self.main_cpp, reconcile_opening)
        self.assertIn("m_runtimeWriteGeneration != runtimeWriteGeneration", reconcile)
        self.assertIn("m_workspaceSnapshotRevision !=", reconcile)
        self.assertNotIn("m_lastAppliedRuntimeWriteGeneration =", reconcile)

        invoke_start = self.main_cpp.index("Q_INVOKABLE bool reconcileRuntimeSnapshotIfIdle()")
        invoke_opening = self.main_cpp.index("{", invoke_start)
        invoke = balanced_block(self.main_cpp, invoke_opening)
        self.assertNotIn("++m_runtimeWriteGeneration", invoke)

    def test_search_only_messages_keep_author_actions_and_avatars(self) -> None:
        apply_results = cpp_void_function_body(
            self.main_cpp, "applyWorkspaceSearchResults"
        )
        self.assertIn('QStringLiteral("authorAvatarId")', apply_results)
        self.assertIn(
            'row.value(QStringLiteral("authorDeviceId")).toString() == m_deviceId',
            apply_results,
        )
        self.assertIn('QStringLiteral("canEdit")', apply_results)
        self.assertIn('QStringLiteral("canDelete")', apply_results)

    def test_profile_completion_is_emitted_after_the_confirming_snapshot(self) -> None:
        self.assertIn("void deviceProfileUpdateFinished(", self.main_cpp)
        update = cpp_void_function_body(self.main_cpp, "runDeviceProfileUpdate")
        applied = update.rfind("guard->applyRuntimeSnapshot(snapshotValue, false)")
        completed = update.rfind("emit guard->deviceProfileUpdateFinished(")
        self.assertGreater(applied, -1)
        self.assertGreater(
            completed,
            applied,
            "the success completion must follow publication of the confirming snapshot",
        )
        self.assertIn(
            "guard->queueRuntimeSnapshotRefreshIfCurrent(",
            update,
            "a profile write whose snapshot failed must queue confirmation",
        )
        self.assertIn("workspaceStateMayHaveChanged", update)
        self.assertIn("deviceProfileAlreadyCurrent", update)
        self.assertIn("personProfileAlreadyCurrent", update)
        self.assertIn("if (!deviceProfileAlreadyCurrent)", update)
        self.assertIn("if (!personProfileAlreadyCurrent)", update)
        self.assertIn(
            "runtimeSnapshotHasDisplayNamePair(", update
        )
        self.assertIn(
            "workspaceId, displayName, profileComplete", update
        )
        self.assertGreaterEqual(
            update.count("latestRuntimeSnapshotValue("),
            2,
            "profile writes need authoritative snapshots before and after missing sub-writes",
        )

        workspace_operation_start = self.main_cpp.index(
            "bool workspaceOperationInFlight() const"
        )
        workspace_operation_opening = self.main_cpp.index(
            "{", workspace_operation_start
        )
        workspace_operation = balanced_block(
            self.main_cpp, workspace_operation_opening
        )
        self.assertIn("m_deviceProfileUpdateInFlight", workspace_operation)
        self.assertIn("finishDeviceProfileUpdate(operationId)", update)

    def test_workspace_transfer_actions_observe_the_composite_barrier(self) -> None:
        for function_name in (
            "publishWorkspace",
            "backupWorkspaceIfIdle",
            "backupConfiguredPeersIfIdle",
            "publishEventWithTrustSnapshot",
            "retryBlobTransfers",
            "startBackupWorkspace",
        ):
            function = cpp_bool_function_body(self.main_cpp, function_name)
            self.assertIn("workspaceOperationInFlight()", function)
            self.assertNotIn("syncInFlight()", function)

        for function_name in (
            "connectPeerEndpointFromField",
            "syncWorkspaceFromPreferredPeer",
            "publishWorkspaceToPreferredPeer",
            "backupWorkspaceToPreferredPeer",
            "pullWorkspaceFromPreferredPeer",
            "retryBlobTransfersWithPreferredPeers",
            "repairHistoryFromPeer",
            "publishEventWithTrustSnapshotToPreferredPeer",
        ):
            function = function_body(self.app, function_name)
            self.assertIn("workspaceOperationInFlight", function)

    def test_pending_joiner_name_retries_after_workspace_operations_settle(self) -> None:
        sync_handler = re.search(
            r"function\s+onSyncInFlightChanged\s*\(\)\s*\{[\s\S]*?\n\s*\}",
            self.app,
        )
        self.assertIsNotNone(sync_handler)
        self.assertIn(
            "root.applyPendingEntryDisplayName()", sync_handler.group(0)
        )

        timeline_handler = re.search(
            r"function\s+onTimelineLoadInFlightChanged\s*\(\)\s*\{[\s\S]*?\n\s*\}",
            self.app,
        )
        self.assertIsNotNone(timeline_handler)
        self.assertIn(
            "root.applyPendingEntryDisplayName()", timeline_handler.group(0)
        )

    def test_read_markers_wait_for_reconciliation_and_profile_writes(self) -> None:
        mark_read = function_body(self.app, "markSelectedChannelRead")
        dispatch = mark_read.index("chaftController.markChannelRead(channelId)")
        self.assertLess(mark_read.index("workspaceOperationInFlight"), dispatch)
        self.assertLess(
            mark_read.index("pendingEntryDisplayNameUpdateInFlight"), dispatch
        )
        self.assertLess(mark_read.index("timelineView.followLatest"), dispatch)
        self.assertIn("markReadDebounce.restart()", mark_read[:dispatch])

        for handler_name in (
            "onSyncInFlightChanged",
            "onTimelineLoadInFlightChanged",
            "onDeviceProfileUpdateFinished",
        ):
            handler = re.search(
                rf"function\s+{handler_name}\s*\([^)]*\)\s*\{{[\s\S]*?\n\s*\}}",
                self.app,
            )
            self.assertIsNotNone(handler)
            self.assertIn("root.scheduleMarkSelectedChannelRead()", handler.group(0))

    def test_workspace_creation_cannot_discard_a_received_approval(self) -> None:
        open_approval = function_body(self.app, "openReceivedApprovalInvite")
        self.assertIn("workspaceEntryDialog.createOperationPending", open_approval)

        complete_create = function_body(self.app, "handleWorkspaceCreateFinished")
        self.assertNotIn("clearKeyTransferJson", complete_create)
        self.assertIn("keyTransferFromJoinResponseInbox", complete_create)
        self.assertIn("openReceivedApprovalInvite(false)", complete_create)

    def test_invalid_address_edits_do_not_trap_live_updates_on(self) -> None:
        preferred_peer = function_body(self.app, "preferredSyncPeerEndpoint")
        self.assertIn("supportedPeerEndpointRouteKind(manualEndpoint)", preferred_peer)
        self.assertIn("chaftController.defaultPeerEndpoint", preferred_peer)

        checkbox = object_block(self.app, "CheckBox", "liveUpdatesCheckBox")
        self.assertIn("root.autoSyncEnabled", checkbox)
        self.assertIn("root.peerEndpointFormIsValid()", checkbox)
        self.assertIn("onToggled: root.autoSyncEnabled = checked", checkbox)

    def test_display_name_is_byte_validated_before_create_and_join(self) -> None:
        field = object_block(self.entry, "LabeledField", "displayNameField")
        self.assertIn("deviceDisplayNameValidationError(text)", field)
        self.assertIn("maximumLength: 128", field)

        submit_create = function_body(self.app, "submitWorkspaceCreate")
        submit_join = function_body(self.app, "submitWorkspaceJoin")
        self.assertIn("deviceDisplayNameValidationError(", submit_create)
        self.assertIn("deviceDisplayNameValidationError(entryDisplayName)", submit_join)

    def test_sent_requests_cannot_be_replaced_by_editing(self) -> None:
        can_edit = function_body(self.entry, "joinRequestCanEditDetails")
        self.assertIn('joinRequestPreparedAction === "prepared"', can_edit)
        self.assertIn('joinRequestPreparedAction === "save-failed"', can_edit)
        for immutable_state in ("sent", "sending", "copied", "save", "send-failed"):
            self.assertNotIn(
                f'joinRequestPreparedAction === "{immutable_state}"',
                can_edit,
                f"{immutable_state} requests must not be replaced by a newly signed request",
            )

        edit_request = function_body(self.entry, "editJoinRequest")
        self.assertIn("if (!root.joinRequestCanEditDetails())", edit_request)

    def test_recovery_passphrase_reaches_crypto_exactly_as_entered(self) -> None:
        submit_join = function_body(self.app, "submitWorkspaceJoin")
        self.assertRegex(
            submit_join,
            r"var\s+passphrase\s*=\s*workspaceEntryDialog\.recoveryPassphraseText\s*(?:\n|;)",
            "blank detection may trim, but the passphrase sent to crypto must not be trimmed",
        )
        self.assertNotRegex(
            submit_join,
            r"var\s+passphrase\s*=\s*workspaceEntryDialog\.recoveryPassphraseText\.trim\(",
        )
        self.assertIn("hasPassphrase = passphrase.trim().length > 0", submit_join)
        self.assertIn(
            "chaftController.importRecoveryBundle(credentialJson, passphrase)", submit_join
        )

        add_device = function_body(self.setup, "createAddAnotherDeviceRecoveryKit")
        self.assertIn(
            "addDeviceRecoveryPassphraseConfirmationField.text\n"
            "                !== addDeviceRecoveryPassphraseField.text",
            add_device,
        )
        self.assertIn(
            "exportRecoveryBundle(addDeviceRecoveryPassphraseField.text)", add_device
        )

    def test_shared_fields_expose_validation_and_dirty_state(self) -> None:
        for contract in (
            "property string supportText",
            "property string errorText",
            "property bool requiredField",
            "readonly property bool dirty",
            "function markClean()",
            "Accessible.description",
        ):
            self.assertIn(contract, self.field)

    def test_security_dialogs_do_not_discard_forms_on_outside_click(self) -> None:
        for path, source in (
            (ENTRY_QML, self.entry),
            (INVITE_QML, self.invite),
            (REVIEW_QML, self.review),
        ):
            self.assertNotIn(
                "Popup.CloseOnPressOutside",
                source,
                f"{path.name} must require an explicit action or Escape",
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
