//! Native strict-V3 compiler handoff execution through the direct LLVM worker.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffReceiptV3, CompilerModuleHandoffSlotV3,
    CompilerModuleHandoffTransactionIdentityV3, ConsumedCompilerModuleHandoffV3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2,
    FinalCompilerModuleCommitmentErrorV3, InertFinalCompilerModuleCommitmentV3,
    InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffIdentityV3,
    InertSemanticCompilerModuleHandoffV3,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, LinkInputKindClosureV1, LinkInputV1,
    LinkOptionV1, LinkOutputV1, LinkPlanError, MultiInputLinkPlanV1, PinnedWorkerV1,
    ProvenanceNodeV1, WorkerExecutionError, WorkerExecutionLimitsV1, WorkerInputV1,
    WorkerMeasurementV1, WorkerOutputConstraintsV1, WorkerProtocolError,
    WorkerRequestConstructionError, WorkerResponseV2,
    first_build_worker_v2::{
        FirstBuildWorkerV2EngineError, execute_reproducible_first_build_worker_v2_engine,
    },
    request_construction::{
        CompilerHandoffRequestBindingV2, construct_first_build_worker_request_v2_from_decoded,
        construct_plan_worker_request_v2_from_decoded, decode_link_options,
        decoded_compiler_module_handoff_v2,
    },
    worker_executor::InertWorkerExecutionV2,
};

const BINDING_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/PROTECTED-WORKER-COMPILER-HANDOFF-BINDING/V3\0";
const EVIDENCE_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/PROTECTED-FIRST-BUILD-WORKER-EVIDENCE/V3\0";

/// Stable identity of one exact protected Worker V3 compiler-handoff binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedCompilerHandoffBindingIdentityV3([u8; 32]);

impl ProtectedCompilerHandoffBindingIdentityV3 {
    /// Returns the domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
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

/// Closed Worker V3 binding derived only from one consumed strict-V3 transaction result.
///
/// Construction strictly redecodes the complete outer owner and repeats its capsule, invocation,
/// pair, compact commitment, and nested V2 associations. The transaction identity itself is
/// accepted only through the move-only artifact-transaction result; it cannot be recomputed here
/// because the producer namespace remains private to that crate.
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

        let exact_bytes = consumed.bytes();
        let decoded = InertSemanticCompilerModuleHandoffV3::decode(exact_bytes)
            .map_err(ProtectedCompilerHandoffBindingErrorV3::OuterHandoff)?;
        if decoded.canonical_bytes() != exact_bytes
            || decoded.identity() != consumed.handoff_identity()
            || decoded != *consumed.handoff()
        {
            return Err(binding_mismatch("outer V3 handoff"));
        }

        let handoff = consumed.handoff();
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
            attempt: consumed.attempt(),
            slot: consumed.slot(),
            transaction_identity: consumed.transaction_identity(),
            outer_handoff_identity: consumed.handoff_identity(),
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
    plan: MultiInputLinkPlanV1,
    bootstrap_request_bytes: Vec<u8>,
    bootstrap: InertProtectedCompilerHandoffExecutionV3,
    replay_request_bytes: Vec<u8>,
    replay: InertProtectedCompilerHandoffExecutionV3,
    _validation: ValidatedProtectedFirstBuildReplayV3,
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

    /// Moves the complete outer V3 owner and exact replay execution to the next typed boundary.
    pub fn into_handoff_and_exact_replay(
        self,
    ) -> (
        InertSemanticCompilerModuleHandoffV3,
        InertProtectedCompilerHandoffExecutionV3,
    ) {
        let Self {
            handoff, replay, ..
        } = self;
        (handoff, replay)
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
    /// Sealed direct-LLVM request construction failed.
    RequestConstruction(WorkerRequestConstructionError),
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
            Self::RequestConstruction(error) => {
                write!(
                    formatter,
                    "strict-V3 worker request construction failed: {error}"
                )
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
            Self::BootstrapDidNotProduceOutput(execution) => write!(
                formatter,
                "strict-V3 bootstrap produced no output at {:?}",
                execution.response().stage()
            ),
            Self::ReplayExecution(error) => {
                write!(
                    formatter,
                    "strict-V3 exact replay worker execution failed: {error}"
                )
            }
            Self::ReplayDidNotProduceOutput { replay, .. } => write!(
                formatter,
                "strict-V3 exact replay produced no output at {:?}",
                replay.response().stage()
            ),
            Self::OutputMismatch { .. } => formatter
                .write_str("strict-V3 bootstrap and exact replay worker output bytes differ"),
            Self::ReplayValidation { field } => {
                write!(formatter, "strict-V3 replay validation failed: {field}")
            }
        }
    }
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
            | Self::ReplayDidNotProduceOutput { .. }
            | Self::OutputMismatch { .. }
            | Self::ReplayValidation { .. } => None,
        }
    }
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
    let binding = ProtectedCompilerHandoffBindingV3::from_consumed(
        &consumed,
        expected_receipt,
        expected_compiler_closure,
    )
    .map_err(ProtectedFirstBuildWorkerV3Error::Binding)?;
    let nested = fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(
        consumed.handoff().module_handoff().canonical_bytes(),
    )
    .map_err(ProtectedFirstBuildWorkerV3Error::CompilerModuleHandoff)?;
    let decoded = decoded_compiler_module_handoff_v2(nested)
        .map_err(ProtectedFirstBuildWorkerV3Error::CompilerModuleHandoff)?;
    let handoff = consumed.into_handoff();

    let result = execute_reproducible_first_build_worker_v2_engine(
        CompilerHandoffRequestBindingV2::ProtectedV3(&binding),
        decoded,
        worker,
        external_providers,
        link_options,
        candidate_output_bound,
        limits,
    )
    .map_err(|error| map_engine_error(binding, error))?;

    validate_replay(binding, worker.measurement(), &result)?;
    let identity = calculate_evidence_identity(binding, worker.measurement(), &result);
    let bootstrap =
        InertProtectedCompilerHandoffExecutionV3::from_execution(binding, result.candidate);
    let replay =
        InertProtectedCompilerHandoffExecutionV3::from_execution(binding, result.authorized);
    Ok(InertProtectedFirstBuildWorkerV3EvidenceV1 {
        identity,
        binding,
        handoff,
        worker: worker.measurement().clone(),
        plan: result.plan,
        bootstrap_request_bytes: result.candidate_request_bytes,
        bootstrap,
        replay_request_bytes: result.authorized_request_bytes,
        replay,
        _validation: ValidatedProtectedFirstBuildReplayV3,
    })
}

fn validate_replay(
    binding: ProtectedCompilerHandoffBindingV3,
    worker: &WorkerMeasurementV1,
    result: &crate::first_build_worker_v2::FirstBuildWorkerV2EngineResult,
) -> Result<ValidatedProtectedFirstBuildReplayV3, ProtectedFirstBuildWorkerV3Error> {
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        &result.candidate_request_bytes,
        result.candidate.response().canonical_bytes(),
    )
    .map_err(|_| replay_error("bootstrap request/response canonical exchange"))?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        &result.authorized_request_bytes,
        result.authorized.response().canonical_bytes(),
    )
    .map_err(|_| replay_error("exact-replay request/response canonical exchange"))?;
    let bootstrap_output = bootstrap
        .response()
        .output()
        .ok_or_else(|| replay_error("missing bootstrap output"))?;
    let replay_output = replay
        .response()
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

    let (_, options) = decode_link_options(result.plan.options())
        .map_err(|_| replay_error("canonical link options"))?;
    let expected_bootstrap = construct_first_build_worker_request_v2_from_decoded(
        CompilerHandoffRequestBindingV2::ProtectedV3(&binding),
        worker,
        &result.decoded,
        bootstrap.request().external_providers().to_vec(),
        options,
        bootstrap.request().output_constraints().clone(),
    )
    .map_err(|_| replay_error("reconstructed bootstrap request"))?;
    if expected_bootstrap.sealed_request().canonical_bytes() != result.candidate_request_bytes {
        return Err(replay_error("bootstrap request identity"));
    }

    let reconstructed_plan = reconstruct_plan(
        &result.decoded,
        bootstrap.request().external_providers(),
        result.plan.options().to_vec(),
        bootstrap_output.identity(),
    )?;
    if reconstructed_plan != result.plan
        || reconstructed_plan.canonical_bytes() != result.plan.canonical_bytes()
    {
        return Err(replay_error("complete canonical link plan"));
    }
    let mut all_inputs = bootstrap.request().external_providers().to_vec();
    all_inputs.push(bootstrap.request().compiler_module().clone());
    all_inputs.sort_by_key(|input| (input.identity(), input.kind()));
    let input_kinds = LinkInputKindClosureV1::new(
        &result.plan,
        all_inputs.iter().map(|input| input.kind()).collect(),
    )
    .map_err(|_| replay_error("link-plan input-kind closure"))?;
    let expected_replay = construct_plan_worker_request_v2_from_decoded(
        CompilerHandoffRequestBindingV2::ProtectedV3(&binding),
        &result.plan,
        worker,
        &result.decoded,
        bootstrap.request().external_providers().to_vec(),
        &input_kinds,
        WorkerOutputConstraintsV1::new(bootstrap_output.identity().byte_len())
            .map_err(|_| replay_error("exact output bound"))?,
    )
    .map_err(|_| replay_error("reconstructed exact-replay request"))?;
    if expected_replay.sealed_request().canonical_bytes() != result.authorized_request_bytes {
        return Err(replay_error("exact-replay request identity"));
    }

    Ok(ValidatedProtectedFirstBuildReplayV3)
}

fn reconstruct_plan(
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    providers: &[WorkerInputV1],
    options: Vec<LinkOptionV1>,
    output_identity: ContentIdentityV1,
) -> Result<MultiInputLinkPlanV1, ProtectedFirstBuildWorkerV3Error> {
    let compiler = WorkerInputV1::new(
        decoded.compiler_module_kind(),
        decoded.compiler_module_bytes().to_vec(),
    )
    .map_err(|_| replay_error("nested compiler module input"))?;
    let mut inputs = providers.to_vec();
    inputs.push(compiler);
    inputs.sort_by_key(|input| (input.identity(), input.kind()));
    for pair in inputs.windows(2) {
        if pair[0].identity() == pair[1].identity() {
            return Err(replay_error("duplicate plan input identity"));
        }
    }
    let target = decoded.target();
    let link_inputs = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target))
        .collect::<Vec<_>>();
    let mut provenance = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ProtectedFirstBuildWorkerV3Error::LinkPlan)?;
    provenance.push(
        ProvenanceNodeV1::new(
            output_identity,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .map_err(ProtectedFirstBuildWorkerV3Error::LinkPlan)?,
    );
    MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        options,
        LinkOutputV1::new(output_identity, target),
        provenance,
    )
    .map_err(ProtectedFirstBuildWorkerV3Error::LinkPlan)
}

const fn replay_error(field: &'static str) -> ProtectedFirstBuildWorkerV3Error {
    ProtectedFirstBuildWorkerV3Error::ReplayValidation { field }
}

fn map_engine_error(
    binding: ProtectedCompilerHandoffBindingV3,
    error: FirstBuildWorkerV2EngineError,
) -> ProtectedFirstBuildWorkerV3Error {
    let wrap = |execution| {
        Box::new(InertProtectedCompilerHandoffExecutionV3::from_execution(
            binding, execution,
        ))
    };
    match error {
        FirstBuildWorkerV2EngineError::LinkPlan(error) => {
            ProtectedFirstBuildWorkerV3Error::LinkPlan(error)
        }
        FirstBuildWorkerV2EngineError::RequestConstruction(error) => {
            ProtectedFirstBuildWorkerV3Error::RequestConstruction(error)
        }
        FirstBuildWorkerV2EngineError::CandidateRequest(error) => {
            ProtectedFirstBuildWorkerV3Error::BootstrapRequest(error)
        }
        FirstBuildWorkerV2EngineError::CandidateExecution(error) => {
            ProtectedFirstBuildWorkerV3Error::BootstrapExecution(error)
        }
        FirstBuildWorkerV2EngineError::CandidateDidNotProduceOutput(execution) => {
            ProtectedFirstBuildWorkerV3Error::BootstrapDidNotProduceOutput(wrap(*execution))
        }
        FirstBuildWorkerV2EngineError::AuthorizedExecution(error) => {
            ProtectedFirstBuildWorkerV3Error::ReplayExecution(error)
        }
        FirstBuildWorkerV2EngineError::AuthorizedDidNotProduceOutput {
            candidate,
            authorized,
        } => ProtectedFirstBuildWorkerV3Error::ReplayDidNotProduceOutput {
            bootstrap: wrap(*candidate),
            replay: wrap(*authorized),
        },
        FirstBuildWorkerV2EngineError::OutputMismatch {
            candidate,
            authorized,
        } => ProtectedFirstBuildWorkerV3Error::OutputMismatch {
            bootstrap: wrap(*candidate),
            replay: wrap(*authorized),
        },
        FirstBuildWorkerV2EngineError::ReplayValidation(error) => {
            ProtectedFirstBuildWorkerV3Error::ReplayValidation {
                field: error.field(),
            }
        }
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
    result: &crate::first_build_worker_v2::FirstBuildWorkerV2EngineResult,
) -> ProtectedFirstBuildWorkerV3IdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(EVIDENCE_IDENTITY_DOMAIN_V3);
    binding.hash_identity_preimage(&mut hasher);
    hash_content(&mut hasher, worker.executable());
    hash_blob(&mut hasher, worker.worker_build_identity().as_bytes());
    hash_blob(&mut hasher, worker.llvm_build_identity().as_bytes());
    hash_blob(&mut hasher, &result.plan.canonical_bytes());
    hash_blob(&mut hasher, &result.candidate_request_bytes);
    hash_blob(&mut hasher, result.candidate.response().canonical_bytes());
    hash_blob(&mut hasher, &result.authorized_request_bytes);
    hash_blob(&mut hasher, result.authorized.response().canonical_bytes());
    ProtectedFirstBuildWorkerV3IdentityV1(hasher.finalize().into())
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
