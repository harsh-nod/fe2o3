use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const TARGET_ENV: &str = "FE2O3_TARGET";
const BACKEND_ENV: &str = "FE2O3_BACKEND";
const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "doctor" => doctor(),
        "build" => cargo_with_backend("build", &rest),
        "run" => cargo_with_backend("run", &rest),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown cargo-fe2o3 command `{other}`");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn doctor() -> ExitCode {
    println!("fe2o3 diagnostics");
    println!("target: {}", amd_gpu_target());

    match detect_rocm_toolchain() {
        Ok(toolchain) => {
            println!("ROCm: {}", toolchain.rocm_path.display());
            println!("clang: {}", toolchain.clang.display());
            println!("ld.lld: {}", toolchain.ld_lld.display());
            if let Some(llc) = toolchain.llc {
                println!("llc: {}", llc.display());
            }
            if let Some(llvm_readobj) = toolchain.llvm_readobj {
                println!("llvm-readobj: {}", llvm_readobj.display());
            }
            println!("HIP: {}", toolchain.hip_library.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ROCm toolchain: {error}");
            ExitCode::FAILURE
        }
    }
}

fn cargo_with_backend(command: &str, args: &[String]) -> ExitCode {
    let workspace_root = match find_workspace_root() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let backend = match find_or_build_backend(&workspace_root) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let artifact_dir = workspace_root.join("target/fe2o3");
    if let Err(error) = std::fs::create_dir_all(&artifact_dir) {
        eprintln!(
            "failed to create fe2o3 artifact directory {}: {error}",
            artifact_dir.display()
        );
        return ExitCode::FAILURE;
    }

    let rustflags = append_rustflags(&[
        format!("-Zcodegen-backend={}", backend.display()),
        "-Zmir-enable-passes=-JumpThreading".to_string(),
    ]);

    eprintln!(
        "cargo fe2o3 {command}: using backend {} for target {}",
        backend.display(),
        amd_gpu_target()
    );

    let status = Command::new("cargo")
        .arg(command)
        .args(args)
        .env("RUSTFLAGS", rustflags)
        .env(HSACO_DIR_ENV, &artifact_dir)
        .env(TARGET_ENV, amd_gpu_target())
        .env("FE2O3_HOST_PASSTHROUGH", "0")
        .status();

    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("failed to run cargo: {error}");
            ExitCode::FAILURE
        }
    }
}

fn find_or_build_backend(workspace_root: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var(BACKEND_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{BACKEND_ENV} points to {}, but that file does not exist",
            path.display()
        ));
    }

    let backend = dylib_path(workspace_root);
    eprintln!("building rustc-codegen-fe2o3 backend...");
    let status = Command::new("cargo")
        .args(["build", "-p", "rustc-codegen-fe2o3"])
        .current_dir(workspace_root)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .status()
        .map_err(|error| format!("failed to build rustc-codegen-fe2o3: {error}"))?;

    if !status.success() {
        return Err("failed to build rustc-codegen-fe2o3".to_string());
    }

    if backend.is_file() {
        Ok(backend)
    } else {
        Err(format!(
            "backend build succeeded, but {} was not produced",
            backend.display()
        ))
    }
}

fn dylib_path(workspace_root: &Path) -> PathBuf {
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("target"));
    target_dir.join("debug/librustc_codegen_fe2o3.so")
}

fn find_workspace_root() -> Result<PathBuf, String> {
    let mut dir = env::current_dir().map_err(|error| format!("failed to read cwd: {error}"))?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates/rustc-codegen-fe2o3").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(
                "could not find fe2o3 workspace root; run cargo-fe2o3 from the repo".to_string(),
            );
        }
    }
}

fn append_rustflags(extra: &[String]) -> String {
    let mut flags = env::var("RUSTFLAGS").unwrap_or_default();
    for flag in extra {
        if !flags.is_empty() {
            flags.push(' ');
        }
        flags.push_str(flag);
    }
    flags
}

#[derive(Debug)]
struct RocmToolchain {
    rocm_path: PathBuf,
    clang: PathBuf,
    ld_lld: PathBuf,
    llc: Option<PathBuf>,
    llvm_readobj: Option<PathBuf>,
    hip_library: PathBuf,
}

fn detect_rocm_toolchain() -> Result<RocmToolchain, String> {
    let rocm_path =
        find_rocm_path().ok_or_else(|| "could not find ROCm; set ROCM_PATH".to_string())?;
    let llvm_bin = rocm_path.join("lib/llvm/bin");
    let clang = require_tool(&llvm_bin, "clang")?;
    let ld_lld = require_tool(&llvm_bin, "ld.lld")?;
    let hip_library = rocm_path.join("lib/libamdhip64.so");
    if !hip_library.is_file() {
        return Err(format!(
            "required ROCm path does not exist: {}",
            hip_library.display()
        ));
    }

    Ok(RocmToolchain {
        rocm_path,
        clang,
        ld_lld,
        llc: optional_tool(&llvm_bin, "llc"),
        llvm_readobj: optional_tool(&llvm_bin, "llvm-readobj"),
        hip_library,
    })
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

fn require_tool(llvm_bin: &Path, name: &str) -> Result<PathBuf, String> {
    let path = llvm_bin.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "required ROCm path does not exist: {}",
            path.display()
        ))
    }
}

fn optional_tool(llvm_bin: &Path, name: &str) -> Option<PathBuf> {
    let path = llvm_bin.join(name);
    path.is_file().then_some(path)
}

fn amd_gpu_target() -> String {
    env::var(TARGET_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gfx1100".to_string())
}

fn print_help() {
    eprintln!(
        "usage: cargo fe2o3 <command>\n\ncommands:\n  doctor   check ROCm/HIP toolchain discovery\n  build    build with the fe2o3 rustc backend\n  run      run with the fe2o3 rustc backend"
    );
}
