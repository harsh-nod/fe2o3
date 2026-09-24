//! Explicit source-composition diagnostics. Never a protected resume route.
use super::{
    Callbacks, Compilation, Compiler, Path, TyCtxt, lower_hex_v1, publish_new_inert_output,
    require_canonical_overflow_checks_v1, transaction_in_active_session_v1,
};
#[cfg(target_os = "linux")]
use crate::production_pipeline::ordered_composition_v1::SourcePromotionRequestV1;
struct CompositionCallbacks<'a> {
    output: &'a Path,
    #[cfg(target_os = "linux")]
    promotion: Option<&'a SourcePromotionRequestV1>,
    calls: usize,
    result: Option<Result<(), String>>,
}
impl Callbacks for CompositionCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.calls != 0 {
            self.result = Some(Err(
                "ordered composition requires exactly one live callback".into(),
            ));
            return Compilation::Stop;
        }
        self.calls = 1;
        self.result = Some(extract(
            tcx,
            self.output,
            #[cfg(target_os = "linux")]
            self.promotion,
        ));
        Compilation::Stop
    }
}
/// Diagnostic-only Rust/MIR32 -> structural composition KIR17 -> inert LLVM.
/// Files cannot recreate the source owner or authorize an artifact or GPU launch.
/// No simulator/native compiler/debugger is invoked. Output must be a new directory.
pub fn run_diagnostic_ordered_composition_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    run(
        args,
        CompositionCallbacks {
            output,
            #[cfg(target_os = "linux")]
            promotion: None,
            calls: 0,
            result: None,
        },
    )
}
/// Explicitly promotes one live-source assembly region into a new Rust helper
/// file, optionally changing its typed instructions/register bindings. The
/// request is bounded untrusted JSON; it does not confer source custody. Original
/// source is never overwritten. Generated source MUST be freshly compiled.
/// Failure may leave a candidate or partial diagnostic directory; no rollback
/// or artifact/launch/proof authority follows from publication.
#[cfg(target_os = "linux")]
pub fn run_ordered_composition_source_promotion_driver_v1(
    args: &[String],
    output: &Path,
    request: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    let request = SourcePromotionRequestV1::read(request)?;
    run(
        args,
        CompositionCallbacks {
            output,
            promotion: Some(&request),
            calls: 0,
            result: None,
        },
    )
}
fn run(args: &[String], mut callbacks: CompositionCallbacks<'_>) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    if fatal.is_some() {
        // A compiler failure cannot imply absence of an attempted publication.
        return Err("ordered composition rustc analysis failed; inspect retained candidate/output before retrying".into());
    }
    if callbacks.calls != 1 {
        return Err("ordered composition did not observe one actual rustc callback".into());
    }
    callbacks
        .result
        .unwrap_or_else(|| Err("ordered composition callback produced no result".into()))
}
fn extract(
    tcx: TyCtxt<'_>,
    output: &Path,
    #[cfg(target_os = "linux")] promotion: Option<&SourcePromotionRequestV1>,
) -> Result<(), String> {
    let owner = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .lower_ordered_composition_diagnostic_v1()?;
    #[cfg(target_os = "linux")]
    let mut owner = owner;
    let composition = owner.materialized().composition();
    let canonical = composition.canonical();
    let (inventory, preflight) = owner.source_identities();
    let (work, retained, peak) = owner.resource_usage();
    let definitions=composition.definitions().iter().map(|d| {
        let s=d.site();
        serde_json::json!({"definition_ordinal":d.key().ordinal(),"function_ordinal":s.function_ordinal(),
            "block_id":s.block().0,"operation_ordinal":s.operation_ordinal(),
            "root_region":s.function_ordinal()==composition.root_function_ordinal()})
    }).collect::<Vec<_>>();
    let mut report = serde_json::json!({
        "schema":"fe2o3-ordered-composition-diagnostic-v1","actual_rustc_callback":true,
        "semantic_mir_version":32,"canonical_kir_version":17,
        "semantic_identity":lower_hex_v1(owner.source_seed().semantic_sha256()),
        "canonical_identity":lower_hex_v1(canonical.identity().digest()),
        "source_inventory_identity":lower_hex_v1(&inventory),"source_preflight_identity":lower_hex_v1(&preflight),
        "root_function_ordinal":composition.root_function_ordinal(),
        "static_region_definitions":composition.definitions().len(),"definitions":definitions,
        "helper_definitions":composition.helpers().len(),"helper_calls":composition.calls().len(),
        "root_qualified_region_occurrences":composition.occurrences().len(),
        "expanded_authored_instructions":composition.expanded_instruction_count(),
        "target":"gfx942:xnack-","wave_width":64,"logical_work_used":work,
        "logical_retained_storage":retained,"logical_peak_storage":peak,
        "ranked_checks":false,"formal_memory_admission":false,"functional_proof":false,
        "source_custody_exported":false,"compiler_custody_exported":false,
        "native_execution":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false,
        "source_promotion":null,
    });
    let mut directory = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory
        .create(output)
        .map_err(|e| format!("fresh ordered composition directory: {e}"))?;
    // Diagnose the original owner, not a fabricated owner of generated source.
    for (name, bytes, limit) in [
        (
            "canonical-v17.bin",
            canonical.canonical_bytes(),
            fe2o3_kernel_ir::MAX_MODULE_BYTES_V1,
        ),
        ("canonical.ll", owner.llvm_ir().as_bytes(), 16 * 1024 * 1024),
    ] {
        publish_new_inert_output(&output.join(name), bytes, limit, name)?;
    }
    #[cfg(target_os = "linux")]
    if let Some(request) = promotion {
        let published = owner.publish_source_candidate_v1(request)?;
        // Emit historical side-effect facts before any later report-write failure.
        eprintln!(
            "fe2o3 source candidate created; original preserved; candidate sha256={}; fresh compilation required",
            lower_hex_v1(&published.candidate_sha256)
        );
        report["source_promotion"] = serde_json::json!({
            "request_sha256":lower_hex_v1(request.request_digest()),
            "original_sha256":lower_hex_v1(&published.original_sha256),
            "candidate_sha256":lower_hex_v1(&published.candidate_sha256),
            "original_bytes":published.original_bytes,"candidate_bytes":published.candidate_bytes,
            "candidate_device":published.candidate_device,"candidate_inode":published.candidate_inode,
            "program_edited":published.program_edited,"created_new":true,"original_overwritten":false,
            "fresh_compilation_required":true,"fresh_compilation_observed":false,
            "source_custody_exported":false,"grants_artifact_or_launch_authority":false,
        });
        let (work, retained, peak) = owner.resource_usage();
        report["logical_work_used"] = work.into();
        report["logical_retained_storage"] = retained.into();
        report["logical_peak_storage"] = peak.into();
    }
    let bytes = serde_json::to_vec(&report).map_err(|e| format!("diagnostic JSON: {e}"))?;
    publish_new_inert_output(
        &output.join("observation.json"),
        &bytes,
        16 * 1024,
        "composition diagnostic JSON",
    )
    .map_err(|e| {
        format!(
            "{e}; candidate publication may already have completed; inspect output before retrying"
        )
    })?;
    eprintln!(
        "fe2o3 ordered composition diagnostic: actual Rust -> MIR32 -> pre-ranked KIR17 -> typed inert LLVM; ranked/formal/functional checks=false; source/compiler custody exported=false; native/hardware/artifact/launch authority=false"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_overflow_policy_is_required_before_source_or_output() {
        let error = run_diagnostic_ordered_composition_extraction_driver_v1(
            &["rustc".into()],
            Path::new("/unopened-ordered-composition"),
        )
        .unwrap_err();
        assert!(error.contains("exactly one canonical"));
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn promotion_requires_overflow_policy_before_request_io() {
        let error = run_ordered_composition_source_promotion_driver_v1(
            &["rustc".into()],
            Path::new("/unopened-composition-output"),
            Path::new("/unopened-request"),
        )
        .unwrap_err();
        assert!(error.contains("exactly one canonical"));
    }
}
