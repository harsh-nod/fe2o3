//! Same-session, create-new source publication; rendered text is never an owner.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Gfx942OrderedProgramRegistersV1,
    Gfx942U32ProgramV1, OrderedProgramDefinitionKeyV1, VerifiedCanonicalKernelIrIdentityV17,
};
use fe2o3_lower_mir_kernel::OrderedCompositionInspectionV1;
use fe2o3_source_isa_observation::source_candidate_io_v1::{RetainedSource, publish};
use sha2::{Digest, Sha256};

use super::AuthenticatedOrderedCompositionSourceSeedV1;

#[path = "production_ordered_composition_publish_file_v1.rs"]
mod file;
#[path = "production_ordered_composition_publish_hir_v1.rs"]
mod hir;
#[path = "production_ordered_composition_publish_origin_v1.rs"]
mod origin;
#[cfg(test)]
#[path = "production_ordered_composition_publish_v1_tests.rs"]
mod tests;
#[path = "production_ordered_composition_publish_text_v1.rs"]
mod text;

const SOURCE_CAP: usize = 64 * 1024;
const HELPER_CAP: usize = 4096;
const CANDIDATE_CAP: usize = SOURCE_CAP + HELPER_CAP + 512;
const PUBLISH_WORK: usize = 16 * 1024 * 1024;
const PUBLISH_SCRATCH: usize = 8 * 1024 * 1024;

/// Typed inert edit only: these types already enforce register and program shape.
/// A successful publication is not a proof that the fresh source compiles.
#[derive(Clone, Copy)]
pub(crate) struct OrderedCompositionTypedEditV1 {
    pub(crate) program: Gfx942U32ProgramV1,
    pub(crate) registers: Gfx942OrderedProgramRegistersV1,
}

pub(crate) struct OrderedCompositionSourcePublishRequestV1<'a> {
    pub(crate) definition: OrderedProgramDefinitionKeyV1,
    pub(crate) expected_canonical: &'a VerifiedCanonicalKernelIrIdentityV17,
    pub(crate) expected_semantic: [u8; 32],
    pub(crate) original_path: &'a str,
    pub(crate) original_sha256: [u8; 32],
    pub(crate) candidate_path: &'a str,
    pub(crate) helper_name: &'a str,
    pub(crate) edit: Option<OrderedCompositionTypedEditV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrderedCompositionSourcePublishEffectV1 {
    NotAttempted,
    /// The no-replace helper may have linked before a later durability refusal.
    /// No rollback or absence claim follows from a failed call.
    MayHaveCreatedCandidate,
}

#[derive(Debug)]
pub(crate) struct OrderedCompositionSourcePublishErrorV1 {
    pub(crate) effect: OrderedCompositionSourcePublishEffectV1,
    pub(crate) reason: PublishReasonV1,
}
#[derive(Debug)]
pub(crate) enum PublishReasonV1 {
    Refused(&'static str),
    Resource(Resource),
}
impl OrderedCompositionSourcePublishErrorV1 {
    fn refused(reason: &'static str) -> Self {
        Self {
            effect: OrderedCompositionSourcePublishEffectV1::NotAttempted,
            reason: PublishReasonV1::Refused(reason),
        }
    }
    fn after_attempt(mut self, attempted: bool) -> Self {
        if attempted {
            self.effect = OrderedCompositionSourcePublishEffectV1::MayHaveCreatedCandidate;
        }
        self
    }
}
impl From<Resource> for OrderedCompositionSourcePublishErrorV1 {
    fn from(value: Resource) -> Self {
        Self {
            effect: OrderedCompositionSourcePublishEffectV1::NotAttempted,
            reason: PublishReasonV1::Resource(value),
        }
    }
}
impl std::fmt::Display for OrderedCompositionSourcePublishErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: ", self.effect)?;
        match &self.reason {
            PublishReasonV1::Refused(reason) => f.write_str(reason),
            PublishReasonV1::Resource(reason) => std::fmt::Display::fmt(reason, f),
        }
    }
}

/// Shared closed naming grammar; this validates inert text, not source custody.
pub(crate) fn validate_ordered_composition_helper_name_v1(
    name: &str,
) -> std::result::Result<(), String> {
    text::helper_name(name).map_err(|error| error.to_string())
}

type Error = OrderedCompositionSourcePublishErrorV1;
type Result<T> = std::result::Result<T, Error>;

/// Historical publication facts only; no frontend, source-owner or launch grant.
/// Returned retained payload is unreserved, as with other prepaid-scope receipts.
pub(crate) struct PublishedOrderedCompositionSourceV1 {
    pub(crate) original_sha256: [u8; 32],
    pub(crate) candidate_sha256: [u8; 32],
    pub(crate) original_bytes: usize,
    pub(crate) candidate_bytes: usize,
    pub(crate) candidate_device: u64,
    pub(crate) candidate_inode: u64,
    pub(crate) program_edited: bool,
}
impl PublishedOrderedCompositionSourceV1 {
    pub(crate) const fn retained_storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/// Called while borrowing the exact lowerer inspection. Its own definition()
/// check joins both resource-ledger identities before any source or IO work.
/// No caller-supplied source range, rustc session or raw replacement is accepted.
pub(crate) fn publish_ordered_composition_source_v1(
    seed: &AuthenticatedOrderedCompositionSourceSeedV1<'_>,
    view: &OrderedCompositionInspectionV1<'_>,
    request: &OrderedCompositionSourcePublishRequestV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<PublishedOrderedCompositionSourceV1> {
    if seed.semantic_sha256() != &request.expected_semantic {
        return Err(Error::refused("publisher semantic seed mismatch"));
    }
    let region = view
        .definition(
            request.definition,
            request.expected_canonical,
            &request.expected_semantic,
            budget,
        )
        .map_err(|_| Error::refused("publisher same-owner inspection selection refused"))?;
    // This leaf is a deliberately root-only first materialization seam: inserting
    // into a helper would introduce a nested call outside the admitted topology.
    seed.require_root_publication(region.source_function())
        .map_err(Error::refused)?;
    let floor = budget.storage();
    let mut attempted = false;
    let result = budget.with_prepaid_scope(floor, 128, PUBLISH_WORK, PUBLISH_SCRATCH, |_budget| {
        text::helper_name(request.helper_name)?;
        for path in [request.original_path, request.candidate_path] {
            fe2o3_source_isa_observation::source_edit_v1::validate_source_edit_path_v1(path)
                .map_err(|_| {
                    Error::refused("publisher requires a bounded relative Rust source path")
                })?;
        }
        if request.original_path == request.candidate_path {
            return Err(Error::refused("publisher requires a fresh candidate path"));
        }
        let (tcx, instance, body) = seed
            .selected(
                &request.expected_semantic,
                region.semantic_function(),
                region.source_function(),
            )
            .map_err(Error::refused)?;
        if !instance.args.is_empty() {
            return Err(Error::refused(
                "publisher refuses generic or const captures",
            ));
        }
        let actual = origin::reobserve(tcx, instance, &region)?;
        let anchor = hir::capture(tcx, instance, body, actual, request.helper_name)?;
        let mut retained = RetainedSource::open(request.original_path)
            .map_err(|_| Error::refused("publisher retained original open refused"))?;
        file::join(&retained, request.original_path, &anchor.file)?;
        let original = retained.original();
        if original.len() > SOURCE_CAP
            || <[u8; 32]>::from(Sha256::digest(original)) != request.original_sha256
        {
            return Err(Error::refused(
                "publisher current source size or digest differs",
            ));
        }
        let original = std::str::from_utf8(original)
            .map_err(|_| Error::refused("publisher original source is not UTF-8"))?;
        let coordinates = file::coordinates(&anchor, original)?;
        let original_program = OrderedCompositionTypedEditV1 {
            program: *region.program().program(),
            registers: region.program().registers(),
        };
        text::require_flat_source(original, &coordinates, original_program)?;
        let chosen = request.edit.unwrap_or(original_program);
        let helper = text::render(request.helper_name, chosen)?;
        let candidate = text::splice(original, &coordinates, request.helper_name, &helper)?;
        let original_bytes = original.len();
        let candidate_sha256 = Sha256::digest(candidate.as_bytes()).into();
        let candidate_bytes = candidate.len();
        let program_edited = chosen.program != *region.program().program()
            || chosen.registers != region.program().registers();
        retained
            .recheck()
            .map_err(|_| Error::refused("publisher original changed before publication"))?;
        attempted = true;
        let published = publish(&mut retained, request.candidate_path, candidate.as_bytes())
            .map_err(|_| Error::refused("publisher create-new publication refused"))?;
        Ok(PublishedOrderedCompositionSourceV1 {
            original_sha256: request.original_sha256,
            candidate_sha256,
            original_bytes,
            candidate_bytes,
            candidate_device: published.device,
            candidate_inode: published.inode,
            program_edited,
        })
    });
    // Includes an unlikely post-callback accounting refusal. Never relabel a
    // possible successful link as NotAttempted merely because cleanup failed.
    result.map_err(|error: Error| error.after_attempt(attempted))
}
