use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const TARGET_ENV: &str = "FE2O3_TARGET";
pub const BACKEND_ENV: &str = "FE2O3_BACKEND";
pub const DUMP_MIR_ENV: &str = "FE2O3_DUMP_MIR";
pub const DUMP_LLVM_ENV: &str = "FE2O3_DUMP_LLVM";

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
}

impl fmt::Display for HsacoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toolchain(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::CommandFailed { program, status } => {
                write!(f, "{} failed with status {status}", program.display())
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

    run(Command::new(&toolchain.clang).args([
        "-target",
        dialect_amdgcn::AMDGPU_TRIPLE,
        "-mcpu",
        target.as_str(),
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
