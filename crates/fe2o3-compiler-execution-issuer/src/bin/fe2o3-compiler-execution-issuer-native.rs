//! Separately pinned native issuer executable; never selected by wire fallback.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::process::ExitCode;

fn main() -> ExitCode {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    // Finite per-occurrence policy, not an estimate of total process CPU or RSS.
    // The native subject decoder also caps logical storage at 256 MiB.
    let mut work = Work::new(1_000_000_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024 * 1024);
    match fe2o3_compiler_execution_issuer::run_inherited_compiler_execution_issuer_v2(&mut budget) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
