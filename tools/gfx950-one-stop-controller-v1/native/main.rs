//! Real MI2 controller, hard-disabled by absent source binding. Not an invocation
//! authorization. Root-only scope/closure/exclusion review is separately required.
#![deny(unsafe_code, unsafe_op_in_unsafe_fn)]
#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_endian = "little")))]
compile_error!("fixed reviewed Linux x86_64 little-endian controller only");
mod clock;
mod config;
mod custody;
mod parent;
mod profile;
mod publication;
mod scope;
mod streams;
mod wire;
use std::process::ExitCode;
fn main() -> ExitCode {
    let clock = clock::Clock::start();
    let options = match config::Options::parse(std::env::args_os().skip(1), clock) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "one-stop native controller refused before spawn: {e:?}; source profile is not enabled"
            );
            return ExitCode::FAILURE;
        }
    };
    // Output reserve is before debugger creation; no late failure allocates it.
    let buffer = match publication::Buffer::reserve() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("one-stop report reserve refused: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    let mut peer = match wire::NativePeer::spawn(&options, clock) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "one-stop debugger setup refused: {:?}; known cleanup={:?}; no whole-family claim",
                e.refusal, e.cleanup
            );
            return ExitCode::FAILURE;
        }
    };
    let result =
        fe2o3_private_one_stop_protocol::observe(&mut peer).map_err(|e| (e.refusal, e.cleanup));
    let okay = result.is_ok();
    if let Err(e) = publication::publish(buffer, &peer, &result, clock) {
        // Protocol success already closed owned handles, or its sole failure cleanup
        // already ran. Do not invoke teardown again just to manufacture a late success.
        let cleanup = match &result {
            Ok(x) => x.cleanup,
            Err((_, c)) => *c,
        };
        eprintln!(
            "one-stop raw observation publication refused: {e:?}; retained cleanup={cleanup:?}"
        );
        return ExitCode::FAILURE;
    }
    if okay {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
