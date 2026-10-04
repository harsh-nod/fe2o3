use std::process::ExitCode;

fn main() -> ExitCode {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    match fe2o3_compiler_execution_issuer::run_inherited_compiler_execution_issuer_v1() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
