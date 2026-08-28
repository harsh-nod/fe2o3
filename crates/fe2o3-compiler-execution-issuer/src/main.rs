use std::process::ExitCode;

fn main() -> ExitCode {
    match fe2o3_compiler_execution_issuer::run_inherited_compiler_execution_issuer_v1() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
