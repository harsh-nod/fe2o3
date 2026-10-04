//! Ordinary source selection only; no diagnostic-byte-to-source route.
use super::*;
use crate::production_pipeline::{CollectedRustStage, ProductionCompilation};
pub(super) fn extract_llvm<'tcx>(
    transaction: ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let lowered = transaction
        .lower_physical_global_copy_target_v21()
        .map_err(|e| e.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    publish_new_inert_output(
        output,
        lowered.llvm_ir().as_bytes(),
        dialect_amdgcn::GFX942_PHYSICAL_GLOBAL_COPY_LLVM_BYTES_V21,
        "physical-global-copy checked canonical LLVM",
    )?;
    eprintln!(
        "fe2o3 production extraction: authenticated Rust -> MIR38 -> ranked/formal checked KIR21 -> exact four-slot compiler ABI -> gfx942:xnack- inert LLVM; {} authored kernarg reads; {}; artifact/launch authority false",
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
        .lower_physical_global_copy_target_v21()
        .map_err(|e| e.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    // Hash the unchanged typed emission before moving its owner. These fixed
    // diagnostic digests do not discharge any runtime condition or survive as
    // a checked source owner in the serialized handoff.
    use sha2::Digest;
    let canonical_identity = *lowered.checked().executable().identity().digest();
    let canonical_llvm_sha256: [u8; 32] = sha2::Sha256::digest(lowered.llvm_ir().as_bytes()).into();
    let prepared =
        crate::production_worker_handoff::prepare_physical_global_copy_worker_handoff_v21(lowered)
            .map_err(|e| e.to_string())?;
    let conditions = prepared.unresolved_abi_conditions();
    let (handoff, descriptor) = prepared
        .into_inert_for_extraction()
        .map_err(|e| e.to_string())?;
    publish_new_inert_output(
        output,
        handoff.canonical_bytes(),
        fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
        "physical-global-copy inert compiler-module handoff",
    )?;
    // A bounded diagnostic relation for ordinary public CLI consumers. It is
    // not a new handoff format, compiler attestation or runtime-binding proof.
    eprintln!(
        "FE2O3_PHYSICAL_GLOBAL_COPY_HANDOFF_RELATION_V21 {}",
        serde_json::json!({
            "schema": "fe2o3-physical-global-copy-inert-handoff-relation-v21",
            "canonical_identity": lower_hex_v1(&canonical_identity),
            "canonical_llvm_sha256": lower_hex_v1(&canonical_llvm_sha256),
            "handoff_sha256": lower_hex_v1(&sha2::Sha256::digest(handoff.canonical_bytes())),
            "descriptor_sha256": lower_hex_v1(&sha2::Sha256::digest(descriptor.canonical_bytes())),
            "runtime_conditions_discharged": false,
            "source_custody_exported": false,
            "host_admitted": false,
            "protected_finalizer_admitted": false,
            "native_llvm_executed": false,
            "hardware_observed": false,
            "grants_artifact_or_launch_authority": false
        })
    );
    eprintln!(
        "fe2o3 production extraction: authenticated Rust -> MIR38 -> ranked/formal checked KIR21 -> exact source/compiler ABI descriptor -> inert worker handoff; {}; {} handoff byte(s); protected/native/host admission unavailable; artifact/launch authority false",
        conditions,
        handoff.canonical_bytes().len()
    );
    Ok(())
}
