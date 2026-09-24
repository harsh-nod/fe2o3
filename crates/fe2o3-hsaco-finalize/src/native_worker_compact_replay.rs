//! Inert native restart coordinates and the shared compact Worker replay tail.
//!
//! This codec does not recover an occurrence, rederive a producer transaction,
//! check Worker replay, or admit an artifact. Those remain production joins.
//! Worker V2 decoding/provider extraction and response-evidence validation retain
//! their existing bounded artifact domain. The ledger below covers only this
//! codec's byte copies, hashes, headers, text and reference vectors, not those
//! artifact operations, external payload owners, spare allocation capacities,
//! allocator overhead or RSS.

use std::{fmt, io::Write, mem::size_of};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffSlotV4, CompilerModuleHandoffTransactionIdentityV4,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V4,
};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV4;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, LinkOptionV1, MAX_LINK_INPUTS, MAX_LINK_OPTION_NAME_BYTES,
    MAX_LINK_OPTION_VALUE_BYTES, MAX_LINK_OPTIONS, MAX_WORKER_TOOLCHAIN_ID_BYTES,
    NativeWorkerDiagnosticV1,
    first_build_worker_v3::{
        OwnedWorkerV3RequestReplayPartsV1, extract_worker_v3_request_replay_parts_v1,
    },
    native_worker_finalization::PreparedFinalizedNativeWorkerHsacoV1,
    worker_v3_compact_finalizer_replay::{
        CompactReplayReaderV1, DecodedCompactReplayTailV1,
        MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1,
        ProtectedWorkerV3CompactFinalizerReplayErrorV1 as TailError,
        ProtectedWorkerV3CompactFinalizerReplayViewV2, WorkerV3ProviderReplayReferenceV1,
        compact_replay_encoded_length, decode_compact_replay_tail, encode_compact_replay_tail,
        validate_construction_parts,
    },
};

const MAGIC: &[u8; 8] = b"F2NCFR01";
const VERSION: u16 = 1;
const CHECKSUM_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-COMPACT-FINALIZER-REPLAY-CHECKSUM/V1\0";
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-COMPACT-FINALIZER-REPLAY-IDENTITY/V1\0";
// Magic/version, three identities, native outer digest/length, attempt, slot, transaction.
const HEADER_BYTES: usize = 8 + 2 + 3 * 32 + 40 + 8 + 16 + 32 + 1 + 32;
const LEGACY_V3_HEADER_BYTES: usize = 8 + 2 + 2 * 32 + 1 + 32;
const MIN_BYTES: usize = HEADER_BYTES + 32;

/// Same tail limits as compact V3, enlarged only by the native occurrence header.
pub const MAX_NATIVE_WORKER_COMPACT_FINALIZER_REPLAY_BYTES_V1: usize =
    MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1 + HEADER_BYTES
        - LEGACY_V3_HEADER_BYTES;

/// No receipt or authority can be constructed from these copied coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerReplayCoordinatesV1 {
    outer: ContentIdentityV1,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV4,
    transaction: [u8; 32],
}
impl NativeWorkerReplayCoordinatesV1 {
    /// Digest is in the native V4 domain, NOT a raw-content SHA256.
    pub const fn outer_identity_coordinates(self) -> ([u8; 32], u64) {
        (*self.outer.sha256(), self.outer.byte_len())
    }
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }
    pub const fn slot(self) -> CompilerModuleHandoffSlotV4 {
        self.slot
    }
    pub const fn transaction_identity(self) -> CompilerModuleHandoffTransactionIdentityV4 {
        CompilerModuleHandoffTransactionIdentityV4::from_bytes(self.transaction)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerCompactFinalizerReplayIdentityV1([u8; 32]);
impl NativeWorkerCompactFinalizerReplayIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Additional logical retained charge, not reserved by the returning codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerCompactReplayStorageV1(usize);
impl NativeWorkerCompactReplayStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only transcript; large native outer, provider and HSACO bytes stay external.
/// A checksum proves only byte integrity. Even a canonical transcript grants no
/// origin, currentness, proof, publication, load or launch authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::NativeWorkerCompactFinalizerReplayV1 as Replay;
/// fn duplicate(value: Replay) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::NativeWorkerCompactFinalizerReplayV1 as Replay;
/// fn forge() -> Replay { Replay::default() }
/// ```
pub struct NativeWorkerCompactFinalizerReplayV1 {
    identity: NativeWorkerCompactFinalizerReplayIdentityV1,
    expected_finalization_identity: [u8; 32],
    source_evidence_identity: [u8; 32],
    binding_identity: [u8; 32],
    coordinates: NativeWorkerReplayCoordinatesV1,
    tail: DecodedCompactReplayTailV1,
    canonical_bytes: Vec<u8>,
    storage: NativeWorkerCompactReplayStorageV1,
}

impl fmt::Debug for NativeWorkerCompactFinalizerReplayV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeWorkerCompactFinalizerReplayV1")
            .field("identity", &self.identity)
            .field("coordinates", &self.coordinates)
            .field("bytes", &self.canonical_bytes.len())
            .finish_non_exhaustive()
    }
}

impl NativeWorkerCompactFinalizerReplayV1 {
    pub const fn identity(&self) -> NativeWorkerCompactFinalizerReplayIdentityV1 {
        self.identity
    }
    pub const fn coordinates(&self) -> NativeWorkerReplayCoordinatesV1 {
        self.coordinates
    }
    pub const fn attempt(&self) -> BuildAttempt {
        self.coordinates.attempt()
    }
    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV4 {
        self.coordinates.slot()
    }
    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV4 {
        self.coordinates.transaction_identity()
    }
    /// Checks coordinates of a separately decoded outer; never rehashes the
    /// payload as raw content or constructs an outer identity/receipt.
    pub fn verify_outer_identity(
        &self,
        actual: InertSemanticCompilerModuleHandoffIdentityV4,
    ) -> Result<()> {
        let expected = self.coordinates.outer;
        if expected.sha256() != actual.sha256() || expected.byte_len() != actual.byte_len() {
            return Err(NativeWorkerCompactReplayErrorV1::Coordinates);
        }
        Ok(())
    }
    pub const fn expected_finalization_identity(&self) -> &[u8; 32] {
        &self.expected_finalization_identity
    }
    pub const fn source_evidence_identity(&self) -> &[u8; 32] {
        &self.source_evidence_identity
    }
    pub const fn binding_identity(&self) -> &[u8; 32] {
        &self.binding_identity
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Transfers the sole canonical byte allocation without copying it.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }
    pub const fn storage(&self) -> NativeWorkerCompactReplayStorageV1 {
        self.storage
    }
    pub(crate) fn replay_view(&self) -> ProtectedWorkerV3CompactFinalizerReplayViewV2<'_> {
        self.tail.replay_view(&self.canonical_bytes)
    }

    /// Caller must already reserve `bytes.len()` for the borrowed input. That
    /// reservation is unchanged on every exit. Success returns a new move-only
    /// owner and its additional, unreserved logical charge; reserve it before
    /// retaining the owner. No fresh budget, hidden clone or receipt is created.
    pub fn decode_canonical(
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, NativeWorkerCompactReplayStorageV1)> {
        let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(bytes.len())?;
        budget.with_prepaid_scope(bytes.len(), 8, quote.work, quote.scratch, |_| {
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| Resource::Allocation)?;
            owned.extend_from_slice(bytes);
            Self::decode_owned(owned)
        })
    }

    fn decode_owned(bytes: Vec<u8>) -> Result<(Self, NativeWorkerCompactReplayStorageV1)> {
        check_length(bytes.len())?;
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        let mut reader = CompactReplayReaderV1::new(body);
        if reader.take(8)? != MAGIC {
            return Err(NativeWorkerCompactReplayErrorV1::Magic);
        }
        if reader.u16()? != VERSION {
            return Err(NativeWorkerCompactReplayErrorV1::Version);
        }
        if hash(CHECKSUM_DOMAIN, body) != checksum {
            return Err(NativeWorkerCompactReplayErrorV1::Checksum);
        }
        let expected_finalization_identity = reader.array()?;
        let source_evidence_identity = reader.array()?;
        let binding_identity = reader.array()?;
        let outer = ContentIdentityV1::from_parts(reader.array()?, reader.u64()?);
        let generation = reader.u64()?;
        let session = reader.array()?;
        let invocation = reader.array()?;
        let slot = match reader.u8()? {
            0 => CompilerModuleHandoffSlotV4::Production,
            _ => return Err(NativeWorkerCompactReplayErrorV1::Coordinates),
        };
        let transaction = reader.array()?;
        let attempt = decode_attempt(generation, session, invocation)?;
        let coordinates = NativeWorkerReplayCoordinatesV1 {
            outer,
            attempt,
            slot,
            transaction,
        };
        if [
            expected_finalization_identity,
            source_evidence_identity,
            binding_identity,
            transaction,
        ]
        .contains(&[0; 32])
            || outer.byte_len() == 0
            || outer.byte_len() > MAX_COMPILER_MODULE_HANDOFF_BYTES_V4 as u64
        {
            return Err(NativeWorkerCompactReplayErrorV1::Coordinates);
        }
        // Always the derivation-capable tail grammar; legacy V1/V2 tails cannot
        // silently drop response derivation bodies by selecting another decoder.
        let tail = decode_compact_replay_tail(&mut reader, true)?;
        let storage = retained_storage(bytes.len(), &tail)?;
        let identity = NativeWorkerCompactFinalizerReplayIdentityV1(hash(IDENTITY_DOMAIN, &bytes));
        Ok((
            Self {
                identity,
                expected_finalization_identity,
                source_evidence_identity,
                binding_identity,
                coordinates,
                tail,
                canonical_bytes: bytes,
                storage,
            },
            storage,
        ))
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

const MAX_PROVIDERS: usize = MAX_LINK_INPUTS - 1;
const MAX_TEXT: usize = 2 * MAX_WORKER_TOOLCHAIN_ID_BYTES
    + MAX_LINK_OPTIONS * (MAX_LINK_OPTION_NAME_BYTES + MAX_LINK_OPTION_VALUE_BYTES);
const FRAME: usize = 2 * size_of::<NativeWorkerCompactFinalizerReplayV1>()
    + 2 * size_of::<DecodedCompactReplayTailV1>()
    + size_of::<CompactReplayReaderV1<'static>>()
    + size_of::<NativeWorkerCompactReplayErrorV1>()
    + size_of::<OwnedWorkerV3RequestReplayPartsV1>()
    + size_of::<Sha256>()
    + 1024;

/// Conservative codec-only schedule for one encode/decode pass of `n` bytes.
///
/// Work = 4096 + 16*n + 128*(127+64). Sixteen linear byte visits cover writing,
/// three domain hashes, decoding, text validation/copy/comparison and retained
/// length accounting; 128 per bounded record covers fixed parsing/comparison.
///
/// ```text
/// scratch = FRAME + n + min(n, MAX_TEXT)
///     + 127 * sizeof(provider reference) + 64 * sizeof(option)
/// ```
///
/// Text is fallibly reserved exactly; vectors reserve only the checked wire
/// counts. Both successful and malformed-prefix allocations
/// fit this schedule. Returned charge uses actual lengths/counts, not maxima.
///
/// Response evidence semantic decoding and external-provider extraction keep
/// their existing artifact limits, NOT this schedule. No process/RSS claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerCompactReplayResourcesV1 {
    work: usize,
    scratch: usize,
}
impl NativeWorkerCompactReplayResourcesV1 {
    pub fn for_wire_length(n: usize) -> Result<Self> {
        check_length(n)?;
        let records = MAX_PROVIDERS
            .checked_add(MAX_LINK_OPTIONS)
            .ok_or(Resource::Arithmetic)?;
        let work = n
            .checked_mul(16)
            .and_then(|v| v.checked_add(records.checked_mul(128)?))
            .and_then(|v| v.checked_add(4096))
            .ok_or(Resource::Arithmetic)?;
        let scratch = FRAME
            .checked_add(n)
            .and_then(|v| v.checked_add(n.min(MAX_TEXT)))
            .and_then(|v| {
                v.checked_add(
                    MAX_PROVIDERS.checked_mul(size_of::<WorkerV3ProviderReplayReferenceV1>())?,
                )
            })
            .and_then(|v| v.checked_add(MAX_LINK_OPTIONS.checked_mul(size_of::<LinkOptionV1>())?))
            .ok_or(Resource::Arithmetic)?;
        Ok(Self { work, scratch })
    }
    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn scratch_storage(self) -> usize {
        self.scratch
    }
}

fn retained_storage(
    n: usize,
    tail: &DecodedCompactReplayTailV1,
) -> Result<NativeWorkerCompactReplayStorageV1> {
    let text = tail
        .link_options
        .iter()
        .try_fold(
            tail.worker.worker_build_identity().len() + tail.worker.llvm_build_identity().len(),
            |sum, option| {
                sum.checked_add(option.name().len())?
                    .checked_add(option.value().len())
            },
        )
        .ok_or(Resource::Arithmetic)?;
    let storage = size_of::<NativeWorkerCompactFinalizerReplayV1>()
        .checked_add(n)
        .and_then(|v| v.checked_add(text))
        .and_then(|v| {
            v.checked_add(
                tail.external_providers
                    .len()
                    .checked_mul(size_of::<WorkerV3ProviderReplayReferenceV1>())?,
            )
        })
        .and_then(|v| {
            v.checked_add(
                tail.link_options
                    .len()
                    .checked_mul(size_of::<LinkOptionV1>())?,
            )
        })
        .ok_or(Resource::Arithmetic)?;
    Ok(NativeWorkerCompactReplayStorageV1(storage))
}

/// Builds a compact transcript without detaching source custody or copying the
/// outer/finalized HSACO. Exact external providers are extracted by the existing
/// V2 helper, remain temporary external artifact owners and are not serialized.
/// The caller's finalized native storage floor remains paid. Besides the codec
/// quote this constructor charges a 512-work, FRAME-storage entry scope; artifact
/// extraction/metadata validation preserve their separate bounded domain.
pub fn prepare_native_worker_compact_finalizer_replay_v1(
    finalized: &PreparedFinalizedNativeWorkerHsacoV1,
    budget: &mut Budget<'_>,
) -> Result<(
    NativeWorkerCompactFinalizerReplayV1,
    NativeWorkerCompactReplayStorageV1,
)> {
    budget.with_prepaid_scope(
        finalized.required_retained_storage(),
        8,
        512,
        FRAME,
        |budget| {
            let source = finalized.source_evidence();
            let request = extract_native_worker_external_providers_v1(finalized)?;
            let bootstrap = source
                .bootstrap_response()
                .replay_metadata()
                .map_err(artifact_error)?;
            let replay = source
                .exact_replay_response()
                .replay_metadata()
                .map_err(artifact_error)?;
            validate_construction_parts(
                request.bootstrap_output_bound,
                &request.external_providers,
                source.plan().options(),
                bootstrap,
                replay,
            )?;
            let n = compact_replay_encoded_length(
                HEADER_BYTES - 8 - 2 - 64,
                true,
                source.worker_measurement(),
                &request.external_providers,
                source.plan().options(),
                bootstrap,
                replay,
            )?;
            let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(n)?;
            let floor = budget.storage();
            budget.with_prepaid_scope(floor, 8, quote.work, quote.scratch, |_| {
                let receipt = source.binding().receipt();
                let attempt = receipt.attempt();
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(n)
                    .map_err(|_| Resource::Allocation)?;
                bytes.extend_from_slice(MAGIC);
                bytes.extend_from_slice(&VERSION.to_le_bytes());
                bytes.extend_from_slice(finalized.identity().as_bytes());
                bytes.extend_from_slice(source.identity().as_bytes());
                bytes.extend_from_slice(source.binding().identity().as_bytes());
                bytes.extend_from_slice(receipt.handoff_identity().sha256());
                bytes.extend_from_slice(&receipt.handoff_identity().byte_len().to_le_bytes());
                bytes.extend_from_slice(&attempt.generation().to_le_bytes());
                bytes.extend_from_slice(attempt.session().as_bytes());
                bytes.extend_from_slice(attempt.invocation().as_bytes());
                bytes.push(receipt.slot() as u8);
                bytes.extend_from_slice(receipt.transaction_identity().as_bytes());
                encode_compact_replay_tail(
                    &mut bytes,
                    true,
                    source.worker_measurement(),
                    source.execution_limits(),
                    request.bootstrap_output_bound,
                    &request.external_providers,
                    source.plan().options(),
                    bootstrap,
                    replay,
                )?;
                let checksum = hash(CHECKSUM_DOMAIN, &bytes);
                bytes.extend_from_slice(&checksum);
                debug_assert_eq!(bytes.len(), n);
                NativeWorkerCompactFinalizerReplayV1::decode_owned(bytes)
            })
        },
    )
}

/// Exact external attachment extraction, using the same frozen V2 wire checks.
/// Returned payload owners use the existing bounded artifact domain, not the
/// compact codec's storage delta. No outer or finalized payload is copied here.
pub(crate) fn extract_native_worker_external_providers_v1(
    finalized: &PreparedFinalizedNativeWorkerHsacoV1,
) -> Result<OwnedWorkerV3RequestReplayPartsV1> {
    let source = finalized.source_evidence();
    extract_worker_v3_request_replay_parts_v1(
        source.bootstrap_request_bytes(),
        source.exact_replay_request_bytes(),
    )
    .map_err(artifact_error)
}

#[derive(Debug)]
pub enum NativeWorkerCompactReplayErrorV1 {
    Resource(Resource),
    Length,
    Magic,
    Version,
    Checksum,
    Coordinates,
    Tail(NativeWorkerDiagnosticV1),
}
type Result<T> = std::result::Result<T, NativeWorkerCompactReplayErrorV1>;
impl From<Resource> for NativeWorkerCompactReplayErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<TailError> for NativeWorkerCompactReplayErrorV1 {
    fn from(value: TailError) -> Self {
        artifact_error(value)
    }
}
impl fmt::Display for NativeWorkerCompactReplayErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(value) => value.fmt(f),
            Self::Length => f.write_str("native compact replay length is invalid"),
            Self::Magic => f.write_str("native compact replay magic mismatch"),
            Self::Version => f.write_str("unsupported native compact replay version"),
            Self::Checksum => f.write_str("native compact replay checksum mismatch"),
            Self::Coordinates => f.write_str("invalid inert native replay coordinates"),
            Self::Tail(value) => write!(f, "native compact replay tail: {value}"),
        }
    }
}
impl std::error::Error for NativeWorkerCompactReplayErrorV1 {}
fn artifact_error(value: impl fmt::Display) -> NativeWorkerCompactReplayErrorV1 {
    NativeWorkerCompactReplayErrorV1::Tail(NativeWorkerDiagnosticV1::from_display(value))
}
fn check_length(n: usize) -> Result<()> {
    if !(MIN_BYTES..=MAX_NATIVE_WORKER_COMPACT_FINALIZER_REPLAY_BYTES_V1).contains(&n) {
        return Err(NativeWorkerCompactReplayErrorV1::Length);
    }
    Ok(())
}
fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

fn decode_attempt(
    generation: u64,
    session: [u8; 16],
    invocation: [u8; 32],
) -> Result<BuildAttempt> {
    // BuildAttempt's binary constructor is intentionally private. Its public
    // inert parser owns the generation/direct-session invariants. Use a fixed
    // buffer, not allocating Display implementations of session/invocation.
    let mut bytes = [0_u8; 20 + 1 + 32 + 1 + 64];
    let mut output = std::io::Cursor::new(bytes.as_mut_slice());
    let invalid = |_| NativeWorkerCompactReplayErrorV1::Coordinates;
    write!(output, "{generation}:").map_err(invalid)?;
    for byte in session {
        write!(output, "{byte:02x}").map_err(invalid)?;
    }
    output.write_all(b":").map_err(invalid)?;
    for byte in invocation {
        write!(output, "{byte:02x}").map_err(invalid)?;
    }
    let n = output.position() as usize;
    let text = std::str::from_utf8(&bytes[..n])
        .map_err(|_| NativeWorkerCompactReplayErrorV1::Coordinates)?;
    BuildAttempt::from_env_value(text).map_err(|_| NativeWorkerCompactReplayErrorV1::Coordinates)
}

#[cfg(test)]
#[path = "native_worker_compact_replay_tests.rs"]
mod tests;
