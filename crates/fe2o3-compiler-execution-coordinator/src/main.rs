#[allow(unsafe_code)]
fn main() -> std::process::ExitCode {
    // SAFETY: this dedicated executable calls the entrypoint exactly once on its
    // main thread, before creating threads, handlers or owners for FDs 3..=16.
    // The service manager supplies the unique descriptors/environment and retains
    // whole-cgroup termination custody (KillMode=mixed). No application work,
    // retry or fallback follows return/unwind; the process terminates.
    match unsafe {
        fe2o3_compiler_execution_coordinator::run_inherited_compiler_execution_coordinator_v3()
    } {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
