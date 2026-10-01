//! Same-live-P0 source materialization. Never a canonical splice or resume grant.
use super::{Budget, SourceOwnedBf16MfmaRegionV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_source_isa_observation::source_candidate_io_v1::{
    BOUNDED_SOURCE_IO_CHUNK_BYTES_V1, RetainedSource, publish_bounded_streaming_v1,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

#[path = "production_tiled_region_publish_hir_v1.rs"]
mod anchor;
#[path = "production_tiled_region_publish_request_v1.rs"]
mod request;
#[cfg(test)]
#[path = "production_tiled_region_publish_v1_tests.rs"]
mod tests;
#[path = "production_tiled_region_publish_text_v1.rs"]
mod text;
pub(crate) use request::Bf16TileSourcePublishRequestV1;

const SOURCE_CAP: usize = 64 * 1024;
const HELPER_CAP: usize = 4096;
const CALL_CAP: usize = 512;
const CANDIDATE_CAP: usize = SOURCE_CAP + HELPER_CAP + CALL_CAP;
const HIR_NODES: usize = 4096;
const HIR_DEPTH: usize = 64;
const NAMES: usize = 4096;
const BLOCKS: usize = 32;
const IDENT_CAP: usize = 64;
// Work is a declared logical unit model, not time or complete rustc query work.
// 1: source opens/zero-fill/read plus all hash/byte compare passes.
// 2: candidate generation, hash, write and streaming verification.
// 3: HIR dispatch, type/path/span checks and name comparison at each node.
// 4: bounded source-map/module/MIR/semantic headers and fixed type/ABI joins.
// Callee resolution is performed for the ONE selected parent only; all other
// expression visits do bounded shape/name work. Upstream query/cache memory is
// outside this controlled scratch observation, never claimed as a total peak.
const BYTE_WORK: usize = 20 * (SOURCE_CAP + 1) + 10 * CANDIDATE_CAP + 8 * HELPER_CAP + 8 * CALL_CAP;
const HIR_WORK: usize = HIR_NODES * (64 + IDENT_CAP);
const LOOKUP_WORK: usize = 3 * NAMES * (8 + IDENT_CAP) + 8 * BLOCKS * BLOCKS + 4096;
const PUBLISH_WORK: usize = BYTE_WORK + HIR_WORK + LOOKUP_WORK;
const PATH_CAP: usize = 1024;
// Original Vec capacity maximum+1, independently checked helper/call/candidate
// buffers, source/name and destination/name slots, descriptor path <=64, one
// streaming buffer, explicit data headers. The 64 HIR frames reserve 512 logical
// bytes each; static assertions below/anchor bound our own frame payloads.
// No claim about allocator metadata, system I/O, std error formatting, stack ABI,
// rustc caches or other already-retained compiler owners follows from this sum.
const BUFFER_STORAGE: usize = SOURCE_CAP
    + 1
    + HELPER_CAP
    + CALL_CAP
    + CANDIDATE_CAP
    + 2 * PATH_CAP
    + 64
    + 4096
    + BOUNDED_SOURCE_IO_CHUNK_BYTES_V1;
const HEADER_STORAGE: usize = size_of::<RetainedSource>()
    + 5 * size_of::<String>()
    + size_of::<std::path::PathBuf>()
    + size_of::<anchor::Anchor>()
    + size_of::<text::Coordinates>()
    + 8 * size_of::<std::fs::File>()
    + 8 * size_of::<std::fs::Metadata>()
    + 2 * size_of::<Sha256>()
    + size_of::<PublishedBf16TileSourceV1>()
    + size_of::<Bf16SourcePublicationProgressV1>()
    + 4096;
const PUBLISH_SCRATCH: usize = BUFFER_STORAGE + HEADER_STORAGE + HIR_DEPTH * 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Bf16TileReturnOrderV1 {
    Identity,
    Swap01,
}
impl Bf16TileReturnOrderV1 {
    pub(crate) const fn permutation(self) -> [u8; 4] {
        match self {
            Self::Identity => [0, 1, 2, 3],
            Self::Swap01 => [1, 0, 2, 3],
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Bf16SourcePublicationEffectV1 {
    NotAttempted,
    MayHaveCreatedCandidate,
}
#[derive(Debug)]
pub(crate) enum Bf16SourcePublishReasonV1 {
    Refused(&'static str),
    Resource(Resource),
}
#[derive(Debug)]
pub(crate) struct Bf16TileSourcePublishErrorV1 {
    pub(crate) effect: Bf16SourcePublicationEffectV1,
    pub(crate) reason: Bf16SourcePublishReasonV1,
}
type Error = Bf16TileSourcePublishErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl Error {
    fn refused(why: &'static str) -> Self {
        Self {
            effect: Bf16SourcePublicationEffectV1::NotAttempted,
            reason: Bf16SourcePublishReasonV1::Refused(why),
        }
    }
    fn with_effect(mut self, effect: Bf16SourcePublicationEffectV1) -> Self {
        self.effect = effect;
        self
    }
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self {
            effect: Bf16SourcePublicationEffectV1::NotAttempted,
            reason: Bf16SourcePublishReasonV1::Resource(error),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {:?}", self.effect, self.reason)
    }
}

/// Historical source-file facts only. Fresh frontend admission is mandatory.
/// Returned facts are unreserved after scratch cleanup; the caller must prepay
/// any retained outcome/progress copies (the pipeline action does so).
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct PublishedBf16TileSourceV1 {
    pub(crate) original_sha256: [u8; 32],
    pub(crate) candidate_sha256: [u8; 32],
    pub(crate) original_bytes: usize,
    pub(crate) candidate_bytes: usize,
    pub(crate) candidate_device: u64,
    pub(crate) candidate_inode: u64,
    pub(crate) return_order: Bf16TileReturnOrderV1,
}
impl PublishedBf16TileSourceV1 {
    pub(crate) const fn grants_compiler_or_launch_authority(&self) -> bool {
        false
    }
}

/// Caller-owned before entering the live callback. Never reset on error/panic;
/// outer source/ledger postflight may fail after publication already happened.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct Bf16SourcePublicationProgressV1 {
    pub(crate) effect: Bf16SourcePublicationEffectV1,
    pub(crate) published: Option<PublishedBf16TileSourceV1>,
}
impl Bf16SourcePublicationProgressV1 {
    pub(crate) const fn new() -> Self {
        Self {
            effect: Bf16SourcePublicationEffectV1::NotAttempted,
            published: None,
        }
    }
    fn require_fresh(&self) -> Result<()> {
        if self.effect != Bf16SourcePublicationEffectV1::NotAttempted || self.published.is_some() {
            return Err(Error::refused("BF16 publication progress cannot be reused")
                .with_effect(self.effect));
        }
        Ok(())
    }
}

fn same_ledger(budget: &Budget<'_>, ledger: Ledger, floor: usize) -> Result<()> {
    if budget.work_ledger_identity_v1() != ledger
        || budget.storage() < floor
        || budget.failed_work().is_some()
        || budget.failed_storage().is_some()
    {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}
fn prepaid<'work, T>(
    budget: &mut Budget<'work>,
    ledger: Ledger,
    floor: usize,
    operation: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    same_ledger(budget, ledger, floor)?;
    budget.with_prepaid_scope(floor, 1, PUBLISH_WORK, PUBLISH_SCRATCH, operation)
}

/// Requires the original live P0 view, its original Work ledger and protected
/// floor. Request digests select that owner; none can construct a source view.
/// Progress must outlive the enclosing source callback, including on unwinds.
pub(crate) fn publish_bf16_tile_helper_source_v1(
    source: &SourceOwnedBf16MfmaRegionV1<'_, '_>,
    request: &Bf16TileSourcePublishRequestV1<'_>,
    budget: &mut Budget<'_>,
    progress: &mut Bf16SourcePublicationProgressV1,
) -> Result<PublishedBf16TileSourceV1> {
    progress.require_fresh()?;
    let result = prepaid(
        budget,
        source.publish_ledger,
        source.publish_floor,
        |_budget| {
            request.validate()?;
            let owner = source.emission().original();
            if owner
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
                != &request.semantic_sha256
                || owner.executable().canonical().identity().digest() != &request.canonical_sha256
                || source.mir_sha256() != &request.mir_sha256
                || source.source().sha256() != &request.original_sha256
            {
                return Err(Error::refused(
                    "BF16 publication current owner selection differs",
                ));
            }
            let anchor = anchor::capture(source, request.helper_name)?;
            let mut retained =
                RetainedSource::open_bounded_streaming_v1(request.original_path, SOURCE_CAP)
                    .map_err(|_| Error::refused("BF16 bounded original retention refused"))?;
            if !source.source().matches_original_snapshot(
                retained
                    .bounded_original_metadata_v1(SOURCE_CAP)
                    .map_err(|_| Error::refused("BF16 original snapshot bound refused"))?,
            ) {
                return Err(Error::refused(
                    "BF16 retained original inode/metadata differs from live source",
                ));
            }
            text::join_file(
                &retained,
                request.original_path,
                &anchor.file,
                source.source().bytes(),
            )?;
            if retained
                .bounded_retained_storage_v1(SOURCE_CAP)
                .map_err(|_| Error::refused("BF16 original capacity refused"))?
                > size_of::<RetainedSource>() + SOURCE_CAP + 1 + PATH_CAP
            {
                return Err(Error::refused("BF16 retained source envelope differs"));
            }
            let original = std::str::from_utf8(retained.original())
                .map_err(|_| Error::refused("BF16 original source is not UTF-8"))?;
            if <[u8; 32]>::from(Sha256::digest(original.as_bytes())) != request.original_sha256 {
                return Err(Error::refused("BF16 original source digest differs"));
            }
            let coordinates = text::coordinates(&anchor, original)?;
            text::require_direct(original, &coordinates)?;
            let helper = text::helper(request.helper_name, request.return_order)?;
            let call = text::call(request.helper_name, original, &coordinates)?;
            let candidate = text::splice(original, &coordinates, &helper, &call)?;
            let original_bytes = original.len();
            let candidate_sha256 = Sha256::digest(candidate.as_bytes()).into();
            let candidate_bytes = candidate.len();
            // Mark BEFORE calling shared staging/link machinery; its error can be
            // after the link. No outer cleanup is permitted to erase this fact.
            progress.effect = Bf16SourcePublicationEffectV1::MayHaveCreatedCandidate;
            let published = publish_bounded_streaming_v1(
                &mut retained,
                request.candidate_path,
                candidate.as_bytes(),
                SOURCE_CAP,
            )
            .map_err(|_| Error::refused("BF16 create-new publication refused"))?;
            let facts = PublishedBf16TileSourceV1 {
                original_sha256: request.original_sha256,
                candidate_sha256,
                original_bytes,
                candidate_bytes,
                candidate_device: published.device,
                candidate_inode: published.inode,
                return_order: request.return_order,
            };
            progress.published = Some(facts);
            Ok(facts)
        },
    );
    result.map_err(|error| error.with_effect(progress.effect))
}
