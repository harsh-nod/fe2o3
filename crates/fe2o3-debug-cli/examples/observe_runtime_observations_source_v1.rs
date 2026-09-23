//! Public sealed-observation acceptance over fresh ordinary Rust exports.
#[path = "runtime_observations_source_v1/run.rs"]
mod run;
fn main() -> std::process::ExitCode {
    run::main()
}
