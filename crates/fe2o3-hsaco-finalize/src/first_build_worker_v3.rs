//! Native strict-V3 compiler handoff execution through the direct LLVM worker.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffReceiptV3, CompilerModuleHandoffSlotV3,
    CompilerModuleHandoffTransactionIdentityV3, ConsumedCompilerModuleHandoffV3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2,
    CompilerModuleSymbolRoleV1, FinalCompilerModuleCommitmentErrorV3,
    InertFinalCompilerModuleCommitmentV3, InertSemanticCompilerModuleHandoffErrorV3,
    InertSemanticCompilerModuleHandoffIdentityV3, InertSemanticCompilerModuleHandoffV3,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, LinkInputV1, LinkOptionV1, LinkOutputV1, LinkPlanError, MAX_LINK_INPUTS,
    MAX_LINK_OPTIONS, MAX_WORKER_REQUEST_BYTES, MAX_WORKER_SYMBOL_BYTES, MAX_WORKER_SYMBOLS,
    MAX_WORKER_TOTAL_INPUT_BYTES, MultiInputLinkPlanV1, PinnedWorkerV1, ProvenanceNodeV1,
    WorkerExecutionError, WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1,
    WorkerMeasurementV1, WorkerOutputConstraintsV1, WorkerProtocolError,
    WorkerRequestConstructionError, WorkerResponseV2,
    first_build_worker_engine::{
        ReproducibleFirstBuildEngineError, ReproducibleFirstBuildEnginePreflight,
        execute_preflighted_reproducible_first_build_engine,
        preflight_reproducible_first_build_engine,
    },
    request_construction::{decode_link_options, decoded_compiler_module_handoff_v2},
    worker_executor::InertWorkerExecutionV2,
};

const BINDING_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/PROTECTED-WORKER-COMPILER-HANDOFF-BINDING/V3\0";
const EVIDENCE_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/PROTECTED-FIRST-BUILD-WORKER-EVIDENCE/V3\0";
const WORKER_REQUEST_MAGIC_V2: &[u8; 8] = b"F3LREQ02";
const WORKER_REQUEST_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/DIRECT-LLVM-WORKER-REQUEST/V2\0";
const PROTECTED_FIRST_BUILD_REQUEST_DOMAIN_V3: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-FIRST-BUILD-WORKER-REQUEST/V3\0";
const PROTECTED_PLAN_REQUEST_DOMAIN_V3: &[u8] =
    b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-PLAN-BOUND-WORKER-REQUEST/V3\0";
const INPUT_KIND_CLOSURE_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-LINK-INPUT-KIND-CLOSURE/V1\0";
const STAGED_COMPILER_FFI_ENVELOPE_DOMAIN_V1: &[u8] = b"FE2O3/STAGED-COMPILER-FFI-ENVELOPE/V1\0";
const WORKER_REQUEST_FIELD_COUNT_V2: usize = 15;
const WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2: usize = 1 + 32 + 8;
const WORKER_REQUEST_FIXED_BUDGET_BYTES_V3: usize = 4096;
const RETAINED_INPUT_COPIES_DURING_PREFLIGHT_V3: usize = 4;
const RETAINED_REQUEST_COPIES_DURING_PREFLIGHT_V3: usize = 3;

/// Maximum aggregate V3 handoff, decoded input, and request working set admitted to preflight.
///
/// This is deliberately lower than the sum of every independent wire maximum. The inherited V2
/// engine temporarily retains several exact copies while sealing candidate and replay requests;
/// admitting all maxima simultaneously would allow a schema-valid request to amplify beyond a
/// practical production working set before the worker starts.
const MAX_PROTECTED_V3_LIVE_INPUT_REQUEST_BYTES_V1: usize = 512 * 1024 * 1024;

/// Stable identity of one exact protected Worker V3 compiler-handoff binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedCompilerHandoffBindingIdentityV3([u8; 32]);

impl ProtectedCompilerHandoffBindingIdentityV3 {
    /// Returns the domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Deterministically validated strict-V3 worker inputs prepared before one-shot consumption.
///
/// This move-only owner binds the durable receipt, exact semantic handoff, compiler closure,
/// measured worker, providers, options, output bound, and both Worker request shapes. It is inert
/// and grants no compiler, process, publication, load, or launch authority.
pub struct PreparedProtectedFirstBuildWorkerV3PreflightV1 {
    binding: ProtectedCompilerHandoffBindingV3,
    worker: WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    engine: ReproducibleFirstBuildEnginePreflight,
}

impl PreparedProtectedFirstBuildWorkerV3PreflightV1 {
    /// Returns the exact transaction and semantic-handoff binding validated by preflight.
    pub const fn binding(&self) -> ProtectedCompilerHandoffBindingV3 {
        self.binding
    }

    /// Returns the exact measured worker selected during preflight.
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    /// Returns the exact execution limits selected during preflight.
    pub const fn execution_limits(&self) -> WorkerExecutionLimitsV1 {
        self.limits
    }

    /// Reports that preflight grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that preflight grants no worker or linker authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Reports that preflight grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
}

/// Exact inert identities expected to remain associated at the Worker V3 boundary.
///
/// These fields are structural and transaction evidence only. They do not authenticate rustc,
/// prove compiler derivation, or grant worker, link, publication, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedCompilerHandoffExpectationV3 {
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV3,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    receipt_byte_len: u64,
    outer_handoff_identity: InertSemanticCompilerModuleHandoffIdentityV3,
    capsule_sha256: [u8; 32],
    capsule_byte_len: u64,
    invocation_digest: [u8; 32],
    pair_binding_sha256: [u8; 32],
    pair_binding_byte_len: u64,
    nested_handoff_identity: CompilerModuleHandoffIdentityV2,
    final_commitment_receipt_sha256: [u8; 32],
    final_commitment_receipt_byte_len: u64,
    final_commitment_sha256: [u8; 32],
    final_commitment_byte_len: u64,
    compiler_closure: CompilerClosureV2,
}

impl ProtectedCompilerHandoffExpectationV3 {
    /// Returns the cooperative build attempt carried by the consumed V3 transaction.
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the exact V3 transaction slot.
    pub const fn slot(self) -> CompilerModuleHandoffSlotV3 {
        self.slot
    }

    /// Returns the attempt-, producer-, slot-, and payload-bound transaction identity.
    pub const fn transaction_identity(self) -> CompilerModuleHandoffTransactionIdentityV3 {
        self.transaction_identity
    }

    /// Returns the exact canonical handoff length retained by the durable receipt.
    pub const fn receipt_byte_len(self) -> u64 {
        self.receipt_byte_len
    }

    /// Returns the terminal identity of the complete outer V3 handoff.
    pub const fn outer_handoff_identity(self) -> InertSemanticCompilerModuleHandoffIdentityV3 {
        self.outer_handoff_identity
    }

    /// Returns the exact semantic capsule SHA-256 identity.
    pub const fn capsule_sha256(self) -> [u8; 32] {
        self.capsule_sha256
    }

    /// Returns the exact semantic capsule canonical length.
    pub const fn capsule_byte_len(self) -> u64 {
        self.capsule_byte_len
    }

    /// Returns the digest rederived from the exact retained rustc invocation descriptor.
    pub const fn invocation_digest(self) -> [u8; 32] {
        self.invocation_digest
    }

    /// Returns the exact capsule-to-module pair-binding SHA-256 identity.
    pub const fn pair_binding_sha256(self) -> [u8; 32] {
        self.pair_binding_sha256
    }

    /// Returns the exact capsule-to-module pair-binding canonical length.
    pub const fn pair_binding_byte_len(self) -> u64 {
        self.pair_binding_byte_len
    }

    /// Returns the exact nested V2 compiler-module handoff identity.
    pub const fn nested_handoff_identity(self) -> CompilerModuleHandoffIdentityV2 {
        self.nested_handoff_identity
    }

    /// Returns the exact final-commitment receipt SHA-256 identity.
    pub const fn final_commitment_receipt_sha256(self) -> [u8; 32] {
        self.final_commitment_receipt_sha256
    }

    /// Returns the exact final-commitment receipt canonical length.
    pub const fn final_commitment_receipt_byte_len(self) -> u64 {
        self.final_commitment_receipt_byte_len
    }

    /// Returns the terminal identity of the compact final-module commitment.
    pub const fn final_commitment_sha256(self) -> [u8; 32] {
        self.final_commitment_sha256
    }

    /// Returns the compact final-module commitment canonical length.
    pub const fn final_commitment_byte_len(self) -> u64 {
        self.final_commitment_byte_len
    }

    /// Returns the compiler closure retained by the exact invocation descriptor.
    pub const fn compiler_closure(self) -> CompilerClosureV2 {
        self.compiler_closure
    }
}

/// Closed Worker V3 binding derived from one durable strict-V3 receipt and exact inert handoff.
///
/// Construction repeats the receipt, capsule, invocation, pair, compact commitment, and nested V2
/// associations. The transaction identity is accepted only from the artifact-transaction receipt;
/// it cannot be recomputed here because the producer namespace remains private to that crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedCompilerHandoffBindingV3 {
    expectation: ProtectedCompilerHandoffExpectationV3,
    identity: ProtectedCompilerHandoffBindingIdentityV3,
}

impl ProtectedCompilerHandoffBindingV3 {
    pub(crate) fn from_consumed(
        consumed: &ConsumedCompilerModuleHandoffV3,
        expected_receipt: CompilerModuleHandoffReceiptV3,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<Self, ProtectedCompilerHandoffBindingErrorV3> {
        if consumed.attempt() != expected_receipt.attempt() {
            return Err(binding_mismatch("parent build attempt"));
        }
        if consumed.slot() != expected_receipt.slot() {
            return Err(binding_mismatch("parent V3 slot"));
        }
        if consumed.transaction_identity() != expected_receipt.transaction_identity() {
            return Err(binding_mismatch("parent V3 transaction identity"));
        }
        if consumed.handoff_identity() != expected_receipt.handoff_identity() {
            return Err(binding_mismatch("parent outer V3 handoff identity"));
        }
        Self::from_handoff(
            consumed.handoff(),
            expected_receipt,
            expected_compiler_closure,
        )
    }

    pub(crate) fn from_handoff(
        handoff: &InertSemanticCompilerModuleHandoffV3,
        expected_receipt: CompilerModuleHandoffReceiptV3,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<Self, ProtectedCompilerHandoffBindingErrorV3> {
        let exact_bytes = handoff.canonical_bytes();
        let expected_identity = expected_receipt.handoff_identity();
        let receipt_byte_len = u64::try_from(expected_receipt.length())
            .map_err(|_| binding_mismatch("parent receipt byte length"))?;
        if handoff.identity() != expected_identity
            || expected_receipt.length() != exact_bytes.len()
            || expected_identity.byte_len() != receipt_byte_len
            || !expected_identity.matches_canonical_bytes(exact_bytes)
        {
            return Err(binding_mismatch("parent outer V3 handoff identity"));
        }
        Self::from_handoff_parts(
            handoff,
            expected_receipt.attempt(),
            expected_receipt.slot(),
            expected_receipt.transaction_identity(),
            expected_receipt.length(),
            expected_compiler_closure,
        )
    }

    /// Rederives a binding after restart from the durable occurrence and compact V2 transcript.
    ///
    /// The slot and transaction identity are inert transaction observations. Every semantic,
    /// invocation, closure, pair, nested-handoff, and final-commitment axis is independently
    /// recovered from the exact outer handoff rather than accepted from replay metadata.
    pub(crate) fn from_replay_parts(
        handoff: &InertSemanticCompilerModuleHandoffV3,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    ) -> Result<Self, ProtectedCompilerHandoffBindingErrorV3> {
        Self::from_handoff_parts(
            handoff,
            attempt,
            slot,
            transaction_identity,
            handoff.canonical_bytes().len(),
            *handoff.capsule().compiler_closure(),
        )
    }

    fn from_handoff_parts(
        handoff: &InertSemanticCompilerModuleHandoffV3,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
        receipt_byte_len: usize,
        expected_compiler_closure: CompilerClosureV2,
    ) -> Result<Self, ProtectedCompilerHandoffBindingErrorV3> {
        if handoff.identity().byte_len()
            != u64::try_from(receipt_byte_len)
                .map_err(|_| binding_mismatch("parent receipt byte length"))?
        {
            return Err(binding_mismatch("parent outer V3 handoff identity"));
        }

        let exact_bytes = handoff.canonical_bytes();
        let receipt_byte_len_u64 = u64::try_from(receipt_byte_len)
            .map_err(|_| binding_mismatch("parent receipt byte length"))?;
        if receipt_byte_len != exact_bytes.len()
            || handoff.identity().byte_len() != receipt_byte_len_u64
            || !handoff.identity().matches_canonical_bytes(exact_bytes)
        {
            return Err(binding_mismatch("parent receipt byte length"));
        }

        let capsule = handoff.capsule();
        let nested = handoff.module_handoff();
        let pair = handoff.pair_binding();
        if pair.capsule_identity() != capsule.identity() {
            return Err(binding_mismatch("semantic capsule pair member"));
        }
        if pair.module_handoff_identity() != nested.identity() {
            return Err(binding_mismatch("nested V2 pair member"));
        }
        if capsule.target() != nested.target() {
            return Err(binding_mismatch("capsule and nested V2 target"));
        }
        if capsule.compiler_closure() != capsule.invocation().compiler_closure() {
            return Err(binding_mismatch("invocation compiler closure"));
        }
        if *capsule.compiler_closure() != expected_compiler_closure {
            return Err(binding_mismatch("parent compiler closure"));
        }

        let final_receipt = capsule.receipts().final_compiler_module_commitment();
        let final_commitment =
            InertFinalCompilerModuleCommitmentV3::decode(final_receipt.canonical_preimage())
                .map_err(ProtectedCompilerHandoffBindingErrorV3::FinalCommitment)?;
        if !final_commitment.matches_handoff(nested) {
            return Err(binding_mismatch("final commitment and nested V2 handoff"));
        }

        let capsule_identity = capsule.identity();
        let pair_identity = pair.identity();
        let final_receipt_identity = final_receipt.identity();
        let final_commitment_identity = final_commitment.identity();
        let expectation = ProtectedCompilerHandoffExpectationV3 {
            attempt,
            slot,
            transaction_identity,
            receipt_byte_len: receipt_byte_len_u64,
            outer_handoff_identity: handoff.identity(),
            capsule_sha256: *capsule_identity.sha256(),
            capsule_byte_len: capsule_identity.byte_len(),
            invocation_digest: *capsule.invocation_digest().as_bytes(),
            pair_binding_sha256: *pair_identity.sha256(),
            pair_binding_byte_len: pair_identity.byte_len(),
            nested_handoff_identity: nested.identity(),
            final_commitment_receipt_sha256: *final_receipt_identity.sha256(),
            final_commitment_receipt_byte_len: final_receipt_identity.byte_len(),
            final_commitment_sha256: *final_commitment_identity.sha256(),
            final_commitment_byte_len: final_commitment_identity.byte_len(),
            compiler_closure: *capsule.compiler_closure(),
        };
        let identity = calculate_binding_identity(expectation);
        Ok(Self {
            expectation,
            identity,
        })
    }

    /// Returns the exact inert associations retained by this binding.
    pub const fn expectation(self) -> ProtectedCompilerHandoffExpectationV3 {
        self.expectation
    }

    /// Returns the domain-separated identity of every retained binding axis.
    pub const fn identity(self) -> ProtectedCompilerHandoffBindingIdentityV3 {
        self.identity
    }

    pub(crate) fn hash_identity_preimage(self, hasher: &mut Sha256) {
        hash_binding_preimage(hasher, self.expectation);
    }

    /// Reports that this structural binding does not authenticate compiler origin.
    pub const fn authenticates_compiler_origin(self) -> bool {
        false
    }

    /// Reports that this structural binding grants no link authority.
    pub const fn grants_link_authority(self) -> bool {
        false
    }

    /// Reports that this structural binding grants no compiler authority.
    pub const fn grants_compiler_authority(self) -> bool {
        false
    }

    /// Reports that this structural binding grants no publication authority.
    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    /// Reports that this structural binding grants no load authority.
    pub const fn grants_load_authority(self) -> bool {
        false
    }

    /// Reports that this structural binding grants no launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Failure while deriving the exact Worker V3 binding from a consumed strict handoff.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedCompilerHandoffBindingErrorV3 {
    /// Strict redecoding of the complete outer V3 owner failed.
    OuterHandoff(InertSemanticCompilerModuleHandoffErrorV3),
    /// Strict decoding of the compact final-module commitment failed.
    FinalCommitment(FinalCompilerModuleCommitmentErrorV3),
    /// Two exact retained relationship axes disagreed.
    RelationshipMismatch { field: &'static str },
}

impl fmt::Display for ProtectedCompilerHandoffBindingErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OuterHandoff(error) => write!(formatter, "invalid strict V3 handoff: {error}"),
            Self::FinalCommitment(error) => {
                write!(
                    formatter,
                    "invalid final compiler-module commitment: {error}"
                )
            }
            Self::RelationshipMismatch { field } => {
                write!(
                    formatter,
                    "strict V3 handoff relationship mismatch: {field}"
                )
            }
        }
    }
}

impl Error for ProtectedCompilerHandoffBindingErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OuterHandoff(error) => Some(error),
            Self::FinalCommitment(error) => Some(error),
            Self::RelationshipMismatch { .. } => None,
        }
    }
}

const fn binding_mismatch(field: &'static str) -> ProtectedCompilerHandoffBindingErrorV3 {
    ProtectedCompilerHandoffBindingErrorV3::RelationshipMismatch { field }
}

/// Move-only measured worker execution retaining its complete strict-V3 binding.
///
/// The underlying exchange uses the existing direct-LLVM Worker V2 wire protocol. This wrapper
/// neither converts the compiler transaction to V2 nor grants artifact authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProtectedCompilerHandoffExecutionV3 {
    binding: ProtectedCompilerHandoffBindingV3,
    execution: InertWorkerExecutionV2,
}

impl InertProtectedCompilerHandoffExecutionV3 {
    fn from_execution(
        binding: ProtectedCompilerHandoffBindingV3,
        execution: InertWorkerExecutionV2,
    ) -> Self {
        Self { binding, execution }
    }

    /// Returns the complete strict-V3 binding retained across execution.
    pub const fn binding(&self) -> ProtectedCompilerHandoffBindingV3 {
        self.binding
    }

    /// Returns the exact measured worker executable identity.
    pub const fn worker_executable(&self) -> ContentIdentityV1 {
        self.execution.worker_executable()
    }

    /// Returns the exact response decoded against this execution's sealed request.
    pub const fn response(&self) -> &WorkerResponseV2 {
        self.execution.response()
    }

    /// Reports that execution evidence grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that execution evidence grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that execution evidence grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Reports that execution evidence grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that execution evidence grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Stable identity of one successful strict-V3 bootstrap and exact replay workflow.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedFirstBuildWorkerV3IdentityV1([u8; 32]);

impl ProtectedFirstBuildWorkerV3IdentityV1 {
    /// Returns the domain-separated evidence identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ValidatedProtectedFirstBuildReplayV3;

/// Inert evidence for one reproducible direct-LLVM worker run retaining exact V3 custody.
///
/// Its identity binds the attempt, V3 slot and transaction, outer handoff, capsule, invocation,
/// pair binding, nested V2 handoff, final commitment, compiler closure, measured worker, complete
/// link plan, and exact bootstrap/replay request and response bytes. These are structural and
/// current transaction observations, not compiler authentication or runtime authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProtectedFirstBuildWorkerV3EvidenceV1 {
    identity: ProtectedFirstBuildWorkerV3IdentityV1,
    binding: ProtectedCompilerHandoffBindingV3,
    handoff: InertSemanticCompilerModuleHandoffV3,
    worker: WorkerMeasurementV1,
    execution_limits: WorkerExecutionLimitsV1,
    plan: MultiInputLinkPlanV1,
    bootstrap_request_bytes: Vec<u8>,
    bootstrap: InertProtectedCompilerHandoffExecutionV3,
    replay_request_bytes: Vec<u8>,
    replay: InertProtectedCompilerHandoffExecutionV3,
    _validation: ValidatedProtectedFirstBuildReplayV3,
}

pub(crate) struct OwnedProtectedFirstBuildWorkerV3ReplayPartsV1 {
    pub(crate) identity: ProtectedFirstBuildWorkerV3IdentityV1,
    pub(crate) binding: ProtectedCompilerHandoffBindingV3,
    pub(crate) handoff: InertSemanticCompilerModuleHandoffV3,
    pub(crate) worker: WorkerMeasurementV1,
    pub(crate) execution_limits: WorkerExecutionLimitsV1,
    pub(crate) plan: MultiInputLinkPlanV1,
    pub(crate) bootstrap_request_bytes: Vec<u8>,
    pub(crate) bootstrap_response: WorkerResponseV2,
    pub(crate) replay_request_bytes: Vec<u8>,
    pub(crate) replay_response: WorkerResponseV2,
}

impl InertProtectedFirstBuildWorkerV3EvidenceV1 {
    /// Returns the complete evidence identity.
    pub const fn identity(&self) -> ProtectedFirstBuildWorkerV3IdentityV1 {
        self.identity
    }

    /// Returns every strict-V3 binding axis retained by this evidence.
    pub const fn binding(&self) -> ProtectedCompilerHandoffBindingV3 {
        self.binding
    }

    /// Returns the complete exact outer semantic compiler-module handoff.
    pub const fn handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        &self.handoff
    }

    /// Returns the exact measured worker declaration.
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    /// Returns the exact resource limits applied to each measured worker execution.
    pub const fn execution_limits(&self) -> WorkerExecutionLimitsV1 {
        self.execution_limits
    }

    /// Returns the complete canonical link plan.
    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        &self.plan
    }

    /// Returns the exact bootstrap request bytes sent to the direct LLVM worker.
    pub fn bootstrap_request_bytes(&self) -> &[u8] {
        &self.bootstrap_request_bytes
    }

    /// Returns the first measured direct-LLVM worker execution.
    pub const fn bootstrap(&self) -> &InertProtectedCompilerHandoffExecutionV3 {
        &self.bootstrap
    }

    /// Returns the exact-output replay request bytes sent to the direct LLVM worker.
    pub fn exact_replay_request_bytes(&self) -> &[u8] {
        &self.replay_request_bytes
    }

    /// Returns the measured exact-output replay execution.
    pub const fn exact_replay(&self) -> &InertProtectedCompilerHandoffExecutionV3 {
        &self.replay
    }

    /// Borrows the exact replay output for inert inspection.
    pub fn output_bytes(&self) -> &[u8] {
        self.replay
            .response()
            .output()
            .expect("validated Worker V3 replay retains output")
            .bytes()
    }

    /// Returns the exact output identity committed by the complete link plan.
    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.plan.output().identity()
    }

    /// Reports that this evidence does not authenticate compiler origin.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    /// Reports that this evidence grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Reports that this evidence grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that this evidence grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that this evidence grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that this evidence grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn into_compact_replay_parts(self) -> OwnedProtectedFirstBuildWorkerV3ReplayPartsV1 {
        let Self {
            identity,
            binding,
            handoff,
            worker,
            execution_limits,
            plan,
            bootstrap_request_bytes,
            bootstrap,
            replay_request_bytes,
            replay,
            _validation: _,
        } = self;
        debug_assert_eq!(bootstrap.binding, binding);
        debug_assert_eq!(replay.binding, binding);
        OwnedProtectedFirstBuildWorkerV3ReplayPartsV1 {
            identity,
            binding,
            handoff,
            worker,
            execution_limits,
            plan,
            bootstrap_request_bytes,
            bootstrap_response: bootstrap.execution.into_response(),
            replay_request_bytes,
            replay_response: replay.execution.into_response(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn recover_inert_protected_first_build_worker_v3_evidence_v1(
    binding: ProtectedCompilerHandoffBindingV3,
    handoff: InertSemanticCompilerModuleHandoffV3,
    worker: WorkerMeasurementV1,
    execution_limits: WorkerExecutionLimitsV1,
    plan: MultiInputLinkPlanV1,
    bootstrap_request_bytes: Vec<u8>,
    bootstrap_response: WorkerResponseV2,
    replay_request_bytes: Vec<u8>,
    replay_response: WorkerResponseV2,
) -> Result<InertProtectedFirstBuildWorkerV3EvidenceV1, ProtectedFirstBuildWorkerV3Error> {
    let decoded = crate::request_construction::decode_compiler_module_handoff_v2(
        handoff.module_handoff().canonical_bytes(),
    )
    .map_err(ProtectedFirstBuildWorkerV3Error::CompilerModuleHandoff)?;
    let validation = validate_replay_parts(
        binding,
        &worker,
        &decoded,
        &plan,
        &bootstrap_request_bytes,
        &bootstrap_response,
        &replay_request_bytes,
        &replay_response,
    )?;
    let identity = calculate_evidence_identity_parts(
        binding,
        &worker,
        execution_limits,
        &plan,
        &bootstrap_request_bytes,
        bootstrap_response.canonical_bytes(),
        &replay_request_bytes,
        replay_response.canonical_bytes(),
    )?;
    let worker_executable = worker.executable();
    let bootstrap = InertProtectedCompilerHandoffExecutionV3::from_execution(
        binding,
        crate::worker_executor::InertWorkerExecutionV2::from_recovered_response(
            worker_executable,
            bootstrap_response,
        ),
    );
    let replay = InertProtectedCompilerHandoffExecutionV3::from_execution(
        binding,
        crate::worker_executor::InertWorkerExecutionV2::from_recovered_response(
            worker_executable,
            replay_response,
        ),
    );
    Ok(InertProtectedFirstBuildWorkerV3EvidenceV1 {
        identity,
        binding,
        handoff,
        worker,
        execution_limits,
        plan,
        bootstrap_request_bytes,
        bootstrap,
        replay_request_bytes,
        replay,
        _validation: validation,
    })
}

/// Failure from the native strict-V3 bootstrap and exact replay workflow.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedFirstBuildWorkerV3Error {
    /// The complete V3 handoff relationships failed closed validation.
    Binding(ProtectedCompilerHandoffBindingErrorV3),
    /// The exact nested V2 compiler module failed strict decoding.
    CompilerModuleHandoff(CompilerModuleHandoffErrorV2),
    /// Canonical link-plan derivation failed.
    LinkPlan(LinkPlanError),
    /// The number of caller-owned external inputs cannot fit the bounded V3 link closure.
    WorkingSetInputCountExceeded { actual: usize, maximum: usize },
    /// The number of caller-owned link options cannot fit the bounded V3 plan.
    WorkingSetOptionCountExceeded { actual: usize, maximum: usize },
    /// Checked working-set accounting overflowed before worker execution.
    WorkingSetArithmeticOverflow { component: &'static str },
    /// Aggregate live handoff, input, and request storage exceeds the V3 production budget.
    WorkingSetBudgetExceeded {
        required_bytes: u64,
        maximum_bytes: u64,
    },
    /// A bounded metadata collection could not reserve its exact capacity.
    WorkingSetAllocationFailed { component: &'static str },
    /// Sealed direct-LLVM request construction failed.
    RequestConstruction(WorkerRequestConstructionError),
    /// Consumed transaction or measured worker did not match the sealed preflight owner.
    PreflightMismatch { field: &'static str },
    /// The bootstrap request was invalid.
    BootstrapRequest(WorkerProtocolError),
    /// The bootstrap worker process failed.
    BootstrapExecution(WorkerExecutionError),
    /// The bootstrap worker completed without output.
    BootstrapDidNotProduceOutput(Box<InertProtectedCompilerHandoffExecutionV3>),
    /// The exact replay worker process failed.
    ReplayExecution(WorkerExecutionError),
    /// The exact replay completed without output.
    ReplayDidNotProduceOutput {
        bootstrap: Box<InertProtectedCompilerHandoffExecutionV3>,
        replay: Box<InertProtectedCompilerHandoffExecutionV3>,
    },
    /// Bootstrap and exact replay produced different bytes.
    OutputMismatch {
        bootstrap: Box<InertProtectedCompilerHandoffExecutionV3>,
        replay: Box<InertProtectedCompilerHandoffExecutionV3>,
    },
    /// Independent exact transcript replay validation failed.
    ReplayValidation { field: &'static str },
}

impl fmt::Display for ProtectedFirstBuildWorkerV3Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(error) => error.fmt(formatter),
            Self::CompilerModuleHandoff(error) => {
                write!(
                    formatter,
                    "invalid nested V2 compiler-module handoff: {error}"
                )
            }
            Self::LinkPlan(error) => write!(formatter, "invalid derived link plan: {error}"),
            Self::WorkingSetInputCountExceeded { actual, maximum } => write!(
                formatter,
                "strict-V3 external input count {actual} exceeds the working-set bound {maximum}"
            ),
            Self::WorkingSetOptionCountExceeded { actual, maximum } => write!(
                formatter,
                "strict-V3 link option count {actual} exceeds the working-set bound {maximum}"
            ),
            Self::WorkingSetArithmeticOverflow { component } => write!(
                formatter,
                "strict-V3 working-set accounting overflowed at {component}"
            ),
            Self::WorkingSetBudgetExceeded {
                required_bytes,
                maximum_bytes,
            } => write!(
                formatter,
                "strict-V3 live input/request working set {required_bytes} exceeds the production budget {maximum_bytes}"
            ),
            Self::WorkingSetAllocationFailed { component } => write!(
                formatter,
                "strict-V3 bounded metadata allocation failed at {component}"
            ),
            Self::RequestConstruction(error) => {
                write!(
                    formatter,
                    "strict-V3 worker request construction failed: {error}"
                )
            }
            Self::PreflightMismatch { field } => {
                write!(formatter, "strict-V3 worker preflight mismatch: {field}")
            }
            Self::BootstrapRequest(error) => {
                write!(formatter, "strict-V3 bootstrap request is invalid: {error}")
            }
            Self::BootstrapExecution(error) => {
                write!(
                    formatter,
                    "strict-V3 bootstrap worker execution failed: {error}"
                )
            }
            Self::BootstrapDidNotProduceOutput(execution) => {
                write!(
                    formatter,
                    "strict-V3 bootstrap produced no output at {:?}",
                    execution.response().stage()
                )?;
                write_worker_diagnostics(formatter, execution.response().diagnostics())
            }
            Self::ReplayExecution(error) => {
                write!(
                    formatter,
                    "strict-V3 exact replay worker execution failed: {error}"
                )
            }
            Self::ReplayDidNotProduceOutput { replay, .. } => {
                write!(
                    formatter,
                    "strict-V3 exact replay produced no output at {:?}",
                    replay.response().stage()
                )?;
                write_worker_diagnostics(formatter, replay.response().diagnostics())
            }
            Self::OutputMismatch { .. } => formatter
                .write_str("strict-V3 bootstrap and exact replay worker output bytes differ"),
            Self::ReplayValidation { field } => {
                write!(formatter, "strict-V3 replay validation failed: {field}")
            }
        }
    }
}

fn write_worker_diagnostics(
    formatter: &mut fmt::Formatter<'_>,
    diagnostics: &[String],
) -> fmt::Result {
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        formatter.write_str(if index == 0 { ": " } else { "; " })?;
        formatter.write_str(diagnostic)?;
    }
    Ok(())
}

impl Error for ProtectedFirstBuildWorkerV3Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Binding(error) => Some(error),
            Self::CompilerModuleHandoff(error) => Some(error),
            Self::LinkPlan(error) => Some(error),
            Self::RequestConstruction(error) => Some(error),
            Self::BootstrapRequest(error) => Some(error),
            Self::BootstrapExecution(error) | Self::ReplayExecution(error) => Some(error),
            Self::BootstrapDidNotProduceOutput(_)
            | Self::WorkingSetInputCountExceeded { .. }
            | Self::WorkingSetOptionCountExceeded { .. }
            | Self::WorkingSetArithmeticOverflow { .. }
            | Self::WorkingSetBudgetExceeded { .. }
            | Self::WorkingSetAllocationFailed { .. }
            | Self::PreflightMismatch { .. }
            | Self::ReplayDidNotProduceOutput { .. }
            | Self::OutputMismatch { .. }
            | Self::ReplayValidation { .. } => None,
        }
    }
}

/// Validates and seals every deterministic strict-V3 worker input before one-shot consumption.
///
/// The caller must retain its artifact-transaction currentness lease while this function borrows
/// the exact handoff named by `expected_receipt`. No worker process is started. Success proves that
/// both candidate and replay request shapes are protocol-valid for the selected configuration.
#[allow(clippy::too_many_arguments)]
pub fn preflight_protected_reproducible_first_build_worker_v3(
    handoff: &InertSemanticCompilerModuleHandoffV3,
    expected_receipt: CompilerModuleHandoffReceiptV3,
    expected_compiler_closure: CompilerClosureV2,
    worker: &PinnedWorkerV1,
    external_providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    candidate_output_bound: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<PreparedProtectedFirstBuildWorkerV3PreflightV1, ProtectedFirstBuildWorkerV3Error> {
    let binding = ProtectedCompilerHandoffBindingV3::from_handoff(
        handoff,
        expected_receipt,
        expected_compiler_closure,
    )
    .map_err(ProtectedFirstBuildWorkerV3Error::Binding)?;
    enforce_protected_v3_working_set_budget(handoff, &external_providers, &link_options)?;
    let decoded = decoded_compiler_module_handoff_v2(handoff.module_handoff().clone())
        .map_err(ProtectedFirstBuildWorkerV3Error::CompilerModuleHandoff)?;
    let engine = preflight_reproducible_first_build_engine(
        &binding,
        decoded,
        worker,
        external_providers,
        link_options,
        candidate_output_bound,
    )
    .map_err(|error| map_engine_error(binding, error))?;
    Ok(PreparedProtectedFirstBuildWorkerV3PreflightV1 {
        binding,
        worker: worker.measurement().clone(),
        limits,
        engine,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProtectedV3WorkingSetDimensions {
    outer_handoff_bytes: usize,
    compiler_module_bytes: usize,
    provider_payload_bytes: usize,
    provider_count: usize,
    option_text_bytes: usize,
    option_count: usize,
    envelope_bytes: usize,
    manifest_bytes: usize,
}

fn enforce_protected_v3_working_set_budget(
    handoff: &InertSemanticCompilerModuleHandoffV3,
    external_providers: &[WorkerInputV1],
    link_options: &[LinkOptionV1],
) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    let provider_payload_bytes = checked_sum(
        external_providers.iter().map(|input| input.bytes().len()),
        "external provider payload bytes",
    )?;
    let option_text_bytes = checked_sum(
        link_options
            .iter()
            .flat_map(|option| [option.name().len(), option.value().len()]),
        "link option text bytes",
    )?;
    let nested = handoff.module_handoff();
    validate_working_set_dimensions(ProtectedV3WorkingSetDimensions {
        outer_handoff_bytes: handoff.canonical_bytes().len(),
        compiler_module_bytes: nested.module_bytes().len(),
        provider_payload_bytes,
        provider_count: external_providers.len(),
        option_text_bytes,
        option_count: link_options.len(),
        envelope_bytes: nested.envelope().canonical_bytes().len(),
        manifest_bytes: nested.symbol_manifest().canonical_bytes().len(),
    })
}

fn validate_working_set_dimensions(
    dimensions: ProtectedV3WorkingSetDimensions,
) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    let maximum_providers = MAX_LINK_INPUTS.checked_sub(1).ok_or(
        ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
            component: "external provider count bound",
        },
    )?;
    if dimensions.provider_count > maximum_providers {
        return Err(
            ProtectedFirstBuildWorkerV3Error::WorkingSetInputCountExceeded {
                actual: dimensions.provider_count,
                maximum: maximum_providers,
            },
        );
    }
    if dimensions.option_count > MAX_LINK_OPTIONS {
        return Err(
            ProtectedFirstBuildWorkerV3Error::WorkingSetOptionCountExceeded {
                actual: dimensions.option_count,
                maximum: MAX_LINK_OPTIONS,
            },
        );
    }

    let total_input_payload_bytes = dimensions
        .compiler_module_bytes
        .checked_add(dimensions.provider_payload_bytes)
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "aggregate input payload bytes",
            },
        )?;
    if total_input_payload_bytes > MAX_WORKER_TOTAL_INPUT_BYTES {
        return Err(ProtectedFirstBuildWorkerV3Error::WorkingSetBudgetExceeded {
            required_bytes: usize_as_u64(total_input_payload_bytes)?,
            maximum_bytes: MAX_WORKER_TOTAL_INPUT_BYTES as u64,
        });
    }

    let input_wire_overhead = dimensions
        .provider_count
        .checked_add(1)
        .and_then(|count| count.checked_mul(WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2))
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "worker input wire overhead",
            },
        )?;
    let expanded_symbol_bytes = dimensions
        .envelope_bytes
        .checked_add(dimensions.manifest_bytes)
        .and_then(|bytes| bytes.checked_mul(3))
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "worker symbol wire estimate",
            },
        )?;
    let request_wire_bytes = WORKER_REQUEST_FIXED_BUDGET_BYTES_V3
        .checked_add(total_input_payload_bytes)
        .and_then(|bytes| bytes.checked_add(input_wire_overhead))
        .and_then(|bytes| bytes.checked_add(expanded_symbol_bytes))
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "worker request wire estimate",
            },
        )?
        .min(MAX_WORKER_REQUEST_BYTES);
    let retained_inputs = total_input_payload_bytes
        .checked_mul(RETAINED_INPUT_COPIES_DURING_PREFLIGHT_V3)
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "retained worker input copies",
            },
        )?;
    let retained_requests = request_wire_bytes
        .checked_mul(RETAINED_REQUEST_COPIES_DURING_PREFLIGHT_V3)
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "retained worker request copies",
            },
        )?;
    let required_bytes = dimensions
        .outer_handoff_bytes
        .checked_add(retained_inputs)
        .and_then(|bytes| bytes.checked_add(retained_requests))
        .and_then(|bytes| bytes.checked_add(dimensions.option_text_bytes))
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "aggregate live input/request bytes",
            },
        )?;
    if required_bytes > MAX_PROTECTED_V3_LIVE_INPUT_REQUEST_BYTES_V1 {
        return Err(ProtectedFirstBuildWorkerV3Error::WorkingSetBudgetExceeded {
            required_bytes: usize_as_u64(required_bytes)?,
            maximum_bytes: MAX_PROTECTED_V3_LIVE_INPUT_REQUEST_BYTES_V1 as u64,
        });
    }
    Ok(())
}

fn checked_sum(
    values: impl IntoIterator<Item = usize>,
    component: &'static str,
) -> Result<usize, ProtectedFirstBuildWorkerV3Error> {
    values.into_iter().try_fold(0_usize, |sum, value| {
        sum.checked_add(value)
            .ok_or(ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow { component })
    })
}

fn usize_as_u64(value: usize) -> Result<u64, ProtectedFirstBuildWorkerV3Error> {
    u64::try_from(value).map_err(|_| {
        ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
            component: "working-set report width",
        }
    })
}

/// Starts the measured worker only after consuming an exact matching preflight owner.
pub fn execute_preflighted_protected_reproducible_first_build_worker_v3(
    consumed: ConsumedCompilerModuleHandoffV3,
    preflight: PreparedProtectedFirstBuildWorkerV3PreflightV1,
    worker: &PinnedWorkerV1,
) -> Result<InertProtectedFirstBuildWorkerV3EvidenceV1, ProtectedFirstBuildWorkerV3Error> {
    let PreparedProtectedFirstBuildWorkerV3PreflightV1 {
        binding,
        worker: expected_worker,
        limits,
        engine,
    } = preflight;
    let expected = binding.expectation();
    if consumed.attempt() != expected.attempt() {
        return Err(preflight_mismatch("build attempt"));
    }
    if consumed.slot() != expected.slot() {
        return Err(preflight_mismatch("transaction slot"));
    }
    if consumed.transaction_identity() != expected.transaction_identity() {
        return Err(preflight_mismatch("transaction identity"));
    }
    if consumed.handoff_identity() != expected.outer_handoff_identity()
        || consumed.bytes().len() as u64 != expected.receipt_byte_len()
        || consumed.handoff().identity() != expected.outer_handoff_identity()
    {
        return Err(preflight_mismatch("semantic handoff"));
    }
    if worker.measurement() != &expected_worker {
        return Err(preflight_mismatch("measured worker"));
    }
    let handoff = consumed.into_handoff();
    let result =
        execute_preflighted_reproducible_first_build_engine(&binding, engine, worker, limits)
            .map_err(|error| map_engine_error(binding, error))?;

    validate_replay(binding, worker.measurement(), &result)?;
    let identity = calculate_evidence_identity(binding, worker.measurement(), limits, &result)?;
    let bootstrap =
        InertProtectedCompilerHandoffExecutionV3::from_execution(binding, result.candidate);
    let replay =
        InertProtectedCompilerHandoffExecutionV3::from_execution(binding, result.authorized);
    Ok(InertProtectedFirstBuildWorkerV3EvidenceV1 {
        identity,
        binding,
        handoff,
        worker: expected_worker,
        execution_limits: limits,
        plan: result.plan,
        bootstrap_request_bytes: result.candidate_request_bytes,
        bootstrap,
        replay_request_bytes: result.authorized_request_bytes,
        replay,
        _validation: ValidatedProtectedFirstBuildReplayV3,
    })
}

const fn preflight_mismatch(field: &'static str) -> ProtectedFirstBuildWorkerV3Error {
    ProtectedFirstBuildWorkerV3Error::PreflightMismatch { field }
}

/// Executes a consumed strict-V3 compiler handoff through the measured direct LLVM worker.
///
/// This entry has no V1/V2 transaction fallback. It accepts
/// [`ConsumedCompilerModuleHandoffV3`] directly and requires the parent's exact publication
/// receipt and compiler closure. It retains the complete outer V3 owner and derives worker inputs
/// only from its exact nested V2 module. The existing worker engine uses upstream LLVM/LLD library
/// APIs in the pinned production worker; this crate neither invokes COMGR nor provides a COMGR
/// fallback. The returned evidence records the worker's measured LLVM build identity but does not
/// independently prove the implementation behind that measurement.
#[allow(clippy::too_many_arguments)]
pub fn execute_protected_reproducible_first_build_worker_v3(
    consumed: ConsumedCompilerModuleHandoffV3,
    expected_receipt: CompilerModuleHandoffReceiptV3,
    expected_compiler_closure: CompilerClosureV2,
    worker: &PinnedWorkerV1,
    external_providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    candidate_output_bound: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertProtectedFirstBuildWorkerV3EvidenceV1, ProtectedFirstBuildWorkerV3Error> {
    ProtectedCompilerHandoffBindingV3::from_consumed(
        &consumed,
        expected_receipt,
        expected_compiler_closure,
    )
    .map_err(ProtectedFirstBuildWorkerV3Error::Binding)?;
    let preflight = preflight_protected_reproducible_first_build_worker_v3(
        consumed.handoff(),
        expected_receipt,
        expected_compiler_closure,
        worker,
        external_providers,
        link_options,
        candidate_output_bound,
        limits,
    )?;
    execute_preflighted_protected_reproducible_first_build_worker_v3(consumed, preflight, worker)
}

fn validate_replay(
    binding: ProtectedCompilerHandoffBindingV3,
    worker: &WorkerMeasurementV1,
    result: &crate::first_build_worker_engine::ReproducibleFirstBuildEngineResult,
) -> Result<ValidatedProtectedFirstBuildReplayV3, ProtectedFirstBuildWorkerV3Error> {
    validate_replay_parts(
        binding,
        worker,
        &result.decoded,
        &result.plan,
        &result.candidate_request_bytes,
        result.candidate.response(),
        &result.authorized_request_bytes,
        result.authorized.response(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_replay_parts(
    binding: ProtectedCompilerHandoffBindingV3,
    worker: &WorkerMeasurementV1,
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    plan: &MultiInputLinkPlanV1,
    bootstrap_request_bytes: &[u8],
    bootstrap_response: &WorkerResponseV2,
    replay_request_bytes: &[u8],
    replay_response: &WorkerResponseV2,
) -> Result<ValidatedProtectedFirstBuildReplayV3, ProtectedFirstBuildWorkerV3Error> {
    let bootstrap_request = BorrowedWorkerRequestV2::decode(bootstrap_request_bytes)
        .map_err(|_| replay_error("bootstrap request canonical transcript"))?;
    let replay_request = BorrowedWorkerRequestV2::decode(replay_request_bytes)
        .map_err(|_| replay_error("exact-replay request canonical transcript"))?;
    validate_request_response_binding(&bootstrap_request, bootstrap_response)
        .map_err(|_| replay_error("bootstrap request/response canonical exchange"))?;
    validate_request_response_binding(&replay_request, replay_response)
        .map_err(|_| replay_error("exact-replay request/response canonical exchange"))?;
    let bootstrap_output = bootstrap_response
        .output()
        .ok_or_else(|| replay_error("missing bootstrap output"))?;
    let replay_output = replay_response
        .output()
        .ok_or_else(|| replay_error("missing exact-replay output"))?;
    if bootstrap_output.bytes() != replay_output.bytes()
        || bootstrap_output.identity() != replay_output.identity()
        || !bootstrap_output
            .identity()
            .matches(bootstrap_output.bytes())
    {
        return Err(replay_error("reproducible output bytes and identity"));
    }

    let (_, options) =
        decode_link_options(plan.options()).map_err(|_| replay_error("canonical link options"))?;
    let request_inputs =
        validate_request_common_fields(&bootstrap_request, worker, decoded, options)?;
    validate_stable_request_fields(&bootstrap_request, &replay_request)
        .map_err(|_| replay_error("stable bootstrap/replay request fields"))?;
    let expected_bootstrap_request_id = calculate_bootstrap_request_id(
        binding,
        worker,
        decoded,
        &bootstrap_request,
        &request_inputs,
    )?;
    if bootstrap_request.request_id() != expected_bootstrap_request_id {
        return Err(replay_error("bootstrap request identity"));
    }

    let (reconstructed_plan, all_inputs) = reconstruct_plan(
        decoded,
        &request_inputs,
        plan.options(),
        bootstrap_output.identity(),
    )?;
    if &reconstructed_plan != plan {
        return Err(replay_error("complete canonical link plan"));
    }
    if decode_u64(replay_request.field(14)).ok() != Some(bootstrap_output.identity().byte_len()) {
        return Err(replay_error("exact-replay output bound"));
    }
    let expected_replay_request_id = calculate_replay_request_id(
        binding,
        worker,
        decoded,
        &replay_request,
        plan,
        &request_inputs,
        &all_inputs,
    )?;
    if replay_request.request_id() != expected_replay_request_id {
        return Err(replay_error("exact-replay request identity"));
    }
    if bootstrap_request.request_id() == replay_request.request_id() {
        return Err(replay_error(
            "distinct bootstrap and replay request identities",
        ));
    }
    Ok(ValidatedProtectedFirstBuildReplayV3)
}

fn reconstruct_plan(
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    request_inputs: &BorrowedRequestInputsV2,
    options: &[LinkOptionV1],
    output_identity: ContentIdentityV1,
) -> Result<(MultiInputLinkPlanV1, Vec<BorrowedInputIdentityV2>), ProtectedFirstBuildWorkerV3Error>
{
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(request_inputs.providers.len().checked_add(1).ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "reconstructed input count",
            },
        )?)
        .map_err(|_| allocation_error("reconstructed input metadata"))?;
    inputs.extend_from_slice(&request_inputs.providers);
    inputs.push(request_inputs.compiler);
    inputs.sort_by_key(|input| (input.identity, input.kind));
    for pair in inputs.windows(2) {
        if pair[0].identity == pair[1].identity {
            return Err(replay_error("duplicate plan input identity"));
        }
    }
    let target = decoded.target();
    let mut link_inputs = Vec::new();
    link_inputs
        .try_reserve_exact(inputs.len())
        .map_err(|_| allocation_error("reconstructed link inputs"))?;
    link_inputs.extend(
        inputs
            .iter()
            .map(|input| LinkInputV1::new(input.identity, target)),
    );
    let mut provenance = Vec::new();
    provenance
        .try_reserve_exact(link_inputs.len().checked_add(1).ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "reconstructed provenance count",
            },
        )?)
        .map_err(|_| allocation_error("reconstructed provenance"))?;
    for input in &link_inputs {
        provenance.push(
            ProvenanceNodeV1::new(input.identity(), Vec::new())
                .map_err(ProtectedFirstBuildWorkerV3Error::LinkPlan)?,
        );
    }
    let mut output_parents = Vec::new();
    output_parents
        .try_reserve_exact(link_inputs.len())
        .map_err(|_| allocation_error("output provenance parents"))?;
    output_parents.extend(link_inputs.iter().map(|input| input.identity()));
    provenance.push(
        ProvenanceNodeV1::new(output_identity, output_parents)
            .map_err(ProtectedFirstBuildWorkerV3Error::LinkPlan)?,
    );
    let mut retained_options = Vec::new();
    retained_options
        .try_reserve_exact(options.len())
        .map_err(|_| allocation_error("reconstructed link options"))?;
    retained_options.extend(options.iter().cloned());
    let plan = MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        retained_options,
        LinkOutputV1::new(output_identity, target),
        provenance,
    )
    .map_err(ProtectedFirstBuildWorkerV3Error::LinkPlan)?;
    Ok((plan, inputs))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BorrowedInputIdentityV2 {
    kind: WorkerInputKindV1,
    identity: ContentIdentityV1,
}

struct BorrowedRequestInputsV2 {
    compiler: BorrowedInputIdentityV2,
    providers: Vec<BorrowedInputIdentityV2>,
}

pub(crate) struct OwnedWorkerV3ProviderReplayPartV1 {
    pub(crate) kind: WorkerInputKindV1,
    pub(crate) identity: ContentIdentityV1,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct OwnedWorkerV3RequestReplayPartsV1 {
    pub(crate) bootstrap_output_bound: u64,
    pub(crate) external_providers: Vec<OwnedWorkerV3ProviderReplayPartV1>,
}

struct BorrowedWorkerRequestV2<'bytes> {
    fields: [&'bytes [u8]; WORKER_REQUEST_FIELD_COUNT_V2],
}

impl<'bytes> BorrowedWorkerRequestV2<'bytes> {
    fn decode(bytes: &'bytes [u8]) -> Result<Self, ()> {
        if bytes.len() > MAX_WORKER_REQUEST_BYTES || !bytes.starts_with(WORKER_REQUEST_MAGIC_V2) {
            return Err(());
        }
        let mut offset = WORKER_REQUEST_MAGIC_V2.len();
        let mut fields = [&[][..]; WORKER_REQUEST_FIELD_COUNT_V2];
        let mut identity_preimage_len = 0;
        for expected_tag in 1..=WORKER_REQUEST_FIELD_COUNT_V2 {
            let header_end = offset.checked_add(6).ok_or(())?;
            let header = bytes.get(offset..header_end).ok_or(())?;
            let tag = u16::from_le_bytes(header[..2].try_into().map_err(|_| ())?);
            if usize::from(tag) != expected_tag {
                return Err(());
            }
            let field_len = u32::from_le_bytes(header[2..].try_into().map_err(|_| ())?) as usize;
            let field_end = header_end.checked_add(field_len).ok_or(())?;
            fields[expected_tag - 1] = bytes.get(header_end..field_end).ok_or(())?;
            if expected_tag == WORKER_REQUEST_FIELD_COUNT_V2 {
                identity_preimage_len = offset;
            }
            offset = field_end;
        }
        if offset != bytes.len() || fields[14].len() != 32 {
            return Err(());
        }
        let mut hasher = Sha256::new();
        hasher.update(WORKER_REQUEST_IDENTITY_DOMAIN_V2);
        hasher.update((identity_preimage_len as u64).to_le_bytes());
        hasher.update(&bytes[..identity_preimage_len]);
        let actual_identity: [u8; 32] = hasher.finalize().into();
        if fields[14] != actual_identity {
            return Err(());
        }
        Ok(Self { fields })
    }

    fn field(&self, tag: usize) -> &'bytes [u8] {
        self.fields[tag - 1]
    }

    fn request_id(&self) -> &'bytes [u8] {
        self.field(1)
    }

    fn request_identity(&self) -> &'bytes [u8] {
        self.field(15)
    }
}

fn validate_request_response_binding(
    request: &BorrowedWorkerRequestV2<'_>,
    response: &WorkerResponseV2,
) -> Result<(), ()> {
    if request.request_id() != response.request_id()
        || request.request_identity() != response.request_identity()
        || request.field(8) != response.compiler_envelope_identity().as_bytes()
    {
        return Err(());
    }
    Ok(())
}

fn validate_request_common_fields(
    request: &BorrowedWorkerRequestV2<'_>,
    worker: &WorkerMeasurementV1,
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    options: crate::WorkerOptionsV1,
) -> Result<BorrowedRequestInputsV2, ProtectedFirstBuildWorkerV3Error> {
    if request.field(1).len() != 32
        || request.field(2) != worker.llvm_build_identity().as_bytes()
        || request.field(3) != worker.worker_build_identity().as_bytes()
        || request.field(4) != encode_content_identity(worker.executable())
        || request.field(5) != decoded.target().to_string().as_bytes()
        || request.field(6) != [code_object_version_byte(decoded.code_object_version())]
        || request.field(7)
            != [
                options.optimization() as u8,
                u8::from(options.strip_debug()),
                u8::from(options.verify_each()),
            ]
        || request.field(8) != decoded.envelope().identity().as_bytes()
    {
        return Err(replay_error("common worker request identity fields"));
    }
    let compiler = validate_compiler_input_field(request.field(9), decoded)
        .map_err(|_| replay_error("compiler module request input"))?;
    let providers = decode_borrowed_inputs(request.field(10))?;
    validate_symbol_closure(request, decoded)?;
    let output_bound =
        decode_u64(request.field(14)).map_err(|_| replay_error("worker request output bound"))?;
    if output_bound == 0 || output_bound > crate::MAX_WORKER_OUTPUT_BYTES as u64 {
        return Err(replay_error("worker request output bound"));
    }
    Ok(BorrowedRequestInputsV2 {
        compiler,
        providers,
    })
}

fn validate_stable_request_fields(
    bootstrap: &BorrowedWorkerRequestV2<'_>,
    replay: &BorrowedWorkerRequestV2<'_>,
) -> Result<(), ()> {
    for tag in 2..=13 {
        if bootstrap.field(tag) != replay.field(tag) {
            return Err(());
        }
    }
    Ok(())
}

fn validate_compiler_input_field(
    field: &[u8],
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
) -> Result<BorrowedInputIdentityV2, ()> {
    let (input, remaining) = decode_borrowed_input(field)?;
    if !remaining.is_empty()
        || input.kind != decoded.compiler_module_kind()
        || field.get(WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2..) != Some(decoded.compiler_module_bytes())
    {
        return Err(());
    }
    Ok(input)
}

fn decode_borrowed_inputs(
    field: &[u8],
) -> Result<Vec<BorrowedInputIdentityV2>, ProtectedFirstBuildWorkerV3Error> {
    let count_bytes = field
        .get(..4)
        .ok_or_else(|| replay_error("provider input count"))?;
    let count = u32::from_le_bytes(
        count_bytes
            .try_into()
            .map_err(|_| replay_error("provider input count"))?,
    ) as usize;
    if count > MAX_LINK_INPUTS.saturating_sub(1) {
        return Err(replay_error("provider input count"));
    }
    let mut inputs: Vec<BorrowedInputIdentityV2> = Vec::new();
    inputs
        .try_reserve_exact(count)
        .map_err(|_| allocation_error("borrowed provider input metadata"))?;
    let mut remaining = field
        .get(4..)
        .ok_or_else(|| replay_error("provider inputs"))?;
    let mut total_payload_bytes = 0_usize;
    for _ in 0..count {
        let (input, next) = decode_borrowed_input(remaining)
            .map_err(|_| replay_error("provider input encoding"))?;
        let payload_bytes = usize::try_from(input.identity.byte_len())
            .map_err(|_| replay_error("provider input length"))?;
        total_payload_bytes = total_payload_bytes
            .checked_add(payload_bytes)
            .ok_or_else(|| replay_error("provider input length"))?;
        if total_payload_bytes > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(replay_error("provider input length"));
        }
        if let Some(previous) = inputs.last()
            && (previous.identity, previous.kind) >= (input.identity, input.kind)
        {
            return Err(replay_error("provider input canonical order"));
        }
        inputs.push(input);
        remaining = next;
    }
    if !remaining.is_empty() {
        return Err(replay_error("provider input trailing bytes"));
    }
    Ok(inputs)
}

pub(crate) fn extract_worker_v3_request_replay_parts_v1(
    bootstrap_request_bytes: &[u8],
    replay_request_bytes: &[u8],
) -> Result<OwnedWorkerV3RequestReplayPartsV1, ProtectedFirstBuildWorkerV3Error> {
    let bootstrap = BorrowedWorkerRequestV2::decode(bootstrap_request_bytes)
        .map_err(|_| replay_error("bootstrap request canonical transcript"))?;
    let replay = BorrowedWorkerRequestV2::decode(replay_request_bytes)
        .map_err(|_| replay_error("exact-replay request canonical transcript"))?;
    validate_stable_request_fields(&bootstrap, &replay)
        .map_err(|_| replay_error("stable bootstrap/replay request fields"))?;
    let bootstrap_output_bound =
        decode_u64(bootstrap.field(14)).map_err(|_| replay_error("bootstrap output bound"))?;
    if bootstrap_output_bound == 0 || bootstrap_output_bound > crate::MAX_WORKER_OUTPUT_BYTES as u64
    {
        return Err(replay_error("bootstrap output bound"));
    }

    let field = bootstrap.field(10);
    let count = u32::from_le_bytes(
        field
            .get(..4)
            .ok_or_else(|| replay_error("provider input count"))?
            .try_into()
            .map_err(|_| replay_error("provider input count"))?,
    ) as usize;
    if count > MAX_LINK_INPUTS.saturating_sub(1) {
        return Err(replay_error("provider input count"));
    }
    let mut external_providers = Vec::new();
    external_providers
        .try_reserve_exact(count)
        .map_err(|_| allocation_error("provider replay owners"))?;
    let mut remaining = field
        .get(4..)
        .ok_or_else(|| replay_error("provider inputs"))?;
    let mut total_payload_bytes = 0_usize;
    let mut previous: Option<(ContentIdentityV1, WorkerInputKindV1)> = None;
    for _ in 0..count {
        let encoded_input = remaining;
        let (input, next) = decode_borrowed_input(encoded_input)
            .map_err(|_| replay_error("provider input encoding"))?;
        if previous.is_some_and(|before| {
            before.0 == input.identity || before >= (input.identity, input.kind)
        }) {
            return Err(replay_error("provider input canonical order"));
        }
        let payload_len = usize::try_from(input.identity.byte_len())
            .map_err(|_| replay_error("provider input length"))?;
        total_payload_bytes = total_payload_bytes
            .checked_add(payload_len)
            .ok_or_else(|| replay_error("provider input length"))?;
        if total_payload_bytes > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(replay_error("provider input length"));
        }
        let payload_end = WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2
            .checked_add(payload_len)
            .ok_or_else(|| replay_error("provider input length"))?;
        let payload = encoded_input
            .get(WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2..payload_end)
            .ok_or_else(|| replay_error("provider input bytes"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(payload_len)
            .map_err(|_| allocation_error("provider replay payload"))?;
        bytes.extend_from_slice(payload);
        external_providers.push(OwnedWorkerV3ProviderReplayPartV1 {
            kind: input.kind,
            identity: input.identity,
            bytes,
        });
        previous = Some((input.identity, input.kind));
        remaining = next;
    }
    if !remaining.is_empty() {
        return Err(replay_error("provider input trailing bytes"));
    }
    Ok(OwnedWorkerV3RequestReplayPartsV1 {
        bootstrap_output_bound,
        external_providers,
    })
}

fn decode_borrowed_input(field: &[u8]) -> Result<(BorrowedInputIdentityV2, &[u8]), ()> {
    let header = field.get(..WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2).ok_or(())?;
    let kind = match header[0] {
        1 => WorkerInputKindV1::LlvmBitcode,
        2 => WorkerInputKindV1::AmdGpuRelocatable,
        3 => WorkerInputKindV1::LlvmTextIr,
        _ => return Err(()),
    };
    let sha256: [u8; 32] = header[1..33].try_into().map_err(|_| ())?;
    let byte_len = u64::from_le_bytes(header[33..41].try_into().map_err(|_| ())?);
    let payload_len = usize::try_from(byte_len).map_err(|_| ())?;
    if payload_len == 0 || payload_len > MAX_WORKER_TOTAL_INPUT_BYTES {
        return Err(());
    }
    let payload_end = WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2
        .checked_add(payload_len)
        .ok_or(())?;
    let payload = field
        .get(WORKER_INPUT_WIRE_OVERHEAD_BYTES_V2..payload_end)
        .ok_or(())?;
    let identity = ContentIdentityV1::from_parts(sha256, byte_len);
    if !identity.matches(payload) {
        return Err(());
    }
    Ok((
        BorrowedInputIdentityV2 { kind, identity },
        field.get(payload_end..).ok_or(())?,
    ))
}

fn decode_borrowed_strings(field: &[u8]) -> Result<Vec<&str>, ProtectedFirstBuildWorkerV3Error> {
    let count = u32::from_le_bytes(
        field
            .get(..4)
            .ok_or_else(|| replay_error("request symbol count"))?
            .try_into()
            .map_err(|_| replay_error("request symbol count"))?,
    ) as usize;
    if count > MAX_WORKER_SYMBOLS {
        return Err(replay_error("request symbol count"));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| allocation_error("borrowed request symbols"))?;
    let mut remaining = field
        .get(4..)
        .ok_or_else(|| replay_error("request symbols"))?;
    let mut previous: Option<&[u8]> = None;
    let mut total = 0_usize;
    for _ in 0..count {
        let length = u32::from_le_bytes(
            remaining
                .get(..4)
                .ok_or_else(|| replay_error("request symbol length"))?
                .try_into()
                .map_err(|_| replay_error("request symbol length"))?,
        ) as usize;
        if length == 0 || length > MAX_WORKER_SYMBOL_BYTES {
            return Err(replay_error("request symbol length"));
        }
        total = total.checked_add(length).ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "request symbol bytes",
            },
        )?;
        if total > MAX_WORKER_SYMBOLS * MAX_WORKER_SYMBOL_BYTES {
            return Err(replay_error("request symbol bytes"));
        }
        let value = remaining
            .get(
                4..4_usize.checked_add(length).ok_or(
                    ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                        component: "request symbol field offset",
                    },
                )?,
            )
            .ok_or_else(|| replay_error("request symbol bytes"))?;
        if !value.is_ascii()
            || value.iter().copied().any(|byte| {
                byte.is_ascii_control()
                    || byte.is_ascii_whitespace()
                    || matches!(byte, b'/' | b'\\' | b'\'' | b'"')
            })
            || previous.is_some_and(|prior| prior >= value)
        {
            return Err(replay_error("request symbol canonical order"));
        }
        values.push(std::str::from_utf8(value).map_err(|_| replay_error("request symbol UTF-8"))?);
        previous = Some(value);
        remaining = remaining
            .get(4 + length..)
            .ok_or_else(|| replay_error("request symbol bytes"))?;
    }
    if !remaining.is_empty() {
        return Err(replay_error("request symbol trailing bytes"));
    }
    Ok(values)
}

fn validate_symbol_closure(
    request: &BorrowedWorkerRequestV2<'_>,
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    use CompilerModuleSymbolRoleV1 as Role;

    let imports = decode_borrowed_strings(request.field(11))?;
    let exports = decode_borrowed_strings(request.field(12))?;
    let final_symbols = decode_borrowed_strings(request.field(13))?;
    let directional = decoded.envelope().directional_symbols();
    if !imports.iter().copied().eq(directional.imports())
        || !imports.iter().copied().eq(decoded
            .symbol_manifest()
            .symbols(Role::UnresolvedExternalImport))
    {
        return Err(replay_error("request import symbol closure"));
    }
    if !exports.iter().copied().eq(directional.exports())
        || !exports
            .iter()
            .copied()
            .eq(decoded.symbol_manifest().symbols(Role::DeviceFfiExport))
    {
        return Err(replay_error("request export symbol closure"));
    }

    let expected_count = [
        Role::KernelEntry,
        Role::KernelDescriptor,
        Role::DeviceFfiExport,
        Role::UnresolvedExternalImport,
    ]
    .into_iter()
    .try_fold(0_usize, |count, role| {
        count.checked_add(decoded.symbol_manifest().role_count(role))
    })
    .ok_or(
        ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
            component: "final symbol closure count",
        },
    )?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(expected_count)
        .map_err(|_| allocation_error("expected final symbol closure"))?;
    for role in [
        Role::KernelEntry,
        Role::KernelDescriptor,
        Role::DeviceFfiExport,
        Role::UnresolvedExternalImport,
    ] {
        expected.extend(decoded.symbol_manifest().symbols(role));
    }
    expected.sort_unstable();
    if final_symbols != expected {
        return Err(replay_error("request final symbol closure"));
    }
    Ok(())
}

fn encode_content_identity(identity: ContentIdentityV1) -> [u8; 40] {
    let mut encoded = [0_u8; 40];
    encoded[..32].copy_from_slice(identity.sha256());
    encoded[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    encoded
}

fn decode_u64(bytes: &[u8]) -> Result<u64, ()> {
    Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| ())?))
}

fn calculate_bootstrap_request_id(
    binding: ProtectedCompilerHandoffBindingV3,
    worker: &WorkerMeasurementV1,
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    request: &BorrowedWorkerRequestV2<'_>,
    inputs: &BorrowedRequestInputsV2,
) -> Result<[u8; 32], ProtectedFirstBuildWorkerV3Error> {
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_FIRST_BUILD_REQUEST_DOMAIN_V3);
    binding.hash_identity_preimage(&mut hasher);
    hash_worker_request_common(&mut hasher, worker, decoded, request, inputs)?;
    Ok(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
fn calculate_replay_request_id(
    binding: ProtectedCompilerHandoffBindingV3,
    worker: &WorkerMeasurementV1,
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    request: &BorrowedWorkerRequestV2<'_>,
    plan: &MultiInputLinkPlanV1,
    inputs: &BorrowedRequestInputsV2,
    all_inputs: &[BorrowedInputIdentityV2],
) -> Result<[u8; 32], ProtectedFirstBuildWorkerV3Error> {
    let input_kind_closure = calculate_input_kind_closure_identity(plan, all_inputs)?;
    let staged_envelope = calculate_staged_envelope_identity(decoded);
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_PLAN_REQUEST_DOMAIN_V3);
    binding.hash_identity_preimage(&mut hasher);
    hasher.update(plan.identity().as_bytes());
    hasher.update(input_kind_closure);
    hasher.update(staged_envelope);
    hash_worker_request_common(&mut hasher, worker, decoded, request, inputs)?;
    Ok(hasher.finalize().into())
}

fn hash_worker_request_common(
    hasher: &mut Sha256,
    worker: &WorkerMeasurementV1,
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    request: &BorrowedWorkerRequestV2<'_>,
    inputs: &BorrowedRequestInputsV2,
) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    hasher.update(request.field(8));
    let manifest_identity = decoded.symbol_manifest().identity();
    hasher.update(manifest_identity.sha256());
    hasher.update(manifest_identity.byte_len().to_le_bytes());
    hash_content(hasher, worker.executable());
    hash_text_bytes(hasher, request.field(3))?;
    hash_text_bytes(hasher, request.field(2))?;
    hash_text_bytes(hasher, request.field(5))?;
    hasher.update(request.field(6));
    hasher.update(request.field(7));
    hash_input_identity(hasher, inputs.compiler);
    hasher.update(usize_as_u64(inputs.providers.len())?.to_le_bytes());
    for provider in &inputs.providers {
        hash_input_identity(hasher, *provider);
    }
    for tag in 11..=13 {
        hash_string_field(hasher, request.field(tag))?;
    }
    hasher.update(request.field(14));
    Ok(())
}

fn calculate_input_kind_closure_identity(
    plan: &MultiInputLinkPlanV1,
    all_inputs: &[BorrowedInputIdentityV2],
) -> Result<[u8; 32], ProtectedFirstBuildWorkerV3Error> {
    if all_inputs.len() != plan.inputs().len() {
        return Err(replay_error("link-plan input-kind count"));
    }
    let mut hasher = Sha256::new();
    hasher.update(INPUT_KIND_CLOSURE_DOMAIN_V1);
    hasher.update(plan.identity().as_bytes());
    hasher.update(usize_as_u64(all_inputs.len())?.to_le_bytes());
    for (planned, actual) in plan.inputs().iter().zip(all_inputs) {
        if planned.identity() != actual.identity {
            return Err(replay_error("link-plan input-kind identity"));
        }
        hash_content(&mut hasher, actual.identity);
        hasher.update([actual.kind as u8]);
    }
    Ok(hasher.finalize().into())
}

fn calculate_staged_envelope_identity(
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
) -> [u8; 32] {
    let bytes = decoded.envelope().canonical_bytes();
    let mut hasher = Sha256::new();
    hasher.update(STAGED_COMPILER_FFI_ENVELOPE_DOMAIN_V1);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hash_input_identity(hasher: &mut Sha256, input: BorrowedInputIdentityV2) {
    hasher.update([input.kind as u8]);
    hash_content(hasher, input.identity);
}

fn hash_string_field(
    hasher: &mut Sha256,
    field: &[u8],
) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    let count = u32::from_le_bytes(
        field
            .get(..4)
            .ok_or_else(|| replay_error("request symbol count"))?
            .try_into()
            .map_err(|_| replay_error("request symbol count"))?,
    ) as usize;
    hasher.update(usize_as_u64(count)?.to_le_bytes());
    let mut remaining = field
        .get(4..)
        .ok_or_else(|| replay_error("request symbols"))?;
    for _ in 0..count {
        let length = u32::from_le_bytes(
            remaining
                .get(..4)
                .ok_or_else(|| replay_error("request symbol length"))?
                .try_into()
                .map_err(|_| replay_error("request symbol length"))?,
        ) as usize;
        let value_end = 4_usize.checked_add(length).ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "request symbol field offset",
            },
        )?;
        let value = remaining
            .get(4..value_end)
            .ok_or_else(|| replay_error("request symbol bytes"))?;
        hash_text_bytes(hasher, value)?;
        remaining = remaining
            .get(value_end..)
            .ok_or_else(|| replay_error("request symbol bytes"))?;
    }
    if !remaining.is_empty() {
        return Err(replay_error("request symbol trailing bytes"));
    }
    Ok(())
}

fn hash_text_bytes(
    hasher: &mut Sha256,
    bytes: &[u8],
) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    hasher.update(usize_as_u64(bytes.len())?.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

const fn code_object_version_byte(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

const fn allocation_error(component: &'static str) -> ProtectedFirstBuildWorkerV3Error {
    ProtectedFirstBuildWorkerV3Error::WorkingSetAllocationFailed { component }
}

const fn replay_error(field: &'static str) -> ProtectedFirstBuildWorkerV3Error {
    ProtectedFirstBuildWorkerV3Error::ReplayValidation { field }
}

fn map_engine_error(
    binding: ProtectedCompilerHandoffBindingV3,
    error: ReproducibleFirstBuildEngineError,
) -> ProtectedFirstBuildWorkerV3Error {
    let wrap = |execution| {
        Box::new(InertProtectedCompilerHandoffExecutionV3::from_execution(
            binding, execution,
        ))
    };
    match error {
        ReproducibleFirstBuildEngineError::LinkPlan(error) => {
            ProtectedFirstBuildWorkerV3Error::LinkPlan(error)
        }
        ReproducibleFirstBuildEngineError::RequestConstruction(error) => {
            ProtectedFirstBuildWorkerV3Error::RequestConstruction(error)
        }
        ReproducibleFirstBuildEngineError::CandidateRequest(error) => {
            ProtectedFirstBuildWorkerV3Error::BootstrapRequest(error)
        }
        ReproducibleFirstBuildEngineError::CandidateExecution(error) => {
            ProtectedFirstBuildWorkerV3Error::BootstrapExecution(error)
        }
        ReproducibleFirstBuildEngineError::CandidateDidNotProduceOutput(execution) => {
            ProtectedFirstBuildWorkerV3Error::BootstrapDidNotProduceOutput(wrap(*execution))
        }
        ReproducibleFirstBuildEngineError::AuthorizedExecution(error) => {
            ProtectedFirstBuildWorkerV3Error::ReplayExecution(error)
        }
        ReproducibleFirstBuildEngineError::AuthorizedDidNotProduceOutput {
            candidate,
            authorized,
        } => ProtectedFirstBuildWorkerV3Error::ReplayDidNotProduceOutput {
            bootstrap: wrap(*candidate),
            replay: wrap(*authorized),
        },
        ReproducibleFirstBuildEngineError::OutputMismatch {
            candidate,
            authorized,
        } => ProtectedFirstBuildWorkerV3Error::OutputMismatch {
            bootstrap: wrap(*candidate),
            replay: wrap(*authorized),
        },
    }
}

fn calculate_binding_identity(
    expectation: ProtectedCompilerHandoffExpectationV3,
) -> ProtectedCompilerHandoffBindingIdentityV3 {
    let mut hasher = Sha256::new();
    hasher.update(BINDING_IDENTITY_DOMAIN_V3);
    hash_binding_preimage(&mut hasher, expectation);
    ProtectedCompilerHandoffBindingIdentityV3(hasher.finalize().into())
}

fn hash_binding_preimage(hasher: &mut Sha256, expectation: ProtectedCompilerHandoffExpectationV3) {
    hash_attempt(hasher, expectation.attempt);
    hasher.update([expectation.slot as u8]);
    hasher.update(expectation.transaction_identity.as_bytes());
    hasher.update(expectation.receipt_byte_len.to_le_bytes());
    hasher.update(expectation.outer_handoff_identity.sha256());
    hasher.update(expectation.outer_handoff_identity.byte_len().to_le_bytes());
    hasher.update(expectation.capsule_sha256);
    hasher.update(expectation.capsule_byte_len.to_le_bytes());
    hasher.update(expectation.invocation_digest);
    hasher.update(expectation.pair_binding_sha256);
    hasher.update(expectation.pair_binding_byte_len.to_le_bytes());
    hasher.update(expectation.nested_handoff_identity.sha256());
    hasher.update(expectation.nested_handoff_identity.byte_len().to_le_bytes());
    hasher.update(expectation.final_commitment_receipt_sha256);
    hasher.update(expectation.final_commitment_receipt_byte_len.to_le_bytes());
    hasher.update(expectation.final_commitment_sha256);
    hasher.update(expectation.final_commitment_byte_len.to_le_bytes());
    hash_compiler_closure(hasher, expectation.compiler_closure);
}

fn calculate_evidence_identity(
    binding: ProtectedCompilerHandoffBindingV3,
    worker: &WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    result: &crate::first_build_worker_engine::ReproducibleFirstBuildEngineResult,
) -> Result<ProtectedFirstBuildWorkerV3IdentityV1, ProtectedFirstBuildWorkerV3Error> {
    calculate_evidence_identity_parts(
        binding,
        worker,
        limits,
        &result.plan,
        &result.candidate_request_bytes,
        result.candidate.response().canonical_bytes(),
        &result.authorized_request_bytes,
        result.authorized.response().canonical_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
fn calculate_evidence_identity_parts(
    binding: ProtectedCompilerHandoffBindingV3,
    worker: &WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    plan: &MultiInputLinkPlanV1,
    bootstrap_request_bytes: &[u8],
    bootstrap_response_bytes: &[u8],
    replay_request_bytes: &[u8],
    replay_response_bytes: &[u8],
) -> Result<ProtectedFirstBuildWorkerV3IdentityV1, ProtectedFirstBuildWorkerV3Error> {
    let mut hasher = Sha256::new();
    hasher.update(EVIDENCE_IDENTITY_DOMAIN_V3);
    binding.hash_identity_preimage(&mut hasher);
    hash_content(&mut hasher, worker.executable());
    hash_blob(&mut hasher, worker.worker_build_identity().as_bytes());
    hash_blob(&mut hasher, worker.llvm_build_identity().as_bytes());
    hasher.update(limits.timeout().as_secs().to_le_bytes());
    hasher.update(limits.timeout().subsec_nanos().to_le_bytes());
    hasher.update((limits.stdout_bytes() as u64).to_le_bytes());
    hasher.update((limits.stderr_bytes() as u64).to_le_bytes());
    hash_canonical_plan_blob(&mut hasher, plan)?;
    hash_blob(&mut hasher, bootstrap_request_bytes);
    hash_blob(&mut hasher, bootstrap_response_bytes);
    hash_blob(&mut hasher, replay_request_bytes);
    hash_blob(&mut hasher, replay_response_bytes);
    Ok(ProtectedFirstBuildWorkerV3IdentityV1(
        hasher.finalize().into(),
    ))
}

fn hash_canonical_plan_blob(
    hasher: &mut Sha256,
    plan: &MultiInputLinkPlanV1,
) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    const LINK_PLAN_DOMAIN_V1: &[u8] = b"FE2O3/AMDGPU-MULTI-INPUT-LINK-PLAN/V1\0";
    const CONTENT_IDENTITY_BYTES: usize = 32 + 8;

    let target = plan.target().to_string();
    let mut byte_len = LINK_PLAN_DOMAIN_V1
        .len()
        .checked_add(4)
        .and_then(|bytes| bytes.checked_add(target.len()))
        .and_then(|bytes| bytes.checked_add(4))
        .and_then(|bytes| {
            plan.inputs()
                .len()
                .checked_mul(CONTENT_IDENTITY_BYTES)
                .and_then(|inputs| bytes.checked_add(inputs))
        })
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "canonical link-plan length",
            },
        )?;
    for option in plan.options() {
        byte_len = byte_len
            .checked_add(4)
            .and_then(|bytes| bytes.checked_add(option.name().len()))
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_add(option.value().len()))
            .ok_or(
                ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                    component: "canonical link-option length",
                },
            )?;
    }
    byte_len = byte_len
        .checked_add(CONTENT_IDENTITY_BYTES)
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or(
            ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                component: "canonical link-plan output length",
            },
        )?;
    for node in plan.provenance() {
        byte_len = byte_len
            .checked_add(CONTENT_IDENTITY_BYTES + 4)
            .and_then(|bytes| {
                node.parents()
                    .len()
                    .checked_mul(CONTENT_IDENTITY_BYTES)
                    .and_then(|parents| bytes.checked_add(parents))
            })
            .ok_or(
                ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                    component: "canonical link-plan provenance length",
                },
            )?;
    }

    hasher.update(usize_as_u64(byte_len)?.to_le_bytes());
    hasher.update(LINK_PLAN_DOMAIN_V1);
    hash_u32(hasher, target.len())?;
    hasher.update(target.as_bytes());
    hash_u32(hasher, plan.inputs().len())?;
    for input in plan.inputs() {
        hash_content(hasher, input.identity());
    }
    hash_u32(hasher, plan.options().len())?;
    for option in plan.options() {
        hash_u32(hasher, option.name().len())?;
        hasher.update(option.name().as_bytes());
        hash_u32(hasher, option.value().len())?;
        hasher.update(option.value().as_bytes());
    }
    hash_content(hasher, plan.output().identity());
    hash_u32(hasher, plan.provenance().len())?;
    for node in plan.provenance() {
        hash_content(hasher, node.identity());
        hash_u32(hasher, node.parents().len())?;
        for parent in node.parents() {
            hash_content(hasher, *parent);
        }
    }
    Ok(())
}

fn hash_u32(hasher: &mut Sha256, value: usize) -> Result<(), ProtectedFirstBuildWorkerV3Error> {
    let value = u32::try_from(value).map_err(|_| {
        ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
            component: "canonical u32 length",
        }
    })?;
    hasher.update(value.to_le_bytes());
    Ok(())
}

fn hash_attempt(hasher: &mut Sha256, attempt: BuildAttempt) {
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
}

fn hash_compiler_closure(hasher: &mut Sha256, closure: CompilerClosureV2) {
    hasher.update(closure.cargo_executable_sha256());
    hasher.update(closure.cargo_binding_trampoline_sha256());
    hasher.update(closure.cargo_fe2o3_binding_wrapper_sha256());
    hasher.update(closure.rustc_executable_sha256());
    hasher.update(closure.rustc_runtime_tree_sha256());
    hasher.update(closure.codegen_backend_sha256());
    hasher.update(
        closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    hasher.update(closure.identity_sha256());
}

fn hash_content(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_blob(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod working_set_tests {
    use super::*;
    use fe2o3_kernel_descriptor::DeviceTargetV1;

    struct Diagnostics<'a>(&'a [String]);

    impl fmt::Display for Diagnostics<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write_worker_diagnostics(formatter, self.0)
        }
    }

    fn dimensions() -> ProtectedV3WorkingSetDimensions {
        ProtectedV3WorkingSetDimensions {
            outer_handoff_bytes: 4096,
            compiler_module_bytes: 1024,
            provider_payload_bytes: 1024,
            provider_count: 1,
            option_text_bytes: 64,
            option_count: 4,
            envelope_bytes: 256,
            manifest_bytes: 256,
        }
    }

    #[test]
    fn worker_failure_diagnostics_remain_visible_and_bounded_by_the_codec() {
        let diagnostics = vec![
            "first validated diagnostic".to_owned(),
            "second validated diagnostic".to_owned(),
        ];
        assert_eq!(
            Diagnostics(&diagnostics).to_string(),
            ": first validated diagnostic; second validated diagnostic"
        );
        assert_eq!(Diagnostics(&[]).to_string(), "");
    }

    #[test]
    fn adversarial_provider_and_option_counts_fail_with_typed_errors() {
        let mut too_many_providers = dimensions();
        too_many_providers.provider_count = MAX_LINK_INPUTS;
        assert_eq!(
            validate_working_set_dimensions(too_many_providers),
            Err(
                ProtectedFirstBuildWorkerV3Error::WorkingSetInputCountExceeded {
                    actual: MAX_LINK_INPUTS,
                    maximum: MAX_LINK_INPUTS - 1,
                }
            )
        );

        let mut too_many_options = dimensions();
        too_many_options.option_count = MAX_LINK_OPTIONS + 1;
        assert_eq!(
            validate_working_set_dimensions(too_many_options),
            Err(
                ProtectedFirstBuildWorkerV3Error::WorkingSetOptionCountExceeded {
                    actual: MAX_LINK_OPTIONS + 1,
                    maximum: MAX_LINK_OPTIONS,
                }
            )
        );
    }

    #[test]
    fn adversarial_aggregate_size_fails_before_any_large_allocation() {
        let mut oversized = dimensions();
        oversized.outer_handoff_bytes = 224 * 1024 * 1024;
        oversized.compiler_module_bytes = 64 * 1024 * 1024;
        oversized.provider_payload_bytes = 0;
        oversized.provider_count = 0;
        oversized.envelope_bytes = 512 * 1024;
        oversized.manifest_bytes = 16 * 1024 * 1024;

        let error = validate_working_set_dimensions(oversized).unwrap_err();
        assert!(matches!(
            error,
            ProtectedFirstBuildWorkerV3Error::WorkingSetBudgetExceeded {
                required_bytes,
                maximum_bytes,
            } if required_bytes > maximum_bytes
                && maximum_bytes == MAX_PROTECTED_V3_LIVE_INPUT_REQUEST_BYTES_V1 as u64
        ));
    }

    #[test]
    fn adversarial_lengths_use_checked_arithmetic() {
        let mut overflowing = dimensions();
        overflowing.outer_handoff_bytes = usize::MAX;
        assert_eq!(
            validate_working_set_dimensions(overflowing),
            Err(
                ProtectedFirstBuildWorkerV3Error::WorkingSetArithmeticOverflow {
                    component: "aggregate live input/request bytes",
                }
            )
        );
    }

    #[test]
    fn streamed_plan_blob_is_identity_compatible_with_canonical_bytes() {
        let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
        let first = ContentIdentityV1::from_parts([0x11; 32], 17);
        let second = ContentIdentityV1::from_parts([0x22; 32], 23);
        let output = ContentIdentityV1::from_parts([0x33; 32], 31);
        let inputs = vec![
            LinkInputV1::new(first, target),
            LinkInputV1::new(second, target),
        ];
        let options = vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ];
        let provenance = vec![
            ProvenanceNodeV1::new(first, Vec::new()).unwrap(),
            ProvenanceNodeV1::new(second, Vec::new()).unwrap(),
            ProvenanceNodeV1::new(output, vec![first, second]).unwrap(),
        ];
        let plan = MultiInputLinkPlanV1::canonicalized(
            target,
            inputs,
            options,
            LinkOutputV1::new(output, target),
            provenance,
        )
        .unwrap();

        let mut streamed = Sha256::new();
        hash_canonical_plan_blob(&mut streamed, &plan).unwrap();
        let mut legacy = Sha256::new();
        hash_blob(&mut legacy, &plan.canonical_bytes());
        assert_eq!(streamed.finalize(), legacy.finalize());
    }
}
