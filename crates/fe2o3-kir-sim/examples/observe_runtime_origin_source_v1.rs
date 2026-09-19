//! Bounded acceptance observer, not a simulator or debugger wire protocol.
#[path = "runtime_origin_source_v1/run.rs"]
mod run;

fn main() -> std::process::ExitCode {
    run::main()
}
