//! Ordinary source selection only; no diagnostic-byte-to-source route.
use super::*;
use crate::production_pipeline::{CollectedRustStage, ProductionCompilation};
pub(super) fn extract_llvm<'tcx>(
    transaction: ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let lowered = transaction
        .lower_physical_entry_target_v20()
        .map_err(|e| e.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    publish_new_inert_output(
        output,
        lowered.llvm_ir().as_bytes(),
        dialect_amdgcn::GFX942_PHYSICAL_ENTRY_LLVM_BYTES_V20,
        "physical-entry checked canonical LLVM",
    )?;
    eprintln!(
        "fe2o3 production extraction: authenticated Rust -> MIR37 -> ranked/formal checked KIR20 -> exact six-slot compiler ABI -> gfx942:xnack- inert LLVM; {} authored kernarg reads; {}; artifact/launch authority false",
        lowered.checked().memory_obligations().kernarg_reads().len(),
        lowered.unresolved_abi_conditions()
    );
    Ok(())
}
pub(super) fn extract_handoff<'tcx>(
    transaction: ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let lowered = transaction
        .lower_physical_entry_target_v20()
        .map_err(|e| e.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    let prepared =
        crate::production_worker_handoff::prepare_physical_entry_worker_handoff_v20(lowered)
            .map_err(|e| e.to_string())?;
    let conditions = prepared.unresolved_abi_conditions();
    let (handoff, _descriptor) = prepared
        .into_inert_for_extraction()
        .map_err(|e| e.to_string())?;
    publish_new_inert_output(
        output,
        handoff.canonical_bytes(),
        fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
        "physical-entry inert compiler-module handoff",
    )?;
    eprintln!(
        "fe2o3 production extraction: authenticated Rust -> MIR37 -> ranked/formal checked KIR20 -> exact source/compiler ABI descriptor -> inert worker handoff; {}; {} handoff byte(s); protected/native/host admission unavailable; artifact/launch authority false",
        conditions,
        handoff.canonical_bytes().len()
    );
    Ok(())
}
