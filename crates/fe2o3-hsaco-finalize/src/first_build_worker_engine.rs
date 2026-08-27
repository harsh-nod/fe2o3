//! Internal reproducible first-build engine for the production Worker V3 transaction.
//!
//! Worker requests and responses still use the frozen V2 wire codec. Those version labels describe
//! serialized bytes only; this module exposes no V2 compilation or publication authority.

use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1,
    LinkPlanError, MultiInputLinkPlanV1, PinnedWorkerV1, ProtectedCompilerHandoffBindingV3,
    ProvenanceNodeV1, WorkerExecutionError, WorkerExecutionLimitsV1, WorkerInputV1,
    WorkerOutputConstraintsV1, WorkerProtocolError, WorkerRequestConstructionError,
    request_construction::{
        ConstructedFirstBuildWorkerRequest, DecodedCompilerModuleHandoffV2,
        construct_first_build_worker_request_from_decoded,
        construct_plan_worker_request_from_decoded, decode_link_options,
    },
    worker_executor::InertWorkerExecutionV2,
};

pub(crate) struct ReproducibleFirstBuildEngineResult {
    pub(crate) decoded: DecodedCompilerModuleHandoffV2,
    pub(crate) plan: MultiInputLinkPlanV1,
    pub(crate) candidate_request_bytes: Vec<u8>,
    pub(crate) candidate: InertWorkerExecutionV2,
    pub(crate) authorized_request_bytes: Vec<u8>,
    pub(crate) authorized: InertWorkerExecutionV2,
}

/// Deterministically validated first-build inputs prepared before worker execution.
///
/// This owner contains no process or artifact authority. Its construction performs every check
/// that depends only on the handoff, configured providers/options/output bound, and measured worker
/// identity. The candidate and replay request shapes are both validated before this value exists.
pub(crate) struct ReproducibleFirstBuildEnginePreflight {
    decoded: DecodedCompilerModuleHandoffV2,
    external_providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    all_inputs: Vec<WorkerInputV1>,
    candidate_request: ConstructedFirstBuildWorkerRequest,
    candidate_request_bytes: Vec<u8>,
}

pub(crate) enum ReproducibleFirstBuildEngineError {
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
pub(crate) fn preflight_reproducible_first_build_engine(
    binding: &ProtectedCompilerHandoffBindingV3,
    decoded: DecodedCompilerModuleHandoffV2,
    worker: &PinnedWorkerV1,
    mut external_providers: Vec<WorkerInputV1>,
    mut link_options: Vec<LinkOptionV1>,
    candidate_output_bound: WorkerOutputConstraintsV1,
) -> Result<ReproducibleFirstBuildEnginePreflight, ReproducibleFirstBuildEngineError> {
    canonicalize_options(&mut link_options)?;
    let (planned_code_object_version, options) = decode_link_options(&link_options)
        .map_err(ReproducibleFirstBuildEngineError::RequestConstruction)?;
    if planned_code_object_version != decoded.code_object_version() {
        return Err(ReproducibleFirstBuildEngineError::RequestConstruction(
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
    .map_err(ReproducibleFirstBuildEngineError::CandidateRequest)?;
    let mut all_inputs = external_providers.clone();
    all_inputs.push(compiler_module);
    all_inputs.sort_by_key(|input| (input.identity(), input.kind()));
    reject_duplicate_content_identities(&all_inputs)?;

    let candidate_request = construct_first_build_worker_request_from_decoded(
        binding,
        worker.measurement(),
        &decoded,
        external_providers.clone(),
        options,
        candidate_output_bound.clone(),
    )
    .map_err(ReproducibleFirstBuildEngineError::RequestConstruction)?;
    let candidate_request_bytes = candidate_request
        .sealed_request()
        .canonical_bytes()
        .to_vec();

    // The replay output identity is worker-produced, but its encoded shape and bounded length are
    // fixed. Validate the complete replay request with a collision-free synthetic identity now so
    // no configuration-only error remains after worker execution begins.
    let synthetic_output =
        synthetic_preflight_output_identity(&all_inputs, &candidate_output_bound);
    let synthetic_plan = derive_plan(
        decoded.target(),
        &all_inputs,
        link_options.clone(),
        synthetic_output,
    )?;
    let input_kinds = LinkInputKindClosureV1::new(
        &synthetic_plan,
        all_inputs.iter().map(|input| input.kind()).collect(),
    )
    .map_err(ReproducibleFirstBuildEngineError::RequestConstruction)?;
    construct_plan_worker_request_from_decoded(
        binding,
        &synthetic_plan,
        worker.measurement(),
        &decoded,
        external_providers.clone(),
        &input_kinds,
        candidate_output_bound.clone(),
    )
    .map_err(ReproducibleFirstBuildEngineError::RequestConstruction)?;

    Ok(ReproducibleFirstBuildEnginePreflight {
        decoded,
        external_providers,
        link_options,
        all_inputs,
        candidate_request,
        candidate_request_bytes,
    })
}

pub(crate) fn execute_preflighted_reproducible_first_build_engine(
    binding: &ProtectedCompilerHandoffBindingV3,
    preflight: ReproducibleFirstBuildEnginePreflight,
    worker: &PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<ReproducibleFirstBuildEngineResult, ReproducibleFirstBuildEngineError> {
    let ReproducibleFirstBuildEnginePreflight {
        decoded,
        external_providers,
        link_options,
        all_inputs,
        candidate_request,
        candidate_request_bytes,
    } = preflight;

    let candidate = worker
        .execute_v2(candidate_request.sealed_request(), limits)
        .map_err(ReproducibleFirstBuildEngineError::CandidateExecution)?;
    let Some(candidate_output) = candidate.response().output() else {
        return Err(
            ReproducibleFirstBuildEngineError::CandidateDidNotProduceOutput(Box::new(candidate)),
        );
    };

    let plan = derive_plan(
        decoded.target(),
        &all_inputs,
        link_options,
        candidate_output.identity(),
    )?;
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, all_inputs.iter().map(|input| input.kind()).collect())
            .map_err(ReproducibleFirstBuildEngineError::RequestConstruction)?;
    let exact_output = WorkerOutputConstraintsV1::new(candidate_output.identity().byte_len())
        .map_err(ReproducibleFirstBuildEngineError::CandidateRequest)?;
    let authorized_request = construct_plan_worker_request_from_decoded(
        binding,
        &plan,
        worker.measurement(),
        &decoded,
        external_providers,
        &input_kinds,
        exact_output,
    )
    .map_err(ReproducibleFirstBuildEngineError::RequestConstruction)?;
    let authorized_request_bytes = authorized_request
        .sealed_request()
        .canonical_bytes()
        .to_vec();
    let authorized = worker
        .execute_v2(authorized_request.sealed_request(), limits)
        .map_err(ReproducibleFirstBuildEngineError::AuthorizedExecution)?;
    let Some(authorized_output) = authorized.response().output() else {
        return Err(
            ReproducibleFirstBuildEngineError::AuthorizedDidNotProduceOutput {
                candidate: Box::new(candidate),
                authorized: Box::new(authorized),
            },
        );
    };
    if candidate_output.bytes() != authorized_output.bytes() {
        return Err(ReproducibleFirstBuildEngineError::OutputMismatch {
            candidate: Box::new(candidate),
            authorized: Box::new(authorized),
        });
    }
    Ok(ReproducibleFirstBuildEngineResult {
        decoded,
        plan,
        candidate_request_bytes,
        candidate,
        authorized_request_bytes,
        authorized,
    })
}

fn canonicalize_options(
    options: &mut [LinkOptionV1],
) -> Result<(), ReproducibleFirstBuildEngineError> {
    options.sort();
    for pair in options.windows(2) {
        if pair[0].name() == pair[1].name() {
            let error = if pair[0].value() == pair[1].value() {
                LinkPlanError::DuplicateOption(pair[0].name().to_owned())
            } else {
                LinkPlanError::ConflictingOption(pair[0].name().to_owned())
            };
            return Err(ReproducibleFirstBuildEngineError::LinkPlan(error));
        }
    }
    Ok(())
}

fn reject_duplicate_content_identities(
    inputs: &[WorkerInputV1],
) -> Result<(), ReproducibleFirstBuildEngineError> {
    for pair in inputs.windows(2) {
        if pair[0].identity() == pair[1].identity() {
            return Err(ReproducibleFirstBuildEngineError::LinkPlan(
                LinkPlanError::DuplicateInput(pair[0].identity()),
            ));
        }
    }
    Ok(())
}

fn synthetic_preflight_output_identity(
    inputs: &[WorkerInputV1],
    output: &WorkerOutputConstraintsV1,
) -> ContentIdentityV1 {
    let mut counter = 0_u64;
    loop {
        let mut hasher = Sha256::new();
        hasher.update(b"FE2O3/FIRST-BUILD/PREFLIGHT-OUTPUT/V1\0");
        hasher.update(counter.to_le_bytes());
        let identity = ContentIdentityV1::from_parts(hasher.finalize().into(), output.max_bytes());
        if inputs.iter().all(|input| input.identity() != identity) {
            return identity;
        }
        counter = counter
            .checked_add(1)
            .expect("finite bounded input identities cannot exhaust u64 preflight probes");
    }
}

fn derive_plan(
    target: fe2o3_kernel_descriptor::DeviceTargetV1,
    inputs: &[WorkerInputV1],
    options: Vec<LinkOptionV1>,
    output_identity: ContentIdentityV1,
) -> Result<MultiInputLinkPlanV1, ReproducibleFirstBuildEngineError> {
    let link_inputs = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target))
        .collect::<Vec<_>>();
    let mut provenance = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ReproducibleFirstBuildEngineError::LinkPlan)?;
    provenance.push(
        ProvenanceNodeV1::new(
            output_identity,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .map_err(ReproducibleFirstBuildEngineError::LinkPlan)?,
    );
    MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        options,
        LinkOutputV1::new(output_identity, target),
        provenance,
    )
    .map_err(ReproducibleFirstBuildEngineError::LinkPlan)
}
