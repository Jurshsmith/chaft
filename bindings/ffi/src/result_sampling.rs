use chaft_runtime::{
    AppliedOpenMlsChannelGroupCommits, AppliedOpenMlsWorkspaceGroupCommits, BlobTransferAttempt,
    BlobTransferRetryReport, ImportedWorkspaceRecoveryBundle, PrunedBlobCache, PublishedWorkspace,
    PulledOpenMlsCatchup, PulledWorkspace, RemovedMemberWithKeyRotation, RemovedMemberWithOpenMls,
    RotatedWorkspaceForSuspectedCompromise, RotatedWorkspaceManualKeys, SyncedWorkspace,
    UpdatedWorkspaceOpenMlsGroups, WorkspaceCompromiseReport, WorkspaceCompromiseResponse,
};

pub(crate) const MAX_RESULT_EVENT_ID_SAMPLE_ROWS: usize = 128;
pub(crate) const MAX_RESULT_ATTEMPT_ID_SAMPLE_ROWS: usize = 128;
pub(crate) const MAX_RESULT_WORKSPACE_ID_SAMPLE_ROWS: usize = 128;
pub(crate) const MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS: usize = 128;
pub(crate) const MAX_RESULT_CHANNEL_ID_SAMPLE_ROWS: usize = 128;
pub(crate) const MAX_RESULT_BLOB_HASH_SAMPLE_ROWS: usize = 64;
pub(crate) const MAX_RESULT_GAP_SAMPLE_ROWS: usize = 64;
pub(crate) const MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS: usize = 64;
pub(crate) const MAX_RESULT_PEER_ERROR_SAMPLE_ROWS: usize = 64;
pub(crate) const MAX_RESULT_PEER_ERROR_MESSAGE_BYTES: usize = 2 * 1024;
pub(crate) const MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS: usize = 32;
pub(crate) const MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS: usize = 64;
pub(crate) const MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS: usize = 64;

pub(crate) fn sample_pruned_blob_cache_report(mut report: PrunedBlobCache) -> PrunedBlobCache {
    report
        .workspace_ids
        .truncate(MAX_RESULT_WORKSPACE_ID_SAMPLE_ROWS);
    report
        .referenced_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_manifest_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_temp_file_paths
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
}

pub(crate) fn sample_published_workspace_report(
    mut report: PublishedWorkspace,
) -> PublishedWorkspace {
    report
        .published_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .published_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .missing_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report.skipped_gaps.truncate(MAX_RESULT_GAP_SAMPLE_ROWS);
    report
        .blob_transfer_attempts
        .truncate(MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS);
    sample_blob_transfer_attempt_reports(&mut report.blob_transfer_attempts);
    report
}

fn sample_blob_transfer_attempt_report(attempt: &mut BlobTransferAttempt) {
    attempt
        .chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    attempt
        .planned_chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    attempt
        .remote_available_chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
}

fn sample_blob_transfer_attempt_reports(attempts: &mut [BlobTransferAttempt]) {
    for attempt in attempts {
        sample_blob_transfer_attempt_report(attempt);
    }
}

fn truncate_string_bytes(value: &mut String, max_bytes: usize) {
    if value.len() <= max_bytes {
        return;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
}

pub(crate) fn sample_pulled_openmls_catchup_report(catchup: &mut PulledOpenMlsCatchup) {
    catchup
        .published_key_package_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    catchup
        .workspace_applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    catchup
        .workspace_provisioned_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    catchup
        .created_channel_group_ids
        .truncate(MAX_RESULT_CHANNEL_ID_SAMPLE_ROWS);
    catchup
        .channel_provisioning_outcomes
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    for outcome in &mut catchup.channel_provisioning_outcomes {
        if let Some(error) = outcome.provisioning_error.as_mut() {
            truncate_string_bytes(error, MAX_RESULT_PEER_ERROR_MESSAGE_BYTES);
        }
    }
    catchup
        .provisioning_errors
        .truncate(MAX_RESULT_PEER_ERROR_SAMPLE_ROWS);
    for error in &mut catchup.provisioning_errors {
        truncate_string_bytes(error, MAX_RESULT_PEER_ERROR_MESSAGE_BYTES);
    }
    catchup
        .channel_groups
        .truncate(MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS);
    for group in &mut catchup.channel_groups {
        group
            .applied_event_ids
            .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
        group
            .provisioned_event_ids
            .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    }
}

pub(crate) fn sample_applied_openmls_workspace_commits_report(
    mut report: AppliedOpenMlsWorkspaceGroupCommits,
) -> AppliedOpenMlsWorkspaceGroupCommits {
    report
        .applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
}

pub(crate) fn sample_applied_openmls_channel_commits_report(
    mut report: AppliedOpenMlsChannelGroupCommits,
) -> AppliedOpenMlsChannelGroupCommits {
    report
        .applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
}

pub(crate) fn sample_updated_workspace_openmls_groups_report(
    mut report: UpdatedWorkspaceOpenMlsGroups,
) -> UpdatedWorkspaceOpenMlsGroups {
    sample_updated_workspace_openmls_groups_report_in_place(&mut report);
    report
}

fn sample_updated_workspace_openmls_groups_report_in_place(
    report: &mut UpdatedWorkspaceOpenMlsGroups,
) {
    report
        .channel_updates
        .truncate(MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS);
    report
        .updated_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
}

pub(crate) fn sample_rotated_workspace_manual_keys_report(
    mut report: RotatedWorkspaceManualKeys,
) -> RotatedWorkspaceManualKeys {
    sample_rotated_workspace_manual_keys_report_in_place(&mut report);
    report
}

fn sample_rotated_workspace_manual_keys_report_in_place(report: &mut RotatedWorkspaceManualKeys) {
    report
        .channel_key_rotations
        .truncate(MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS);
    report
        .rotated_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
}

pub(crate) fn sample_removed_member_with_key_rotation_report(
    mut report: RemovedMemberWithKeyRotation,
) -> RemovedMemberWithKeyRotation {
    report
        .channel_key_rotations
        .truncate(MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS);
    report
}

pub(crate) fn sample_removed_member_with_openmls_report(
    mut report: RemovedMemberWithOpenMls,
) -> RemovedMemberWithOpenMls {
    report
        .channel_openmls_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    if let Some(manual_key_rotation) = &mut report.manual_key_rotation {
        sample_rotated_workspace_manual_keys_report_in_place(manual_key_rotation);
    }
    report
}

pub(crate) fn sample_rotated_workspace_for_suspected_compromise_report(
    mut report: RotatedWorkspaceForSuspectedCompromise,
) -> RotatedWorkspaceForSuspectedCompromise {
    sample_rotated_workspace_for_suspected_compromise_report_in_place(&mut report);
    report
}

fn sample_rotated_workspace_for_suspected_compromise_report_in_place(
    report: &mut RotatedWorkspaceForSuspectedCompromise,
) {
    report
        .rotated_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    if let Some(openmls_updates) = &mut report.openmls_updates {
        sample_updated_workspace_openmls_groups_report_in_place(openmls_updates);
    }
    if let Some(manual_key_rotation) = &mut report.manual_key_rotation {
        sample_rotated_workspace_manual_keys_report_in_place(manual_key_rotation);
    }
}

pub(crate) fn sample_workspace_compromise_report(
    mut report: WorkspaceCompromiseReport,
) -> WorkspaceCompromiseReport {
    sample_workspace_compromise_report_in_place(&mut report);
    report
}

fn sample_workspace_compromise_report_in_place(report: &mut WorkspaceCompromiseReport) {
    report
        .signals
        .truncate(MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS);
}

fn sample_compromise_response_lists(response: &mut WorkspaceCompromiseResponse) {
    sample_workspace_compromise_report_in_place(&mut response.report);
    response
        .responded_signal_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    response
        .already_handled_signal_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
}

fn sample_compromise_response_report(response: &mut WorkspaceCompromiseResponse) {
    sample_compromise_response_lists(response);
    if let Some(rotation) = &mut response.rotation {
        rotation
            .rotated_event_ids
            .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
        rotation.openmls_updates = None;
        rotation.manual_key_rotation = None;
    }
}

pub(crate) fn sample_compromise_response_report_with_rotation_samples(
    mut response: WorkspaceCompromiseResponse,
) -> WorkspaceCompromiseResponse {
    sample_compromise_response_lists(&mut response);
    if let Some(rotation) = &mut response.rotation {
        sample_rotated_workspace_for_suspected_compromise_report_in_place(rotation);
    }
    response
}

pub(crate) fn sample_pulled_workspace_report(mut report: PulledWorkspace) -> PulledWorkspace {
    report
        .requested_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .fetched_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .fetched_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .missing_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .ignored_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .invite_profile_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report.gaps.truncate(MAX_RESULT_GAP_SAMPLE_ROWS);
    sample_pulled_openmls_catchup_report(&mut report.openmls_catchup);
    if let Some(response) = &mut report.compromise_response {
        sample_compromise_response_report(response);
    }
    report
}

pub(crate) fn sample_synced_workspace_report(mut report: SyncedWorkspace) -> SyncedWorkspace {
    report.published = sample_published_workspace_report(report.published);
    report.pulled = sample_pulled_workspace_report(report.pulled);
    report
}

pub(crate) fn sample_blob_transfer_retry_report(
    mut report: BlobTransferRetryReport,
) -> BlobTransferRetryReport {
    report
        .pending_attempt_ids
        .truncate(MAX_RESULT_ATTEMPT_ID_SAMPLE_ROWS);
    report
        .retried_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .reconciled_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .missing_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .skipped_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .peer_errors
        .truncate(MAX_RESULT_PEER_ERROR_SAMPLE_ROWS);
    for peer_error in &mut report.peer_errors {
        truncate_string_bytes(&mut peer_error.message, MAX_RESULT_PEER_ERROR_MESSAGE_BYTES);
    }
    report
        .blob_transfer_attempts
        .truncate(MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS);
    sample_blob_transfer_attempt_reports(&mut report.blob_transfer_attempts);
    report
}

pub(crate) fn sample_imported_workspace_recovery_bundle_report(
    mut report: ImportedWorkspaceRecoveryBundle,
) -> ImportedWorkspaceRecoveryBundle {
    report
        .imported_channel_ids
        .truncate(MAX_RESULT_CHANNEL_ID_SAMPLE_ROWS);
    report
}
