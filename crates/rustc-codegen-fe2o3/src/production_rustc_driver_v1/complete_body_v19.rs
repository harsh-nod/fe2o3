//! Existing extraction routes dispatch here only after the live authenticated
//! transaction selects its actual complete-body marker. There is no new flag
//! that converts inert bytes/model records into source ownership.
use super::*;
use crate::production_pipeline::{CollectedRustStage, ProductionCompilation};

pub(super) fn extract_llvm<'tcx>(
    transaction: ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let lowered = transaction
        .lower_complete_body_target_v19()
        .map_err(|error| error.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    publish_new_inert_output(
        output,
        lowered.llvm_ir().as_bytes(),
        16 * 1024,
        "complete-body canonical LLVM",
    )?;
    eprintln!(
        "fe2o3 production extraction: authenticated Rust -> MIR36 -> ranked/formal checked KIR19 -> gfx942:xnack- LLVM; {} LLVM byte(s), artifact/launch authority false",
        lowered.llvm_ir().len()
    );
    Ok(())
}

pub(super) fn extract_handoff<'tcx>(
    transaction: ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let lowered = transaction
        .lower_complete_body_target_v19()
        .map_err(|error| error.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    let prepared =
        crate::production_worker_handoff::prepare_complete_body_worker_handoff_v19(lowered)
            .map_err(|error| error.to_string())?;
    let (handoff, _descriptor) = prepared
        .into_validated_parts()
        .map_err(|error| error.to_string())?;
    publish_new_inert_output(
        output,
        handoff.canonical_bytes(),
        fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
        "complete-body compiler-module handoff",
    )?;
    eprintln!(
        "fe2o3 production extraction: authenticated Rust -> MIR36 -> ranked/formal checked KIR19 -> gfx942:xnack- LLVM -> inert compiler-module handoff; {} handoff byte(s), artifact/launch authority false",
        handoff.canonical_bytes().len()
    );
    Ok(())
}
