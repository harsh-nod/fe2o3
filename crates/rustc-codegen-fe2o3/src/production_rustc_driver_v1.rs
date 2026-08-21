//! Process-isolated AMD rustc entry for production semantic extraction.

use std::env;
use std::path::PathBuf;

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;

#[derive(Default)]
struct ProductionExtractionCallbacksV1 {
    ranked_memory: bool,
    result: Option<Result<(), String>>,
}

impl Callbacks for ProductionExtractionCallbacksV1 {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(if self.ranked_memory {
            extract_ranked_memory_in_active_session_v1(tcx)
        } else {
            extract_in_active_session_v1(tcx)
        });
        Compilation::Stop
    }
}

fn transaction_in_active_session_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<
    crate::production_pipeline_v1::ProductionCompilationV1<
        'tcx,
        crate::production_pipeline_v1::CollectedRustStageV1<'tcx>,
    >,
    String,
> {
    let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_before_collection(
        tcx,
        &crate::AmdGpuTarget::new(dialect_mir::GFX942_TARGET_CPU),
    )
    .map_err(|error| {
        format!("production extraction target authentication failed before monomorphization: {error}")
    })?;
    let partitions = tcx.collect_and_partition_mono_items(());
    let kernel_count = crate::collector::count_kernels_in_cgus(tcx, partitions.codegen_units);
    if kernel_count == 0 {
        return Err(
            "production extraction found no registered kernel in the active AMD rustc session"
                .to_owned(),
        );
    }
    crate::production_pipeline_v1::reject_custom_llvm_configuration(
        crate::has_custom_llvm_configuration(tcx.sess),
    )
    .map_err(|error| format!("production extraction {error}"))?;
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .map_err(|error| format!("production extraction collection failed: {error}"))?;
    let crate_name = tcx.crate_name(LOCAL_CRATE);
    let local_source = tcx
        .sess
        .local_crate_source_file()
        .and_then(|source| source.local_path().map(PathBuf::from));
    let producer = crate::artifact_transaction::ProducerIdentity::from_codegen(
        crate_name.as_str(),
        local_source.as_deref(),
    )
    .map_err(|error| format!("production extraction producer identity failed: {error}"))?;
    let output_dir = env::current_dir()
        .map_err(|error| format!("production extraction working directory failed: {error}"))?;
    crate::production_pipeline_v1::ProductionCompilationV1::from_collected_device_closure(
        tcx, closure, producer, output_dir, None,
    )
    .map_err(|error| format!("production extraction transaction construction failed: {error}"))
}

fn extract_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    Err(transaction_in_active_session_v1(tcx)?
        .require_semantic_mir_import()
        .to_string())
}

fn extract_ranked_memory_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    let ranked = transaction_in_active_session_v1(tcx)?
        .verify_ranked_memory()
        .map_err(|error| error.to_string())?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> bounds-verified lowering input for `{}`; {} semantic function(s), {} callable record(s), {} retained identity/transaction binding(s), artifact/launch authority {}, bounds clean {}\n{}",
        ranked.function_name(),
        ranked.semantic_function_count(),
        ranked.semantic_callable_count(),
        ranked.retained_identity_and_transaction_binding_count(),
        ranked.grants_artifact_or_launch_authority(),
        ranked.bounds_are_clean(),
        ranked.ranked_ir(),
    );
    Ok(())
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

/// Runs the same production importer followed by generic ranked-memory
/// construction and verification, without granting artifact authority.
pub fn run_production_ranked_extraction_driver_v1(args: &[String]) -> Result<(), String> {
    let mut callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: true,
        result: None,
    };
    rustc_driver::run_compiler(args, &mut callbacks);
    callbacks.result.unwrap_or_else(|| {
        Err("production ranked extraction callback did not reach rustc analysis".to_owned())
    })
}
