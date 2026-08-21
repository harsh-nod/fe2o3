//! Reproducible first-build bootstrap for compiler-FFI-aware Worker V2 links.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffIdentityV1, CompilerModuleHandoffIdentityV2,
    CompilerModuleHandoffSlotV2, ConsumedCompilerModuleHandoffV1, ConsumedCompilerModuleHandoffV2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeIdentityV1, CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV2,
    CompilerModuleSymbolManifestIdentityV1, CompilerModuleSymbolManifestV1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertCompilerHandoffExecutionV2, InertProtectedCompilerHandoffExecutionV2,
    LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1, LinkPlanError,
    MultiInputLinkPlanV1, PinnedWorkerV1, ProvenanceNodeV1, WorkerExecutionError,
    WorkerExecutionLimitsV1, WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerProtocolError, WorkerRequestConstructionError, WorkerResponseV2,
    request_construction::{
        CompilerHandoffRequestBindingV2, DecodedCompilerModuleHandoffV2,
        ProtectedCompilerHandoffBindingV2, construct_first_build_worker_request_v2_from_decoded,
        construct_plan_worker_request_v2_from_decoded, decode_compiler_module_handoff_v2,
        decode_link_options,
    },
    worker_executor::InertWorkerExecutionV2,
};

const FIRST_BUILD_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/REPRODUCIBLE-FIRST-BUILD-V2-AUTHENTICATED-REPLAY-EVIDENCE/V1\0";
const PROTECTED_FIRST_BUILD_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-FIRST-BUILD-V2-REPLAY-EVIDENCE/V1\0";

/// Stable identity of one successful reproducible first-build workflow.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FirstBuildWorkerV2IdentityV1([u8; 32]);

impl FirstBuildWorkerV2IdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert evidence that a compiler-aware V2 bootstrap and exact V2 replay produced identical bytes.
///
/// This evidence is move-only. Borrow it to inspect the complete bootstrap and replay transcript,
/// then consume it with [`Self::into_authorized_execution`] when transferring the exact replay to
/// a typed review boundary.
#[derive(Debug, Eq, PartialEq)]
pub struct InertFirstBuildWorkerV2EvidenceV1 {
    identity: FirstBuildWorkerV2IdentityV1,
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    compiler_envelope: CompilerFfiEnvelopeV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    worker: WorkerMeasurementV1,
    plan: MultiInputLinkPlanV1,
    candidate_request_bytes: Vec<u8>,
    candidate: InertCompilerHandoffExecutionV2,
    authorized_request_bytes: Vec<u8>,
    authorized: InertCompilerHandoffExecutionV2,
}

impl InertFirstBuildWorkerV2EvidenceV1 {
    pub const fn identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.identity
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.handoff_identity
    }

    pub const fn manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.symbol_manifest.identity()
    }

    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.compiler_envelope.identity()
    }

    pub const fn compiler_envelope(&self) -> &CompilerFfiEnvelopeV1 {
        &self.compiler_envelope
    }

    pub const fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        &self.symbol_manifest
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        &self.plan
    }

    /// Returns the identity of the complete retained native link plan.
    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.plan.identity()
    }

    /// First compiler-aware V2 execution under the caller's output ceiling.
    pub const fn bootstrap(&self) -> &InertCompilerHandoffExecutionV2 {
        &self.candidate
    }

    /// Exact canonical Worker V2 request bytes used by the bootstrap execution.
    pub fn bootstrap_request_bytes(&self) -> &[u8] {
        &self.candidate_request_bytes
    }

    /// Compatibility name for the first compiler-aware V2 execution.
    pub const fn candidate(&self) -> &InertCompilerHandoffExecutionV2 {
        self.bootstrap()
    }

    /// Second compiler-aware V2 execution bound to the first execution's exact output.
    pub const fn exact_replay(&self) -> &InertCompilerHandoffExecutionV2 {
        &self.authorized
    }

    pub const fn authorized(&self) -> &InertCompilerHandoffExecutionV2 {
        self.exact_replay()
    }

    /// Exact canonical Worker V2 request bytes used by the exact replay.
    pub fn authorized_request_bytes(&self) -> &[u8] {
        &self.authorized_request_bytes
    }

    pub const fn authorized_request_id(&self) -> &[u8; 32] {
        self.authorized.response().request_id()
    }

    pub const fn authorized_request_identity(&self) -> &[u8; 32] {
        self.authorized.response().request_identity()
    }

    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.plan.output().identity()
    }

    /// Borrows the exact-replay output for inert inspection only.
    ///
    /// These bytes grant no publication, load, or launch authority. A typed production route must
    /// not retain or copy them; it must consume this evidence through
    /// [`Self::into_authorized_execution`] and establish its own reviewed runtime capability.
    pub fn output_bytes(&self) -> &[u8] {
        self.authorized
            .response()
            .output()
            .expect("successful first-build evidence retains a V2 output")
            .bytes()
    }

    /// Consumes the reproducibility evidence and transfers sole exact-replay execution custody.
    ///
    /// The bootstrap execution and both retained request transcripts are dropped during this
    /// transition. The returned execution remains inert and grants no publication, load, or launch
    /// authority; a typed reviewer must consume it before any such capability can be created.
    pub fn into_authorized_execution(self) -> InertCompilerHandoffExecutionV2 {
        let Self { authorized, .. } = self;
        authorized
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

/// Stable identity of one closure-protected reproducible first-build workflow.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedFirstBuildWorkerV2IdentityV1([u8; 32]);

impl ProtectedFirstBuildWorkerV2IdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert replay evidence retaining the exact V2 handoff identity and full compiler closure.
///
/// This type is move-only and side-by-side with [`InertFirstBuildWorkerV2EvidenceV1`]. Its
/// protected evidence identity uses a separate domain and binds every compiler-closure field.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProtectedFirstBuildWorkerV2EvidenceV1 {
    identity: ProtectedFirstBuildWorkerV2IdentityV1,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV2,
    handoff_identity: CompilerModuleHandoffIdentityV2,
    compiler_closure: CompilerClosureV2,
    compiler_envelope: CompilerFfiEnvelopeV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    worker: WorkerMeasurementV1,
    plan: MultiInputLinkPlanV1,
    candidate_request_bytes: Vec<u8>,
    candidate: InertProtectedCompilerHandoffExecutionV2,
    authorized_request_bytes: Vec<u8>,
    authorized: InertProtectedCompilerHandoffExecutionV2,
}

impl InertProtectedFirstBuildWorkerV2EvidenceV1 {
    pub const fn identity(&self) -> ProtectedFirstBuildWorkerV2IdentityV1 {
        self.identity
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn slot(&self) -> CompilerModuleHandoffSlotV2 {
        self.slot
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.handoff_identity
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub const fn manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.symbol_manifest.identity()
    }

    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.compiler_envelope.identity()
    }

    pub const fn compiler_envelope(&self) -> &CompilerFfiEnvelopeV1 {
        &self.compiler_envelope
    }

    pub const fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        &self.symbol_manifest
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        &self.plan
    }

    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.plan.identity()
    }

    pub const fn bootstrap(&self) -> &InertProtectedCompilerHandoffExecutionV2 {
        &self.candidate
    }

    pub fn bootstrap_request_bytes(&self) -> &[u8] {
        &self.candidate_request_bytes
    }

    pub const fn candidate(&self) -> &InertProtectedCompilerHandoffExecutionV2 {
        self.bootstrap()
    }

    pub const fn exact_replay(&self) -> &InertProtectedCompilerHandoffExecutionV2 {
        &self.authorized
    }

    pub const fn authorized(&self) -> &InertProtectedCompilerHandoffExecutionV2 {
        self.exact_replay()
    }

    pub fn authorized_request_bytes(&self) -> &[u8] {
        &self.authorized_request_bytes
    }

    pub const fn authorized_request_id(&self) -> &[u8; 32] {
        self.authorized.response().request_id()
    }

    pub const fn authorized_request_identity(&self) -> &[u8; 32] {
        self.authorized.response().request_identity()
    }

    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.plan.output().identity()
    }

    pub fn output_bytes(&self) -> &[u8] {
        self.authorized
            .response()
            .output()
            .expect("successful protected first-build evidence retains a V2 output")
            .bytes()
    }

    pub fn into_authorized_execution(self) -> InertProtectedCompilerHandoffExecutionV2 {
        let Self { authorized, .. } = self;
        authorized
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
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

/// Failure from the two-execution first-build workflow.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FirstBuildWorkerV2Error {
    CompilerModuleHandoff(CompilerModuleHandoffErrorV2),
    LinkPlan(LinkPlanError),
    RequestConstruction(WorkerRequestConstructionError),
    CandidateRequest(WorkerProtocolError),
    CandidateExecution(WorkerExecutionError),
    CandidateDidNotProduceOutput(Box<InertCompilerHandoffExecutionV2>),
    AuthorizedExecution(WorkerExecutionError),
    AuthorizedDidNotProduceOutput {
        candidate: Box<InertCompilerHandoffExecutionV2>,
        authorized: Box<InertCompilerHandoffExecutionV2>,
    },
    OutputMismatch {
        candidate: Box<InertCompilerHandoffExecutionV2>,
        authorized: Box<InertCompilerHandoffExecutionV2>,
    },
}

impl fmt::Display for FirstBuildWorkerV2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompilerModuleHandoff(error) => {
                write!(
                    formatter,
                    "invalid consumed compiler-module handoff: {error}"
                )
            }
            Self::LinkPlan(error) => write!(formatter, "invalid derived link plan: {error}"),
            Self::RequestConstruction(error) => {
                write!(
                    formatter,
                    "first-build request construction failed: {error}"
                )
            }
            Self::CandidateRequest(error) => {
                write!(formatter, "bootstrap Worker V2 request is invalid: {error}")
            }
            Self::CandidateExecution(error) => {
                write!(formatter, "bootstrap Worker V2 execution failed: {error}")
            }
            Self::CandidateDidNotProduceOutput(candidate) => {
                let response = candidate.response();
                write!(
                    formatter,
                    "bootstrap Worker V2 did not produce output at {:?}: {:?}",
                    response.stage(),
                    response.diagnostics()
                )
            }
            Self::AuthorizedExecution(error) => {
                write!(
                    formatter,
                    "exact-replay Worker V2 execution failed: {error}"
                )
            }
            Self::AuthorizedDidNotProduceOutput { authorized, .. } => {
                let response = authorized.response();
                write!(
                    formatter,
                    "exact-replay Worker V2 did not produce output at {:?}: {:?}",
                    response.stage(),
                    response.diagnostics()
                )
            }
            Self::OutputMismatch { .. } => formatter
                .write_str("bootstrap Worker V2 and exact-replay Worker V2 output bytes differ"),
        }
    }
}

impl Error for FirstBuildWorkerV2Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerModuleHandoff(error) => Some(error),
            Self::LinkPlan(error) => Some(error),
            Self::RequestConstruction(error) => Some(error),
            Self::CandidateRequest(error) => Some(error),
            Self::CandidateExecution(error) | Self::AuthorizedExecution(error) => Some(error),
            Self::CandidateDidNotProduceOutput(_)
            | Self::AuthorizedDidNotProduceOutput { .. }
            | Self::OutputMismatch { .. } => None,
        }
    }
}

/// Failure from the closure-protected two-execution first-build workflow.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedFirstBuildWorkerV2Error {
    CompilerModuleHandoff(CompilerModuleHandoffErrorV2),
    LinkPlan(LinkPlanError),
    RequestConstruction(WorkerRequestConstructionError),
    CandidateRequest(WorkerProtocolError),
    CandidateExecution(WorkerExecutionError),
    CandidateDidNotProduceOutput(Box<InertProtectedCompilerHandoffExecutionV2>),
    AuthorizedExecution(WorkerExecutionError),
    AuthorizedDidNotProduceOutput {
        candidate: Box<InertProtectedCompilerHandoffExecutionV2>,
        authorized: Box<InertProtectedCompilerHandoffExecutionV2>,
    },
    OutputMismatch {
        candidate: Box<InertProtectedCompilerHandoffExecutionV2>,
        authorized: Box<InertProtectedCompilerHandoffExecutionV2>,
    },
}

impl fmt::Display for ProtectedFirstBuildWorkerV2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompilerModuleHandoff(error) => write!(
                formatter,
                "invalid closure-protected compiler-module handoff: {error}"
            ),
            Self::LinkPlan(error) => write!(formatter, "invalid derived link plan: {error}"),
            Self::RequestConstruction(error) => {
                write!(formatter, "protected first-build request failed: {error}")
            }
            Self::CandidateRequest(error) => {
                write!(
                    formatter,
                    "protected bootstrap Worker V2 request is invalid: {error}"
                )
            }
            Self::CandidateExecution(error) => {
                write!(
                    formatter,
                    "protected bootstrap Worker V2 execution failed: {error}"
                )
            }
            Self::CandidateDidNotProduceOutput(candidate) => {
                let response = candidate.response();
                write!(
                    formatter,
                    "protected bootstrap Worker V2 did not produce output at {:?}: {:?}",
                    response.stage(),
                    response.diagnostics()
                )
            }
            Self::AuthorizedExecution(error) => write!(
                formatter,
                "protected exact-replay Worker V2 execution failed: {error}"
            ),
            Self::AuthorizedDidNotProduceOutput { authorized, .. } => {
                let response = authorized.response();
                write!(
                    formatter,
                    "protected exact-replay Worker V2 did not produce output at {:?}: {:?}",
                    response.stage(),
                    response.diagnostics()
                )
            }
            Self::OutputMismatch { .. } => formatter.write_str(
                "protected bootstrap Worker V2 and exact-replay Worker V2 output bytes differ",
            ),
        }
    }
}

impl Error for ProtectedFirstBuildWorkerV2Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerModuleHandoff(error) => Some(error),
            Self::LinkPlan(error) => Some(error),
            Self::RequestConstruction(error) => Some(error),
            Self::CandidateRequest(error) => Some(error),
            Self::CandidateExecution(error) | Self::AuthorizedExecution(error) => Some(error),
            Self::CandidateDidNotProduceOutput(_)
            | Self::AuthorizedDidNotProduceOutput { .. }
            | Self::OutputMismatch { .. } => None,
        }
    }
}

/// Bootstraps an exact-output V2 plan through two compiler-aware Worker V2 executions.
///
/// The consumed handoff is decoded before either request is built. The bootstrap and exact replay
/// use the same exact compiler-module and external-provider bytes. The bootstrap output is inert;
/// its identity becomes the expected plan output. Success requires a second V2 execution to
/// reproduce those bytes exactly and grants no publication, loading, or launch authority.
pub fn execute_reproducible_first_build_worker_v2(
    consumed: ConsumedCompilerModuleHandoffV1,
    worker: &PinnedWorkerV1,
    external_providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    candidate_output_bound: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertFirstBuildWorkerV2EvidenceV1, FirstBuildWorkerV2Error> {
    let attempt = consumed.attempt();
    let handoff_identity = consumed.identity();
    let decoded = decode_compiler_module_handoff_v2(consumed.bytes())
        .map_err(FirstBuildWorkerV2Error::CompilerModuleHandoff)?;
    let binding = CompilerHandoffRequestBindingV2::Existing {
        attempt,
        handoff_identity,
    };
    let result = execute_reproducible_first_build_worker_v2_engine(
        binding,
        decoded,
        worker,
        external_providers,
        link_options,
        candidate_output_bound,
        limits,
    )
    .map_err(|error| map_existing_engine_error(attempt, handoff_identity, error))?;
    let candidate = InertCompilerHandoffExecutionV2::from_execution(
        attempt,
        handoff_identity,
        result.candidate,
    );
    let authorized = InertCompilerHandoffExecutionV2::from_execution(
        attempt,
        handoff_identity,
        result.authorized,
    );
    let identity = calculate_evidence_identity(
        attempt,
        handoff_identity,
        result.decoded.symbol_manifest().identity(),
        worker.measurement(),
        &result.plan,
        &candidate,
        &authorized,
    );
    Ok(InertFirstBuildWorkerV2EvidenceV1 {
        identity,
        attempt,
        handoff_identity,
        compiler_envelope: result.decoded.envelope().clone(),
        symbol_manifest: result.decoded.symbol_manifest().clone(),
        worker: worker.measurement().clone(),
        plan: result.plan,
        candidate_request_bytes: result.candidate_request_bytes,
        candidate,
        authorized_request_bytes: result.authorized_request_bytes,
        authorized,
    })
}

/// Executes the closure-protected first-build bootstrap and exact replay path.
pub fn execute_protected_reproducible_first_build_worker_v2(
    consumed: ConsumedCompilerModuleHandoffV2,
    worker: &PinnedWorkerV1,
    external_providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    candidate_output_bound: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertProtectedFirstBuildWorkerV2EvidenceV1, ProtectedFirstBuildWorkerV2Error> {
    let binding = ProtectedCompilerHandoffBindingV2::from_consumed(&consumed);
    let decoded = decode_compiler_module_handoff_v2(consumed.bytes())
        .map_err(ProtectedFirstBuildWorkerV2Error::CompilerModuleHandoff)?;
    let result = execute_reproducible_first_build_worker_v2_engine(
        CompilerHandoffRequestBindingV2::Protected(&binding),
        decoded,
        worker,
        external_providers,
        link_options,
        candidate_output_bound,
        limits,
    )
    .map_err(|error| map_protected_engine_error(binding, error))?;
    let candidate = protected_execution(binding, result.candidate);
    let authorized = protected_execution(binding, result.authorized);
    let identity = calculate_protected_evidence_identity(
        binding,
        result.decoded.symbol_manifest().identity(),
        worker.measurement(),
        &result.plan,
        &candidate,
        &authorized,
    );
    Ok(InertProtectedFirstBuildWorkerV2EvidenceV1 {
        identity,
        attempt: binding.attempt(),
        slot: binding.slot(),
        handoff_identity: binding.handoff_identity(),
        compiler_closure: binding.compiler_closure(),
        compiler_envelope: result.decoded.envelope().clone(),
        symbol_manifest: result.decoded.symbol_manifest().clone(),
        worker: worker.measurement().clone(),
        plan: result.plan,
        candidate_request_bytes: result.candidate_request_bytes,
        candidate,
        authorized_request_bytes: result.authorized_request_bytes,
        authorized,
    })
}

struct FirstBuildWorkerV2EngineResult {
    decoded: DecodedCompilerModuleHandoffV2,
    plan: MultiInputLinkPlanV1,
    candidate_request_bytes: Vec<u8>,
    candidate: InertWorkerExecutionV2,
    authorized_request_bytes: Vec<u8>,
    authorized: InertWorkerExecutionV2,
}

enum FirstBuildWorkerV2EngineError {
    LinkPlan(LinkPlanError),
    RequestConstruction(WorkerRequestConstructionError),
    CandidateRequest(WorkerProtocolError),
    CandidateExecution(WorkerExecutionError),
    CandidateDidNotProduceOutput(Box<InertWorkerExecutionV2>),
    AuthorizedExecution(WorkerExecutionError),
    AuthorizedDidNotProduceOutput {
        candidate: Box<InertWorkerExecutionV2>,
        authorized: Box<InertWorkerExecutionV2>,
    },
    OutputMismatch {
        candidate: Box<InertWorkerExecutionV2>,
        authorized: Box<InertWorkerExecutionV2>,
    },
}

#[allow(clippy::too_many_arguments)]
fn execute_reproducible_first_build_worker_v2_engine(
    binding: CompilerHandoffRequestBindingV2<'_>,
    decoded: DecodedCompilerModuleHandoffV2,
    worker: &PinnedWorkerV1,
    mut external_providers: Vec<WorkerInputV1>,
    mut link_options: Vec<LinkOptionV1>,
    candidate_output_bound: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<FirstBuildWorkerV2EngineResult, FirstBuildWorkerV2EngineError> {
    canonicalize_options(&mut link_options)?;
    let (planned_code_object_version, options) = decode_link_options(&link_options)
        .map_err(FirstBuildWorkerV2EngineError::RequestConstruction)?;
    if planned_code_object_version != decoded.code_object_version() {
        return Err(FirstBuildWorkerV2EngineError::RequestConstruction(
            WorkerRequestConstructionError::CodeObjectVersionMismatch {
                planned: planned_code_object_version,
                requested: decoded.code_object_version(),
            },
        ));
    }

    external_providers.sort_by_key(|input| (input.identity(), input.kind()));
    let compiler_module = WorkerInputV1::new(
        decoded.compiler_module_kind(),
        decoded.compiler_module_bytes().to_vec(),
    )
    .map_err(FirstBuildWorkerV2EngineError::CandidateRequest)?;
    let mut all_inputs = external_providers.clone();
    all_inputs.push(compiler_module);
    all_inputs.sort_by_key(|input| (input.identity(), input.kind()));
    reject_duplicate_content_identities(&all_inputs)?;

    let candidate_request = construct_first_build_worker_request_v2_from_decoded(
        binding,
        worker.measurement(),
        &decoded,
        external_providers.clone(),
        options,
        candidate_output_bound,
    )
    .map_err(FirstBuildWorkerV2EngineError::RequestConstruction)?;
    let candidate_request_bytes = candidate_request
        .sealed_request()
        .canonical_bytes()
        .to_vec();
    let candidate = worker
        .execute_v2(candidate_request.sealed_request(), limits)
        .map_err(FirstBuildWorkerV2EngineError::CandidateExecution)?;
    let Some(candidate_output) = candidate.response().output() else {
        return Err(FirstBuildWorkerV2EngineError::CandidateDidNotProduceOutput(
            Box::new(candidate),
        ));
    };

    let plan = derive_plan(
        decoded.target(),
        &all_inputs,
        link_options,
        candidate_output.identity(),
    )?;
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, all_inputs.iter().map(|input| input.kind()).collect())
            .map_err(FirstBuildWorkerV2EngineError::RequestConstruction)?;
    let exact_output = WorkerOutputConstraintsV1::new(candidate_output.identity().byte_len())
        .map_err(FirstBuildWorkerV2EngineError::CandidateRequest)?;
    let authorized_request = construct_plan_worker_request_v2_from_decoded(
        binding,
        &plan,
        worker.measurement(),
        &decoded,
        external_providers,
        &input_kinds,
        exact_output,
    )
    .map_err(FirstBuildWorkerV2EngineError::RequestConstruction)?;
    let authorized_request_bytes = authorized_request
        .sealed_request()
        .canonical_bytes()
        .to_vec();
    let authorized = worker
        .execute_v2(authorized_request.sealed_request(), limits)
        .map_err(FirstBuildWorkerV2EngineError::AuthorizedExecution)?;
    let Some(authorized_output) = authorized.response().output() else {
        return Err(
            FirstBuildWorkerV2EngineError::AuthorizedDidNotProduceOutput {
                candidate: Box::new(candidate),
                authorized: Box::new(authorized),
            },
        );
    };
    if candidate_output.bytes() != authorized_output.bytes() {
        return Err(FirstBuildWorkerV2EngineError::OutputMismatch {
            candidate: Box::new(candidate),
            authorized: Box::new(authorized),
        });
    }

    Ok(FirstBuildWorkerV2EngineResult {
        decoded,
        plan,
        candidate_request_bytes,
        candidate,
        authorized_request_bytes,
        authorized,
    })
}

fn map_existing_engine_error(
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    error: FirstBuildWorkerV2EngineError,
) -> FirstBuildWorkerV2Error {
    let wrap = |execution| {
        Box::new(InertCompilerHandoffExecutionV2::from_execution(
            attempt,
            handoff_identity,
            execution,
        ))
    };
    match error {
        FirstBuildWorkerV2EngineError::LinkPlan(error) => FirstBuildWorkerV2Error::LinkPlan(error),
        FirstBuildWorkerV2EngineError::RequestConstruction(error) => {
            FirstBuildWorkerV2Error::RequestConstruction(error)
        }
        FirstBuildWorkerV2EngineError::CandidateRequest(error) => {
            FirstBuildWorkerV2Error::CandidateRequest(error)
        }
        FirstBuildWorkerV2EngineError::CandidateExecution(error) => {
            FirstBuildWorkerV2Error::CandidateExecution(error)
        }
        FirstBuildWorkerV2EngineError::CandidateDidNotProduceOutput(candidate) => {
            FirstBuildWorkerV2Error::CandidateDidNotProduceOutput(wrap(*candidate))
        }
        FirstBuildWorkerV2EngineError::AuthorizedExecution(error) => {
            FirstBuildWorkerV2Error::AuthorizedExecution(error)
        }
        FirstBuildWorkerV2EngineError::AuthorizedDidNotProduceOutput {
            candidate,
            authorized,
        } => FirstBuildWorkerV2Error::AuthorizedDidNotProduceOutput {
            candidate: wrap(*candidate),
            authorized: wrap(*authorized),
        },
        FirstBuildWorkerV2EngineError::OutputMismatch {
            candidate,
            authorized,
        } => FirstBuildWorkerV2Error::OutputMismatch {
            candidate: wrap(*candidate),
            authorized: wrap(*authorized),
        },
    }
}

fn protected_execution(
    binding: ProtectedCompilerHandoffBindingV2,
    execution: InertWorkerExecutionV2,
) -> InertProtectedCompilerHandoffExecutionV2 {
    InertProtectedCompilerHandoffExecutionV2::from_execution(
        binding.attempt(),
        binding.slot(),
        binding.handoff_identity(),
        binding.compiler_closure(),
        execution,
    )
}

fn map_protected_engine_error(
    binding: ProtectedCompilerHandoffBindingV2,
    error: FirstBuildWorkerV2EngineError,
) -> ProtectedFirstBuildWorkerV2Error {
    let wrap = |execution| Box::new(protected_execution(binding, execution));
    match error {
        FirstBuildWorkerV2EngineError::LinkPlan(error) => {
            ProtectedFirstBuildWorkerV2Error::LinkPlan(error)
        }
        FirstBuildWorkerV2EngineError::RequestConstruction(error) => {
            ProtectedFirstBuildWorkerV2Error::RequestConstruction(error)
        }
        FirstBuildWorkerV2EngineError::CandidateRequest(error) => {
            ProtectedFirstBuildWorkerV2Error::CandidateRequest(error)
        }
        FirstBuildWorkerV2EngineError::CandidateExecution(error) => {
            ProtectedFirstBuildWorkerV2Error::CandidateExecution(error)
        }
        FirstBuildWorkerV2EngineError::CandidateDidNotProduceOutput(candidate) => {
            ProtectedFirstBuildWorkerV2Error::CandidateDidNotProduceOutput(wrap(*candidate))
        }
        FirstBuildWorkerV2EngineError::AuthorizedExecution(error) => {
            ProtectedFirstBuildWorkerV2Error::AuthorizedExecution(error)
        }
        FirstBuildWorkerV2EngineError::AuthorizedDidNotProduceOutput {
            candidate,
            authorized,
        } => ProtectedFirstBuildWorkerV2Error::AuthorizedDidNotProduceOutput {
            candidate: wrap(*candidate),
            authorized: wrap(*authorized),
        },
        FirstBuildWorkerV2EngineError::OutputMismatch {
            candidate,
            authorized,
        } => ProtectedFirstBuildWorkerV2Error::OutputMismatch {
            candidate: wrap(*candidate),
            authorized: wrap(*authorized),
        },
    }
}

fn canonicalize_options(options: &mut [LinkOptionV1]) -> Result<(), FirstBuildWorkerV2EngineError> {
    options.sort();
    for pair in options.windows(2) {
        if pair[0].name() == pair[1].name() {
            let error = if pair[0].value() == pair[1].value() {
                LinkPlanError::DuplicateOption(pair[0].name().to_owned())
            } else {
                LinkPlanError::ConflictingOption(pair[0].name().to_owned())
            };
            return Err(FirstBuildWorkerV2EngineError::LinkPlan(error));
        }
    }
    Ok(())
}

fn reject_duplicate_content_identities(
    inputs: &[WorkerInputV1],
) -> Result<(), FirstBuildWorkerV2EngineError> {
    for pair in inputs.windows(2) {
        if pair[0].identity() == pair[1].identity() {
            return Err(FirstBuildWorkerV2EngineError::LinkPlan(
                LinkPlanError::DuplicateInput(pair[0].identity()),
            ));
        }
    }
    Ok(())
}

fn derive_plan(
    target: fe2o3_kernel_descriptor::DeviceTargetV1,
    inputs: &[WorkerInputV1],
    options: Vec<LinkOptionV1>,
    output_identity: ContentIdentityV1,
) -> Result<MultiInputLinkPlanV1, FirstBuildWorkerV2EngineError> {
    let link_inputs = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target))
        .collect::<Vec<_>>();
    let mut provenance = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(FirstBuildWorkerV2EngineError::LinkPlan)?;
    provenance.push(
        ProvenanceNodeV1::new(
            output_identity,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .map_err(FirstBuildWorkerV2EngineError::LinkPlan)?,
    );
    MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        options,
        LinkOutputV1::new(output_identity, target),
        provenance,
    )
    .map_err(FirstBuildWorkerV2EngineError::LinkPlan)
}

fn calculate_evidence_identity(
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    worker: &WorkerMeasurementV1,
    plan: &MultiInputLinkPlanV1,
    candidate: &InertCompilerHandoffExecutionV2,
    authorized: &InertCompilerHandoffExecutionV2,
) -> FirstBuildWorkerV2IdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(FIRST_BUILD_EVIDENCE_DOMAIN_V1);
    hash_attempt(&mut hasher, attempt);
    hasher.update(handoff_identity.as_bytes());
    hash_first_build_evidence(
        &mut hasher,
        manifest_identity,
        worker,
        plan,
        candidate.response(),
        authorized.response(),
    );
    FirstBuildWorkerV2IdentityV1(hasher.finalize().into())
}

fn calculate_protected_evidence_identity(
    binding: ProtectedCompilerHandoffBindingV2,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    worker: &WorkerMeasurementV1,
    plan: &MultiInputLinkPlanV1,
    candidate: &InertProtectedCompilerHandoffExecutionV2,
    authorized: &InertProtectedCompilerHandoffExecutionV2,
) -> ProtectedFirstBuildWorkerV2IdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_FIRST_BUILD_EVIDENCE_DOMAIN_V1);
    hash_attempt(&mut hasher, binding.attempt());
    hasher.update([binding.slot() as u8]);
    hasher.update(binding.handoff_identity().as_bytes());
    hash_compiler_closure_v2(&mut hasher, binding.compiler_closure());
    hash_first_build_evidence(
        &mut hasher,
        manifest_identity,
        worker,
        plan,
        candidate.response(),
        authorized.response(),
    );
    ProtectedFirstBuildWorkerV2IdentityV1(hasher.finalize().into())
}

fn hash_first_build_evidence(
    hasher: &mut Sha256,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    worker: &WorkerMeasurementV1,
    plan: &MultiInputLinkPlanV1,
    candidate: &WorkerResponseV2,
    authorized: &WorkerResponseV2,
) {
    hash_manifest(hasher, manifest_identity);
    hash_content(hasher, worker.executable());
    hash_text(hasher, worker.worker_build_identity());
    hash_text(hasher, worker.llvm_build_identity());
    hasher.update(plan.identity().as_bytes());
    hasher.update(candidate.request_id());
    hasher.update(candidate.request_identity());
    hash_authenticated_response_evidence(hasher, candidate);
    hasher.update(authorized.request_id());
    hasher.update(authorized.request_identity());
    hash_authenticated_response_evidence(hasher, authorized);
    hasher.update(authorized.compiler_envelope_identity().as_bytes());
    hash_content(hasher, plan.output().identity());
}

fn hash_authenticated_response_evidence(hasher: &mut Sha256, response: &WorkerResponseV2) {
    match response.response_identity() {
        Some(identity) => {
            hasher.update([1]);
            hasher.update(identity);
        }
        None => hasher.update([0]),
    }
    match response.device_library_provider() {
        Some(provider) => {
            hasher.update([1]);
            hasher.update(provider.manifest_identity());
        }
        None => hasher.update([0]),
    }
}

fn hash_attempt(hasher: &mut Sha256, attempt: BuildAttempt) {
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
}

fn hash_compiler_closure_v2(hasher: &mut Sha256, closure: CompilerClosureV2) {
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

fn hash_manifest(hasher: &mut Sha256, identity: CompilerModuleSymbolManifestIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_text(hasher: &mut Sha256, text: &str) {
    hasher.update((text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
}
