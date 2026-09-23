//! Explicit developer diagnostic; the caller supplies complete targeted rustc argv.
#![feature(rustc_private)]
#![forbid(unsafe_code)]
use std::path::Path;
use std::process::ExitCode;
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 || args[2] != "--" {
        eprintln!(
            "usage: diagnostic_ordered_program_origin_v1 RAW_V17 ORIGIN_JSON -- RUSTC_ARGV..."
        );
        return ExitCode::FAILURE;
    }
    match rustc_codegen_fe2o3::run_diagnostic_ordered_program_origin_driver_v1(
        &args[3..],
        Path::new(&args[0]),
        Path::new(&args[1]),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ordered origin diagnostic: {error}");
            ExitCode::FAILURE
        }
    }
}
