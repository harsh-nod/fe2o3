use std::process::ExitCode;

fn main() -> ExitCode {
    match fe2o3_compiler_execution_supervisor::run_inherited_protected_issuer_service_v1() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
