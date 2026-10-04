//! Explicit, disposable native-code engineering process. Not a service authority.

#![allow(unsafe_code)]

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::io::IsTerminal;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerMode {
    Default,
    ActivePoll,
    TokenProgram,
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((unique_id, mode)) = parse_args(&args) else {
        eprintln!(
            "usage: fe2o3-gfx950-engineering-worker --device-unique-id N --allow-unauthenticated-machine-code [--diagnostic-active-poll-10ms | --diagnostic-token-program-v1]"
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
    let result = unsafe {
        match mode {
            WorkerMode::ActivePoll => {
                fe2o3_kfd::run_gfx950_engineering_worker_active_poll_10ms_unchecked_v1(unique_id)
            }
            WorkerMode::TokenProgram => {
                fe2o3_kfd::run_gfx950_engineering_worker_token_program_unchecked_v1(unique_id)
            }
            WorkerMode::Default => fe2o3_kfd::run_gfx950_engineering_worker_unchecked_v1(unique_id),
        }
    };
    if let Err(error) = result {
        eprintln!("gfx950 engineering worker terminated: {error}");
        std::process::exit(1);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn parse_args(args: &[String]) -> Option<(u64, WorkerMode)> {
    let (selector, value, acknowledgement, mode) = match args {
        [selector, value, acknowledgement] => {
            (selector, value, acknowledgement, WorkerMode::Default)
        }
        [selector, value, acknowledgement, policy] if policy == "--diagnostic-active-poll-10ms" => {
            (selector, value, acknowledgement, WorkerMode::ActivePoll)
        }
        [selector, value, acknowledgement, policy] if policy == "--diagnostic-token-program-v1" => {
            (selector, value, acknowledgement, WorkerMode::TokenProgram)
        }
        _ => return None,
    };
    if selector != "--device-unique-id" || acknowledgement != "--allow-unauthenticated-machine-code"
    {
        return None;
    }
    value.parse::<u64>().ok().map(|id| (id, mode))
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[path = "../gfx950_engineering_worker_wait_tests.rs"]
mod wait_tests;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    eprintln!("gfx950 engineering worker requires Linux x86_64");
    std::process::exit(2);
}
