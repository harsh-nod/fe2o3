#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
#[path = "gfx950_cold_debug_v1/run.rs"]
mod run;

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
fn main() -> std::process::ExitCode {
    run::main()
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_endian = "little")))]
fn main() -> std::process::ExitCode {
    eprintln!("cold debug preparation requires Linux x86_64 little-endian");
    std::process::ExitCode::FAILURE
}
