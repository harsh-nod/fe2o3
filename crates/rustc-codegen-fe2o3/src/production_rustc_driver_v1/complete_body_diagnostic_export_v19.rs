//! Actual-source diagnostic export, with no serialized compiler-source owner.
use super::{
    Callbacks, Compilation, Compiler, Path, TyCtxt, lower_hex_v1, publish_new_inert_output,
    require_canonical_overflow_checks_v1, transaction_in_active_session_v1,
};

struct CompleteBodyCallbacks<'a> {
    output: &'a Path,
    calls: usize,
    result: Option<Result<(), String>>,
}
impl Callbacks for CompleteBodyCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(extract(tcx, self.output));
        Compilation::Stop
    }
}

/// Exports exact KIR19 from the normal authenticated MIR36 source continuation.
///
/// The live target owner passes the existing ranked/formal checks and canonical
/// LLVM relation before its immutable executable bytes are borrowed. This
/// exporter does not publish LLVM/handoff, invoke a native worker, run the CPU
/// or hardware, or export source custody. Raw bytes can enter only diagnostic
/// consumers; they cannot recreate this target or a protected compiler owner.
/// No source map, persisted schedule, old simulation-bundle or census schema
/// is relabeled as V19. The caller provides the complete targeted rustc argv.
pub fn run_diagnostic_complete_body_kir_extraction_driver_v19(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    let mut callbacks = CompleteBodyCallbacks {
        output,
        calls: 0,
        result: None,
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    if fatal.is_some() {
        return Err("diagnostic complete-body KIR V19 rustc analysis failed".into());
    }
    if callbacks.calls != 1 {
        return Err(
            "diagnostic complete-body KIR V19 requires exactly one actual rustc callback".into(),
        );
    }
    callbacks.result.unwrap_or_else(|| {
        Err("diagnostic complete-body KIR V19 callback produced no result".into())
    })
}

fn extract(tcx: TyCtxt<'_>, output: &Path) -> Result<(), String> {
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    if !transaction.has_authenticated_complete_body_v19() {
        return Err(
            "diagnostic KIR V19 requires an actual authenticated complete-body marker".into(),
        );
    }
    let target = transaction
        .lower_complete_body_target_v19()
        .map_err(|error| error.to_string())?;
    let checked = target.checked();
    let executable = checked.executable();
    let [kernel] = executable.module().kernels.as_slice() else {
        return Err("diagnostic KIR V19 requires one exact source kernel".into());
    };
    publish_new_inert_output(
        output,
        executable.canonical_bytes(),
        fe2o3_kernel_ir::MAX_MODULE_BYTES_V1,
        "diagnostic canonical KIR V19",
    )?;
    let metadata = serde_json::json!({
        "schema": "fe2o3-diagnostic-kir-v19-observation",
        "kernel": kernel.id.as_str(),
        "canonical_identity": lower_hex_v1(executable.identity().digest()),
        "canonical_bytes": executable.canonical_bytes().len(),
        "semantic_mir_v36": lower_hex_v1(checked.semantic_ssa().source_semantic().semantic_sha256().as_bytes()),
        "exported_source_authentication": false,
        "exported_compiler_authentication": false,
        "artifact_or_launch_authority": false,
        "hardware_observed": false,
    });
    eprintln!("fe2o3 diagnostic KIR19 input: {metadata}");
    eprintln!(
        "fe2o3 diagnostic extraction: actual Rust -> MIR36 -> normal checked KIR19; raw canonical bytes are diagnostic input, not compiler resume; source maps, physical VGPR/EXEC captures and hardware observations unavailable; artifact/launch authority false"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_overflow_contract_refuses_before_any_compiler_callback() {
        let error = run_diagnostic_complete_body_kir_extraction_driver_v19(
            &["rustc".into()],
            Path::new("/unopened-complete-body-v19"),
        )
        .unwrap_err();
        assert!(error.contains("exactly one canonical"));
    }
}
