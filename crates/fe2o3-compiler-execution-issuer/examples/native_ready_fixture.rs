//! Synthetic native launch/readiness fixture, never a production issuer.
//!
//! Build with scripts/build-native-ready-fixture.sh. The artifact fixes the
//! mode; the child reads neither argv nor environment. Default example builds
//! select ready. All modes admit native inputs and the exact 65533/65533 profile.
//! No signing key, durable state, compiler request, or recovery API is used.
//!
//! ready: emit ReadyV2, close every writer, then await cancellation or one exact
//! service-peer packet containing the single inert stop byte 0x01. The fixture
//! client must send it only after publication, to exercise Serving -> Exited.
//! no-eof: emit ReadyV2 and retain fd9. trailing: emit ReadyV2 plus a zero byte,
//! then close fd9. silent: retain fd9 without writing. Negative modes only wait
//! for cancellation. Every wait has a 25-second deadline and finite attempts;
//! expiry is failure, never evidence of successful readiness or cancellation.

use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV2 as PolicyCapability,
    CompilerExecutionServiceLaunchCapabilityV2 as LaunchCapability,
};
use fe2o3_compiler_execution_issuer::{
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 as MANIFEST_FD,
    COMPILER_EXECUTION_ISSUER_PEER_FD_V1 as PEER_FD,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 as POLICY_FD,
    COMPILER_EXECUTION_ISSUER_READY_FD_V1 as READY_FD,
    CompilerExecutionIssuerLaunchInputsV2 as Inputs,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as MANIFEST_WORK,
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V2 as READY_WORK,
    CompilerExecutionServiceReadyV2 as Ready,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Profile,
    REQUIRE_OWNED_SIGCHLD_WORK_V2, protected_service_secure_start_address_v1,
    require_owned_sigchld_v2,
};
use std::{
    os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
    process::ExitCode,
    time::{Duration, Instant},
};

#[path = "native_ready_fixture/io.rs"]
mod io;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Ready,
    NoEof,
    Trailing,
    Silent,
}

// Cargo's ordinary example discovery needs no manifest or feature changes.
// The dedicated script passes --check-cfg and exactly one of these values.
#[allow(unexpected_cfgs)]
const MODE: Mode = {
    let selected = [
        cfg!(fe2o3_native_ready_fixture = "ready"),
        cfg!(fe2o3_native_ready_fixture = "no-eof"),
        cfg!(fe2o3_native_ready_fixture = "trailing"),
        cfg!(fe2o3_native_ready_fixture = "silent"),
    ];
    assert!(selected[0] as u8 + selected[1] as u8 + selected[2] as u8 + selected[3] as u8 <= 1);
    if selected[1] {
        Mode::NoEof
    } else if selected[2] {
        Mode::Trailing
    } else if selected[3] {
        Mode::Silent
    } else {
        Mode::Ready
    }
};

const SERVICE_ID: u32 = 65_533;
const SOURCE_READY_FD: RawFd = 209;
const WRITE_ATTEMPTS: usize = 32;
const WAIT_ATTEMPTS: usize = 512;
// Fixed I/O census <= 44: unused-FD closure <= 20 (1 range + 8 close/check
// pairs), writer/peer admission 6+4, input source closes 4, getpid 1, source209
// checks 2, writer close/check 2, peer/input drops 1+2, initial clock reads 2.
// Each loop attempt needs at most four more operations, including its clock.
const FIXED_IO_ATTEMPTS: usize = 64;
const _: () = assert!(20 + 6 + 4 + 4 + 1 + 2 + 2 + 1 + 2 + 2 <= FIXED_IO_ATTEMPTS);
const IO_WORK: usize = 1024 * (FIXED_IO_ATTEMPTS + 4 * WRITE_ATTEMPTS + 4 * WAIT_ATTEMPTS);
// Fixed descriptor owners, syscall structs, packets and control/error scratch.
// These are logical charges, not bounds on Rust startup, RSS, or generated stack.
const IO_STORAGE: usize = 4096 + 2 * size_of::<libc::stat>() + READY_BYTES + 1;
const INPUT_WORK: usize = 8
    + 2 * PolicyCapability::IO_WORK
    + 2 * LaunchCapability::IO_WORK
    + POLICY_WORK
    + 2 * MANIFEST_WORK;
const TOTAL_WORK: usize = IO_WORK
    + Profile::CAPTURE_WORK
    + Profile::REVALIDATE_CURRENT_WORK
    + Namespaces::CAPTURE_WORK
    + Namespaces::REVALIDATE_SELF_WORK
    + REQUIRE_OWNED_SIGCHLD_WORK_V2
    + INPUT_WORK
    + 2 * READY_WORK;
const _: () = assert!(READY_BYTES == 120);

struct Refused;
type Result<T> = std::result::Result<T, Refused>;
impl From<Resource> for Refused {
    fn from(_: Resource) -> Self {
        Self
    }
}

fn require(condition: bool) -> Result<()> {
    if condition { Ok(()) } else { Err(Refused) }
}

fn main() -> ExitCode {
    std::hint::black_box(protected_service_secure_start_address_v1());
    let mut work = Work::new(TOTAL_WORK);
    let mut budget = Budget::new(&mut work, 1_000_000);
    // Prepay every raw syscall attempt, cleanup and fixed I/O frame before I/O.
    // Native API work is charged separately on this same, never replaced ledger.
    let result = budget.with_prepaid_scope(0, 8, IO_WORK, IO_STORAGE, run);
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(budget: &mut Budget<'_>) -> Result<()> {
    // Check before any capture or duplication could reuse the launcher source.
    io::require_closed(SOURCE_READY_FD)?;
    io::close_unused()?;
    let writer = io::take_writer()?;
    let peer = io::take_peer()?;

    let credentials = Credentials::new(SERVICE_ID, SERVICE_ID).map_err(|_| Refused)?;
    let (profile, storage) = Profile::capture(credentials, budget).map_err(|_| Refused)?;
    budget.reserve_storage(storage.additional_storage())?;
    require_owned_sigchld_v2(budget).map_err(|_| Refused)?;
    let (namespaces, storage) = Namespaces::capture_self(budget).map_err(|_| Refused)?;
    budget.reserve_storage(storage.additional_storage())?;

    budget.reserve_storage(Inputs::INPUT_STORAGE)?;
    let (inputs, storage) = Inputs::from_inherited(budget).map_err(|_| Refused)?;
    budget.reserve_storage(storage.additional_storage())?;
    io::close_checked(POLICY_FD)?;
    io::close_checked(MANIFEST_FD)?;
    budget.release_storage(Inputs::INPUT_STORAGE)?;

    let pid = std::process::id();
    let (ready, storage) =
        Ready::new(pid, inputs.manifest(), inputs.policy(), budget).map_err(|_| Refused)?;
    budget.reserve_storage(storage.additional_storage())?;
    require(
        ready
            .matches_launch(pid, inputs.manifest(), inputs.policy(), budget)
            .map_err(|_| Refused)?,
    )?;
    profile.revalidate_current(budget).map_err(|_| Refused)?;
    namespaces.revalidate_self(budget).map_err(|_| Refused)?;
    io::require_closed(SOURCE_READY_FD)?;

    let mut packet = [0; READY_BYTES + 1];
    packet[..READY_BYTES].copy_from_slice(ready.canonical_bytes());
    if MODE != Mode::Silent {
        let length = READY_BYTES + usize::from(MODE == Mode::Trailing);
        io::write_packet(&writer, &packet[..length])?;
    }
    // No duplication of fd9 occurred. All other inherited descriptors were
    // closed or independently admitted as sockets/sealed regular files.
    let held_writer = if matches!(MODE, Mode::Ready | Mode::Trailing) {
        io::close_checked(writer.into_raw_fd())?;
        None
    } else {
        Some(writer)
    };
    let outcome = io::wait_for_finish(&peer, MODE == Mode::Ready);
    drop((held_writer, peer, ready, inputs, namespaces, profile));
    // with_prepaid_scope retires all remaining charges after these owners drop.
    outcome
}
