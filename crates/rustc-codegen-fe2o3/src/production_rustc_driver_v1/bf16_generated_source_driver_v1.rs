//! Public read-only source admission. The exact normal ranked refusal remains
//! a required observation, never converted into normal compiler success.
use super::*;

struct GeneratedCallbacksV1<'a> {
    output: &'a Path,
    calls: usize,
    result: Option<Result<(), String>>,
}

impl Callbacks for GeneratedCallbacksV1<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.calls != 0 {
            self.result = Some(Err(
                "generated BF16 inspection requires one callback; retain outputs".into(),
            ));
            return Compilation::Stop;
        }
        self.calls = 1;
        self.result = Some(extract_generated(tcx, self.output));
        Compilation::Stop
    }
}

/// Re-admit actual generated Rust through rustc, the authenticated nominal
/// source importer and existing pre-ranked constructor. This diagnostic command
/// requires the unchanged normal ranked refusal; it emits no normal artifact,
/// simulation result, source file or launch capability. Always retain a failed
/// output directory; an absent report after panic/abort is not a rollback.
pub fn run_bf16_generated_source_admission_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    validate_output(output)?;
    let mut callbacks = GeneratedCallbacksV1 {
        output,
        calls: 0,
        result: None,
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    if fatal.is_some() {
        return Err("generated BF16 rustc analysis failed; retain output/diagnostics".into());
    }
    if callbacks.calls != 1 {
        return Err("generated BF16 inspection did not observe one actual rustc callback".into());
    }
    callbacks
        .result
        .unwrap_or_else(|| Err("generated BF16 inspection produced no result".into()))
}

fn generated_report() -> serde_json::Value {
    serde_json::json!({
        "schema": "fe2o3-bf16-generated-source-admission-v1",
        "mode": "inspect_generated_source",
        "status": "failed",
        "target": "gfx942:xnack-", "wave_width": 64,
        "actual_rustc_callback": true,
        "source_postflight_ok": false,
        "source_admitted": false,
        "nominal_pre_ranked_materialized": false,
        "normal_ranked_attempted": false,
        "normal_ranked_admitted": false,
        "normal_refusal": null,
        "admission": null,
        "canonical_digest_domain": "compiler_identity_digest_not_sha256_serialized_bytes",
        "source_writes_performed": false,
        "simulation_performed": false,
        "artifact_emitted": false,
        "native_execution": false,
        "hardware_observed": false,
        "grants_artifact_or_launch_authority": false,
        "memory_measurement": "unavailable"
    })
}

fn extract_generated(tcx: TyCtxt<'_>, output: &Path) -> Result<(), String> {
    let original = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(output)
        .map_err(|error| format!("fresh generated BF16 output directory: {error}"))?;
    let mut report = generated_report();
    let failure = match original.inspect_bf16_generated_source_admission_v1() {
        Ok(facts) => {
            report["admission"] = serde_json::json!({
                "source_sha256": lower_hex_v1(&facts.source_sha256),
                "source_bytes": facts.source_bytes,
                "semantic_sha256": lower_hex_v1(&facts.semantic_sha256),
                "root_mir_sha256": lower_hex_v1(&facts.root_mir_sha256),
                "helper_mir_sha256": lower_hex_v1(&facts.helper_mir_sha256),
                "helper_source_signature_sha256": lower_hex_v1(&facts.helper_source_signature_sha256),
                "helper_fn_abi_sha256": lower_hex_v1(&facts.helper_fn_abi_sha256),
                "canonical_identity": lower_hex_v1(&facts.canonical_identity),
                "root": facts.root.to_string(),
                "helper": facts.helper.to_string(),
                "call_block": facts.call_block.to_string(),
                "return_permutation": facts.return_permutation,
                "copied_facts_are_source_authority": false
            });
            report["source_postflight_ok"] = true.into();
            report["source_admitted"] = true.into();
            report["nominal_pre_ranked_materialized"] = true.into();
            report["normal_ranked_attempted"] = true.into();
            report["normal_refusal"] = facts.normal_refusal.into();
            report["status"] = "nominal_source_admitted_normal_ranked_refused".into();
            None
        }
        Err(error) => Some(format!("generated BF16 source admission: {error}")),
    };
    // Reuse the exact existing 16-KiB serialization/create-new/fsync machinery.
    // Dynamic JSON/error formatting is not claimed to be a whole-memory bound.
    finish_report(output, &report, failure)
}

#[cfg(test)]
#[path = "bf16_generated_source_driver_v1_tests.rs"]
mod tests;
