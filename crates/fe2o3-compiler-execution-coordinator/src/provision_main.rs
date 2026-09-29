#[allow(unsafe_code)]
fn main() -> std::process::ExitCode {
    // SAFETY: dedicated single-threaded main owns argv and the initial C
    // environment exclusively, has not admitted activation descriptors, and
    // calls once. Return or unwind terminates this process, never retries.
    match unsafe {
        fe2o3_compiler_execution_coordinator::run_compiler_execution_reference_provisioner_v3()
    } {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
