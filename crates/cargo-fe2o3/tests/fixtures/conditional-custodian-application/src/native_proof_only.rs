//! Actual V5/currentness/proof custody with no GPU device or runtime operation.
mod native_bootstrap;
mod native_proof_case;

fn main() {
    let result = native_proof_case::parse(std::env::args_os().skip(1)).and_then(|case| {
        native_bootstrap::run_proof_only(
            &case.producer_source,
            native_proof_case::SCHEMA,
            native_proof_case::FIELDS,
        )
    });
    if let Err(error) = result {
        eprintln!("native proof-only application refused: {error}");
        std::process::exit(1);
    }
}
