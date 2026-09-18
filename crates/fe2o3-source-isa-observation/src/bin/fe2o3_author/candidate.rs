//! Explicit text-only candidate creation, not source promotion or admission.

use std::fmt::Write as _;

use fe2o3_source_isa_observation::multilevel_authoring_v1::{
    AuthoringRegionSelectorV1, AuthoringSnapshotV1,
};
use fe2o3_source_isa_observation::source_edit_v1::{
    SourceEditRangeV1, SourceEditRequestV1, prepare_source_edit_v1, validate_source_edit_path_v1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
#[path = "candidate_fs.rs"]
mod filesystem;

pub(super) struct Options {
    pub selector: AuthoringRegionSelectorV1,
    pub helper: String,
    pub source: String,
    pub expected_source_sha256: String,
    pub expected_proposal_sha256: String,
    pub candidate: String,
}

impl Options {
    pub(super) fn validate(&self) -> Result<(), String> {
        super::preview::validate_arguments(&self.source, &self.expected_source_sha256)?;
        validate_source_edit_path_v1(&self.candidate).map_err(|error| error.to_string())?;
        if self.source == self.candidate {
            return Err("candidate path must differ from the original source path".into());
        }
        if self.expected_proposal_sha256.len() != 64
            || !self
                .expected_proposal_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("proposal baseline must be a canonical lowercase SHA-256 digest".into());
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub(super) struct Receipt {
    schema: &'static str,
    status: &'static str,
    source_path: String,
    source_sha256: String,
    source_bytes: u32,
    proposal_sha256: String,
    candidate_path: String,
    candidate_sha256: String,
    candidate_bytes: u32,
    candidate_device: String,
    candidate_inode: String,
    candidate_mode: &'static str,
    source_currentness: &'static str,
    original_source_written: bool,
    existing_candidate_replaced: bool,
    source_content_association: &'static str,
    semantic_application: &'static str,
    syntax_validation: &'static str,
    requires_fresh_frontend_admission: bool,
    compilation_performed: bool,
    grants_source_authentication: bool,
    grants_proof_authority: bool,
    grants_production_resume: bool,
    undo: &'static str,
}

#[cfg(target_os = "linux")]
pub(super) fn create(snapshot: &AuthoringSnapshotV1, options: &Options) -> Result<Receipt, String> {
    options.validate()?;
    let region = snapshot
        .select_region(&options.selector)
        .map_err(|error| error.to_string())?;
    let mut span = None;
    for operation in &region.operations {
        let [current] = operation.source_spans.as_slice() else {
            return Err("selection must have one identical source span for every operation".into());
        };
        if span.is_some_and(|previous| previous != current) {
            return Err("selection must have one identical source span for every operation".into());
        }
        span = Some(current);
    }
    let span = span.ok_or("selected operation has no source span")?;
    // Only the explicit path is opened; the selected display path is inert.
    let mut source = filesystem::RetainedSource::open(&options.source)?;
    let source_bytes = source.original().len() as u32;
    let proposal = prepare_source_edit_v1(
        snapshot,
        &SourceEditRequestV1 {
            selector: options.selector.clone(),
            relative_path: options.source.clone(),
            expected_source_sha256: options.expected_source_sha256.clone(),
            expected_source_bytes: source_bytes,
            source_file_identity: span.file_identity.clone(),
            source_display_path: span.display_path.clone(),
            insertion: SourceEditRangeV1 {
                start: source_bytes,
                end: source_bytes,
            },
            helper_name: options.helper.clone(),
        },
        source.original(),
    )
    .map_err(|error| error.to_string())?;
    // Match exactly the existing preview CLI's compact JSON and final newline.
    // No serialized proposal is deserialized into an owner or trusted object.
    let mut preview = serde_json::to_vec(&proposal).map_err(|error| error.to_string())?;
    preview.push(b'\n');
    let mut proposal_sha256 = String::with_capacity(64);
    for byte in Sha256::digest(&preview) {
        write!(proposal_sha256, "{byte:02x}").map_err(|error| error.to_string())?;
    }
    if proposal_sha256 != options.expected_proposal_sha256 {
        return Err("fresh proposal differs from the exact reviewed proposal digest".into());
    }
    let candidate = proposal
        .preview_bytes(&options.source, source.original())
        .map_err(|error| error.to_string())?;
    let published = filesystem::publish(&mut source, &options.candidate, &candidate)?;
    Ok(Receipt {
        schema: "fe2o3-source-candidate-receipt-v1",
        status: "new_text_candidate_created",
        source_path: options.source.clone(),
        source_sha256: options.expected_source_sha256.clone(),
        source_bytes,
        proposal_sha256,
        candidate_path: options.candidate.clone(),
        candidate_sha256: proposal.expected_after_sha256().to_owned(),
        candidate_bytes: proposal.expected_after_bytes(),
        candidate_device: published.device.to_string(),
        candidate_inode: published.inode.to_string(),
        candidate_mode: "owner_read_write_only_0600",
        source_currentness: "exact_bytes_and_retained_parent_name_at_final_read_not_pathname_or_source_cas",
        original_source_written: false,
        existing_candidate_replaced: false,
        source_content_association: "caller_asserted_not_authenticated_by_source_map_identity",
        semantic_application: "unavailable_requires_explicit_caller_source_integration",
        syntax_validation: "not_performed_source_root_context_may_need_integration",
        requires_fresh_frontend_admission: true,
        compilation_performed: false,
        grants_source_authentication: false,
        grants_proof_authority: false,
        grants_production_resume: false,
        undo: "explicit_user_removal_only_no_automatic_undo",
    })
}

#[cfg(not(target_os = "linux"))]
pub(super) fn create(_: &AuthoringSnapshotV1, _: &Options) -> Result<Receipt, String> {
    Err("source candidate publication requires Linux descriptor-relative O_TMPFILE support".into())
}
