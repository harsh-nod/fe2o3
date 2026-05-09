use rustc_codegen_fe2o3::{AmdGpuTarget, RocmToolchain};
use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "doctor" => doctor(),
        "build" => cargo_passthrough("build", &rest),
        "run" => cargo_passthrough("run", &rest),
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
    println!("target: {}", AmdGpuTarget::from_env_or_default());

    match RocmToolchain::detect() {
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

fn cargo_passthrough(command: &str, args: &[String]) -> ExitCode {
    eprintln!(
        "cargo fe2o3 {command}: device MIR codegen is not wired yet; running `cargo {command}` for host code"
    );
    let status = Command::new("cargo")
        .arg(command)
        .args(args)
        .env("FE2O3_HOST_PASSTHROUGH", "1")
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

fn print_help() {
    eprintln!(
        "usage: cargo fe2o3 <command>\n\ncommands:\n  doctor   check ROCm/HIP toolchain discovery\n  build    host cargo build passthrough for now\n  run      host cargo run passthrough for now"
    );
}
