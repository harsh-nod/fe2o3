//! Explicit pre-ranked source diagnostics; exported bytes carry no source custody.

use super::{
    Path, ProductionExtractionCallbacksV1, TyCtxt, lower_hex_v1, publish_new_inert_output,
    run_production_driver_v1, transaction_in_active_session_v1,
};

#[cfg(test)]
#[path = "ordered_region_diagnostic_export_v16_tests.rs"]
mod tests;

/// Runs actual Rust source admission for the closed ordered-region profile and
/// publishes its exact raw canonical V16 bytes for diagnostic CPU consumers.
///
/// This explicit pre-ranked route does not run ranked or functional proof,
/// export source-variable maps, enter LLVM/artifact publication, or grant
/// compiler/source authentication to the output. It never falls back to another
/// extraction mode. The caller supplies the complete targeted rustc argv.
pub fn run_diagnostic_ordered_region_kir_extraction_driver_v16(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        diagnostic_kir_v16_output: Some(output.to_path_buf()),
        ..ProductionExtractionCallbacksV1::default()
    };
    run_production_driver_v1(
        args,
        callbacks,
        "diagnostic ordered-region KIR V16 callback did not reach rustc analysis",
    )
}

pub(super) fn extract_in_active_session_v16(tcx: TyCtxt<'_>, output: &Path) -> Result<(), String> {
    let owner = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .observe_ordered_region_v31()?;
    let executable = owner.materialized().executable();
    let bytes = executable.canonical().canonical_bytes();
    publish_new_diagnostic_kir_v16(output, bytes)?;
    let (inventory, preflight) = owner.authenticated_source_identities();
    eprintln!(
        "fe2o3 diagnostic extraction: actual Rust -> admitted semantic MIR -> semantic SSA -> pre_ranked_diagnostic exact KIR V16; declared_target=gfx942:xnack-, {} kernel(s), canonical_identity {}, {} byte(s), retained_source_inventory {}, retained_source_preflight {}; ranked_checks=false, functional_proof=false, exported_source_authentication=false, exported_compiler_authentication=false, protected_admission=false, artifact/load/launch/hardware_authority=false, source_variable_map=unavailable; raw bytes are diagnostic input, not production resume",
        executable.module().kernels.len(),
        lower_hex_v1(executable.identity().digest()),
        bytes.len(),
        lower_hex_v1(&inventory),
        lower_hex_v1(&preflight),
    );
    Ok(())
}

fn publish_new_diagnostic_kir_v16(output: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_new_inert_output(
        output,
        bytes,
        fe2o3_kernel_ir::MAX_MODULE_BYTES_V1,
        "diagnostic canonical KIR V16",
    )
}
