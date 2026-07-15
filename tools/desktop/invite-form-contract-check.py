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
        cls.entry = read(ENTRY_QML)
        cls.invite = read(INVITE_QML)
        cls.review = read(REVIEW_QML)
        cls.setup = read(SETUP_QML)
        cls.field = read(FIELD_QML)

    def test_inviter_enters_an_invite_label_not_the_joiners_name(self) -> None:
        self.assertIn('label: "Invite label (optional)"', self.invite)
        self.assertIn('placeholderText: "e.g. Design team"', self.invite)
        self.assertIn("Each joiner chooses their own name.", self.invite)
        self.assertNotIn('label: "Name or label (optional)"', self.invite)
        self.assertNotIn('placeholderText: "e.g. Sam Rivera"', self.invite)

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
