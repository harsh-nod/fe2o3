//! Explicit read-only gfx950 artifact/device observations. Never loads a kernel.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "gfx950_checked_artifact_v1/run.rs"]
mod run;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() -> std::process::ExitCode {
    run::main()
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() -> std::process::ExitCode {
    eprintln!("gfx950 checked observations require Linux x86_64");
    std::process::ExitCode::FAILURE
}
