//! Explicit, disposable native-code engineering process. Not a service authority.

#![allow(unsafe_code)]

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::io::IsTerminal;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let unique_id = match args.as_slice() {
        [selector, value, acknowledgement]
            if selector == "--device-unique-id"
                && acknowledgement == "--allow-unauthenticated-machine-code" =>
        {
            value.parse::<u64>().ok()
        }
        _ => None,
    };
    let Some(unique_id) = unique_id else {
        eprintln!(
            "usage: fe2o3-gfx950-engineering-worker --device-unique-id N --allow-unauthenticated-machine-code"
        );
        std::process::exit(2);
    };
    if std::io::stdin().is_terminal()
        || std::io::stdout().is_terminal()
        || std::fs::read_dir("/proc/self/task").map_or(true, |tasks| tasks.take(2).count() != 1)
    {
        eprintln!(
            "engineering worker requires a dedicated single-threaded process and framed pipes"
        );
        std::process::exit(2);
    }
    // SAFETY: this executable is the explicit expert trust boundary, not a safe
    // Rust raw-launch API. Its operator opted into supplied machine code and
    // its ABI obligations. The process owns one private KFD VM, shares no Rust
    // pointers with its parent, and immediately terminates after any error.
    let result = unsafe { fe2o3_kfd::run_gfx950_engineering_worker_unchecked_v1(unique_id) };
    if let Err(error) = result {
        eprintln!("gfx950 engineering worker terminated: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    eprintln!("gfx950 engineering worker requires Linux x86_64");
    std::process::exit(2);
}
