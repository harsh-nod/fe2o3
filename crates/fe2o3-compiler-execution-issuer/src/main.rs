#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
core::arch::global_asm!(include_str!("secure_start_x86_64.S"), options(att_syntax));

use std::process::ExitCode;

fn main() -> ExitCode {
    match fe2o3_compiler_execution_issuer::run_inherited_compiler_execution_issuer_v1() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
