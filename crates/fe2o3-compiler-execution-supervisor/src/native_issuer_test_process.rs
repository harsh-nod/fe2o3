//! Opt-in protected supervisor -> real inherited native binary -> readiness/client.
//! No observed compiler subject, issuance, external anchor signature or GPU credit.
use super::{MeasuredImage, spawn_locked_role};
use crate::authority_v2_test_process::{
    ChildGuard, IO_TIMEOUT, frame, inherited_control, pair, receive_packet, receive_sized_packet,
    require_child_credentials, send_packet, spawn_role,
};
use fe2o3_compiler_execution_client::CompilerExecutionClientV2 as WireClient;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionServiceLaunchManifestV2 as Manifest, CompilerExecutionServiceReadyV2 as Ready,
    CompilerExecutionSupervisorHandoffV2 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    os::fd::{AsFd, OwnedFd},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[path = "native_issuer_supervisor_tests.rs"]
mod supervisor;

const OPT_IN: &str = "FE2O3_RUN_NATIVE_ISSUER_SUPERVISOR_TEST";
const CASE_ENV: &str = "FE2O3_NATIVE_ISSUER_SUPERVISOR_CASE";
const PREFIX: &str = "native_consuming_test_process::native_issuer::";
const ROLE_ENV: &str = "FE2O3_SUPERVISOR_V2_FIXTURE_ROLE";
const BOUND: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum Case {
    Native,
    Legacy,
    Corrupt,
}
impl Case {
    fn id(self) -> u32 {
        match self {
            Self::Native => 0,
            Self::Legacy => 1,
            Self::Corrupt => 2,
        }
    }
    fn parse(id: u32) -> Self {
        match id {
            0 => Self::Native,
            1 => Self::Legacy,
            2 => Self::Corrupt,
            // fe2o3-hygiene: allow-panic -- private fixture role must have a known case.
            _ => panic!("invalid native issuer fixture case"),
        }
    }
    fn image(self) -> &'static str {
        if self == Self::Legacy {
            "FE2O3_STATIC_COMPILER_EXECUTION_ISSUER"
        } else {
            "FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE"
        }
    }
}

#[test]
#[ignore = "isolated root container and real packaged native issuer/launcher required"]
fn inherited_native_readiness_and_client_cancel() {
    coordinate(Case::Native);
}
#[test]
#[ignore = "isolated root container, native launcher and separately built V1 image required"]
fn legacy_image_cannot_serve_native_launch() {
    coordinate(Case::Legacy);
}
#[test]
#[ignore = "isolated root container and real packaged native issuer/launcher required"]
fn corrupt_native_journal_prevents_readiness() {
    coordinate(Case::Corrupt);
}

fn coordinate(case: Case) {
    assert_eq!(std::env::var(OPT_IN).as_deref(), Ok("1"));
    assert!(
        Path::new("/.dockerenv").is_file(),
        "disposable container only"
    );
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let before = rustix::thread::capabilities(None).unwrap();
    let bits = rustix::thread::capabilities_secure_bits().unwrap();
    let last: u32 = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(last <= 63);
    drop(MeasuredImage::from_env("FE2O3_STATIC_PREEXEC_LAUNCHER"));
    drop(MeasuredImage::from_env(case.image()));
    let (anchor_control, child) = pair();
    let mut anchor = spawn_role(
        "authority_v2_test_process::anchor_process_helper",
        "anchor",
        65_534,
        child,
    );
    let (packet, [peer]) =
        receive_packet::<1>(&anchor_control, Instant::now() + IO_TIMEOUT).unwrap();
    let anchor_pid = anchor.0.id();
    assert_eq!(packet, frame(b"ANC2", anchor_pid));
    let pidfd = rustix::process::pidfd_open(
        rustix::process::Pid::from_raw(i32::try_from(anchor_pid).unwrap()).unwrap(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap();
    let (submitter_control, child) = pair();
    let mut submitter = spawn_role(
        &format!("{PREFIX}submitter_role"),
        "native-issuer-submitter",
        65_532,
        child,
    );
    send_packet(&submitter_control, &frame(b"NIS2", case.id()), &[]).unwrap();
    let (control, child) = pair();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &format!("{PREFIX}supervisor_role"),
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROLE_ENV, "native-issuer-supervisor")
        .env(CASE_ENV, case.id().to_string())
        .env_remove(OPT_IN);
    let mut supervisor = spawn_locked_role(command, last, child);
    assert_eq!(rustix::thread::capabilities(None).unwrap(), before);
    assert_eq!(rustix::thread::capabilities_secure_bits().unwrap(), bits);
    send_packet(
        &control,
        &packet,
        &[peer.as_fd(), pidfd.as_fd(), submitter_control.as_fd()],
    )
    .unwrap();
    drop((peer, pidfd, submitter_control));
    let deadline = Instant::now() + Duration::from_secs(45);
    let (done, []) = receive_packet::<0>(&control, deadline).unwrap();
    assert_eq!(done, frame(b"DONE", anchor_pid));
    assert!(supervisor.wait_until(deadline).unwrap().success());
    assert!(submitter.wait_until(deadline).unwrap().success());
    send_packet(&anchor_control, &frame(b"STOP", anchor_pid), &[]).unwrap();
    assert!(
        anchor
            .wait_until(Instant::now() + IO_TIMEOUT)
            .unwrap()
            .success()
    );
    eprintln!("FE2O3_NATIVE_INHERITED_TRANSCRIPT_OK case={case:?}; all roles exited");
}

#[test]
#[ignore = "private locked supervisor child; coordinator only"]
fn supervisor_role() {
    // Dynamic libtest bootstrap only; actual issuer uses the secure static entry.
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable).unwrap();
    require_child_credentials("native-issuer-supervisor", 65_533);
    let case = Case::parse(std::env::var(CASE_ENV).unwrap().parse().unwrap());
    let control = inherited_control();
    let (packet, [peer, pidfd, submitter]) =
        receive_packet::<3>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&packet[..4], b"ANC2");
    supervisor::exercise(case, &peer, &pidfd, &submitter);
    send_packet(&submitter, &frame(b"STOP", 0), &[]).unwrap();
    let (done, []) = receive_packet::<0>(&submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&done[..4], b"DONE");
    send_packet(
        &control,
        &frame(b"DONE", u32::from_le_bytes(packet[4..].try_into().unwrap())),
        &[],
    )
    .unwrap();
}

#[test]
#[ignore = "private distinct-UID submitter child; coordinator only"]
fn submitter_role() {
    require_child_credentials("native-issuer-submitter", 65_532);
    let control = inherited_control();
    let (packet, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&packet[..4], b"NIS2");
    let case = Case::parse(u32::from_le_bytes(packet[4..].try_into().unwrap()));
    let (client_control, child_input) = pair();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &format!("{PREFIX}client_role"),
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROLE_ENV, "native-issuer-client")
        .stdin(Stdio::from(child_input))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = ChildGuard(
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap(),
    );
    drop(command);
    send_packet(&client_control, &frame(b"NIS2", case.id()), &[]).unwrap();
    let (packet, [peer, pidfd]) =
        receive_packet::<2>(&client_control, Instant::now() + IO_TIMEOUT).unwrap();
    let pid = child.0.id();
    assert_eq!(packet, frame(b"CLI2", pid));
    let mut work = Work::new(10_000_000_000);
    let mut b = Budget::new(&mut work, 10_000_000);
    let policy = crate::authority_v2::tests::measured_policy(
        MeasuredImage::from_env(case.image()).issuer_measurement(),
        7,
        &mut b,
    );
    let (m, charge) = Manifest::new(
        Client::new(pid, 65_532, 65_532).unwrap(),
        Anchor::new(65_534, 65_534).unwrap(),
        &policy,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (handoff, charge) = Handoff::new(
        Client::new(std::process::id(), 65_532, 65_532).unwrap(),
        m,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (sent, held) = pair();
    send_packet(
        &held,
        handoff.canonical_bytes(),
        &[peer.as_fd(), pidfd.as_fd()],
    )
    .unwrap();
    send_packet(&control, &frame(b"HOF2", pid), &[sent.as_fd()]).unwrap();
    drop((sent, peer, pidfd));
    let (step, []) = receive_packet::<0>(&control, Instant::now() + BOUND).unwrap();
    if case == Case::Native {
        assert_eq!(&step[..4], b"PUB2");
        let issuer_pid = u32::from_le_bytes(step[4..].try_into().unwrap());
        let (bytes, []) =
            receive_sized_packet::<READY_BYTES, 0>(&held, Instant::now() + IO_TIMEOUT).unwrap();
        b.reserve_storage(bytes.len()).unwrap();
        let (ready, charge) = Ready::decode(&bytes, &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(
            ready
                .matches_launch(issuer_pid, handoff.launch_manifest(), &policy, &mut b)
                .unwrap()
        );
        require_eof(&held);
        send_packet(&control, &bytes, &[]).unwrap();
        let (step, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
        assert_eq!(step, frame(b"FIN2", issuer_pid));
        send_packet(&client_control, &frame(b"FIN2", pid), &[]).unwrap();
        let (done, []) = receive_packet::<0>(&client_control, Instant::now() + BOUND).unwrap();
        assert_eq!(done, frame(b"FIN2", pid));
        send_packet(&control, &frame(b"FIN2", issuer_pid), &[]).unwrap();
    } else {
        assert_eq!(step, frame(b"REF2", case.id()));
        require_eof(&held);
    }
    let (stop, []) = receive_packet::<0>(&control, Instant::now() + BOUND).unwrap();
    assert_eq!(stop, frame(b"STOP", 0));
    send_packet(&client_control, &frame(b"STOP", pid), &[]).unwrap();
    assert!(
        child
            .wait_until(Instant::now() + IO_TIMEOUT)
            .unwrap()
            .success()
    );
    send_packet(&control, &frame(b"DONE", pid), &[]).unwrap();
}

fn require_eof(peer: &OwnedFd) {
    let deadline = Instant::now() + IO_TIMEOUT;
    for _ in 0..1024 {
        match rustix::net::recv(peer, &mut [0; 1][..], rustix::net::RecvFlags::DONTWAIT) {
            Ok((_, n)) => {
                assert_eq!(n, 0, "unexpected readiness/control bytes");
                return;
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => (),
            Err(e) => {
                assert!(false, "control EOF: {e}");
            }
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(false, "readiness/control EOF exceeded its bound");
}

#[test]
#[ignore = "private live distinct-UID client child; submitter only"]
fn client_role() {
    require_child_credentials("native-issuer-client", 65_532);
    let control = inherited_control();
    let (case, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&case[..4], b"NIS2");
    let case = Case::parse(u32::from_le_bytes(case[4..].try_into().unwrap()));
    let (peer, held) = pair();
    let pid = std::process::id();
    let pidfd = rustix::process::pidfd_open(
        rustix::process::getpid(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap();
    send_packet(
        &control,
        &frame(b"CLI2", pid),
        &[peer.as_fd(), pidfd.as_fd()],
    )
    .unwrap();
    drop((peer, pidfd));
    let (mut step, []) = receive_packet::<0>(&control, Instant::now() + BOUND).unwrap();
    if case == Case::Native {
        assert_eq!(step, frame(b"FIN2", pid));
        let mut work = Work::new(10_000_000_000);
        let mut b = Budget::new(&mut work, 10_000_000);
        let policy = crate::authority_v2::tests::measured_policy(
            MeasuredImage::from_env(case.image()).issuer_measurement(),
            7,
            &mut b,
        );
        b.reserve_storage(WireClient::PEER_STORAGE).unwrap();
        WireClient::admit(held, BOUND, &mut b)
            .unwrap()
            .cancel(&policy)
            .unwrap();
        send_packet(&control, &frame(b"FIN2", pid), &[]).unwrap();
        (step, []) = receive_packet::<0>(&control, Instant::now() + BOUND).unwrap();
    } else {
        drop(held);
    }
    assert_eq!(step, frame(b"STOP", pid));
}
