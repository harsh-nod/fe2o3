#![feature(rustc_private)]

extern crate rustc_codegen_llvm;
extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod amdgpu_llvm;
mod artifact_transaction;
mod collector;
mod kernel_ir_lowering;
mod mir_import;
mod record_lowering;
mod trusted_device_items;

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
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const TARGET_ENV: &str = "FE2O3_TARGET";
pub const BACKEND_ENV: &str = "FE2O3_BACKEND";
pub const VERBOSE_ENV: &str = "FE2O3_VERBOSE";
pub const DUMP_MIR_ENV: &str = "FE2O3_DUMP_MIR";
pub const DUMP_LLVM_ENV: &str = "FE2O3_DUMP_LLVM";
pub const VERIFY_KERNEL_IR_ENV: &str = "FE2O3_VERIFY_KERNEL_IR";
pub const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";

pub struct Fe2o3CodegenBackend {
    config: BackendConfig,
    llvm_backend: Box<dyn CodegenBackend>,
}

#[derive(Clone, Debug, Default)]
pub struct BackendConfig {
    pub verbose: bool,
    pub dump_mir: bool,
    pub dump_llvm: bool,
    pub verify_kernel_ir: bool,
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
            hsaco_output_dir: env::var(HSACO_DIR_ENV).ok().map(PathBuf::from),
            target: AmdGpuTarget::from_env_or_default(),
        }
    }
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

            if kernel_count > 0 {
                let output_dir = output_dir.expect("kernel output was required above");
                match amdgpu_llvm::emit_collection_after_preflight(
                    &producer,
                    output_dir,
                    &self.config.target,
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
                        collector::dump_device_functions(tcx, &collection.functions);
                        let mir_module = mir_import::import_collection(tcx, &collection);
                        match run_optional_kernel_ir_analysis(self.config.verify_kernel_ir, || {
                            kernel_ir_lowering::translate_and_verify(&mir_module)
                        }) {
                            Ok(Some(module)) => eprintln!(
                                "[rustc-codegen-fe2o3] verified MIR kernel IR analysis: {} kernel(s), {} function(s)",
                                module.kernels.len(),
                                module.functions.len()
                            ),
                            Ok(None) => {}
                            Err(errors) => {
                                return Err(amdgpu_llvm::EmitError::Preflight {
                                    reason: format!("MIR kernel IR analysis failed: {errors}"),
                                });
                            }
                        }
                        let lowering_plan = record_lowering::plan_from_module(&mir_module);
                        if self.config.dump_mir {
                            eprintln!("{}", mir_module.summary());
                            eprintln!("{}", lowering_plan.summary());
                        }
                        amdgpu_llvm::prepare_collection(tcx, &collection, Some(&lowering_plan))
                    },
                ) {
                    Ok(artifacts) => {
                        for artifact in artifacts {
                            eprintln!(
                                "[rustc-codegen-fe2o3] emitted {}: LLVM IR {}, HSACO {}",
                                artifact.kernel_name,
                                artifact.llvm_ir_path.display(),
                                artifact.hsaco_path.display()
                            );
                        }
                    }
                    Err(error) => {
                        tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] device codegen failed: {error}"
                        ));
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
                ) {
                    tcx.dcx().fatal(format!(
                        "[rustc-codegen-fe2o3] zero-kernel artifact reconciliation failed: {error}"
                    ));
                }
            }

            self.llvm_backend.codegen_crate(tcx, crate_info)
        })
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        sess: &Session,
        outputs: &OutputFilenames,
    ) -> (CompiledModules, FxIndexMap<WorkProductId, WorkProduct>) {
        self.llvm_backend
            .join_codegen(ongoing_codegen, sess, outputs)
    }

    fn link(
        &self,
        sess: &Session,
        compiled_modules: CompiledModules,
        crate_info: CrateInfo,
        metadata: EncodedMetadata,
        outputs: &OutputFilenames,
    ) {
        self.llvm_backend
            .link(sess, compiled_modules, crate_info, metadata, outputs);
    }
}

#[unsafe(no_mangle)]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    let config = BackendConfig::from_env();
    let llvm_backend = rustc_codegen_llvm::LlvmCodegenBackend::new();

    Box::new(Fe2o3CodegenBackend {
        config,
        llvm_backend,
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
        AmdGpuTarget, BackendConfig, managed_artifact_output, run_optional_kernel_ir_analysis,
        validate_hsaco_metadata_text,
    };
    use std::path::{Path, PathBuf};

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
    fn optional_kernel_ir_analysis_is_fail_closed_when_enabled() {
        let result = run_optional_kernel_ir_analysis(true, || Err::<(), _>("translation failed"));

        assert_eq!(result, Err("translation failed"));
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
