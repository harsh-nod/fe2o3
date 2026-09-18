//! Fixed Policy5 actual-O observation, without protected publication authority.
use super::*;

#[cfg(test)]
#[path = "production_rustc_driver_fixed_checked_output_v1_tests.rs"]
mod tests;

struct FixedCheckedOutputCallbacksV1<'a> {
    output: &'a Path,
    result: Option<Result<(), String>>,
}

impl Callbacks for FixedCheckedOutputCallbacksV1<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(extract_in_active_session(tcx, self.output));
        Compilation::Stop
    }
}

fn extract_in_active_session(tcx: TyCtxt<'_>, output: &Path) -> Result<(), String> {
    let stage = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .verify_general_kernel_checks()
    .and_then(|ranked| ranked.lower_fixed_checked_output_v1())
    .map_err(|error| error.to_string())?;
    let original = stage.original_digest();
    let erased = stage.erased_digest().copied();
    let final_output = *stage.output().canonical().identity().digest();
    let policy = stage.checked_output().execution().policy_version();
    let retained_floor = stage.retained_storage_floor_v1();
    let (handoff, descriptor) = stage
        .into_worker_handoff_extraction_v1()
        .map_err(|error| error.to_string())?;
    let bytes = handoff.module_bytes();
    publish_new_extraction_bytes_v1(
        output,
        bytes,
        dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES,
        "fixed checked-output LLVM extraction",
    )?;
    eprintln!(
        "fe2o3 fixed checked-output extraction: original N {}, distinct E {:?}, actual O {}, fixed policy {}, target {:?}, {} ordered kernel(s), {} LLVM byte(s), retained logical floor {}; extraction-only observation, proof/compiler/artifact/load/launch authority false",
        lower_hex_v1(&original),
        erased.as_ref().map(|digest| lower_hex_v1(digest)),
        lower_hex_v1(&final_output),
        policy,
        handoff.target(),
        descriptor.table().kernels().len(),
        bytes.len(),
        retained_floor,
    );
    Ok(())
}

/// Extracts actual final-O LLVM through one compiled checked schedule.
/// The authenticated source owner alone selects Direct or UnitLocal erasure.
/// This is not the legacy default or the protected compiler-handoff publisher.
pub fn run_production_fixed_checked_output_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    let mut callbacks = FixedCheckedOutputCallbacksV1 {
        output,
        result: None,
    };
    rustc_driver::run_compiler(args, &mut callbacks);
    callbacks.result.unwrap_or_else(|| {
        Err("fixed checked-output extraction callback did not reach rustc analysis".to_owned())
    })
}
