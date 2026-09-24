//! Opt-in, isolated process scaffolding for the public native consuming path.
//! Register this module with `#[cfg(test)] mod native_consuming_test_process;`.
#![allow(unsafe_code)]

use crate::authority_v2_test_process::{
    ChildGuard, IO_TIMEOUT, frame, inherited_control, pair, receive_packet,
    require_child_credentials, send_packet, spawn_role,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
use fe2o3_protected_service_profile::PROTECTED_SERVICE_SECUREBITS_V1;
use fe2o3_protected_static_executable::ProtectedStaticExecutableMeasurementV1 as Measurement;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek},
    os::{
        fd::{AsFd, OwnedFd},
        unix::{
            fs::{MetadataExt, OpenOptionsExt},
            process::CommandExt,
        },
    },
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const OPT_IN: &str = "FE2O3_RUN_NATIVE_CONSUMING_SUPERVISOR_V2_TEST";
const CASE: &str = "FE2O3_NATIVE_CONSUMING_FIXTURE_CASE";
const ROLE: &str = "FE2O3_SUPERVISOR_V2_FIXTURE_ROLE";
const SUPERVISOR_UID: u32 = 65_533;
const CHILD_TIMEOUT: Duration = Duration::from_secs(45);
// Debug-mode custody repeatedly measures full static images on a shared CPU.
// These positive stages need more time than a single fixture control packet.
pub(crate) const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_IMAGE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Case {
    Ready,
    MissingEof,
    Trailing,
    DropBeforeReady,
}

impl Case {
    pub(crate) fn from_id(id: u32) -> Self {
        match id {
            0 => Self::Ready,
            1 => Self::MissingEof,
            2 => Self::Trailing,
            3 => Self::DropBeforeReady,
            // fe2o3-hygiene: allow-panic -- invalid private test coordinator case.
            _ => panic!("unknown native consuming fixture case {id}"),
        }
    }

    pub(crate) fn id(self) -> u32 {
        match self {
            Self::Ready => 0,
            Self::MissingEof => 1,
            Self::Trailing => 2,
            Self::DropBeforeReady => 3,
        }
    }

    pub(crate) fn image_env(self) -> &'static str {
        match self {
            Self::Ready => "FE2O3_NATIVE_READY_FIXTURE_READY",
            Self::MissingEof => "FE2O3_NATIVE_READY_FIXTURE_NO_EOF",
            Self::Trailing => "FE2O3_NATIVE_READY_FIXTURE_TRAILING",
            Self::DropBeforeReady => "FE2O3_NATIVE_READY_FIXTURE_SILENT",
        }
    }
}

pub(crate) struct MeasuredImage {
    pub(crate) file: File,
    pub(crate) measurement: Measurement,
}

impl MeasuredImage {
    /// Trusted fixture input measurement; native provisioning performs admission
    /// and seals the actual images on the original request ledger afterward.
    pub(crate) fn from_env(name: &str) -> Self {
        // fe2o3-hygiene: allow-panic -- missing opt-in test artifact is a fixture failure.
        let path = std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required"));
        assert!(Path::new(&path).is_absolute(), "{name} must be absolute");
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            // fe2o3-hygiene: allow-panic -- inaccessible supplied test artifact must fail.
            .unwrap_or_else(|error| panic!("open {name}={path:?}: {error}"));
        let metadata = file.metadata().unwrap();
        assert!(metadata.is_file());
        assert!((1..=MAX_IMAGE_BYTES).contains(&metadata.len()));
        assert_eq!(metadata.mode() & 0o6022, 0, "fixture image permissions");
        assert!(
            metadata.mode() & 0o111 != 0,
            "fixture image must be executable"
        );
        assert!(
            ![65_532, 65_533, 65_534].contains(&metadata.uid()),
            "fixture roles must not own the supplied executable"
        );
        let mut digest = Sha256::new();
        let mut bytes = [0; 64 * 1024];
        let mut read = 0_u64;
        loop {
            let count = file.read(&mut bytes).unwrap();
            if count == 0 {
                break;
            }
            read += count as u64;
            assert!(read <= MAX_IMAGE_BYTES);
            digest.update(&bytes[..count]);
        }
        assert_eq!(
            read,
            metadata.len(),
            "fixture image changed while measuring"
        );
        file.rewind().unwrap();
        let measurement =
            Measurement::new(digest.finalize().into(), read, MAX_IMAGE_BYTES).unwrap();
        eprintln!("native fixture image {name}: {measurement:?}");
        Self { file, measurement }
    }

    pub(crate) fn issuer_measurement(&self) -> CompilerExecutionIssuerMeasurementV1 {
        CompilerExecutionIssuerMeasurementV1::new(
            self.measurement.sha256(),
            self.measurement.byte_len(),
        )
        .unwrap()
    }
}

#[test]
#[ignore = "opt-in isolated root container; real static native readiness and publication"]
fn native_consuming_ready() {
    coordinate(Case::Ready);
}

#[test]
#[ignore = "opt-in isolated root container; native readiness without private EOF"]
fn native_consuming_missing_eof() {
    coordinate(Case::MissingEof);
}

#[test]
#[ignore = "opt-in isolated root container; native readiness with trailing data"]
fn native_consuming_trailing() {
    coordinate(Case::Trailing);
}

#[test]
#[ignore = "opt-in isolated root container; drop live native custody before readiness"]
fn native_consuming_drop_before_ready() {
    coordinate(Case::DropBeforeReady);
}

fn coordinate(case: Case) {
    assert_eq!(std::env::var(OPT_IN).as_deref(), Ok("1"));
    assert_eq!(rustix::process::getuid().as_raw(), 0);
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(rustix::process::getegid().as_raw(), 0);
    assert!(
        Path::new("/.dockerenv").is_file(),
        "isolated Docker fixture only"
    );
    let parent_securebits = rustix::thread::capabilities_secure_bits().unwrap();
    let parent_capabilities = rustix::thread::capabilities(None).unwrap();
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let caps = status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:\t"))
        .unwrap();
    let caps = u64::from_str_radix(caps, 16).unwrap();
    let required = (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8);
    assert_eq!(
        caps & required,
        required,
        "requires KILL/SETGID/SETUID/SETPCAP"
    );
    let cap_last: u32 = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(cap_last <= 63);
    // Fail on missing inputs before starting any persistent fixture role.
    drop(MeasuredImage::from_env("FE2O3_STATIC_PREEXEC_LAUNCHER"));
    drop(MeasuredImage::from_env(case.image_env()));

    let (anchor_control, child_control) = pair();
    let mut anchor = spawn_role(
        "authority_v2_test_process::anchor_process_helper",
        "anchor",
        65_534,
        child_control,
    );
    let anchor_pid = anchor.0.id();
    let process = rustix::process::Pid::from_raw(i32::try_from(anchor_pid).unwrap()).unwrap();
    let pidfd = rustix::process::pidfd_open(process, rustix::process::PidfdFlags::empty()).unwrap();
    let (payload, [peer]) =
        receive_packet::<1>(&anchor_control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(payload, frame(b"ANC2", anchor_pid));

    let (submitter_control, child_control) = pair();
    let mut submitter = spawn_role(
        "handoff_v2_test_process::native_consuming_submitter_process_helper",
        "native-consuming-submitter",
        65_532,
        child_control,
    );
    send_packet(&submitter_control, &frame(b"NCF2", case.id()), &[]).unwrap();
    let (supervisor_control, child_control) = pair();
    let mut supervisor = spawn_locked_supervisor(case, cap_last, child_control);
    assert_eq!(
        rustix::thread::capabilities_secure_bits().unwrap(),
        parent_securebits
    );
    assert_eq!(
        rustix::thread::capabilities(None).unwrap(),
        parent_capabilities
    );
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let deadline = Instant::now() + CHILD_TIMEOUT;
    send_packet(
        &supervisor_control,
        &payload,
        &[peer.as_fd(), pidfd.as_fd(), submitter_control.as_fd()],
    )
    .unwrap();
    drop((peer, pidfd, submitter_control));
    let (completed, []) = receive_packet::<0>(&supervisor_control, deadline).unwrap();
    assert_eq!(completed, frame(b"DONE", anchor_pid));
    assert!(supervisor.wait_until(deadline).unwrap().success());
    assert!(submitter.wait_until(deadline).unwrap().success());
    send_packet(&anchor_control, &frame(b"STOP", anchor_pid), &[]).unwrap();
    assert!(
        anchor
            .wait_until(Instant::now() + IO_TIMEOUT)
            .unwrap()
            .success()
    );
    eprintln!("native consuming {case:?}: verified completion packet and all role exits");
}

fn spawn_locked_supervisor(case: Case, cap_last: u32, control: OwnedFd) -> ChildGuard {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "native_consuming_test_process::locked_supervisor_process_helper",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROLE, "native-consuming-supervisor")
        .env(CASE, case.id().to_string())
        .env_remove(OPT_IN)
        .env_remove("FE2O3_RUN_PRIVILEGED_SUPERVISOR_V2_TEST")
        .env("TMPDIR", "/tmp")
        .current_dir("/tmp")
        .stdin(Stdio::from(control))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // SAFETY: the private pre-exec child uses fixed syscall inputs only. Do not
    // add CommandExt::uid/gid: installation still requires root capabilities.
    unsafe {
        command.pre_exec(move || install_profile(cap_last));
    }
    let child =
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap();
    drop(command);
    ChildGuard(child)
}

fn install_profile(cap_last: u32) -> io::Result<()> {
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )?;
    // SAFETY: scalar-only process-local syscall.
    unsafe {
        libc::umask(0o077);
    }
    rustix::thread::set_capabilities_secure_bits(
        rustix::thread::CapabilitiesSecureBits::from_bits_retain(PROTECTED_SERVICE_SECUREBITS_V1),
    )?;
    // SAFETY: fixed scalar prctl operations only remove capabilities.
    if unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    for capability in 0..=cap_last {
        if unsafe { libc::prctl(libc::PR_CAPBSET_DROP, capability, 0, 0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: no supplementary group array; fixed real/effective/saved IDs.
    if unsafe { libc::setgroups(0, std::ptr::null()) } != 0
        || unsafe { libc::setresgid(SUPERVISOR_UID, SUPERVISOR_UID, SUPERVISOR_UID) } != 0
        || unsafe { libc::setresuid(SUPERVISOR_UID, SUPERVISOR_UID, SUPERVISOR_UID) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    rustix::thread::set_capabilities(
        None,
        rustix::thread::CapabilitySets {
            effective: rustix::thread::CapabilitySet::empty(),
            permitted: rustix::thread::CapabilitySet::empty(),
            inheritable: rustix::thread::CapabilitySet::empty(),
        },
    )?;
    rustix::thread::set_no_new_privs(true)?;
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)?;
    Ok(())
}

#[test]
#[ignore = "private locked supervisor role, selected only by the consuming coordinator"]
fn locked_supervisor_process_helper() {
    // Dynamic libtest exec resets dumpability. This is test bootstrap, not
    // evidence of the static issuer's secure entry or a native profile bypass.
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable).unwrap();
    require_child_credentials("native-consuming-supervisor", SUPERVISOR_UID);
    let case = Case::from_id(std::env::var(CASE).unwrap().parse().unwrap());
    let control = inherited_control();
    let (payload, [peer, pidfd, submitter]) =
        receive_packet::<3>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&payload[..4], b"ANC2");
    let anchor_pid = u32::from_le_bytes(payload[4..].try_into().unwrap());
    assert_ne!(anchor_pid, 0);
    crate::launch_v2::tests::exercise_consuming(case, &peer, &pidfd, &submitter);
    send_packet(&submitter, &frame(b"STOP", 0), &[]).unwrap();
    let (completed, []) = receive_packet::<0>(&submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&completed[..4], b"DONE");
    send_packet(&control, &frame(b"DONE", anchor_pid), &[]).unwrap();
}
