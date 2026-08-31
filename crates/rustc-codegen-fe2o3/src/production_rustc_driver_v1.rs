//! Process-isolated AMD rustc entry for production semantic extraction.

use std::env;
use std::fs::OpenOptions;
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;

#[derive(Default)]
struct ProductionExtractionCallbacksV1 {
    ranked_memory: bool,
    amdgpu_llvm_output: Option<PathBuf>,
    expected_llvm_target: Option<&'static str>,
    gfx942_compiler_handoff_output: Option<PathBuf>,
    simulation_bundle_output: Option<PathBuf>,
    simulation_bundle_v2: bool,
    result: Option<Result<(), String>>,
}

impl Callbacks for ProductionExtractionCallbacksV1 {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(
            if let Some(output) = self.simulation_bundle_output.as_deref() {
                if self.simulation_bundle_v2 {
                    extract_simulation_bundle_in_active_session_v2(tcx, output)
                } else {
                    extract_simulation_bundle_in_active_session_v1(tcx, output)
                }
            } else if let Some(output) = self.gfx942_compiler_handoff_output.as_deref() {
                extract_gfx942_compiler_handoff_in_active_session_v1(tcx, output)
            } else if let Some(output) = self.amdgpu_llvm_output.as_deref() {
                extract_amdgpu_llvm_in_active_session_v1(tcx, output, self.expected_llvm_target)
            } else if self.ranked_memory {
                extract_ranked_memory_in_active_session_v1(tcx)
            } else {
                extract_in_active_session_v1(tcx)
            },
        );
        Compilation::Stop
    }
}

fn transaction_in_active_session_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    debug_source_capture: crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2,
) -> Result<
    crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
    >,
    String,
> {
    let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
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
    match debug_source_capture {
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled => {
            crate::production_pipeline::ProductionCompilation::from_collected_device_closure_for_extraction(
                tcx, closure, producer, output_dir,
            )
        }
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables => {
            crate::production_pipeline::ProductionCompilation::from_collected_device_closure_for_simulation_v2(
                tcx, closure, producer, output_dir,
            )
        }
    }
    .map_err(|error| format!("production extraction transaction construction failed: {error}"))
}

fn extract_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    Err(transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .require_semantic_mir_import()
    .to_string())
}

fn extract_ranked_memory_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    let ranked = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .verify_general_kernel_checks()
    .map_err(|error| error.to_string())?;
    if let [root] = ranked.ranked_roots() {
        eprintln!(
            "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> safety-verified lowering input for `{}`; {} semantic function(s), {} callable record(s), {} retained identity/transaction binding(s), artifact/launch authority {}, all mandatory kernel checks clean {}, bounds clean {}\n{}",
            root.function_name(),
            ranked.semantic_function_count(),
            ranked.semantic_callable_count(),
            ranked.retained_identity_and_transaction_binding_count(),
            ranked.grants_artifact_or_launch_authority(),
            ranked.all_kernel_checks_are_clean(),
            ranked.bounds_are_clean(),
            root.ranked_ir(),
        );
    } else {
        eprintln!(
            "fe2o3 production extraction: Rust -> semantic MIR -> ordered ranked PLIRON roster; {} kernel root(s), {} semantic function(s), {} callable record(s), {} retained identity/transaction binding(s), artifact/launch authority {}, all mandatory kernel checks clean {}, bounds clean {}",
            ranked.ranked_root_count(),
            ranked.semantic_function_count(),
            ranked.semantic_callable_count(),
            ranked.retained_identity_and_transaction_binding_count(),
            ranked.grants_artifact_or_launch_authority(),
            ranked.all_kernel_checks_are_clean(),
            ranked.bounds_are_clean(),
        );
        for (ordinal, root) in ranked.ranked_roots().iter().enumerate() {
            eprintln!(
                "ranked root {ordinal}: `{}`, kernel binding {}, source rank {}, all mandatory kernel checks clean {}, bounds clean {}\n{}",
                root.function_name(),
                lower_hex_v1(root.kernel_binding()),
                root.source_rank(),
                root.all_kernel_checks_are_clean(),
                root.bounds_are_clean(),
                root.ranked_ir(),
            );
        }
    }
    Ok(())
}

fn extract_amdgpu_llvm_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let lowered = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .lower_production_target()
    .map_err(|error| error.to_string())?;
    if let Some(expected_target) = expected_target
        && lowered.target_name() != expected_target
    {
        return Err(format!(
            "production LLVM extraction expected live target {expected_target:?}; found {:?}",
            lowered.target_name()
        ));
    }
    std::fs::write(output, lowered.llvm_ir()).map_err(|error| {
        format!(
            "failed to write production {} LLVM extraction `{}`: {error}",
            lowered.target_name(),
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> {} LLVM; {} semantic function(s), {} semantic u32 induction certificate(s) for {} checked addition(s), {} correspondence block(s), {} formal access(es), {} ranked dynamic-index discharge(s), ordered workgroups {:?}, {} LLVM byte(s), artifact/launch authority {}",
        lowered.canonical_kernel_ir_version(),
        lowered.guarded_store_count(),
        lowered.target_name(),
        lowered.semantic_function_count(),
        lowered.semantic_u32_induction_certificate_count(),
        lowered.semantic_u32_induction_checked_addition_count(),
        lowered.correspondence_block_count(),
        lowered.formal_access_count(),
        lowered.ranked_dynamic_index_discharge_count(),
        lowered.workgroup_sizes(),
        lowered.llvm_ir().len(),
        lowered.grants_artifact_or_launch_authority(),
    );
    Ok(())
}

fn extract_gfx942_compiler_handoff_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let lowered = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .lower_production_target()
    .map_err(|error| error.to_string())?;
    if lowered.target_name() != fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1 {
        return Err(format!(
            "production gfx942 compiler handoff expected live target {:?}; found {:?}",
            fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1,
            lowered.target_name()
        ));
    }
    let canonical_kernel_ir_version = lowered.canonical_kernel_ir_version();
    let guarded_store_count = lowered.guarded_store_count();
    let handoff = lowered
        .into_inert_worker_handoff_for_extraction()
        .map_err(|error| error.to_string())?;
    std::fs::write(output, handoff.canonical_bytes()).map_err(|error| {
        format!(
            "failed to write inert production compiler-module handoff extraction `{}`: {error}",
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> gfx942 LLVM -> compiler-bound inert handoff; {} handoff byte(s), artifact/launch authority false",
        canonical_kernel_ir_version,
        guarded_store_count,
        handoff.canonical_bytes().len(),
    );
    Ok(())
}

fn extract_simulation_bundle_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .export_simulation_bundle_v1()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle_v1(output, bundle.canonical_bytes())?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> semantic MIR -> ranked PLIRON checks -> sole target-neutral Kernel IR lowering -> exact verified KIR V7 simulation bundle; target {}, {} kernel(s), simulation_bundle_subject {}, content {}, {} byte(s), compiler_execution_binding=extraction_only_unavailable, authenticates_compiler_execution=false, debug map {}, proof/artifact/compiler/hardware/load/launch authority false",
        bundle.target(),
        bundle.kernel_count(),
        lower_hex_v1(bundle.subject_identity()),
        lower_hex_v1(bundle.identity().as_bytes()),
        bundle.canonical_bytes().len(),
        if bundle.debug_map().is_some() {
            "present"
        } else {
            "none"
        },
    );
    Ok(())
}

fn extract_simulation_bundle_in_active_session_v2(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
    )?
    .export_simulation_bundle_v2()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle_v2(output, bundle.canonical_bytes())?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> semantic MIR -> ranked PLIRON checks -> sole target-neutral Kernel IR lowering -> explicit simulation bundle V2 with compiler-produced source variables; target {}, {} kernel(s), simulation_bundle_subject {}, content {}, {} byte(s), compiler_execution_binding=extraction_only_unavailable, authenticates_compiler_execution=false, debug map V2 present, proof/artifact/compiler/hardware/load/launch authority false",
        bundle.target(),
        bundle.kernel_count(),
        lower_hex_v1(bundle.subject_identity()),
        lower_hex_v1(bundle.identity().as_bytes()),
        bundle.canonical_bytes().len(),
    );
    Ok(())
}

fn publish_new_simulation_bundle_v1(output: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_new_simulation_bundle(
        output,
        bytes,
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V1,
    )
}

fn publish_new_simulation_bundle_v2(output: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_new_simulation_bundle(
        output,
        bytes,
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V2,
    )
}

fn publish_new_simulation_bundle(
    output: &Path,
    bytes: &[u8],
    maximum: usize,
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err("refusing to publish an empty or oversized simulation bundle".to_owned());
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(output).map_err(|error| {
        format!(
            "failed to create new simulation bundle output `{}`: {error}",
            output.display()
        )
    })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        return Err(format!(
            "failed to publish simulation bundle `{}`; the create-new partial output was retained for fail-closed cleanup: {error}",
            output.display()
        ));
    }
    #[cfg(unix)]
    {
        let descriptor = file.metadata().map_err(|error| {
            format!(
                "failed to inspect published simulation bundle descriptor `{}`: {error}",
                output.display()
            )
        })?;
        let path = std::fs::symlink_metadata(output).map_err(|error| {
            format!(
                "failed to re-inspect published simulation bundle path `{}`: {error}",
                output.display()
            )
        })?;
        if !descriptor.is_file()
            || descriptor.len() != bytes.len() as u64
            || descriptor.dev() != path.dev()
            || descriptor.ino() != path.ino()
            || path.file_type().is_symlink()
        {
            return Err(format!(
                "simulation bundle output `{}` changed identity during publication",
                output.display()
            ));
        }
    }
    Ok(())
}

fn lower_hex_v1(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

/// Runs one already-targeted rustc invocation in this process.
///
/// The caller must provide the complete rustc argument vector, including argv0.
/// No host compiler values cross this boundary. The callback discovers roots,
/// collects, and imports synchronously inside the AMD `TyCtxt` it receives.
pub fn run_production_extraction_driver_v1(args: &[String]) -> Result<(), String> {
    run_production_driver_v1(
        args,
        ProductionExtractionCallbacksV1::default(),
        "production extraction callback did not reach rustc analysis",
    )
}

/// Runs the same production importer followed by generic ranked-memory
/// construction and verification, without granting artifact authority.
pub fn run_production_ranked_extraction_driver_v1(args: &[String]) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: true,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        gfx942_compiler_handoff_output: None,
        simulation_bundle_output: None,
        simulation_bundle_v2: false,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production ranked extraction callback did not reach rustc analysis",
    )
}

/// Runs the complete production analysis and exact live-target lowering
/// transaction, emitting deterministic AMDGPU LLVM text to the selected path.
pub fn run_production_amdgpu_llvm_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        amdgpu_llvm_output: Some(output.to_path_buf()),
        expected_llvm_target: None,
        gfx942_compiler_handoff_output: None,
        simulation_bundle_output: None,
        simulation_bundle_v2: false,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production AMDGPU extraction callback did not reach rustc analysis",
    )
}

/// Compatibility entry point for the original exact gfx942 extraction API.
pub fn run_production_gfx942_llvm_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        amdgpu_llvm_output: Some(output.to_path_buf()),
        expected_llvm_target: Some(fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1),
        gfx942_compiler_handoff_output: None,
        simulation_bundle_output: None,
        simulation_bundle_v2: false,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production gfx942 extraction callback did not reach rustc analysis",
    )
}

/// Runs the complete production analysis and lowering transaction and emits
/// its compiler-bound nested handoff for inert worker integration testing.
/// The result carries no publication, artifact, load, or launch authority.
pub fn run_production_gfx942_compiler_handoff_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        gfx942_compiler_handoff_output: Some(output.to_path_buf()),
        simulation_bundle_output: None,
        simulation_bundle_v2: false,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production compiler-handoff extraction callback did not reach rustc analysis",
    )
}

/// Runs the sole production source transaction through target-neutral lowering
/// and publishes one authority-free exact KIR V7 simulation bundle. This path
/// never enters LLVM, artifact publication, a runtime, or hardware fallback.
pub fn run_production_simulation_bundle_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        gfx942_compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_v2: false,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle extraction callback did not reach rustc analysis",
    )
}

/// Runs the opt-in V2 simulation export with compiler-produced source-variable
/// records. V1 remains the default and byte-compatible extraction route.
pub fn run_production_simulation_bundle_extraction_driver_v2(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        gfx942_compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_v2: true,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle V2 extraction callback did not reach rustc analysis",
    )
}

fn run_production_driver_v1(
    args: &[String],
    mut callbacks: ProductionExtractionCallbacksV1,
    missing_callback: &'static str,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    rustc_driver::run_compiler(args, &mut callbacks);
    callbacks
        .result
        .unwrap_or_else(|| Err(missing_callback.to_owned()))
}

fn require_canonical_overflow_checks_v1(args: &[String]) -> Result<(), String> {
    let mut observed = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--" {
            break;
        }
        if argument == "-C" || argument == "--codegen" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("production rustc option `{argument}` has no value"))?;
            if value.starts_with("overflow-checks=") {
                observed.push((argument.as_str(), value.as_str()));
            }
            index += 2;
            continue;
        }
        if argument.starts_with("-Coverflow-checks=")
            || argument.starts_with("--codegen=overflow-checks=")
        {
            observed.push((argument.as_str(), ""));
        }
        index += 1;
    }
    if observed == [("-Coverflow-checks=on", "")] {
        Ok(())
    } else {
        Err(format!(
            "production rustc invocation requires exactly one canonical `-Coverflow-checks=on`; observed {observed:?}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_driver_requires_one_canonical_overflow_policy() {
        require_canonical_overflow_checks_v1(&[
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "kernel".to_owned(),
            "-Coverflow-checks=on".to_owned(),
        ])
        .unwrap();

        for rejected in [
            vec!["rustc"],
            vec!["rustc", "-Coverflow-checks=off"],
            vec!["rustc", "-C", "overflow-checks=on"],
            vec![
                "rustc",
                "-Coverflow-checks=on",
                "--codegen=overflow-checks=on",
            ],
            vec!["rustc", "--", "-Coverflow-checks=on"],
        ] {
            let args = rejected.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert!(
                require_canonical_overflow_checks_v1(&args)
                    .unwrap_err()
                    .contains("requires exactly one canonical")
            );
        }
    }

    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-simulation-bundle-output-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn simulation_bundle_output_is_create_new_exact_and_private() {
        let root = scratch();
        let output = root.join("kernel.fe2sim");
        publish_new_simulation_bundle_v1(&output, b"exact-bundle").unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"exact-bundle");
        assert!(publish_new_simulation_bundle_v1(&output, b"replacement").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"exact-bundle");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, symlink};
            assert_eq!(
                std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let link = root.join("link.fe2sim");
            symlink(&output, &link).unwrap();
            assert!(publish_new_simulation_bundle_v1(&link, b"replacement").is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simulation_bundle_output_rejects_empty_and_oversized_payloads() {
        let root = scratch();
        assert!(publish_new_simulation_bundle_v1(&root.join("empty"), b"").is_err());
        assert!(
            publish_new_simulation_bundle_v1(
                &root.join("oversized"),
                &vec![0; fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V1 + 1],
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
