//! Reproducible first-build bootstrap for compiler-FFI-aware Worker V2 links.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffIdentityV1, ConsumedCompilerModuleHandoffV1,
};
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeIdentityV1, CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV2,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestIdentityV1,
    CompilerModuleSymbolManifestV1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertCompilerHandoffExecutionV2, InertWorkerExecutionV1,
    LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1, LinkPlanError,
    MultiInputLinkPlanV1, PinnedWorkerV1, ProvenanceNodeV1, WorkerExecutionError,
    WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, WorkerProtocolError, WorkerRequestConstructionError,
    WorkerRequestV1,
    request_construction::{
        LinkSymbolClosureV1, construct_worker_request_v2_from_consumed_handoff,
        decode_link_options, derive_manifest_symbol_closure,
    },
};

const CANDIDATE_REQUEST_ID_DOMAIN_V1: &[u8] =
    b"FE2O3/FIRST-BUILD-GENERIC-LINK-CANDIDATE-REQUEST-ID/V1\0";
const FIRST_BUILD_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/REPRODUCIBLE-FIRST-BUILD-WORKER-V2-EVIDENCE/V1\0";

/// Stable identity of one successful reproducible first-build workflow.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FirstBuildWorkerV2IdentityV1([u8; 32]);

impl FirstBuildWorkerV2IdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert evidence that GenericLink V1 and compiler-FFI-aware V2 produced identical bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertFirstBuildWorkerV2EvidenceV1 {
    identity: FirstBuildWorkerV2IdentityV1,
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    compiler_envelope: CompilerFfiEnvelopeV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    worker: WorkerMeasurementV1,
    plan: MultiInputLinkPlanV1,
    candidate: InertWorkerExecutionV1,
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

    pub const fn candidate(&self) -> &InertWorkerExecutionV1 {
        &self.candidate
    }

    pub const fn authorized(&self) -> &InertCompilerHandoffExecutionV2 {
        &self.authorized
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
            .expect("successful first-build evidence retains a V2 output")
            .bytes()
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
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FirstBuildWorkerV2Error {
    CompilerModuleHandoff(CompilerModuleHandoffErrorV2),
    LinkPlan(LinkPlanError),
    RequestConstruction(WorkerRequestConstructionError),
    CandidateRequest(WorkerProtocolError),
    CandidateExecution(WorkerExecutionError),
    CandidateDidNotProduceOutput(Box<InertWorkerExecutionV1>),
    AuthorizedExecution(WorkerExecutionError),
    AuthorizedDidNotProduceOutput {
        candidate: Box<InertWorkerExecutionV1>,
        authorized: Box<InertCompilerHandoffExecutionV2>,
    },
    OutputMismatch {
        candidate: Box<InertWorkerExecutionV1>,
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
                write!(
                    formatter,
                    "GenericLink candidate request is invalid: {error}"
                )
            }
            Self::CandidateExecution(error) => {
                write!(formatter, "GenericLink candidate execution failed: {error}")
            }
            Self::CandidateDidNotProduceOutput(candidate) => {
                let response = candidate.response();
                write!(
                    formatter,
                    "GenericLink candidate did not produce output at {:?}: {:?}",
                    response.stage(),
                    response.diagnostics()
                )
            }
            Self::AuthorizedExecution(error) => {
                write!(formatter, "Worker V2 execution failed: {error}")
            }
            Self::AuthorizedDidNotProduceOutput { authorized, .. } => {
                let response = authorized.response();
                write!(
                    formatter,
                    "Worker V2 did not produce output at {:?}: {:?}",
                    response.stage(),
                    response.diagnostics()
                )
            }
            Self::OutputMismatch { .. } => formatter.write_str(
                "GenericLink candidate and compiler-FFI-aware Worker V2 output bytes differ",
            ),
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

/// Bootstraps an exact-output V2 plan with an independently classified GenericLink candidate.
///
/// The consumed handoff is decoded before either request is built. The candidate and V2 request
/// use the same exact compiler-module and external-provider bytes. The candidate output is inert
/// GenericLink evidence; its identity becomes the expected plan output. Success requires V2 to
/// reproduce the candidate bytes exactly and grants no publication, loading, or launch authority.
pub fn execute_reproducible_first_build_worker_v2(
    consumed: ConsumedCompilerModuleHandoffV1,
    worker: &PinnedWorkerV1,
    mut external_providers: Vec<WorkerInputV1>,
    mut link_options: Vec<LinkOptionV1>,
    candidate_output_bound: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertFirstBuildWorkerV2EvidenceV1, FirstBuildWorkerV2Error> {
    let attempt = consumed.attempt();
    let handoff_identity = consumed.identity();
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())
        .map_err(FirstBuildWorkerV2Error::CompilerModuleHandoff)?;
    let parts = handoff.into_parts();
    let target = parts.target();
    let code_object_version = parts.code_object_version();
    let (envelope, symbol_manifest, module) = parts.into_envelope_manifest_and_module();
    let compiler_envelope_identity = envelope.identity();
    let manifest_identity = symbol_manifest.identity();
    let directional_symbols = envelope.directional_symbols();
    let symbols = derive_manifest_symbol_closure(
        &symbol_manifest,
        directional_symbols.imports().map(str::to_owned).collect(),
        directional_symbols.exports().map(str::to_owned).collect(),
    )
    .map_err(FirstBuildWorkerV2Error::RequestConstruction)?;

    canonicalize_options(&mut link_options)?;
    let (planned_code_object_version, options) =
        decode_link_options(&link_options).map_err(FirstBuildWorkerV2Error::RequestConstruction)?;
    if planned_code_object_version != code_object_version {
        return Err(FirstBuildWorkerV2Error::RequestConstruction(
            WorkerRequestConstructionError::CodeObjectVersionMismatch {
                planned: planned_code_object_version,
                requested: code_object_version,
            },
        ));
    }

    external_providers.sort_by_key(|input| (input.identity(), input.kind()));
    let module_kind = match module.kind() {
        CompilerModuleKindV1::LlvmTextIr => WorkerInputKindV1::LlvmTextIr,
        CompilerModuleKindV1::LlvmBitcode => WorkerInputKindV1::LlvmBitcode,
    };
    let compiler_module = WorkerInputV1::new(module_kind, module.into_bytes())
        .map_err(FirstBuildWorkerV2Error::CandidateRequest)?;
    let mut all_inputs = external_providers.clone();
    all_inputs.push(compiler_module);
    all_inputs.sort_by_key(|input| (input.identity(), input.kind()));
    reject_duplicate_content_identities(&all_inputs)?;

    let candidate_request_id = calculate_candidate_request_id(
        attempt,
        handoff_identity,
        compiler_envelope_identity,
        manifest_identity,
        worker.measurement(),
        target,
        code_object_version,
        options,
        &all_inputs,
        &symbols,
        &link_options,
        &candidate_output_bound,
    );
    let candidate_request = WorkerRequestV1::new(
        candidate_request_id,
        worker.measurement().llvm_build_identity(),
        target,
        code_object_version,
        options,
        all_inputs.clone(),
        symbols.required_symbols().to_vec(),
        symbols.required_symbols().to_vec(),
        candidate_output_bound,
    )
    .map_err(FirstBuildWorkerV2Error::CandidateRequest)?;
    let candidate = worker
        .execute(&candidate_request, limits)
        .map_err(FirstBuildWorkerV2Error::CandidateExecution)?;
    let Some(candidate_output) = candidate.response().output() else {
        return Err(FirstBuildWorkerV2Error::CandidateDidNotProduceOutput(
            Box::new(candidate),
        ));
    };

    let plan = derive_plan(
        target,
        &all_inputs,
        link_options,
        candidate_output.identity(),
    )?;
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, all_inputs.iter().map(|input| input.kind()).collect())
            .map_err(FirstBuildWorkerV2Error::RequestConstruction)?;
    let exact_output = WorkerOutputConstraintsV1::new(candidate_output.identity().byte_len())
        .map_err(FirstBuildWorkerV2Error::CandidateRequest)?;
    let authorized_request = construct_worker_request_v2_from_consumed_handoff(
        &plan,
        worker.measurement(),
        consumed,
        external_providers,
        &input_kinds,
        exact_output,
    )
    .map_err(FirstBuildWorkerV2Error::RequestConstruction)?;
    let authorized = worker
        .execute_compiler_handoff_v2(&authorized_request, limits)
        .map_err(FirstBuildWorkerV2Error::AuthorizedExecution)?;
    let Some(authorized_output) = authorized.response().output() else {
        return Err(FirstBuildWorkerV2Error::AuthorizedDidNotProduceOutput {
            candidate: Box::new(candidate),
            authorized: Box::new(authorized),
        });
    };
    if candidate_output.bytes() != authorized_output.bytes() {
        return Err(FirstBuildWorkerV2Error::OutputMismatch {
            candidate: Box::new(candidate),
            authorized: Box::new(authorized),
        });
    }

    let identity = calculate_evidence_identity(
        attempt,
        handoff_identity,
        manifest_identity,
        worker.measurement(),
        &plan,
        &candidate,
        &authorized,
    );
    Ok(InertFirstBuildWorkerV2EvidenceV1 {
        identity,
        attempt,
        handoff_identity,
        compiler_envelope: envelope,
        symbol_manifest,
        worker: worker.measurement().clone(),
        plan,
        candidate,
        authorized,
    })
}

fn canonicalize_options(options: &mut [LinkOptionV1]) -> Result<(), FirstBuildWorkerV2Error> {
    options.sort();
    for pair in options.windows(2) {
        if pair[0].name() == pair[1].name() {
            let error = if pair[0].value() == pair[1].value() {
                LinkPlanError::DuplicateOption(pair[0].name().to_owned())
            } else {
                LinkPlanError::ConflictingOption(pair[0].name().to_owned())
            };
            return Err(FirstBuildWorkerV2Error::LinkPlan(error));
        }
    }
    Ok(())
}

fn reject_duplicate_content_identities(
    inputs: &[WorkerInputV1],
) -> Result<(), FirstBuildWorkerV2Error> {
    for pair in inputs.windows(2) {
        if pair[0].identity() == pair[1].identity() {
            return Err(FirstBuildWorkerV2Error::LinkPlan(
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
) -> Result<MultiInputLinkPlanV1, FirstBuildWorkerV2Error> {
    let link_inputs = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target))
        .collect::<Vec<_>>();
    let mut provenance = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(FirstBuildWorkerV2Error::LinkPlan)?;
    provenance.push(
        ProvenanceNodeV1::new(
            output_identity,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .map_err(FirstBuildWorkerV2Error::LinkPlan)?,
    );
    MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        options,
        LinkOutputV1::new(output_identity, target),
        provenance,
    )
    .map_err(FirstBuildWorkerV2Error::LinkPlan)
}

#[allow(clippy::too_many_arguments)]
fn calculate_candidate_request_id(
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    compiler_envelope_identity: CompilerFfiEnvelopeIdentityV1,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    worker: &WorkerMeasurementV1,
    target: fe2o3_kernel_descriptor::DeviceTargetV1,
    code_object_version: fe2o3_kernel_descriptor::CodeObjectVersion,
    options: crate::WorkerOptionsV1,
    inputs: &[WorkerInputV1],
    symbols: &LinkSymbolClosureV1,
    link_options: &[LinkOptionV1],
    output: &WorkerOutputConstraintsV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_REQUEST_ID_DOMAIN_V1);
    hash_attempt(&mut hasher, attempt);
    hasher.update(handoff_identity.as_bytes());
    hasher.update(compiler_envelope_identity.as_bytes());
    hash_manifest(&mut hasher, manifest_identity);
    hash_content(&mut hasher, worker.executable());
    hash_text(&mut hasher, worker.worker_build_identity());
    hash_text(&mut hasher, worker.llvm_build_identity());
    hash_text(&mut hasher, &target.to_string());
    hasher.update([code_object_version_byte(code_object_version)]);
    hasher.update([
        options.optimization() as u8,
        u8::from(options.strip_debug()),
        u8::from(options.verify_each()),
    ]);
    hasher.update((inputs.len() as u64).to_le_bytes());
    for input in inputs {
        hasher.update([input.kind() as u8]);
        hash_content(&mut hasher, input.identity());
    }
    hash_strings(&mut hasher, symbols.required_symbols());
    hash_strings(&mut hasher, symbols.import_symbols());
    hash_strings(&mut hasher, symbols.export_symbols());
    hasher.update((link_options.len() as u64).to_le_bytes());
    for option in link_options {
        hash_text(&mut hasher, option.name());
        hash_text(&mut hasher, option.value());
    }
    hasher.update(output.max_bytes().to_le_bytes());
    hasher.finalize().into()
}

fn calculate_evidence_identity(
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    worker: &WorkerMeasurementV1,
    plan: &MultiInputLinkPlanV1,
    candidate: &InertWorkerExecutionV1,
    authorized: &InertCompilerHandoffExecutionV2,
) -> FirstBuildWorkerV2IdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(FIRST_BUILD_EVIDENCE_DOMAIN_V1);
    hash_attempt(&mut hasher, attempt);
    hasher.update(handoff_identity.as_bytes());
    hash_manifest(&mut hasher, manifest_identity);
    hash_content(&mut hasher, worker.executable());
    hash_text(&mut hasher, worker.worker_build_identity());
    hash_text(&mut hasher, worker.llvm_build_identity());
    hasher.update(plan.identity().as_bytes());
    hasher.update(candidate.response().request_id());
    hasher.update(candidate.response().request_identity());
    hasher.update(authorized.response().request_id());
    hasher.update(authorized.response().request_identity());
    hasher.update(
        authorized
            .response()
            .compiler_envelope_identity()
            .as_bytes(),
    );
    hash_content(&mut hasher, plan.output().identity());
    FirstBuildWorkerV2IdentityV1(hasher.finalize().into())
}

fn hash_attempt(hasher: &mut Sha256, attempt: BuildAttempt) {
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
}

fn hash_content(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_manifest(hasher: &mut Sha256, identity: CompilerModuleSymbolManifestIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_strings(hasher: &mut Sha256, strings: &[String]) {
    hasher.update((strings.len() as u64).to_le_bytes());
    for string in strings {
        hash_text(hasher, string);
    }
}

fn hash_text(hasher: &mut Sha256, text: &str) {
    hasher.update((text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
}

const fn code_object_version_byte(version: fe2o3_kernel_descriptor::CodeObjectVersion) -> u8 {
    match version {
        fe2o3_kernel_descriptor::CodeObjectVersion::V4 => 4,
        fe2o3_kernel_descriptor::CodeObjectVersion::V5 => 5,
        fe2o3_kernel_descriptor::CodeObjectVersion::V6 => 6,
    }
}
