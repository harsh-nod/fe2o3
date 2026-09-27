//! Conditional V5 restart framing, not source recovery or publication authority.
use super::{
    codec::{self, Core, Schema},
    *,
};
use fe2o3_artifact_transaction::{
    CompilerModuleHandoffSlotV5 as Slot, CompilerModuleHandoffTransactionIdentityV5 as Transaction,
};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV5 as Outer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalWorkerCompactReplayIdentityV5([u8; 32]);
impl ConditionalWorkerCompactReplayIdentityV5 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Copied V5 coordinates carry no receipt, currentness or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalWorkerReplayCoordinatesV5 {
    outer: ContentIdentityV1,
    attempt: BuildAttempt,
    transaction: [u8; 32],
}
impl ConditionalWorkerReplayCoordinatesV5 {
    pub const fn outer_identity_coordinates(self) -> ([u8; 32], u64) {
        (*self.outer.sha256(), self.outer.byte_len())
    }
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }
    pub const fn slot(self) -> Slot {
        Slot::Production
    }
    pub const fn transaction_identity(self) -> Transaction {
        Transaction::from_bytes(self.transaction)
    }
}

/// Move-only conditional transcript. The complete V5 handoff, provider payloads
/// and artifact remain separately retained inputs. Checksums authenticate none
/// of them. The V4 codec and all ordinary publication entrypoints reject this
/// distinct family; no down-conversion is provided.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{ConditionalWorkerCompactFinalizerReplayV5 as C,
///     NativeWorkerCompactFinalizerReplayV1 as N};
/// fn downgrade(value: C) -> N { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::ConditionalWorkerCompactFinalizerReplayV5 as C;
/// fn duplicate(value: C) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{ConditionalWorkerReplayCoordinatesV5 as C,
///     NativeWorkerReplayCoordinatesV1 as N};
/// fn downgrade(value: C) -> N { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::ConditionalWorkerCompactFinalizerReplayV5 as C;
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV4;
/// fn wrong_family(value: &C, outer: InertSemanticCompilerModuleHandoffIdentityV4) {
///     let _ = value.verify_outer_identity(outer);
/// }
/// ```
/// The matching V5 observation remains available without granting authority:
/// ```
/// use fe2o3_hsaco_finalize::{ConditionalWorkerCompactFinalizerReplayV5 as C,
///     NativeWorkerCompactReplayErrorV1 as E};
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV5;
/// fn matching_family(value: &C, outer: InertSemanticCompilerModuleHandoffIdentityV5) -> Result<(), E> {
///     value.verify_outer_identity(outer)
/// }
/// ```
pub struct ConditionalWorkerCompactFinalizerReplayV5 {
    core: Core,
}
impl ConditionalWorkerCompactFinalizerReplayV5 {
    pub const fn identity(&self) -> ConditionalWorkerCompactReplayIdentityV5 {
        ConditionalWorkerCompactReplayIdentityV5(self.core.identity)
    }
    pub const fn coordinates(&self) -> ConditionalWorkerReplayCoordinatesV5 {
        ConditionalWorkerReplayCoordinatesV5 {
            outer: self.core.header.outer,
            attempt: self.core.header.attempt,
            transaction: self.core.header.transaction,
        }
    }
    pub fn verify_outer_identity(&self, actual: Outer) -> Result<()> {
        self.core
            .header
            .verify_outer(actual.sha256(), actual.byte_len())
    }
    /// Compare only inert coordinates against an already retained finalizer.
    /// This does not replay source, Worker exchanges or artifact inspection.
    pub fn verify_finalized_coordinates(
        &self,
        finalized: &crate::PreparedFinalizedConditionalWorkerHsacoV5,
    ) -> Result<()> {
        let source = finalized.source();
        let receipt = source.binding().receipt();
        self.verify_outer_identity(receipt.handoff_identity())?;
        if self.expected_finalization_identity() != finalized.identity()
            || self.source_evidence_identity() != source.identity()
            || self.binding_identity() != source.binding().identity()
            || self.coordinates().attempt() != receipt.attempt()
            || self.coordinates().slot() != receipt.slot()
            || self.coordinates().transaction_identity() != receipt.transaction_identity()
        {
            return Err(NativeWorkerCompactReplayErrorV1::Coordinates);
        }
        Ok(())
    }
    pub const fn expected_finalization_identity(&self) -> &[u8; 32] {
        &self.core.header.finalization
    }
    pub const fn source_evidence_identity(&self) -> &[u8; 32] {
        &self.core.header.source
    }
    pub const fn binding_identity(&self) -> &[u8; 32] {
        &self.core.header.binding
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.core.bytes
    }
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.core.bytes
    }
    pub const fn storage(&self) -> NativeWorkerCompactReplayStorageV1 {
        self.core.storage
    }
    pub(crate) fn replay_view(&self) -> ProtectedWorkerV3CompactFinalizerReplayViewV2<'_> {
        self.core.tail.replay_view(&self.core.bytes)
    }
    /// Prepay the borrowed wire; reserve the returned additional storage before
    /// retention. The shared codec quote covers framing, not Worker/proof replay,
    /// caller spare capacities, native I/O, allocator overhead or RSS.
    pub fn decode_canonical(
        bytes: &[u8],
        b: &mut Budget<'_>,
    ) -> Result<(Self, NativeWorkerCompactReplayStorageV1)> {
        let core = Core::decode(bytes, Schema::Conditional, b)?;
        let storage = core.storage;
        Ok((Self { core }, storage))
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

/// Derive framing from the actual retained conditional finalizer owner without
/// cloning its handoff, artifact or proof owners. Source custody remains with
/// the caller, prepaid for the entire operation. Uses the same frozen Worker
/// request/response parser and derivation-capable tail as native V4.
pub fn prepare_conditional_worker_compact_finalizer_replay_v5(
    finalized: &crate::PreparedFinalizedConditionalWorkerHsacoV5,
    b: &mut Budget<'_>,
) -> Result<(
    ConditionalWorkerCompactFinalizerReplayV5,
    NativeWorkerCompactReplayStorageV1,
)> {
    let core = codec::prepare(
        finalized.required_retained_storage(),
        Schema::Conditional,
        b,
        || {
            let source = finalized.source();
            let receipt = source.binding().receipt();
            codec::Inputs {
                header: codec::Header {
                    finalization: *finalized.identity(),
                    source: *source.identity(),
                    binding: *source.binding().identity(),
                    outer: ContentIdentityV1::from_parts(
                        *receipt.handoff_identity().sha256(),
                        receipt.handoff_identity().byte_len(),
                    ),
                    attempt: receipt.attempt(),
                    slot: receipt.slot() as u8,
                    transaction: *receipt.transaction_identity().as_bytes(),
                },
                requests: [
                    source.bootstrap_request_bytes(),
                    source.replay_request_bytes(),
                ],
                responses: [source.bootstrap_response(), source.replay_response()],
                worker: source.worker_measurement(),
                limits: source.execution_limits(),
                options: source.plan().options(),
            }
        },
    )?;
    let storage = core.storage;
    Ok((ConditionalWorkerCompactFinalizerReplayV5 { core }, storage))
}

const _: () = assert!(
    size_of::<ConditionalWorkerCompactFinalizerReplayV5>()
        == size_of::<NativeWorkerCompactFinalizerReplayV1>()
);
