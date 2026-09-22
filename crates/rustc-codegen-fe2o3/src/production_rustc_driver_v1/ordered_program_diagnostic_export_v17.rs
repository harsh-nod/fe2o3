//! Explicit pre-ranked program diagnostics; raw V17 carries no source custody.

use super::{
    Path, ProductionExtractionCallbacksV1, TyCtxt, lower_hex_v1, publish_new_inert_output,
    run_production_driver_v1, transaction_in_active_session_v1,
};

#[cfg(test)]
#[path = "ordered_program_diagnostic_export_v17_tests.rs"]
mod tests;

/// Runs actual source admission for the closed one-to-sixteen-step ordered u32
/// program and publishes the exact raw canonical V17 diagnostic CPU input.
///
/// This explicit route retains source custody while obtaining the one immutable
/// executable owner, but does not export that custody in the bytes. It runs no
/// ranked/functional proof, source-map export, LLVM or artifact publication and
/// never falls back to V16, a simulation bundle or a production resume path.
/// The caller supplies the complete targeted rustc argv.
pub fn run_diagnostic_ordered_program_kir_extraction_driver_v17(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        diagnostic_kir_v17_output: Some(output.to_path_buf()),
        ..ProductionExtractionCallbacksV1::default()
    };
    run_production_driver_v1(
        args,
        callbacks,
        "diagnostic ordered-program KIR V17 callback did not reach rustc analysis",
    )
}

pub(super) fn extract_in_active_session_v17(tcx: TyCtxt<'_>, output: &Path) -> Result<(), String> {
    let owner = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .observe_ordered_program_v32()?;
    let executable = owner.materialized().executable();
    let bytes = executable.canonical().canonical_bytes();
    publish_new_diagnostic_kir_v17(output, bytes)?;
    let (inventory, preflight) = owner.authenticated_source_identities();
    eprintln!(
        "fe2o3 diagnostic extraction: actual Rust -> admitted semantic MIR V32 -> semantic SSA -> pre_ranked_diagnostic exact KIR V17; declared_target=gfx942:xnack-, declared_wave=64, {} kernel(s), canonical_identity {}, {} byte(s), retained_source_inventory {}, retained_source_preflight {}; ranked_checks=false, functional_proof=false, exported_source_authentication=false, exported_compiler_authentication=false, protected_admission=false, artifact/load/launch/hardware_authority=false, source_variable_map=unavailable, physical_register_values=unavailable, instruction_microsteps=unavailable; raw bytes are diagnostic input, not production resume",
        executable.module().kernels.len(),
        lower_hex_v1(executable.identity().digest()),
        bytes.len(),
        lower_hex_v1(&inventory),
        lower_hex_v1(&preflight),
    );
    eprintln!(
        "{}",
        source_identity_line_v17(
            owner
                .materialized()
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            executable.identity().digest(),
        )
    );
    Ok(())
}

// Two already-computed fixed-size identities from this one live owner.
// This additive diagnostic does not export source custody or change admission.
fn source_identity_line_v17(semantic: &[u8; 32], canonical: &[u8; 32]) -> String {
    format!(
        "fe2o3 diagnostic source identities: semantic_mir_v32 {}, canonical_kir_v17 {}; observation_only=true, exported_source_authentication=false",
        lower_hex_v1(semantic),
        lower_hex_v1(canonical),
    )
}

fn publish_new_diagnostic_kir_v17(output: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_new_inert_output(
        output,
        bytes,
        fe2o3_kernel_ir::MAX_MODULE_BYTES_V1,
        "diagnostic canonical KIR V17",
    )
}
