//! Separate opt-in driver. Existing V17 export and inspector output are unchanged.
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;
use std::path::Path;

/// Observe one normal source-admitted ordered program and publish an inert,
/// separately versioned origin report alongside its exact raw V17 bytes.
/// This does not emit a source-authenticated bundle or run LLVM/GPU compilation.
/// Both paths must be fresh. Publication is create-new, not a two-file atomic
/// transaction: an earlier output is retained if the later write fails.
pub fn run_diagnostic_ordered_program_origin_driver_v1(
    args: &[String],
    kir_output: &Path,
    origin_output: &Path,
) -> Result<(), String> {
    if args.is_empty()
        || args.len() > 4096
        || args
            .iter()
            .try_fold(0_usize, |size, arg| size.checked_add(arg.len()))
            .is_none_or(|size| size > 1024 * 1024)
    {
        return Err("origin rustc argument bound exceeded".to_owned());
    }
    if kir_output == origin_output
        || kir_output.as_os_str().is_empty()
        || origin_output.as_os_str().is_empty()
    {
        return Err("origin and raw KIR outputs must be distinct nonempty paths".to_owned());
    }
    super::require_canonical_overflow_checks_v1(args)?;
    let mut callbacks = OriginCallbacks {
        kir_output,
        origin_output,
        result: None,
    };
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks)).err();
    let result = callbacks
        .result
        .unwrap_or_else(|| Err("ordered origin callback did not reach rustc analysis".to_owned()));
    if let Some(fatal) = fatal {
        fatal.raise();
    }
    result
}

struct OriginCallbacks<'a> {
    kir_output: &'a Path,
    origin_output: &'a Path,
    result: Option<Result<(), String>>,
}
impl Callbacks for OriginCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(if self.result.is_some() {
            Err("ordered origin callback reached analysis more than once".to_owned())
        } else {
            extract(tcx, self.kir_output, self.origin_output)
        });
        Compilation::Stop
    }
}
fn extract(tcx: TyCtxt<'_>, kir_output: &Path, origin_output: &Path) -> Result<(), String> {
    let (owner, report) = super::transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .observe_ordered_program_origin_v1()?;
    // Finish all observations/serialization before publishing either file.
    let report_bytes = report.bytes()?;
    let raw = owner
        .materialized()
        .executable()
        .canonical()
        .canonical_bytes();
    super::publish_new_inert_output(
        kir_output,
        raw,
        fe2o3_kernel_ir::MAX_MODULE_BYTES_V1,
        "diagnostic canonical KIR V17",
    )?;
    super::publish_new_inert_output(
        origin_output,
        &report_bytes,
        crate::production_ordered_origin_report_v1::MAX_ORIGIN_REPORT_BYTES_V1,
        "diagnostic ordered-program origin V1",
    )?;
    super::ordered_program_diagnostic_export_v17::report_source_identities_v17(&owner);
    Ok(())
}
