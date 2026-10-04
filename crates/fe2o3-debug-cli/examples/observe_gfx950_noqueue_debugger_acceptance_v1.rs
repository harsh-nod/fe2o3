//! Engineering-only, launch-owned no-queue debugger observation.
//! Does not dispatch, sample GPU registers, attach to an arbitrary PID, or mint
//! debugger acceptance. See the reviewed supervisor contract in the donor.
#![deny(unsafe_code, unsafe_op_in_unsafe_fn)]

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
#[path = "gfx950_noqueue_debugger_acceptance_v1/config.rs"]
mod config;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
#[path = "gfx950_noqueue_debugger_acceptance_v1/custody.rs"]
mod custody;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
#[path = "../src/rocgdb_noqueue_acceptance_v1/parser.rs"]
mod parser;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
#[path = "gfx950_noqueue_debugger_acceptance_v1/protocol.rs"]
mod protocol;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
#[path = "../src/rocgdb_mi_parser_v3.rs"]
mod rocgdb_mi_parser_v3;
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
#[path = "gfx950_noqueue_debugger_acceptance_v1/wire.rs"]
mod wire;

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_endian = "little"))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let options = match config::Options::parse(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(reason) => {
            eprintln!(
                "closed no-queue debugger qualification refused before spawn: {reason:?}\n{}",
                config::USAGE
            );
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut peer = match wire::Native::spawn(&options) {
        Ok(peer) => peer,
        Err(reason) => {
            eprintln!("debugger setup refused; no observer continuation attempted: {reason:?}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let Some(observer) = options.observer.path.to_str() else {
        return std::process::ExitCode::FAILURE;
    };
    let result = protocol::run(&mut peer, observer, &options.arguments);
    let okay = result.is_ok();
    let value = match result {
        Ok(observation) => serde_json::json!({
            "schema":"diagnostic-gfx950-noqueue-debugger-object-observation-v1",
            "status":"same_host_stop_code_object_and_original_elf_matched",
            "observation":observation,
        }),
        Err(reason) => {
            let cleanup = peer.cleanup();
            serde_json::json!({
                "schema":"diagnostic-gfx950-noqueue-debugger-object-observation-v1",
                "status":"refused","reason":format!("{reason:?}"),
                "native_effects":"possible_in_owned_inferior_no_rollback_claim",
                "cleanup":cleanup,
                "debugger_acceptance_observed":false,"physical_register_capture":false,
            })
        }
    };
    let value = serde_json::json!({
        "result":value,
        "pins":{"debugger_sha256":config::hex(&options.debugger.sha256),
            "observer_sha256":config::hex(&options.observer.sha256),"artifact_sha256":config::hex(&options.artifact.sha256)},
        "transcript":peer.transcript(),
        "limits":{"runtime_loaded_success_producer":"unavailable",
            "foreign_runtime_lifetime_exclusion":"external_reviewed_supervisor_contract",
            "dynamic_loader_closure":"external_pin_and_no_mutation_contract",
            "inferior_parent":"owned_debugger_not_controller",
            "queue_and_gpu_execution":"not_implemented",
            "same_stop_gpu_physical_state":"not_implemented"},
    });
    let Ok(mut bytes) = serde_json::to_vec(&value) else {
        return std::process::ExitCode::FAILURE;
    };
    // Two retained streams total <=8MiB, hex <=16MiB, commands <=128KiB.
    if bytes.len() > 18 * 1024 * 1024 {
        return std::process::ExitCode::FAILURE;
    }
    bytes.push(b'\n');
    if std::io::stdout().lock().write_all(&bytes).is_err() {
        return std::process::ExitCode::FAILURE;
    }
    if okay {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_endian = "little")))]
fn main() -> std::process::ExitCode {
    eprintln!("requires Linux x86_64 little-endian");
    std::process::ExitCode::FAILURE
}

#[cfg(all(
    test,
    target_os = "linux",
    target_arch = "x86_64",
    target_endian = "little"
))]
#[path = "gfx950_noqueue_debugger_acceptance_v1/tests.rs"]
mod tests;

// The native producer's replacement ELF is not yet built/pinned/qualified.
// Staged join implementation is exercised only by CPU fixtures until a
// separately reviewed fixed-executable route is allocated.
#[cfg(all(
    test,
    target_os = "linux",
    target_arch = "x86_64",
    target_endian = "little"
))]
#[path = "gfx950_noqueue_debugger_acceptance_v1/native_runtime_events_v1.rs"]
mod native_runtime_events_v1;
