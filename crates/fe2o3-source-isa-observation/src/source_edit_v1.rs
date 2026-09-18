//! Conflict-checked text previews for an explicitly requested draft-helper insertion.
//!
//! A source-map file identity is a rustc file identity, not a source-content hash.
//! This module therefore keeps the caller's exact byte commitment independent of
//! the selected bundle/span association. No span establishes a safe Rust insertion
//! boundary. A preview is only a proposed text edit, never semantic promotion.

use std::{fmt, str};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::multilevel_authoring_v1::{
    AuthoringErrorV1, AuthoringRegionSelectorV1, AuthoringRustCandidateV1, AuthoringSnapshotV1,
    AuthoringSourceSpanV1, MAX_AUTHORING_SOURCE_BYTES_V1,
};

pub const MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1: usize = 1024 * 1024;
pub const MAX_SOURCE_EDIT_PATH_BYTES_V1: usize = 1024;
pub const MAX_SOURCE_EDIT_OUTPUT_BYTES_V1: usize =
    MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 + MAX_AUTHORING_SOURCE_BYTES_V1 + 1;

/// Byte offsets into the exact original UTF-8 source, never line/column guesses.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEditRangeV1 {
    pub start: u32,
    pub end: u32,
}

/// Explicit untrusted input. The V1 profile admits only an EOF insertion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEditRequestV1 {
    pub selector: AuthoringRegionSelectorV1,
    /// Lexical path relative to a separately established workspace root.
    pub relative_path: String,
    /// Independent caller baseline; this is not the source-map file identity.
    pub expected_source_sha256: String,
    pub expected_source_bytes: u32,
    pub source_file_identity: String,
    /// Exact inert display label from the selected source map.
    pub source_display_path: String,
    pub insertion: SourceEditRangeV1,
    pub helper_name: String,
}

/// Inert preview whose private fields can be constructed only by checking input.
///
/// There is deliberately no deserializer or constructor from a serialized report.
/// Keeping this object does not authorize a filesystem write or compilation.
///
/// ```compile_fail
/// use fe2o3_source_isa_observation::source_edit_v1::SourceEditProposalV1;
/// fn retarget(proposal: &mut SourceEditProposalV1) {
///     proposal.relative_path = "different.rs".to_owned();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_source_isa_observation::source_edit_v1::SourceEditProposalV1;
/// let _: SourceEditProposalV1 = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceEditProposalV1 {
    schema: &'static str,
    relative_path: String,
    expected_source_sha256: String,
    expected_source_bytes: u32,
    expected_after_sha256: String,
    expected_after_bytes: u32,
    insertion: SourceEditRangeV1,
    source_map_identity: String,
    selected_source_span: AuthoringSourceSpanV1,
    draft_helper: AuthoringRustCandidateV1,
    inserted_source: String,
    text_preview_status: &'static str,
    semantic_application: &'static str,
    source_content_association: &'static str,
    filesystem_application: &'static str,
    requires_fresh_frontend_admission: bool,
    grants_source_authentication: bool,
    grants_proof_authority: bool,
    grants_production_resume: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEditErrorV1 {
    Authoring(AuthoringErrorV1),
    UnsafePath,
    PathMismatch,
    InvalidDigest,
    EmptySource,
    ResourceLimit,
    InvalidUtf8,
    InvalidByteRange,
    UnsupportedEditRange,
    StaleSource,
    MissingSourceSpan,
    AmbiguousSourceSpan,
    SourceSpanSubstitution,
    InvalidProposal,
}

impl fmt::Display for SourceEditErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authoring(error) => error.fmt(formatter),
            other => formatter.write_str(match other {
                Self::UnsafePath => {
                    "source edit requires a bounded normalized relative Rust source path"
                }
                Self::PathMismatch => "current source path differs from the explicit proposal path",
                Self::InvalidDigest => {
                    "source baseline must be a canonical lowercase SHA-256 digest"
                }
                Self::EmptySource => {
                    "empty source cannot be replaced by hidden whole-file generation"
                }
                Self::ResourceLimit => "source edit exceeds the bounded text profile",
                Self::InvalidUtf8 => "source edit requires valid UTF-8 original bytes",
                Self::InvalidByteRange => "source byte range is out of bounds or splits UTF-8",
                Self::UnsupportedEditRange => {
                    "only explicit EOF insertion is supported; source replacement is unavailable"
                }
                Self::StaleSource => "current source bytes differ from the exact caller baseline",
                Self::MissingSourceSpan => "selected operation has no source span",
                Self::AmbiguousSourceSpan => {
                    "selection must have one identical source span for every operation"
                }
                Self::SourceSpanSubstitution => {
                    "requested source file identity or display label differs from selection"
                }
                Self::InvalidProposal => "source edit proposal content commitment is inconsistent",
                Self::Authoring(_) => unreachable!(),
            }),
        }
    }
}

impl std::error::Error for SourceEditErrorV1 {}
impl From<AuthoringErrorV1> for SourceEditErrorV1 {
    fn from(value: AuthoringErrorV1) -> Self {
        Self::Authoring(value)
    }
}
type Result<T> = std::result::Result<T, SourceEditErrorV1>;

/// Prepares a text-only append of a complete generated helper to an existing file.
///
/// The selector is rechecked against the immutable owner. The independently supplied
/// source hash prevents later byte substitution; it does not authenticate association
/// with the source map. No Rust parsing, source application, or compilation occurs.
pub fn prepare_source_edit_v1(
    snapshot: &AuthoringSnapshotV1,
    request: &SourceEditRequestV1,
    original: &[u8],
) -> Result<SourceEditProposalV1> {
    validate_source_edit_path_v1(&request.relative_path)?;
    validate_digest(&request.expected_source_sha256)?;
    let original_text = validate_original(original)?;
    if original.len() != request.expected_source_bytes as usize
        || sha256(original) != request.expected_source_sha256
    {
        return Err(SourceEditErrorV1::StaleSource);
    }
    validate_range(original_text, request.insertion)?;
    if request.insertion.start != request.insertion.end
        || request.insertion.end as usize != original.len()
    {
        return Err(SourceEditErrorV1::UnsupportedEditRange);
    }
    let region = snapshot.select_region(&request.selector)?;
    let mut selected_span = None;
    for operation in &region.operations {
        let span = match operation.source_spans.as_slice() {
            [] => return Err(SourceEditErrorV1::MissingSourceSpan),
            [span] => span,
            _ => return Err(SourceEditErrorV1::AmbiguousSourceSpan),
        };
        if selected_span
            .as_ref()
            .is_some_and(|selected| selected != span)
        {
            return Err(SourceEditErrorV1::AmbiguousSourceSpan);
        }
        selected_span = Some(span.clone());
    }
    let selected_span = selected_span.ok_or(SourceEditErrorV1::MissingSourceSpan)?;
    if selected_span.file_identity != request.source_file_identity
        || selected_span.display_path != request.source_display_path
    {
        return Err(SourceEditErrorV1::SourceSpanSubstitution);
    }
    // This checks only that the explicit association is structurally possible.
    // It does not establish that these are the compiler-observed source bytes.
    let source_range = SourceEditRangeV1 {
        start: parse_span_offset(&selected_span.byte_start)?,
        end: parse_span_offset(&selected_span.byte_end)?,
    };
    validate_range(original_text, source_range)?;
    if source_range.start == source_range.end {
        return Err(SourceEditErrorV1::InvalidByteRange);
    }
    let draft_helper = snapshot.materialize_typed_rust(&request.selector, &request.helper_name)?;
    if draft_helper.source.is_empty() || draft_helper.source.len() > MAX_AUTHORING_SOURCE_BYTES_V1 {
        return Err(SourceEditErrorV1::ResourceLimit);
    }
    // Always separate the complete helper from an existing final line comment.
    // Existing bytes, including an existing final newline, remain unchanged.
    let mut inserted_source = String::with_capacity(draft_helper.source.len() + 1);
    inserted_source.push('\n');
    inserted_source.push_str(&draft_helper.source);
    let after = append_bytes(original, inserted_source.as_bytes())?;
    Ok(SourceEditProposalV1 {
        schema: "fe2o3-source-edit-proposal-v1",
        relative_path: request.relative_path.clone(),
        expected_source_sha256: request.expected_source_sha256.clone(),
        expected_source_bytes: request.expected_source_bytes,
        expected_after_sha256: sha256(&after),
        expected_after_bytes: after.len() as u32,
        insertion: request.insertion,
        source_map_identity: snapshot.summary().source_map_identity,
        selected_source_span: selected_span,
        draft_helper,
        inserted_source,
        text_preview_status: "conflict_checked_explicit_eof_insertion_preview",
        semantic_application: "unavailable_source_insertion_boundary_unverified",
        source_content_association: "caller_asserted_not_authenticated_by_source_map_identity",
        filesystem_application: "unavailable_pure_preview_no_filesystem_write",
        requires_fresh_frontend_admission: true,
        grants_source_authentication: false,
        grants_proof_authority: false,
        grants_production_resume: false,
    })
}

impl SourceEditProposalV1 {
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }
    pub fn expected_source_sha256(&self) -> &str {
        &self.expected_source_sha256
    }
    pub const fn expected_source_bytes(&self) -> u32 {
        self.expected_source_bytes
    }
    pub fn expected_after_sha256(&self) -> &str {
        &self.expected_after_sha256
    }
    pub const fn expected_after_bytes(&self) -> u32 {
        self.expected_after_bytes
    }
    pub const fn insertion(&self) -> SourceEditRangeV1 {
        self.insertion
    }
    pub fn selected_source_span(&self) -> &AuthoringSourceSpanV1 {
        &self.selected_source_span
    }
    pub fn draft_helper(&self) -> &AuthoringRustCandidateV1 {
        &self.draft_helper
    }

    /// Rechecks lexical path and exact bytes immediately before any caller action.
    /// Symlink resolution, file custody, and write authorization belong to the caller.
    pub fn validate_current(&self, relative_path: &str, current: &[u8]) -> Result<()> {
        validate_source_edit_path_v1(relative_path)?;
        if relative_path != self.relative_path {
            return Err(SourceEditErrorV1::PathMismatch);
        }
        validate_original(current)?;
        if current.len() != self.expected_source_bytes as usize
            || sha256(current) != self.expected_source_sha256
        {
            return Err(SourceEditErrorV1::StaleSource);
        }
        Ok(())
    }

    /// Returns proposed bytes only; it neither mutates source nor grants admission.
    /// Reusing this proposal with its own output fails the original-byte commitment.
    pub fn preview_bytes(&self, relative_path: &str, current: &[u8]) -> Result<Vec<u8>> {
        self.validate_current(relative_path, current)?;
        let after = append_bytes(current, self.inserted_source.as_bytes())?;
        if after.len() != self.expected_after_bytes as usize
            || sha256(&after) != self.expected_after_sha256
        {
            return Err(SourceEditErrorV1::InvalidProposal);
        }
        Ok(after)
    }
}

fn validate_original(bytes: &[u8]) -> Result<&str> {
    if bytes.is_empty() {
        return Err(SourceEditErrorV1::EmptySource);
    }
    if bytes.len() > MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 {
        return Err(SourceEditErrorV1::ResourceLimit);
    }
    str::from_utf8(bytes).map_err(|_| SourceEditErrorV1::InvalidUtf8)
}

fn validate_range(source: &str, range: SourceEditRangeV1) -> Result<()> {
    let start = range.start as usize;
    let end = range.end as usize;
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err(SourceEditErrorV1::InvalidByteRange);
    }
    Ok(())
}

fn parse_span_offset(value: &str) -> Result<u32> {
    let number = value
        .parse::<u32>()
        .map_err(|_| SourceEditErrorV1::InvalidByteRange)?;
    if number.to_string() != value {
        return Err(SourceEditErrorV1::InvalidByteRange);
    }
    Ok(number)
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SourceEditErrorV1::InvalidDigest);
    }
    Ok(())
}

/// Checks the exact lexical path profile before a caller performs filesystem I/O.
/// This does not resolve symlinks, open files, or establish filesystem custody.
pub fn validate_source_edit_path_v1(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > MAX_SOURCE_EDIT_PATH_BYTES_V1 || !path.ends_with(".rs") {
        return Err(SourceEditErrorV1::UnsafePath);
    }
    for component in path.split('/') {
        if component.is_empty()
            || component.len() > 255
            || component.starts_with('.')
            || !component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(SourceEditErrorV1::UnsafePath);
        }
    }
    Ok(())
}

fn append_bytes(original: &[u8], insertion: &[u8]) -> Result<Vec<u8>> {
    let length = original
        .len()
        .checked_add(insertion.len())
        .ok_or(SourceEditErrorV1::ResourceLimit)?;
    if length > MAX_SOURCE_EDIT_OUTPUT_BYTES_V1 {
        return Err(SourceEditErrorV1::ResourceLimit);
    }
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| SourceEditErrorV1::ResourceLimit)?;
    after.extend_from_slice(original);
    after.extend_from_slice(insertion);
    Ok(after)
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}

#[cfg(test)]
#[path = "source_edit_v1_tests.rs"]
mod tests;
