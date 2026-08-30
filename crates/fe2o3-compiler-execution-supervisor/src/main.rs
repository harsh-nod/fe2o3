use std::process::ExitCode;

fn main() -> ExitCode {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    match fe2o3_compiler_execution_supervisor::run_inherited_protected_issuer_service_v1() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
