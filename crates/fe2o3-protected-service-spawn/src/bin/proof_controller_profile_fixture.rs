//! Non-authoritative fixed-entry qualification of the complete proof-controller profile.

use fe2o3_protected_service_profile::{
    ProofControllerCredentialProfileV1, ProofControllerProcessProfileV1,
    protected_service_secure_start_address_v1,
};
use std::os::fd::{FromRawFd, OwnedFd};

fn main() {
    std::hint::black_box(protected_service_secure_start_address_v1());
    if qualify().is_err() {
        std::process::exit(98);
    }
}

#[allow(unsafe_code)]
fn qualify() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments != [std::ffi::OsString::from("fe2o3-protected-service")]
        || std::env::vars_os().next().is_some()
    {
        return Err("fixture requires fixed argv and empty environment".into());
    }
    let profile = ProofControllerProcessProfileV1::capture(
        ProofControllerCredentialProfileV1::new(61000, 61000)?,
    )?;
    let mut signal = 0;
    // SAFETY: PR_GET_PDEATHSIG writes exactly one live integer in this fixed child.
    if unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &raw mut signal, 0, 0, 0) } != 0
        || signal != libc::SIGKILL
    {
        return Err("parent-death signal changed across exec".into());
    }
    profile.revalidate_current()?;
    // SAFETY: the isolated root fixture installs the one inherited report endpoint at FD3.
    let report = unsafe { OwnedFd::from_raw_fd(3) };
    rustix::io::fcntl_setfd(&report, rustix::io::FdFlags::CLOEXEC)?;
    if rustix::io::write(&report, &[signal as u8])? != 1 {
        return Err("short fixture report".into());
    }
    loop {
        // SAFETY: do not consume the parent's release packet or depend on endpoint EOF.
        unsafe {
            libc::pause();
        }
    }
}
