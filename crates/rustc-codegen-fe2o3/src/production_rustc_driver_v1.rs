//! Process-isolated AMD rustc entry for production semantic extraction.

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;

#[derive(Default)]
struct ProductionExtractionCallbacksV1 {
    result: Option<Result<(), String>>,
}

impl Callbacks for ProductionExtractionCallbacksV1 {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(extract_in_active_session_v1(tcx));
        Compilation::Stop
    }
}

fn extract_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    let partitions = tcx.collect_and_partition_mono_items(());
    let kernel_count = crate::collector::count_kernels_in_cgus(tcx, partitions.codegen_units);
    if kernel_count == 0 {
        return Err(
            "production extraction found no registered kernel in the active AMD rustc session"
                .to_owned(),
        );
    }
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        crate::AmdGpuTarget::new(dialect_mir::GFX942_TARGET_CPU),
    )
    .map_err(|error| format!("production extraction collection failed: {error}"))?;
    Err(crate::collector::require_production_semantic_import_v1(tcx, closure).to_string())
}

/// Runs one already-targeted rustc invocation in this process.
///
/// The caller must provide the complete rustc argument vector, including argv0.
/// No host compiler values cross this boundary. The callback discovers roots,
/// collects, and imports synchronously inside the AMD `TyCtxt` it receives.
pub fn run_production_extraction_driver_v1(args: &[String]) -> Result<(), String> {
    let mut callbacks = ProductionExtractionCallbacksV1::default();
    rustc_driver::run_compiler(args, &mut callbacks);
    callbacks.result.unwrap_or_else(|| {
        Err("production extraction callback did not reach rustc analysis".to_owned())
    })
}
