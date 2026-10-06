//! Shared prepaid staging and transcript checks for native source families.
use crate::{
    LinkOptionV1, NativeFirstBuildWorkerErrorV1 as Error, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    first_build_worker_binding::WorkerCompilerBinding,
    first_build_worker_engine::{
        ReproducibleFirstBuildEnginePreflight as Preflight,
        ReproducibleFirstBuildEngineResult as Execution,
        execute_preflighted_reproducible_first_build_engine as execute,
        preflight_reproducible_first_build_engine as prepare,
    },
    first_build_worker_native::{engine_error, failure},
    first_build_worker_native_resources::NativeWorkerResourceQuote as Quote,
    first_build_worker_v3::{
        calculate_worker_evidence_identity_parts, enforce_worker_working_set_budget,
        validate_replay_parts,
    },
    request_construction::decode_compiler_module_handoff_v2,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffV2;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
pub(crate) const PREFLIGHT_ENTRY_WORK: usize = 2048;

/// The enclosing typed adapter retains and prepays its source. The returned
/// engine/quote are unreserved; reserve quote.preflight_storage + owner_header
/// before another operation or retaining them outside the caller's scope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_native_engine(
    binding: WorkerCompilerBinding<'_>,
    outer_len: usize,
    module: &CompilerModuleHandoffV2,
    worker: &PinnedWorkerV1,
    providers: Vec<WorkerInputV1>,
    options: Vec<LinkOptionV1>,
    output: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    owner_header: usize,
    budget: &mut Budget<'_>,
) -> Result<(Preflight, Quote), Error> {
    if providers.len() >= crate::MAX_LINK_INPUTS || options.len() > crate::MAX_LINK_OPTIONS {
        return Err(failure(
            "working set",
            "provider or option count exceeds the shared bound",
        ));
    }
    budget.charge_work(PREFLIGHT_ENTRY_WORK)?;
    enforce_worker_working_set_budget(outer_len, module, &providers, &options)
        .map_err(|e| failure("working set", e))?;
    let quote = Quote::new(module, &providers, &options, &output, limits)
        .map_err(|e| failure("resource quote", format_args!("{e:?}")))?;
    let storage = quote
        .preflight_storage
        .checked_add(owner_header)
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(0, 0, quote.preflight_work, storage, |_| {
        let decoded = decode_compiler_module_handoff_v2(module.canonical_bytes())
            .map_err(|e| failure("module decode", e))?;
        let engine =
            prepare(binding, decoded, worker, providers, options, output).map_err(engine_error)?;
        Ok((engine, quote))
    })
}

/// Called only inside the adapter's prepaid quote.execution_work/storage scope.
pub(crate) fn execute_native_engine(
    binding: WorkerCompilerBinding<'_>,
    engine: Preflight,
    measurement: &WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    worker: &PinnedWorkerV1,
) -> Result<(Execution, [u8; 32]), Error> {
    if worker.measurement() != measurement {
        return Err(Error::PreflightMismatch("measured worker"));
    }
    let result = execute(binding, engine, worker, limits).map_err(engine_error)?;
    validate_replay_parts(
        binding,
        measurement,
        &result.decoded,
        &result.plan,
        &result.candidate_request_bytes,
        result.candidate.response(),
        &result.authorized_request_bytes,
        result.authorized.response(),
    )
    .map_err(|e| failure("transcript replay", e))?;
    let identity = calculate_worker_evidence_identity_parts(
        binding,
        measurement,
        limits,
        &result.plan,
        &result.candidate_request_bytes,
        result.candidate.response().canonical_bytes(),
        &result.authorized_request_bytes,
        result.authorized.response().canonical_bytes(),
    )
    .map_err(|e| failure("evidence identity", e))?;
    Ok((result, identity))
}
