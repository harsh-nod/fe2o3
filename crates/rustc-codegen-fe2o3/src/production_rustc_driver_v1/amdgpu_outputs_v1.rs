//! Live-session AMDGPU executable LLVM and inert handoff publication.
//! Source collection, target checks and all route refusals remain unchanged;
//! this leaf keeps output handling separate from the rustc callback dispatcher.
use super::*;

pub(super) fn extract_amdgpu_llvm_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
    expected_target: Option<&str>,
    census: Option<&SourceCensusRecorder>,
) -> Result<(), String> {
    let neutral_provider_observation = match (
        crate::trusted_device_items::definition(
            tcx,
            crate::trusted_device_items::TrustedDeviceItem::WorkgroupCollectivesCurrent,
        ),
        crate::trusted_device_items::definition(
            tcx,
            crate::trusted_device_items::TrustedDeviceItem::WorkgroupReduceSum,
        ),
    ) {
        (None, None) => None,
        (Some(_), Some(_)) => {
            let (collectives_current, provider_closure) =
                crate::trusted_device_items::authenticated_compiler_definition_observation_v1(
                    tcx,
                    crate::trusted_device_items::TrustedDeviceItem::WorkgroupCollectivesCurrent,
                )
                .map_err(|error| {
                    format!("neutral workgroup provider observation failed: {error}")
                })?;
            let (reduce_sum, reduce_provider_closure) =
                crate::trusted_device_items::authenticated_compiler_definition_observation_v1(
                    tcx,
                    crate::trusted_device_items::TrustedDeviceItem::WorkgroupReduceSum,
                )
                .map_err(|error| {
                    format!("neutral workgroup provider observation failed: {error}")
                })?;
            if provider_closure != reduce_provider_closure {
                return Err(
                    "neutral workgroup provider observations name different source closures".into(),
                );
            }
            Some((collectives_current, reduce_sum, provider_closure))
        }
        _ => {
            return Err(
                "neutral workgroup provider observation found an incomplete authenticated pair"
                    .into(),
            );
        }
    };
    let transaction = transaction_with_census_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        census,
    )?;
    if transaction.has_authenticated_physical_entry_v20() {
        return Err("MIR37/KIR20 physical-entry production LLVM requires mandatory ranked/formal/descriptor continuation; only explicit pre-ranked diagnostics are available".into());
    }
    if transaction.has_authenticated_complete_body_v19() {
        return complete_body_v19::extract_llvm(transaction, output, expected_target);
    }
    let lowered = transaction
        .lower_production_target()
        .map_err(|error| error.to_string())?;
    if let Some(expected_target) = expected_target
        && lowered.target_name() != expected_target
    {
        return Err(format!(
            "production LLVM extraction expected live target {expected_target:?}; found {:?}",
            lowered.target_name()
        ));
    }
    std::fs::write(output, lowered.llvm_ir()).map_err(|error| {
        format!(
            "failed to write production {} LLVM extraction `{}`: {error}",
            lowered.target_name(),
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> target-KIR optimizer ({} pass(es), {} mutating, epoch {}..={}) -> {} LLVM; {} semantic function(s), {} semantic u32 induction certificate(s) for {} checked addition(s), {} correspondence block(s), {} formal access(es), {} ranked dynamic-index discharge(s), ordered workgroups {:?}, {} LLVM byte(s), artifact/launch authority {}",
        lowered.canonical_kernel_ir_version(),
        lowered.guarded_store_count(),
        lowered.target_optimization_pass_count(),
        lowered.target_optimization_mutating_pass_count(),
        lowered.target_optimization_initial_epoch(),
        lowered.target_optimization_final_epoch(),
        lowered.target_name(),
        lowered.semantic_function_count(),
        lowered.semantic_u32_induction_certificate_count(),
        lowered.semantic_u32_induction_checked_addition_count(),
        lowered.correspondence_block_count(),
        lowered.formal_access_count(),
        lowered.ranked_dynamic_index_discharge_count(),
        lowered.workgroup_sizes(),
        lowered.llvm_ir().len(),
        lowered.grants_artifact_or_launch_authority(),
    );
    if let Some((collectives_current, reduce_sum, provider_closure)) = neutral_provider_observation
    {
        eprintln!(
            "fe2o3 production extraction: authenticated rustc provider definitions `{collectives_current}` and `{reduce_sum}` in source closure {}; this is a compiler build observation, not package or runtime authority",
            lower_hex_v1(&provider_closure),
        );
    }
    Ok(())
}

pub(super) fn extract_amdgpu_compiler_handoff_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
    expected_target: Option<&str>,
    census: Option<&SourceCensusRecorder>,
) -> Result<(), String> {
    if env::var_os(EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1).is_some() {
        return extract_amdgpu_semantic_compiler_handoff_in_active_session_v3(
            tcx,
            output,
            expected_target,
            census,
        );
    }
    let transaction = transaction_with_census_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        census,
    )?;
    if transaction.has_authenticated_physical_entry_v20() {
        return Err("MIR37/KIR20 physical-entry normal worker handoff is unavailable before mandatory ranked/formal/descriptor continuation".into());
    }
    if transaction.has_authenticated_complete_body_v19() {
        return complete_body_v19::extract_handoff(transaction, output, expected_target);
    }
    let lowered = transaction
        .lower_production_target()
        .map_err(|error| error.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    let target_name = lowered.target_name().to_owned();
    let canonical_kernel_ir_version = lowered.canonical_kernel_ir_version();
    let guarded_store_count = lowered.guarded_store_count();
    let handoff = lowered
        .into_inert_worker_handoff_for_extraction()
        .map_err(|error| error.to_string())?;
    std::fs::write(output, handoff.canonical_bytes()).map_err(|error| {
        format!(
            "failed to write inert production compiler-module handoff extraction `{}`: {error}",
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> {} LLVM -> compiler-bound inert handoff; {} handoff byte(s), artifact/launch authority false",
        canonical_kernel_ir_version,
        guarded_store_count,
        target_name,
        handoff.canonical_bytes().len(),
    );
    Ok(())
}

fn extract_amdgpu_semantic_compiler_handoff_in_active_session_v3(
    tcx: TyCtxt<'_>,
    output: &Path,
    expected_target: Option<&str>,
    census: Option<&SourceCensusRecorder>,
) -> Result<(), String> {
    let invocation = inert_extraction_invocation_v3()?;
    let transaction = transaction_with_census_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        census,
    )?;
    if transaction.has_authenticated_physical_entry_v20() {
        return Err("MIR37/KIR20 physical-entry kernels have no admitted semantic V3/protected handoff route".into());
    }
    if transaction.has_authenticated_complete_body_v19() {
        return Err("MIR36/KIR19 complete-body kernels have no admitted semantic V3/protected handoff route".to_owned());
    }
    let lowered = transaction
        .lower_production_target()
        .map_err(|error| error.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    let target_name = lowered.target_name().to_owned();
    let canonical_kernel_ir_version = lowered.canonical_kernel_ir_version();
    let guarded_store_count = lowered.guarded_store_count();
    let handoff = lowered
        .into_inert_semantic_worker_handoff_for_extraction(invocation)
        .map_err(|error| error.to_string())?;
    std::fs::write(output, handoff.canonical_bytes()).map_err(|error| {
        format!(
            "failed to write inert production semantic compiler-module handoff extraction `{}`: {error}",
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> {} LLVM -> proof-carrying semantic compiler-bound inert handoff; {} handoff byte(s), artifact/launch authority false",
        canonical_kernel_ir_version,
        guarded_store_count,
        target_name,
        handoff.canonical_bytes().len(),
    );
    Ok(())
}
