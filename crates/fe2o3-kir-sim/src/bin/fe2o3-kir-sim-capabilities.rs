#![forbid(unsafe_code)]

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let matrix = fe2o3_kir_sim::semantic_capability_matrix_v1();
    let mut stdout = std::io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &matrix).is_err() || stdout.write_all(b"\n").is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
