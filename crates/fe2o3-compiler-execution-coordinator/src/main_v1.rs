fn main() -> std::process::ExitCode {
    match fe2o3_compiler_execution_coordinator::run_inherited_compiler_execution_coordinator_v1() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
