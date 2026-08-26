//! Process-isolated AMD rustc entry for production semantic extraction.

use std::env;
use std::path::{Path, PathBuf};

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;

#[derive(Default)]
struct ProductionExtractionCallbacksV1 {
    ranked_memory: bool,
    gfx942_llvm_output: Option<PathBuf>,
    result: Option<Result<(), String>>,
}

impl Callbacks for ProductionExtractionCallbacksV1 {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(if let Some(output) = self.gfx942_llvm_output.as_deref() {
            extract_gfx942_llvm_in_active_session_v1(tcx, output)
        } else if self.ranked_memory {
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
    crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
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
    crate::production_pipeline::reject_custom_llvm_configuration(
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
    crate::production_pipeline::ProductionCompilation::from_collected_device_closure_for_extraction(
        tcx, closure, producer, output_dir,
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
        .verify_general_kernel_checks()
        .map_err(|error| error.to_string())?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> safety-verified lowering input for `{}`; {} semantic function(s), {} callable record(s), {} retained identity/transaction binding(s), artifact/launch authority {}, all mandatory kernel checks clean {}, bounds clean {}\n{}",
        ranked.function_name(),
        ranked.semantic_function_count(),
        ranked.semantic_callable_count(),
        ranked.retained_identity_and_transaction_binding_count(),
        ranked.grants_artifact_or_launch_authority(),
        ranked.all_kernel_checks_are_clean(),
        ranked.bounds_are_clean(),
        ranked.ranked_ir(),
    );
    Ok(())
}

fn extract_gfx942_llvm_in_active_session_v1(tcx: TyCtxt<'_>, output: &Path) -> Result<(), String> {
    let lowered = transaction_in_active_session_v1(tcx)?
        .lower_gfx942()
        .map_err(|error| error.to_string())?;
    std::fs::write(output, lowered.llvm_ir()).map_err(|error| {
        format!(
            "failed to write production gfx942 LLVM extraction `{}`: {error}",
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR -> composed formal/ranked memory -> gfx942 LLVM; {} semantic function(s), {} correspondence block(s), {} formal access(es), {} ranked dynamic-index discharge(s), workgroup {:?}, {} LLVM byte(s), artifact/launch authority {}",
        lowered.semantic_function_count(),
        lowered.correspondence_block_count(),
        lowered.formal_access_count(),
        lowered.ranked_dynamic_index_discharge_count(),
        lowered.workgroup_size(),
        lowered.llvm_ir().len(),
        lowered.grants_artifact_or_launch_authority(),
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
        gfx942_llvm_output: None,
        result: None,
    };
    rustc_driver::run_compiler(args, &mut callbacks);
    callbacks.result.unwrap_or_else(|| {
        Err("production ranked extraction callback did not reach rustc analysis".to_owned())
    })
}

/// Runs the complete production analysis and lowering transaction, emitting
/// only deterministic gfx942 LLVM text to the explicitly selected path.
pub fn run_production_gfx942_llvm_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let mut callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        gfx942_llvm_output: Some(output.to_path_buf()),
        result: None,
    };
    rustc_driver::run_compiler(args, &mut callbacks);
    callbacks.result.unwrap_or_else(|| {
        Err("production gfx942 extraction callback did not reach rustc analysis".to_owned())
    })
}
