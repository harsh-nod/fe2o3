#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_codegen_llvm;
extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
#[cfg(test)]
extern crate rustc_interface;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;

mod amdgpu_llvm;
#[allow(dead_code)]
mod closure_profile_v1;
mod collected_executable_scalar_control_flow_v2;
mod collected_scalar_gemm_v1;
mod collector;
mod compiler_descriptor;
mod compiler_ffi_adapter;
mod device_ffi;
pub mod executable_scalar_control_flow_v1;
mod frontend_record_bridge;
mod host_object;
mod kernel_ir_codegen;
mod kernel_ir_lowering;
mod mir_import;
#[allow(dead_code)]
mod mir_import_v2;
mod monomorphization_dead;
mod record_lowering;
#[allow(dead_code)]
mod rust_type_layout;
#[allow(dead_code)]
mod rust_type_layout_general;
mod rust_type_layout_v3;
pub mod s09_identity_v2;
pub mod scalar_mir_v2;
mod semantic_features;
pub mod semantic_layout_bridge;
pub mod semantic_type_adapter_v2;
mod semantic_witness;
mod source_debug;
mod static_registration;
#[cfg(test)]
mod test_temp_dir;
mod trusted_device_items;
mod typed_artifact;
mod worker_v2_producer;

use fe2o3_artifact_transaction as artifact_transaction;
use rustc_codegen_ssa::traits::CodegenBackend;
use rustc_codegen_ssa::{CompiledModules, CrateInfo};
use rustc_data_structures::fx::FxIndexMap;
use rustc_metadata::EncodedMetadata;
use rustc_middle::dep_graph::{WorkProduct, WorkProductId};
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_session::Session;
use rustc_session::config::OutputFilenames;
use std::any::Any;
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_FINALIZED_LLVM_IR_BYTES: usize = 16 * 1024 * 1024;
const MAX_FINALIZED_HSACO_BYTES: usize = 4 * 1024 * 1024;
const TEMP_DIRECTORY_ATTEMPTS: usize = 64;
static NEXT_HOST_OBJECT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub const TARGET_ENV: &str = "FE2O3_TARGET";
pub const BACKEND_ENV: &str = "FE2O3_BACKEND";
pub const VERBOSE_ENV: &str = "FE2O3_VERBOSE";
pub const DUMP_MIR_ENV: &str = "FE2O3_DUMP_MIR";
pub const DUMP_LLVM_ENV: &str = "FE2O3_DUMP_LLVM";
pub const VERIFY_KERNEL_IR_ENV: &str = "FE2O3_VERIFY_KERNEL_IR";
pub const CODEGEN_PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";
pub const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
pub const BUILD_ATTEMPT_ENV: &str = "FE2O3_BUILD_ATTEMPT_V1";
pub const TILED_GEMM_FRONTEND_TEST_LLVM_DIR_ENV: &str =
    "FE2O3_TEST_RETAIN_TILED_GEMM_FRONTEND_LLVM_DIR";

pub struct Fe2o3CodegenBackend {
    config: BackendConfig,
    llvm_backend: Box<dyn CodegenBackend>,
    pending_host_objects: Mutex<Vec<TemporaryHostObjects>>,
}

struct OngoingFe2o3Codegen {
    llvm_codegen: Box<dyn Any>,
    host_objects: host_object::GeneratedHostObjects,
    temporary_host_objects: TemporaryHostObjects,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedKernelRootV1 {
    logical_name: String,
    export_name: String,
    profile: collector::TypedKernelProfile,
    kernel_binding: reserved_fe2o3_symbols::KernelBindingIdV1,
    type_identities: collector::TypedArgumentListV1<fe2o3_artifacts::TypeIdentity>,
}

#[derive(Debug, Default)]
struct TemporaryHostObjects {
    entries: Vec<TemporaryHostObject>,
}

#[derive(Debug)]
struct TemporaryHostObject {
    directory: PathBuf,
    object: PathBuf,
}

impl TemporaryHostObjects {
    fn reserve(&mut self, parent: &Path, artifact_id: &str) -> Result<PathBuf, TypedVerticalError> {
        let artifact_prefix = artifact_id
            .get(..16)
            .ok_or_else(|| TypedVerticalError::InvalidArtifactId(artifact_id.to_owned()))?;
        for _ in 0..TEMP_DIRECTORY_ATTEMPTS {
            let sequence = NEXT_HOST_OBJECT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let directory = parent.join(format!(
                ".fe2o3-host-object-{}-{sequence}-{}",
                std::process::id(),
                artifact_prefix
            ));
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(&directory) {
                Ok(()) => {
                    let object = directory.join("artifact.o");
                    self.entries.push(TemporaryHostObject {
                        directory,
                        object: object.clone(),
                    });
                    return Ok(object);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(TypedVerticalError::TemporaryDirectory {
                        path: directory,
                        source,
                    });
                }
            }
        }
        Err(TypedVerticalError::TemporaryDirectoryExhausted(
            parent.to_path_buf(),
        ))
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Drop for TemporaryHostObjects {
    fn drop(&mut self) {
        for entry in self.entries.iter().rev() {
            let _ = fs::remove_file(&entry.object);
            let _ = fs::remove_dir(&entry.directory);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodegenPipeline {
    LegacyV1,
    KernelIrV1,
    KernelIrWorkerV2,
    CollectedExecutableScalarControlFlowV2,
    CollectedScalarGemmV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PipelineSelection {
    Valid(CodegenPipeline),
    Invalid(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum BuildAttemptSelection {
    #[default]
    Direct,
    Managed(artifact_transaction::BuildAttempt),
    Invalid(String),
}

impl BuildAttemptSelection {
    fn from_env() -> Self {
        match env::var(BUILD_ATTEMPT_ENV) {
            Err(env::VarError::NotPresent) => Self::Direct,
            Err(env::VarError::NotUnicode(_)) => {
                Self::Invalid(format!("{BUILD_ATTEMPT_ENV} is not valid UTF-8"))
            }
            Ok(value) => match artifact_transaction::BuildAttempt::from_env_value(&value) {
                Ok(attempt) => Self::Managed(attempt),
                Err(error) => Self::Invalid(format!(
                    "{BUILD_ATTEMPT_ENV} is not a canonical build attempt: {error}"
                )),
            },
        }
    }

    fn resolve(&self) -> Result<Option<artifact_transaction::BuildAttempt>, &str> {
        match self {
            Self::Direct => Ok(None),
            Self::Managed(attempt) => Ok(Some(*attempt)),
            Self::Invalid(reason) => Err(reason),
        }
    }
}

impl PipelineSelection {
    fn from_env() -> Self {
        Self::from_value(env::var_os(CODEGEN_PIPELINE_ENV).as_deref())
    }

    fn from_value(value: Option<&OsStr>) -> Self {
        match value {
            None => Self::Valid(CodegenPipeline::LegacyV1),
            Some(value) if value == "legacy-v1" => Self::Valid(CodegenPipeline::LegacyV1),
            Some(value) if value == "kernel-ir-v1" => Self::Valid(CodegenPipeline::KernelIrV1),
            Some(value) if value == "kernel-ir-worker-v2" => {
                Self::Valid(CodegenPipeline::KernelIrWorkerV2)
            }
            Some(value)
                if value
                    == collected_executable_scalar_control_flow_v2::COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V2 =>
            {
                Self::Valid(CodegenPipeline::CollectedExecutableScalarControlFlowV2)
            }
            Some(value)
                if value == collected_scalar_gemm_v1::COLLECTED_SCALAR_GEMM_PIPELINE_V1 =>
            {
                Self::Valid(CodegenPipeline::CollectedScalarGemmV1)
            }
            Some(value) => Self::Invalid(format!(
                "{CODEGEN_PIPELINE_ENV} must be unset or exactly `legacy-v1`, `kernel-ir-v1`, `kernel-ir-worker-v2`, `{}`, or `{}`; found {value:?}",
                collected_executable_scalar_control_flow_v2::COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V2,
                collected_scalar_gemm_v1::COLLECTED_SCALAR_GEMM_PIPELINE_V1,
            )),
        }
    }

    fn resolve(&self) -> Result<CodegenPipeline, amdgpu_llvm::EmitError> {
        match self {
            Self::Valid(pipeline) => Ok(*pipeline),
            Self::Invalid(reason) => Err(amdgpu_llvm::EmitError::Preflight {
                reason: reason.clone(),
            }),
        }
    }
}

impl Default for PipelineSelection {
    fn default() -> Self {
        Self::Valid(CodegenPipeline::LegacyV1)
    }
}

#[derive(Clone, Debug, Default)]
pub struct BackendConfig {
    pub verbose: bool,
    pub dump_mir: bool,
    pub dump_llvm: bool,
    pub verify_kernel_ir: bool,
    codegen_pipeline: PipelineSelection,
    build_attempt: BuildAttemptSelection,
    pub hsaco_output_dir: Option<PathBuf>,
    pub target: AmdGpuTarget,
}

impl BackendConfig {
    pub fn from_env() -> Self {
        Self {
            verbose: env_flag(VERBOSE_ENV),
            dump_mir: env_flag(DUMP_MIR_ENV),
            dump_llvm: env_flag(DUMP_LLVM_ENV),
            verify_kernel_ir: env_flag(VERIFY_KERNEL_IR_ENV),
            codegen_pipeline: PipelineSelection::from_env(),
            build_attempt: BuildAttemptSelection::from_env(),
            hsaco_output_dir: env::var(HSACO_DIR_ENV).ok().map(PathBuf::from),
            target: AmdGpuTarget::from_env_or_default(),
        }
    }
}

fn dump_authenticated_frontend_contracts(
    frontend: &frontend_record_bridge::CompilerFrontendRecordV1,
) {
    let contracts = frontend.kernel_contracts();
    if contracts.is_empty() {
        return;
    }
    let bytes = contracts
        .iter()
        .map(|record| record.canonical_bytes().len())
        .sum::<usize>();
    let assembly_blocks = contracts
        .iter()
        .map(|record| record.reachable_assembly().blocks() as usize)
        .sum::<usize>();
    let effectful = contracts
        .iter()
        .filter(|record| {
            record
                .contract()
                .unsafe_assembly()
                .is_some_and(|assembly| assembly.effect_bits() != 0)
        })
        .count();
    let operand_options = contracts.iter().fold((0_u16, 0_u16), |combined, record| {
        let assembly = record.reachable_assembly();
        (
            combined.0 | assembly.operand_bits(),
            combined.1 | assembly.option_bits(),
        )
    });
    eprintln!(
        "[rustc-codegen-fe2o3] authenticated {} kernel frontend contract(s), {bytes} canonical byte(s), {assembly_blocks} reachable asm block(s), {effectful} effectful declaration(s), operand/options union {:#x}/{:#x}",
        contracts.len(),
        operand_options.0,
        operand_options.1
    );
}

impl CodegenBackend for Fe2o3CodegenBackend {
    fn name(&self) -> &'static str {
        "fe2o3"
    }

    fn init(&self, sess: &Session) {
        self.llvm_backend.init(sess);
    }

    fn print_version(&self) {
        println!(
            "rustc-codegen-fe2o3 {} (wrapping rustc_codegen_llvm)",
            env!("CARGO_PKG_VERSION")
        );
        self.llvm_backend.print_version();
    }

    fn target_cpu(&self, sess: &Session) -> String {
        self.llvm_backend.target_cpu(sess)
    }

    fn target_config(&self, sess: &Session) -> rustc_codegen_ssa::TargetConfig {
        self.llvm_backend.target_config(sess)
    }

    fn provide(&self, providers: &mut rustc_middle::util::Providers) {
        self.llvm_backend.provide(providers);
    }

    fn codegen_crate(&self, tcx: TyCtxt<'_>, crate_info: &CrateInfo) -> Box<dyn Any> {
        with_no_trimmed_paths!({
            let mono_partitions = tcx.collect_and_partition_mono_items(());
            let kernel_count = collector::count_kernels_in_cgus(tcx, mono_partitions.codegen_units);
            let crate_name = tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
            let output_dir = match managed_artifact_output(&self.config, kernel_count) {
                Ok(output_dir) => output_dir,
                Err(()) => tcx.dcx().fatal(format!(
                    "[rustc-codegen-fe2o3] {HSACO_DIR_ENV} must name a managed artifact directory when compiling kernels"
                )),
            };
            let build_attempt = match self.config.build_attempt.resolve() {
                Ok(attempt) => attempt,
                Err(reason) => tcx.dcx().fatal(format!(
                    "[rustc-codegen-fe2o3] invalid managed build attempt: {reason}"
                )),
            };
            let local_source = tcx
                .sess
                .local_crate_source_file()
                .and_then(|source| source.local_path().map(Path::to_path_buf));
            let producer = match artifact_transaction::ProducerIdentity::from_codegen(
                crate_name.as_str(),
                local_source.as_deref(),
            ) {
                Ok(producer) => producer,
                Err(error) => tcx.dcx().fatal(format!(
                    "[rustc-codegen-fe2o3] invalid local artifact producer: {error}"
                )),
            };

            if self.config.verbose || kernel_count > 0 {
                eprintln!(
                    "[rustc-codegen-fe2o3] crate `{crate_name}`: {} CGU(s), {kernel_count} kernel candidate(s), target {}",
                    mono_partitions.codegen_units.len(),
                    self.config.target,
                );
            }

            let mut generated_host_objects = host_object::GeneratedHostObjects::default();
            let mut temporary_host_objects = TemporaryHostObjects::default();
            if kernel_count > 0 {
                let output_dir = output_dir.expect("kernel output was required above");
                let codegen_pipeline = self.config.codegen_pipeline.clone();
                if matches!(
                    codegen_pipeline,
                    PipelineSelection::Valid(
                        CodegenPipeline::CollectedExecutableScalarControlFlowV2
                    )
                ) {
                    let lowering = (|| -> Result<_, String> {
                        let collection = collector::collect_device_functions(
                            tcx,
                            mono_partitions.codegen_units,
                            self.config.verbose,
                        )
                        .map_err(|error| error.to_string())?;
                        let frontend_record =
                            frontend_record_bridge::extract_frontend_record_v1(tcx, &collection)
                                .map_err(|error| {
                                    format!("frontend record extraction failed: {error}")
                                })?;
                        if self.config.verbose {
                            eprintln!(
                                "[rustc-codegen-fe2o3] validated frontend record: {} function(s), {} canonical byte(s)",
                                frontend_record.unit().functions().len(),
                                frontend_record.canonical_bytes().len()
                            );
                        }
                        collector::dump_device_functions(tcx, &collection.functions);
                        let custom_llvm_pipeline = !tcx.sess.opts.cg.llvm_args.is_empty()
                            || !tcx.sess.opts.cg.passes.is_empty();
                        collected_executable_scalar_control_flow_v2::authenticate_collected_executable_scalar_control_flow_v2(
                            tcx,
                            &collection,
                            &self.config.target,
                            custom_llvm_pipeline,
                        )
                        .map_err(|error| error.to_string())
                    })();
                    match lowering {
                        Ok(admission) => tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] {} authenticated collected KernelEntry `{}` export `{}` with exact reviewed root MIR {} and exact reachable InternalHelper `{}` MIR {}; path-independent portable MIR semantics {}; compiler semantics {}; sealed collected authority {}; {}; no executable authority, Kernel IR, LLVM, LLD, HSACO, or legacy fallback was entered",
                            collected_executable_scalar_control_flow_v2::COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V2,
                            admission.root_instance_identity(),
                            admission.kernel_export(),
                            admission.root_identity_hex(),
                            admission.helper_instance_identity(),
                            admission.helper_identity_hex(),
                            admission.portable_mir_semantic_hex(),
                            admission.compiler_semantics_hex(),
                            admission.authority_hex(),
                            collected_executable_scalar_control_flow_v2::NEXT_LOWERING_DEPENDENCY,
                        )),
                        Err(error) => tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] {} rejected the collected program without fallback: {error}",
                            collected_executable_scalar_control_flow_v2::COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V2,
                        )),
                    }
                } else if matches!(
                    codegen_pipeline,
                    PipelineSelection::Valid(CodegenPipeline::CollectedScalarGemmV1)
                ) {
                    let preparation = (|| -> Result<_, String> {
                        let attempt = build_attempt.ok_or_else(|| {
                            format!(
                                "{} requires a managed {BUILD_ATTEMPT_ENV}",
                                collected_scalar_gemm_v1::COLLECTED_SCALAR_GEMM_PIPELINE_V1,
                            )
                        })?;
                        let collection = collector::collect_device_functions(
                            tcx,
                            mono_partitions.codegen_units,
                            self.config.verbose,
                        )
                        .map_err(|error| error.to_string())?;
                        let frontend_record =
                            frontend_record_bridge::extract_frontend_record_v1(tcx, &collection)
                                .map_err(|error| {
                                    format!("frontend record extraction failed: {error}")
                                })?;
                        if self.config.verbose {
                            eprintln!(
                                "[rustc-codegen-fe2o3] validated scalar GEMM frontend record: {} function(s), {} canonical byte(s)",
                                frontend_record.unit().functions().len(),
                                frontend_record.canonical_bytes().len()
                            );
                        }
                        collector::dump_device_functions(tcx, &collection.functions);
                        let custom_llvm_pipeline = !tcx.sess.opts.cg.llvm_args.is_empty()
                            || !tcx.sess.opts.cg.passes.is_empty();
                        let mut receipt =
                            collected_scalar_gemm_v1::authenticate_collected_scalar_gemm_v1(
                                tcx,
                                &collection,
                                &self.config.target,
                                custom_llvm_pipeline,
                            )
                            .map_err(|error| error.to_string())?;
                        let root_instance_identity = receipt.root_instance_identity().to_owned();
                        let kernel_export = receipt.kernel_export().to_owned();
                        let portable_mir_semantic = receipt.portable_mir_semantic_hex();
                        let compiler_semantics = receipt.compiler_semantics_hex();
                        let frontend_authority = receipt.authority_hex();
                        let frontend_authority_commitment = *receipt.authority_commitment();
                        let authenticated_module =
                            receipt.consume().map_err(|error| error.to_string())?;
                        let handoff = worker_v2_producer::prepare_scalar_gemm_v1_worker_handoff(
                            authenticated_module,
                        )
                        .map_err(|error| error.to_string())?;
                        if handoff.frontend_authority_commitment() != &frontend_authority_commitment
                        {
                            return Err(
                                "prepared scalar GEMM handoff lost frontend authority binding"
                                    .to_owned(),
                            );
                        }
                        let canonical_handoff_bytes = handoff.handoff().canonical_bytes().len();
                        let llvm_bytes = handoff.handoff().module_bytes().len();
                        let publication =
                            worker_v2_producer::publish_prepared_scalar_gemm_v1_worker_handoff(
                                output_dir, &producer, attempt, handoff,
                            )
                            .map_err(|error| error.to_string())?;
                        Ok((
                            root_instance_identity,
                            kernel_export,
                            portable_mir_semantic,
                            compiler_semantics,
                            frontend_authority,
                            canonical_handoff_bytes,
                            llvm_bytes,
                            publication.length(),
                        ))
                    })();
                    match preparation {
                        Ok((
                            root_instance_identity,
                            kernel_export,
                            portable_mir_semantic,
                            compiler_semantics,
                            frontend_authority,
                            canonical_handoff_bytes,
                            llvm_bytes,
                            publication_bytes,
                        )) => eprintln!(
                            "[rustc-codegen-fe2o3] {} consumed its single-use frontend receipt for exact collected KernelEntry `{}` export `{}` with reviewed path-independent portable MIR {}; exact ABI/roles A:&[f32], B:&[f32], C:DisjointSlice<f32>, m:u32, n:u32, k:u32; explicit kernarg {} bytes, complete kernarg {} bytes; exact 256x1x1 one-dimensional launch; row-major sequential f32 source semantics; target {}; COV{}; compiler semantics {}; sealed frontend authority {}; published exact inert Worker V2 compiler-module handoff ({} canonical bytes, {} LLVM bytes, {} receipt bytes) with compiler descriptor and frontend-authority sections from canonical scalar GEMM Kernel IR; measured Worker execution, raw-HSACO inspection, finalization, durable HSACO publication, load, launch, and COMGR were not entered by the backend",
                            collected_scalar_gemm_v1::COLLECTED_SCALAR_GEMM_PIPELINE_V1,
                            root_instance_identity,
                            kernel_export,
                            portable_mir_semantic,
                            collected_scalar_gemm_v1::SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1,
                            collected_scalar_gemm_v1::SCALAR_GEMM_COMPLETE_KERNARG_BYTES_V1,
                            collected_scalar_gemm_v1::EXACT_SCALAR_GEMM_TARGET_V1,
                            collected_scalar_gemm_v1::SCALAR_GEMM_CODE_OBJECT_VERSION_V1,
                            compiler_semantics,
                            frontend_authority,
                            canonical_handoff_bytes,
                            llvm_bytes,
                            publication_bytes,
                        ),
                        Err(error) => tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] {} rejected the collected program without fallback: {error}",
                            collected_scalar_gemm_v1::COLLECTED_SCALAR_GEMM_PIPELINE_V1,
                        )),
                    }
                } else if matches!(
                    codegen_pipeline,
                    PipelineSelection::Valid(CodegenPipeline::KernelIrWorkerV2)
                ) {
                    let attempt = build_attempt.unwrap_or_else(|| {
                        tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] {CODEGEN_PIPELINE_ENV}=kernel-ir-worker-v2 requires a managed {BUILD_ATTEMPT_ENV}"
                        ))
                    });
                    let publication = (|| -> Result<_, String> {
                        let collection = collector::collect_device_functions(
                            tcx,
                            mono_partitions.codegen_units,
                            self.config.verbose,
                        )
                        .map_err(|error| error.to_string())?;
                        if self.config.verbose
                            && let Some(envelope) = &collection.compiler_ffi_observation
                        {
                            let inspection = envelope.inspection();
                            eprintln!(
                                "[rustc-codegen-fe2o3] collected compiler FFI envelope {}: {} import(s), {} export(s), {} compiler-module definition requirement(s)",
                                envelope.identity().to_hex(),
                                inspection.import_count(),
                                inspection.export_count(),
                                inspection.requires_compiler_module_definition_count(),
                            );
                        }
                        let frontend_record =
                            frontend_record_bridge::extract_frontend_record_v1(tcx, &collection)
                                .map_err(|error| {
                                    format!("frontend record extraction failed: {error}")
                                })?;
                        if self.config.verbose {
                            eprintln!(
                                "[rustc-codegen-fe2o3] validated frontend record: {} function(s), {} canonical byte(s)",
                                frontend_record.unit().functions().len(),
                                frontend_record.canonical_bytes().len()
                            );
                            dump_authenticated_frontend_contracts(&frontend_record);
                        }
                        collector::dump_device_functions(tcx, &collection.functions);
                        let descriptor_roots =
                            compiler_descriptor::typed_descriptor_roots_from_collection(
                                tcx,
                                &collection.functions,
                            )
                            .map_err(|error| {
                                format!(
                                    "{CODEGEN_PIPELINE_ENV}=kernel-ir-worker-v2 typed descriptor extraction failed: {error}"
                                )
                            })?;
                        let mir_module = mir_import::import_collection(tcx, &collection).map_err(
                            |error| {
                                format!(
                                    "{CODEGEN_PIPELINE_ENV}=kernel-ir-worker-v2 compiler FFI MIR import failed: {error}"
                                )
                            },
                        )?;
                        let module = kernel_ir_lowering::translate_and_verify_for_session(
                            &mir_module,
                            &self.config.target,
                            tcx.sess,
                        )
                            .map_err(|errors| {
                                format!(
                                    "{CODEGEN_PIPELINE_ENV}=kernel-ir-worker-v2 compiler-module MIR translation failed: {errors}"
                                )
                            })?;
                        let source_debug = source_debug::collect_requested_profile(
                            tcx,
                            &collection,
                            &mir_module,
                            &self.config.target,
                        )
                        .map_err(|error| format!("source-debug profile rejected: {error}"))?;
                        // Incomplete collections fail at the envelope boundary without requiring
                        // an external host-object toolchain. Complete collections still prepare
                        // every witness before publishing the handoff.
                        let (semantic_witness_objects, semantic_witness_temporary) = if collection
                            .compiler_ffi_observation
                            .is_some()
                        {
                            generate_semantic_witness_host_objects(
                                    &descriptor_roots,
                                    output_dir,
                                    tcx.sess.target.llvm_target.as_ref(),
                                )
                                .map_err(|error| {
                                    format!(
                                        "{CODEGEN_PIPELINE_ENV}=kernel-ir-worker-v2 semantic-witness emission failed: {error}"
                                    )
                                })?
                        } else {
                            (
                                host_object::GeneratedHostObjects::default(),
                                TemporaryHostObjects::default(),
                            )
                        };
                        eprintln!(
                            "[rustc-codegen-fe2o3] selected kernel-ir-worker-v2: verified compiler-module candidate with {} kernel(s), {} function(s)",
                            module.kernels.len(),
                            module.functions.len()
                        );
                        if self.config.dump_mir {
                            eprintln!("{}", mir_module.summary());
                        }
                        let receipt =
                            worker_v2_producer::publish_worker_v2_compiler_module_with_descriptors(
                                output_dir,
                                &producer,
                                Some(attempt),
                                collection.compiler_ffi_observation.as_ref(),
                                &module,
                                &descriptor_roots,
                                source_debug.as_ref(),
                            )
                            .map_err(|error| error.to_string())?;
                        Ok((
                            receipt,
                            semantic_witness_objects,
                            semantic_witness_temporary,
                        ))
                    })();
                    match publication {
                        Ok((receipt, objects, temporary)) => {
                            generated_host_objects = objects;
                            temporary_host_objects = temporary;
                            eprintln!(
                                "[rustc-codegen-fe2o3] published inert Worker V2 compiler-module handoff: {} canonical byte(s)",
                                receipt.length()
                            );
                        }
                        Err(error) => tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] Worker V2 producer failed: {error}"
                        )),
                    }
                } else {
                    let mut typed_roots = Vec::new();
                    match amdgpu_llvm::emit_collection_after_preflight(
                        &producer,
                        output_dir,
                        &self.config.target,
                        build_attempt,
                        || {
                            let collection = collector::collect_device_functions(
                                tcx,
                                mono_partitions.codegen_units,
                                self.config.verbose,
                            )
                            .map_err(|error| {
                                amdgpu_llvm::EmitError::Preflight {
                                    reason: error.to_string(),
                                }
                            })?;
                            typed_roots = typed_roots_from_collection(&collection.functions)
                                .map_err(|error| amdgpu_llvm::EmitError::Preflight {
                                    reason: error.to_string(),
                                })?;
                            if self.config.verbose
                                && let Some(envelope) = &collection.compiler_ffi_observation
                            {
                                let inspection = envelope.inspection();
                                eprintln!(
                                    "[rustc-codegen-fe2o3] collected compiler FFI envelope {}: {} import(s), {} export(s), {} compiler-module definition requirement(s)",
                                    envelope.identity().to_hex(),
                                    inspection.import_count(),
                                    inspection.export_count(),
                                    inspection.requires_compiler_module_definition_count(),
                                );
                            }
                            let frontend_record =
                                frontend_record_bridge::extract_frontend_record_v1(
                                    tcx,
                                    &collection,
                                )
                                .map_err(|error| {
                                    amdgpu_llvm::EmitError::Preflight {
                                        reason: format!(
                                            "frontend record extraction failed: {error}"
                                        ),
                                    }
                                })?;
                            if self.config.verbose {
                                eprintln!(
                                    "[rustc-codegen-fe2o3] validated frontend record: {} function(s), {} canonical byte(s)",
                                    frontend_record.unit().functions().len(),
                                    frontend_record.canonical_bytes().len()
                                );
                                dump_authenticated_frontend_contracts(&frontend_record);
                            }
                            collector::dump_device_functions(tcx, &collection.functions);
                            let mir_module = mir_import::import_collection(tcx, &collection)
                                .map_err(|error| amdgpu_llvm::EmitError::Preflight {
                                    reason: format!("compiler FFI MIR import failed: {error}"),
                                })?;
                            match codegen_pipeline.resolve()? {
                            CodegenPipeline::LegacyV1 => {
                                match run_optional_kernel_ir_analysis(
                                    self.config.verify_kernel_ir,
                                    || {
                                        kernel_ir_lowering::translate_and_verify_for_session(
                                            &mir_module,
                                            &self.config.target,
                                            tcx.sess,
                                        )
                                    },
                                ) {
                                    Ok(Some(module)) => eprintln!(
                                        "[rustc-codegen-fe2o3] verified MIR kernel IR analysis: {} kernel(s), {} function(s)",
                                        module.kernels.len(),
                                        module.functions.len()
                                    ),
                                    Ok(None) => {}
                                    Err(errors) => {
                                        return Err(amdgpu_llvm::EmitError::Preflight {
                                            reason: format!(
                                                "MIR kernel IR analysis failed: {errors}"
                                            ),
                                        });
                                    }
                                }
                                let lowering_plan = record_lowering::plan_from_module(&mir_module);
                                if self.config.dump_mir {
                                    eprintln!("{}", mir_module.summary());
                                    eprintln!("{}", lowering_plan.summary());
                                }
                                amdgpu_llvm::prepare_collection(
                                    tcx,
                                    &collection,
                                    Some(&lowering_plan),
                                )
                            }
                            CodegenPipeline::KernelIrV1 => {
                                let module = kernel_ir_lowering::translate_and_verify_for_session(
                                    &mir_module,
                                    &self.config.target,
                                    tcx.sess,
                                )
                                    .map_err(|errors| amdgpu_llvm::EmitError::Preflight {
                                        reason: format!(
                                            "{CODEGEN_PIPELINE_ENV}=kernel-ir-v1 MIR translation failed: {errors}"
                                        ),
                                    })?;
                                if let Some(directory) =
                                    env::var_os(TILED_GEMM_FRONTEND_TEST_LLVM_DIR_ENV)
                                {
                                    let directory = PathBuf::from(directory);
                                    kernel_ir_codegen::retain_tiled_gemm_frontend_test_llvm(
                                        &module,
                                        &directory,
                                    )?;
                                    eprintln!(
                                        "[rustc-codegen-fe2o3] retained test-only tiled GEMM imported LLVM observation: {}",
                                        directory
                                            .join(kernel_ir_codegen::TILED_GEMM_FRONTEND_TEST_LLVM_FILE)
                                            .display()
                                    );
                                }
                                eprintln!(
                                    "[rustc-codegen-fe2o3] selected kernel-ir-v1: verified {} kernel(s), {} function(s)",
                                    module.kernels.len(),
                                    module.functions.len()
                                );
                                if self.config.dump_mir {
                                    eprintln!("{}", mir_module.summary());
                                }
                                let kernel_names = collection
                                    .functions
                                    .iter()
                                    .filter(|function| function.is_kernel_entry())
                                    .map(|function| function.export_name.clone())
                                    .collect::<Vec<_>>();
                                kernel_ir_codegen::prepare_fill_collection(module, &kernel_names)
                            }
                            CodegenPipeline::KernelIrWorkerV2 => {
                                Err(amdgpu_llvm::EmitError::Preflight {
                                    reason: "internal error: Worker V2 entered the legacy artifact transaction"
                                        .to_owned(),
                                })
                            }
                            CodegenPipeline::CollectedExecutableScalarControlFlowV2 => {
                                Err(amdgpu_llvm::EmitError::Preflight {
                                    reason: "internal error: collected executable scalar-control-flow V2 entered the legacy artifact transaction"
                                        .to_owned(),
                                })
                            }
                            CodegenPipeline::CollectedScalarGemmV1 => {
                                Err(amdgpu_llvm::EmitError::Preflight {
                                    reason: "internal error: collected scalar GEMM V1 entered the legacy artifact transaction"
                                        .to_owned(),
                                })
                            }
                        }
                        },
                    ) {
                        Ok(artifacts) => {
                            match generate_typed_host_objects(
                                &typed_roots,
                                &artifacts,
                                output_dir,
                                &self.config.target,
                                tcx.sess.target.llvm_target.as_ref(),
                            ) {
                                Ok((objects, temporary)) => {
                                    generated_host_objects = objects;
                                    temporary_host_objects = temporary;
                                }
                                Err(error) => tcx.dcx().fatal(format!(
                                    "[rustc-codegen-fe2o3] typed artifact binding failed: {error}"
                                )),
                            }
                            for artifact in &artifacts {
                                eprintln!(
                                    "[rustc-codegen-fe2o3] emitted {}: LLVM IR {}, HSACO {}",
                                    artifact.kernel_name,
                                    artifact.llvm_ir.path().display(),
                                    artifact.hsaco.path().display()
                                );
                            }
                        }
                        Err(error) => {
                            tcx.dcx().fatal(format!(
                                "[rustc-codegen-fe2o3] device codegen failed: {error}"
                            ));
                        }
                    }
                }
            } else if let Some(output_dir) = output_dir {
                let collection = collector::CollectionResult::default();
                if let Err(error) = amdgpu_llvm::emit_collection(
                    tcx,
                    &collection,
                    &producer,
                    None,
                    output_dir,
                    &self.config.target,
                    build_attempt,
                ) {
                    tcx.dcx().fatal(format!(
                        "[rustc-codegen-fe2o3] zero-kernel artifact reconciliation failed: {error}"
                    ));
                }
            }

            let llvm_codegen = self.llvm_backend.codegen_crate(tcx, crate_info);
            Box::new(OngoingFe2o3Codegen {
                llvm_codegen,
                host_objects: generated_host_objects,
                temporary_host_objects,
            }) as Box<dyn Any>
        })
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        sess: &Session,
        outputs: &OutputFilenames,
    ) -> (CompiledModules, FxIndexMap<WorkProductId, WorkProduct>) {
        let ongoing_codegen = match ongoing_codegen.downcast::<OngoingFe2o3Codegen>() {
            Ok(ongoing_codegen) => *ongoing_codegen,
            Err(_) => sess.dcx().fatal(
                "[rustc-codegen-fe2o3] internal error: ongoing codegen state had the wrong type",
            ),
        };
        let OngoingFe2o3Codegen {
            llvm_codegen,
            host_objects,
            temporary_host_objects,
        } = ongoing_codegen;
        let (mut compiled_modules, work_products) =
            self.llvm_backend.join_codegen(llvm_codegen, sess, outputs);
        if let Err(error) = host_objects.append_to(&mut compiled_modules) {
            sess.dcx().fatal(format!(
                "[rustc-codegen-fe2o3] generated host-object injection failed: {error}"
            ));
        }
        if !temporary_host_objects.is_empty() {
            match self.pending_host_objects.lock() {
                Ok(mut pending) => pending.push(temporary_host_objects),
                Err(_) => sess.dcx().fatal(
                    "[rustc-codegen-fe2o3] generated host-object lifetime state was poisoned",
                ),
            }
        }
        (compiled_modules, work_products)
    }

    fn link(
        &self,
        sess: &Session,
        compiled_modules: CompiledModules,
        crate_info: CrateInfo,
        metadata: EncodedMetadata,
        outputs: &OutputFilenames,
    ) {
        let temporary_host_objects = match self.pending_host_objects.lock() {
            Ok(mut pending) => std::mem::take(&mut *pending),
            Err(_) => sess
                .dcx()
                .fatal("[rustc-codegen-fe2o3] generated host-object lifetime state was poisoned"),
        };
        self.llvm_backend
            .link(sess, compiled_modules, crate_info, metadata, outputs);
        drop(temporary_host_objects);
    }
}

fn typed_roots_from_collection(
    functions: &[collector::CollectedFunction<'_>],
) -> Result<Vec<TypedKernelRootV1>, TypedVerticalError> {
    functions
        .iter()
        .filter_map(|function| {
            function.typed_profile.map(|profile| {
                if !function.is_kernel_entry() {
                    return Err(TypedVerticalError::InvalidCollectedRoot {
                        export_name: function.export_name.clone(),
                        reason: "typed profile is attached to a non-root device function",
                    });
                }
                if matches!(
                    profile,
                    collector::TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. }
                ) {
                    return Err(TypedVerticalError::InvalidCollectedRoot {
                        export_name: function.export_name.clone(),
                        reason: "general typed V3 requires kernel-ir-worker-v2 shared publication",
                    });
                }
                let logical_name = function.logical_name.clone().ok_or_else(|| {
                    TypedVerticalError::InvalidCollectedRoot {
                        export_name: function.export_name.clone(),
                        reason: "typed kernel root has no logical name",
                    }
                })?;
                let kernel_binding = function.kernel_binding.ok_or_else(|| {
                    TypedVerticalError::InvalidCollectedRoot {
                        export_name: function.export_name.clone(),
                        reason: "typed kernel root has no validated kernel binding",
                    }
                })?;
                let type_identities =
                    function.typed_layout_identities.clone().ok_or_else(|| {
                        TypedVerticalError::InvalidCollectedRoot {
                            export_name: function.export_name.clone(),
                            reason: "typed kernel root has no rustc-derived layout identities",
                        }
                    })?;
                if !profile.accepts_argument_count(type_identities.len()) {
                    return Err(TypedVerticalError::InvalidCollectedRoot {
                        export_name: function.export_name.clone(),
                        reason: "typed kernel argument count does not match its profile",
                    });
                }
                Ok(TypedKernelRootV1 {
                    logical_name,
                    export_name: function.export_name.clone(),
                    profile,
                    kernel_binding,
                    type_identities,
                })
            })
        })
        .collect()
}

fn generate_typed_host_objects(
    roots: &[TypedKernelRootV1],
    artifacts: &[amdgpu_llvm::DeviceArtifact],
    output_dir: &Path,
    target: &AmdGpuTarget,
    host_triple: &str,
) -> Result<(host_object::GeneratedHostObjects, TemporaryHostObjects), TypedVerticalError> {
    let mut objects = host_object::GeneratedHostObjects::default();
    let mut temporary = TemporaryHostObjects::default();
    if roots.is_empty() {
        return Ok((objects, temporary));
    }

    let artifact_indexes = match_typed_artifacts(roots, artifacts)?;
    let rocm = RocmToolchain::detect().map_err(TypedVerticalError::Toolchain)?;
    let toolchain = host_object::HostObjectToolchain::from_rocm(&rocm)
        .map_err(TypedVerticalError::HostObject)?;

    for (root, artifact_index) in roots.iter().zip(artifact_indexes) {
        let artifact = &artifacts[artifact_index];
        let llvm_ir =
            finalized_artifact_bytes(&artifact.llvm_ir, "LLVM IR", MAX_FINALIZED_LLVM_IR_BYTES)?;
        let hsaco = finalized_artifact_bytes(&artifact.hsaco, "HSACO", MAX_FINALIZED_HSACO_BYTES)?;
        let generated = match root.profile {
            collector::TypedKernelProfile::VecAddRustcLayoutV2 => {
                let type_identities: [fe2o3_artifacts::TypeIdentity; 3] =
                    root.type_identities.as_slice().try_into().map_err(|_| {
                        TypedVerticalError::InvalidCollectedRoot {
                            export_name: root.export_name.clone(),
                            reason: "typed vecadd profile requires exactly three layout identities",
                        }
                    })?;
                typed_artifact::validate_typed_vecadd_hsaco_v2(&root.export_name, target, hsaco)
                    .map_err(TypedVerticalError::Artifact)?;
                typed_artifact::build_typed_vecadd_artifact_v1(
                    &root.logical_name,
                    &root.export_name,
                    root.kernel_binding,
                    type_identities,
                    target,
                    llvm_ir,
                    hsaco.to_vec(),
                )
                .map_err(TypedVerticalError::Artifact)?
            }
            collector::TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. } => {
                return Err(TypedVerticalError::InvalidCollectedRoot {
                    export_name: root.export_name.clone(),
                    reason: "general typed V3 cannot enter legacy artifact generation",
                });
            }
        };
        let object_path = temporary.reserve(output_dir, generated.artifact_id())?;
        let object = host_object::generate_host_object(
            &toolchain,
            host_triple,
            &object_path,
            generated.artifact_id(),
            root.kernel_binding,
            generated.container(),
        )
        .map_err(TypedVerticalError::HostObject)?;
        objects
            .register(object)
            .map_err(TypedVerticalError::HostObject)?;
    }

    Ok((objects, temporary))
}

fn generate_semantic_witness_host_objects(
    roots: &[compiler_descriptor::TypedDescriptorRootV1],
    output_dir: &Path,
    host_triple: &str,
) -> Result<(host_object::GeneratedHostObjects, TemporaryHostObjects), TypedVerticalError> {
    let plans = semantic_witness::plans_from_descriptor_roots(roots)
        .map_err(TypedVerticalError::SemanticWitness)?;
    let mut objects = host_object::GeneratedHostObjects::default();
    let mut temporary = TemporaryHostObjects::default();
    if plans.is_empty() {
        return Ok((objects, temporary));
    }

    let rocm = RocmToolchain::detect().map_err(TypedVerticalError::Toolchain)?;
    let toolchain = host_object::HostObjectToolchain::from_rocm(&rocm)
        .map_err(TypedVerticalError::HostObject)?;
    for plan in plans {
        let kernel_binding = plan.kernel_binding();
        let object_path = temporary.reserve(output_dir, &kernel_binding.to_hex())?;
        let object = host_object::generate_semantic_witness_host_object(
            &toolchain,
            host_triple,
            &object_path,
            kernel_binding,
            plan.payload(),
        )
        .map_err(TypedVerticalError::HostObject)?;
        objects
            .register(object)
            .map_err(TypedVerticalError::HostObject)?;
    }
    Ok((objects, temporary))
}

fn match_typed_artifacts(
    roots: &[TypedKernelRootV1],
    artifacts: &[amdgpu_llvm::DeviceArtifact],
) -> Result<Vec<usize>, TypedVerticalError> {
    let mut artifacts_by_name = std::collections::BTreeMap::new();
    for (index, artifact) in artifacts.iter().enumerate() {
        if artifacts_by_name
            .insert(artifact.kernel_name.as_str(), index)
            .is_some()
        {
            return Err(TypedVerticalError::DuplicatePublishedArtifact(
                artifact.kernel_name.clone(),
            ));
        }
    }

    let mut logical_names = std::collections::BTreeSet::new();
    let mut export_names = std::collections::BTreeSet::new();
    let mut matches = Vec::with_capacity(roots.len());
    for root in roots {
        if !valid_ascii_symbol_stem(&root.logical_name) {
            return Err(TypedVerticalError::InvalidSymbolName(
                root.logical_name.clone(),
            ));
        }
        if !valid_ascii_symbol_stem(&root.export_name) {
            return Err(TypedVerticalError::InvalidSymbolName(
                root.export_name.clone(),
            ));
        }
        if !logical_names.insert(root.logical_name.as_str()) {
            return Err(TypedVerticalError::DuplicateLogicalName(
                root.logical_name.clone(),
            ));
        }
        if !export_names.insert(root.export_name.as_str()) {
            return Err(TypedVerticalError::DuplicateExportName(
                root.export_name.clone(),
            ));
        }
        let index = artifacts_by_name
            .get(root.export_name.as_str())
            .copied()
            .ok_or_else(|| TypedVerticalError::MissingPublishedArtifact {
                logical_name: root.logical_name.clone(),
                export_name: root.export_name.clone(),
            })?;
        matches.push(index);
    }
    Ok(matches)
}

fn valid_ascii_symbol_stem(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    name.len() <= 128
        && (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn finalized_artifact_bytes<'artifact>(
    snapshot: &'artifact artifact_transaction::FinalizedArtifactSnapshot,
    kind: &'static str,
    maximum: usize,
) -> Result<&'artifact [u8], TypedVerticalError> {
    let bytes = snapshot.bytes();
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(TypedVerticalError::InvalidFinalizedArtifactSize {
            kind,
            path: snapshot.path().to_path_buf(),
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(bytes)
}

#[derive(Debug)]
enum TypedVerticalError {
    InvalidCollectedRoot {
        export_name: String,
        reason: &'static str,
    },
    DuplicatePublishedArtifact(String),
    DuplicateLogicalName(String),
    DuplicateExportName(String),
    MissingPublishedArtifact {
        logical_name: String,
        export_name: String,
    },
    InvalidSymbolName(String),
    InvalidArtifactId(String),
    InvalidFinalizedArtifactSize {
        kind: &'static str,
        path: PathBuf,
        actual: usize,
        maximum: usize,
    },
    TemporaryDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    TemporaryDirectoryExhausted(PathBuf),
    Toolchain(ToolchainError),
    Artifact(typed_artifact::TypedArtifactError),
    SemanticWitness(semantic_witness::SemanticWitnessError),
    HostObject(host_object::HostObjectError),
}

impl fmt::Display for TypedVerticalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCollectedRoot {
                export_name,
                reason,
            } => write!(formatter, "invalid typed root `{export_name}`: {reason}"),
            Self::DuplicatePublishedArtifact(name) => {
                write!(formatter, "published artifact name `{name}` is duplicated")
            }
            Self::DuplicateLogicalName(name) => {
                write!(
                    formatter,
                    "typed kernel logical name `{name}` is duplicated"
                )
            }
            Self::DuplicateExportName(name) => {
                write!(formatter, "typed kernel export name `{name}` is duplicated")
            }
            Self::MissingPublishedArtifact {
                logical_name,
                export_name,
            } => write!(
                formatter,
                "typed kernel `{logical_name}` has no finalized artifact named `{export_name}`"
            ),
            Self::InvalidSymbolName(name) => write!(
                formatter,
                "typed kernel name `{name}` is not a bounded ASCII linker symbol stem"
            ),
            Self::InvalidArtifactId(id) => {
                write!(formatter, "typed artifact has an invalid content ID `{id}`")
            }
            Self::InvalidFinalizedArtifactSize {
                kind,
                path,
                actual,
                maximum,
            } => write!(
                formatter,
                "finalized {kind} {} has invalid size {actual}; maximum is {maximum}",
                path.display()
            ),
            Self::TemporaryDirectory { path, source } => write!(
                formatter,
                "failed to create private host-object directory {}: {source}",
                path.display()
            ),
            Self::TemporaryDirectoryExhausted(path) => write!(
                formatter,
                "could not reserve a unique host-object directory below {}",
                path.display()
            ),
            Self::Toolchain(error) => write!(formatter, "{error}"),
            Self::Artifact(error) => write!(formatter, "{error}"),
            Self::SemanticWitness(error) => write!(formatter, "{error}"),
            Self::HostObject(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TypedVerticalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TemporaryDirectory { source, .. } => Some(source),
            Self::Toolchain(error) => Some(error),
            Self::Artifact(error) => Some(error),
            Self::SemanticWitness(error) => Some(error),
            Self::HostObject(error) => Some(error),
            Self::InvalidCollectedRoot { .. }
            | Self::DuplicatePublishedArtifact(_)
            | Self::DuplicateLogicalName(_)
            | Self::DuplicateExportName(_)
            | Self::MissingPublishedArtifact { .. }
            | Self::InvalidSymbolName(_)
            | Self::InvalidArtifactId(_)
            | Self::InvalidFinalizedArtifactSize { .. }
            | Self::TemporaryDirectoryExhausted(_) => None,
        }
    }
}

#[unsafe(no_mangle)]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    let config = BackendConfig::from_env();
    let llvm_backend = rustc_codegen_llvm::LlvmCodegenBackend::new();

    Box::new(Fe2o3CodegenBackend {
        config,
        llvm_backend,
        pending_host_objects: Mutex::new(Vec::new()),
    })
}

#[derive(Clone, Debug)]
pub struct DeviceCodegenConfig {
    pub output_dir: PathBuf,
    pub output_name: String,
    pub target: AmdGpuTarget,
    pub verbose: bool,
    pub dump_mir: bool,
    pub dump_llvm: bool,
}

impl Default for DeviceCodegenConfig {
    fn default() -> Self {
        Self {
            output_dir: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            output_name: "kernel".to_string(),
            target: AmdGpuTarget::from_env_or_default(),
            verbose: false,
            dump_mir: env_flag(DUMP_MIR_ENV),
            dump_llvm: env_flag(DUMP_LLVM_ENV),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmdGpuTarget {
    name: String,
}

impl AmdGpuTarget {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    pub fn from_env_or_default() -> Self {
        env::var(TARGET_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(Self::new)
            .unwrap_or_else(|| Self::new("gfx1100"))
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl Default for AmdGpuTarget {
    fn default() -> Self {
        Self::from_env_or_default()
    }
}

impl fmt::Display for AmdGpuTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Clone, Debug)]
pub struct RocmToolchain {
    pub rocm_path: PathBuf,
    pub clang: PathBuf,
    pub ld_lld: PathBuf,
    pub llc: Option<PathBuf>,
    pub llvm_readobj: Option<PathBuf>,
    pub hip_library: PathBuf,
}

impl RocmToolchain {
    pub fn detect() -> Result<Self, ToolchainError> {
        let rocm_path = find_rocm_path().ok_or(ToolchainError::MissingRocm)?;
        let llvm_bin = rocm_path.join("lib/llvm/bin");
        let clang = require_tool(&llvm_bin, "clang")?;
        let ld_lld = require_tool(&llvm_bin, "ld.lld")?;
        let hip_library = rocm_path.join("lib/libamdhip64.so");
        if !hip_library.is_file() {
            return Err(ToolchainError::MissingPath(hip_library));
        }

        Ok(Self {
            rocm_path,
            clang,
            ld_lld,
            llc: optional_tool(&llvm_bin, "llc"),
            llvm_readobj: optional_tool(&llvm_bin, "llvm-readobj"),
            hip_library,
        })
    }
}

#[derive(Debug)]
pub enum ToolchainError {
    MissingRocm,
    MissingPath(PathBuf),
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRocm => write!(f, "could not find ROCm; set ROCM_PATH"),
            Self::MissingPath(path) => {
                write!(f, "required ROCm path does not exist: {}", path.display())
            }
        }
    }
}

impl std::error::Error for ToolchainError {}

#[derive(Debug)]
pub enum HsacoError {
    Toolchain(ToolchainError),
    Io(std::io::Error),
    CommandFailed {
        program: PathBuf,
        status: std::process::ExitStatus,
    },
    InvalidMetadata {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for HsacoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toolchain(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::CommandFailed { program, status } => {
                write!(f, "{} failed with status {status}", program.display())
            }
            Self::InvalidMetadata { path, reason } => {
                write!(
                    f,
                    "{} has invalid AMDGPU metadata: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for HsacoError {}

impl From<ToolchainError> for HsacoError {
    fn from(error: ToolchainError) -> Self {
        Self::Toolchain(error)
    }
}

impl From<std::io::Error> for HsacoError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn compile_llvm_ir_to_hsaco(
    ll_path: impl AsRef<Path>,
    hsaco_path: impl AsRef<Path>,
    target: &AmdGpuTarget,
) -> Result<(), HsacoError> {
    let toolchain = RocmToolchain::detect()?;
    compile_llvm_ir_to_hsaco_with_toolchain(ll_path, hsaco_path, target, &toolchain)
}

pub fn compile_llvm_ir_to_hsaco_with_toolchain(
    ll_path: impl AsRef<Path>,
    hsaco_path: impl AsRef<Path>,
    target: &AmdGpuTarget,
    toolchain: &RocmToolchain,
) -> Result<(), HsacoError> {
    let ll_path = ll_path.as_ref();
    let hsaco_path = hsaco_path.as_ref();
    let obj_path = hsaco_path.with_extension("o");

    let mcpu = format!("-mcpu={}", target.as_str());
    run(Command::new(&toolchain.clang).args([
        "-target",
        dialect_amdgcn::AMDGPU_TRIPLE,
        mcpu.as_str(),
        "-x",
        "ir",
        "-c",
        path_arg(ll_path).as_str(),
        "-o",
        path_arg(&obj_path).as_str(),
    ]))?;

    run(Command::new(&toolchain.ld_lld).args([
        "-shared",
        path_arg(&obj_path).as_str(),
        "-o",
        path_arg(hsaco_path).as_str(),
    ]))?;
    validate_hsaco_metadata(hsaco_path, target, toolchain)?;

    Ok(())
}

fn validate_hsaco_metadata(
    hsaco_path: &Path,
    target: &AmdGpuTarget,
    toolchain: &RocmToolchain,
) -> Result<(), HsacoError> {
    let Some(llvm_readobj) = &toolchain.llvm_readobj else {
        return Ok(());
    };

    let output = Command::new(llvm_readobj)
        .args(["--notes", path_arg(hsaco_path).as_str()])
        .output()?;
    if !output.status.success() {
        return Err(HsacoError::CommandFailed {
            program: llvm_readobj.clone(),
            status: output.status,
        });
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.stderr.is_empty() {
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    let kernel_name = hsaco_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    validate_hsaco_metadata_text(&text, target, kernel_name).map_err(|reason| {
        HsacoError::InvalidMetadata {
            path: hsaco_path.to_path_buf(),
            reason,
        }
    })
}

fn validate_hsaco_metadata_text(
    text: &str,
    target: &AmdGpuTarget,
    kernel_name: &str,
) -> Result<(), String> {
    if !text.contains("Format: elf64-amdgpu") {
        return Err("expected elf64-amdgpu format".to_string());
    }

    let target_line = text
        .lines()
        .find(|line| line.trim_start().starts_with("amdhsa.target:"))
        .ok_or_else(|| "missing amdhsa.target metadata".to_string())?;
    if !target_line.contains(target.as_str()) {
        return Err(format!(
            "expected target {}, got {}",
            target.as_str(),
            target_line.trim()
        ));
    }

    if !kernel_name.is_empty() {
        let expected_name = format!(".name:           {kernel_name}");
        if !text.contains(&expected_name) {
            return Err(format!("missing kernel metadata name `{kernel_name}`"));
        }
    }

    Ok(())
}

fn run(command: &mut Command) -> Result<(), HsacoError> {
    let program = command.get_program().into();
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(HsacoError::CommandFailed { program, status })
    }
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

fn managed_artifact_output(
    config: &BackendConfig,
    kernel_count: usize,
) -> Result<Option<&Path>, ()> {
    match config.hsaco_output_dir.as_deref() {
        Some(path) if !path.as_os_str().is_empty() => Ok(Some(path)),
        Some(_) => Err(()),
        None if kernel_count == 0 => Ok(None),
        None => Err(()),
    }
}

fn run_optional_kernel_ir_analysis<T, E>(
    enabled: bool,
    analysis: impl FnOnce() -> Result<T, E>,
) -> Result<Option<T>, E> {
    if enabled {
        analysis().map(Some)
    } else {
        Ok(None)
    }
}

fn find_rocm_path() -> Option<PathBuf> {
    for var in ["ROCM_PATH", "HIP_PATH"] {
        if let Ok(value) = env::var(var) {
            let path = PathBuf::from(value);
            if path.join("lib/libamdhip64.so").is_file() {
                return Some(path);
            }
        }
    }

    ["/opt/rocm", "/opt/rocm-7.2.0", "/opt/rocm-7.1.0"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.join("lib/libamdhip64.so").is_file())
}

fn require_tool(llvm_bin: &Path, name: &str) -> Result<PathBuf, ToolchainError> {
    let path = llvm_bin.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(ToolchainError::MissingPath(path))
    }
}

fn optional_tool(llvm_bin: &Path, name: &str) -> Option<PathBuf> {
    let path = llvm_bin.join(name);
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::{
        AmdGpuTarget, BackendConfig, BuildAttemptSelection, CodegenPipeline, PipelineSelection,
        TemporaryHostObjects, TypedKernelRootV1, TypedVerticalError, finalized_artifact_bytes,
        generate_typed_host_objects, managed_artifact_output, match_typed_artifacts,
        run_optional_kernel_ir_analysis, validate_hsaco_metadata_text,
    };
    use crate::amdgpu_llvm::DeviceArtifact;
    use crate::collector::TypedKernelProfile;
    use fe2o3_artifact_transaction::FinalizedArtifactSnapshot;
    use rustc_codegen_ssa::CompiledModules;
    use std::ffi::OsStr;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-typed-vertical-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn typed_root(logical_name: &str, export_name: &str) -> TypedKernelRootV1 {
        let crate_binding =
            reserved_fe2o3_symbols::derive_crate_binding_id_v1("fixture", ["metadata"]);
        TypedKernelRootV1 {
            logical_name: logical_name.to_owned(),
            export_name: export_name.to_owned(),
            profile: TypedKernelProfile::VecAddRustcLayoutV2,
            kernel_binding: reserved_fe2o3_symbols::derive_kernel_binding_id_v1(
                crate_binding,
                reserved_fe2o3_symbols::TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
                logical_name,
                export_name,
            ),
            type_identities: crate::collector::TypedArgumentListV1::new(vec![
                fe2o3_artifacts::TypeIdentity::new(
                fe2o3_artifacts::DeclaredRustTypeIdentity::from_untrusted_bytes(
                    fe2o3_artifacts::DigestBytes::from_bytes([0x31; 32]),
                ),
                fe2o3_artifacts::DeclaredRustLayoutIdentity::from_untrusted_bytes(
                    fe2o3_artifacts::DigestBytes::from_bytes([0x32; 32]),
                ),
            ); 3])
            .unwrap(),
        }
    }

    fn published_artifact(name: &str) -> DeviceArtifact {
        DeviceArtifact {
            kernel_name: name.to_owned(),
            llvm_ir: FinalizedArtifactSnapshot::from_bytes(
                format!("{name}.ll"),
                b"test-ir".to_vec(),
            ),
            hsaco: FinalizedArtifactSnapshot::from_bytes(
                format!("{name}.hsaco"),
                b"test-hsaco".to_vec(),
            ),
        }
    }

    #[test]
    fn kernels_require_an_explicit_managed_artifact_directory() {
        let mut config = BackendConfig::default();
        assert_eq!(managed_artifact_output(&config, 0), Ok(None));
        assert_eq!(managed_artifact_output(&config, 1), Err(()));

        config.hsaco_output_dir = Some(PathBuf::new());
        assert_eq!(managed_artifact_output(&config, 1), Err(()));

        config.hsaco_output_dir = Some(PathBuf::from("target/fe2o3"));
        assert_eq!(
            managed_artifact_output(&config, 1),
            Ok(Some(Path::new("target/fe2o3")))
        );
    }

    #[test]
    fn build_attempt_selection_is_direct_or_exactly_canonical() {
        assert_eq!(BuildAttemptSelection::default().resolve(), Ok(None));
        let value = concat!(
            "7:11111111111111111111111111111111:",
            "2222222222222222222222222222222222222222222222222222222222222222"
        );
        let selection = BuildAttemptSelection::Managed(
            fe2o3_artifact_transaction::BuildAttempt::from_env_value(value).unwrap(),
        );
        assert_eq!(selection.resolve().unwrap().unwrap().to_env_value(), value);
        assert!(
            BuildAttemptSelection::Invalid("bad".to_owned())
                .resolve()
                .is_err()
        );
    }

    #[test]
    fn ordinary_kernels_leave_generated_host_objects_empty() {
        let artifacts = [published_artifact("ordinary")];
        let (objects, temporary) = generate_typed_host_objects(
            &[],
            &artifacts,
            Path::new("/not-consulted"),
            &AmdGpuTarget::new("gfx942"),
            "unsupported-host-is-not-consulted",
        )
        .expect("ordinary kernels do not enter typed artifact generation");
        assert!(temporary.is_empty());

        let mut modules = CompiledModules {
            modules: Vec::new(),
            allocator_module: None,
        };
        objects.append_to(&mut modules).unwrap();
        assert!(modules.modules.is_empty());
    }

    #[test]
    fn typed_roots_match_finalized_artifacts_by_export_name() {
        let roots = [typed_root("add", "vecadd"), typed_root("sum", "vector_sum")];
        let artifacts = [
            published_artifact("ordinary"),
            published_artifact("vector_sum"),
            published_artifact("vecadd"),
        ];

        assert_eq!(match_typed_artifacts(&roots, &artifacts).unwrap(), [2, 1]);
    }

    #[test]
    fn typed_artifact_matching_rejects_missing_duplicates_and_invalid_symbols() {
        let missing = match_typed_artifacts(
            &[typed_root("vecadd", "vecadd")],
            &[published_artifact("other")],
        );
        assert!(matches!(
            missing,
            Err(TypedVerticalError::MissingPublishedArtifact { .. })
        ));

        let duplicate_artifacts = match_typed_artifacts(
            &[typed_root("vecadd", "vecadd")],
            &[published_artifact("vecadd"), published_artifact("vecadd")],
        );
        assert!(matches!(
            duplicate_artifacts,
            Err(TypedVerticalError::DuplicatePublishedArtifact(name)) if name == "vecadd"
        ));

        let duplicate_logical = match_typed_artifacts(
            &[
                typed_root("vecadd", "first"),
                typed_root("vecadd", "second"),
            ],
            &[published_artifact("first"), published_artifact("second")],
        );
        assert!(matches!(
            duplicate_logical,
            Err(TypedVerticalError::DuplicateLogicalName(name)) if name == "vecadd"
        ));

        let invalid = match_typed_artifacts(
            &[typed_root("not-ascii-linker-safe", "vecadd")],
            &[published_artifact("vecadd")],
        );
        assert!(matches!(
            invalid,
            Err(TypedVerticalError::InvalidSymbolName(name)) if name == "not-ascii-linker-safe"
        ));
    }

    #[test]
    fn finalized_artifact_snapshots_are_exact_and_bounded() {
        let directory = TestDirectory::new();
        let path = directory.0.join("vecadd.ll");
        fs::write(&path, b"newer-path-generation").unwrap();
        let artifact = FinalizedArtifactSnapshot::from_bytes(&path, b"finalized-ir".to_vec());
        assert_eq!(
            finalized_artifact_bytes(&artifact, "LLVM IR", 64).unwrap(),
            b"finalized-ir"
        );
        fs::write(&path, b"replacement-generation").unwrap();
        assert_eq!(
            finalized_artifact_bytes(&artifact, "LLVM IR", 64).unwrap(),
            b"finalized-ir"
        );

        let empty = FinalizedArtifactSnapshot::from_bytes("empty.ll", Vec::new());
        assert!(matches!(
            finalized_artifact_bytes(&empty, "LLVM IR", 64),
            Err(TypedVerticalError::InvalidFinalizedArtifactSize { actual: 0, .. })
        ));
        assert!(matches!(
            finalized_artifact_bytes(&artifact, "LLVM IR", 4),
            Err(TypedVerticalError::InvalidFinalizedArtifactSize {
                actual: 12,
                maximum: 4,
                ..
            })
        ));
    }

    #[test]
    fn temporary_host_objects_survive_until_their_owner_drops() {
        const ARTIFACT_ID: &str =
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let directory = TestDirectory::new();
        let mut temporary = TemporaryHostObjects::default();
        let object = temporary.reserve(&directory.0, ARTIFACT_ID).unwrap();
        let object_directory = object.parent().unwrap().to_path_buf();
        fs::write(&object, b"host-object").unwrap();
        assert!(object.is_file());

        drop(temporary);
        assert!(!object.exists());
        assert!(!object_directory.exists());
    }

    #[test]
    fn optional_kernel_ir_analysis_is_fail_closed_when_enabled() {
        let result = run_optional_kernel_ir_analysis(true, || Err::<(), _>("translation failed"));

        assert_eq!(result, Err("translation failed"));
    }

    #[test]
    fn production_pipeline_selection_is_versioned_and_strict() {
        assert_eq!(
            PipelineSelection::from_value(None),
            PipelineSelection::Valid(CodegenPipeline::LegacyV1)
        );
        assert_eq!(
            PipelineSelection::from_value(Some(OsStr::new("legacy-v1"))),
            PipelineSelection::Valid(CodegenPipeline::LegacyV1)
        );
        assert_eq!(
            PipelineSelection::from_value(Some(OsStr::new("kernel-ir-v1"))),
            PipelineSelection::Valid(CodegenPipeline::KernelIrV1)
        );
        assert_eq!(
            PipelineSelection::from_value(Some(OsStr::new("kernel-ir-worker-v2"))),
            PipelineSelection::Valid(CodegenPipeline::KernelIrWorkerV2)
        );

        for invalid in ["", "legacy", "kernel-ir", "worker-v2", "true", "1"] {
            let selection = PipelineSelection::from_value(Some(OsStr::new(invalid)));
            let error = selection.resolve().expect_err("selector must be exact");
            let message = error.to_string();
            assert!(message.contains("FE2O3_CODEGEN_PIPELINE"));
            assert!(message.contains("legacy-v1"));
            assert!(message.contains("kernel-ir-v1"));
            assert!(message.contains("kernel-ir-worker-v2"));
        }
    }

    #[test]
    fn accepts_expected_hsaco_metadata() {
        let text = r#"
File: target/fe2o3/vecadd.hsaco
Format: elf64-amdgpu
AMDGPU Metadata: ---
amdhsa.kernels:
  - .name:           vecadd
amdhsa.target:   amdgcn-amd-amdhsa--gfx1201
...
"#;

        validate_hsaco_metadata_text(text, &AmdGpuTarget::new("gfx1201"), "vecadd").unwrap();
    }

    #[test]
    fn rejects_target_mismatch() {
        let text = r#"
File: target/fe2o3/vecadd.hsaco
Format: elf64-amdgpu
AMDGPU Metadata: ---
amdhsa.kernels:
  - .name:           vecadd
amdhsa.target:   amdgcn-amd-amdhsa--gfx1100
...
"#;

        let error = validate_hsaco_metadata_text(text, &AmdGpuTarget::new("gfx1201"), "vecadd")
            .unwrap_err();
        assert!(error.contains("expected target gfx1201"));
    }

    #[test]
    fn rejects_kernel_name_mismatch() {
        let text = r#"
File: target/fe2o3/scale.hsaco
Format: elf64-amdgpu
AMDGPU Metadata: ---
amdhsa.kernels:
  - .name:           vecadd
amdhsa.target:   amdgcn-amd-amdhsa--gfx1201
...
"#;

        let error =
            validate_hsaco_metadata_text(text, &AmdGpuTarget::new("gfx1201"), "scale").unwrap_err();
        assert!(error.contains("missing kernel metadata name `scale`"));
    }
}
