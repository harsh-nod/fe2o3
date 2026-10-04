//! Compatibility entry point for the installed ordered-program inspector.
#![forbid(unsafe_code)]
fn main() -> std::process::ExitCode {
    fe2o3_kir_sim_cli::run_ordered_program_inspector()
}
