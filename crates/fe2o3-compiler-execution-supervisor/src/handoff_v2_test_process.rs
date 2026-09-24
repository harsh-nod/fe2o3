//! Actual same-UID submitter/client and distinct-UID supervisor fixture roles.
use crate::authority_v2_test_process::*;
use crate::native_consuming_test_process::{
    Case as ConsumingCase, LIFECYCLE_TIMEOUT, MeasuredImage,
};
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
    os::fd::AsFd,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const UID: u32 = 65_532;

#[test]
#[ignore = "private actual submitter role for the isolated native supervisor fixture"]
fn submitter_process_helper() {
    require_child_credentials("submitter", UID);
    let control = inherited_control();
    run_submitter(control, None);
}

#[test]
#[ignore = "private submitter role for the real native consuming fixture"]
fn native_consuming_submitter_process_helper() {
    require_child_credentials("native-consuming-submitter", UID);
    let control = inherited_control();
    let (request, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&request[..4], b"NCF2");
    let case = ConsumingCase::from_id(u32::from_le_bytes(request[4..].try_into().unwrap()));
    run_submitter(control, Some(case));
}

fn run_submitter(control: std::os::fd::OwnedFd, consuming: Option<ConsumingCase>) {
    let (client_control, child_input) = pair();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "handoff_v2_test_process::client_process_helper",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_SUPERVISOR_V2_FIXTURE_ROLE", "client")
        .stdin(Stdio::from(child_input))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // Inherit the already dropped credentials; do not call setgroups as non-root.
    let mut child = ChildGuard(
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap(),
    );
    drop(command);
    let (payload, [service_peer, pidfd]) =
        receive_packet::<2>(&client_control, Instant::now() + IO_TIMEOUT).unwrap();
    let pid = child.0.id();
    assert_eq!(payload, frame(b"CLI2", pid));
    let fixture = consuming
        .is_none()
        .then(|| crate::tests::Fixture::new("handoff-submitter"));
    let issuer =
        consuming.map(|case| MeasuredImage::from_env(case.image_env()).issuer_measurement());
    let mut held_control = None;
    let mut anchor_pid = 0;
    loop {
        let (request, []) =
            receive_packet::<0>(&control, Instant::now() + Duration::from_secs(40)).unwrap();
        if request == frame(b"STOP", 0) {
            break;
        }
        if &request[..4] == b"ANC2" {
            anchor_pid = u32::from_le_bytes(request[4..].try_into().unwrap());
            continue;
        }
        assert_eq!(&request[..4], b"HOF2");
        let case = u32::from_le_bytes(request[4..].try_into().unwrap());
        let mut work = Work::new(10_000_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        let policy = if let Some(issuer) = issuer {
            assert_eq!(
                case, 0,
                "consuming fixture uses an unmodified valid handoff"
            );
            crate::authority_v2::tests::measured_policy(issuer, 7, &mut budget)
        } else {
            crate::authority_v2::tests::policy(
                fixture.as_ref().unwrap(),
                if case == 1 { 8 } else { 7 },
                &mut budget,
            )
        };
        let other_pid = (1..=4)
            .find(|candidate| ![pid, std::process::id(), anchor_pid].contains(candidate))
            .unwrap();
        let client = Client::new(
            match case {
                4 => other_pid,
                9 => anchor_pid,
                _ => pid,
            },
            UID,
            UID,
        )
        .unwrap();
        let service = Anchor::new(if case == 2 { 65_530 } else { 65_534 }, 65_534).unwrap();
        let (manifest, storage) = Manifest::new(client, service, &policy, &mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let submitter = Client::new(
            if case == 3 {
                other_pid
            } else {
                std::process::id()
            },
            UID,
            UID,
        )
        .unwrap();
        let (handoff, storage) = Handoff::new(submitter, manifest, &mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let (sent, held) = pair();
        if (11..=13).contains(&case) {
            enable_pidfd(&sent);
        }
        match case {
            5 => send_packet(
                &held,
                handoff.canonical_bytes(),
                &[pidfd.as_fd(), service_peer.as_fd()],
            )
            .unwrap(),
            6 => send_packet(
                &held,
                handoff.canonical_bytes(),
                &[service_peer.as_fd(), service_peer.as_fd()],
            )
            .unwrap(),
            7 => send_packet(&held, handoff.canonical_bytes(), &[service_peer.as_fd()]).unwrap(),
            8 => send_packet(
                &held,
                &handoff.canonical_bytes()[..1],
                &[service_peer.as_fd(), pidfd.as_fd()],
            )
            .unwrap(),
            10 => {
                let wrong_pidfd = rustix::process::pidfd_open(
                    rustix::process::getpid(),
                    rustix::process::PidfdFlags::empty(),
                )
                .unwrap();
                send_packet(
                    &held,
                    handoff.canonical_bytes(),
                    &[service_peer.as_fd(), wrong_pidfd.as_fd()],
                )
                .unwrap();
            }
            12 => send_packet(&held, handoff.canonical_bytes(), &[]).unwrap(),
            13 => send_excess_rights(&held, handoff.canonical_bytes(), &service_peer),
            14 => {
                let mut bytes = *handoff.canonical_bytes();
                bytes[0] ^= 1;
                send_packet(&held, &bytes, &[service_peer.as_fd(), pidfd.as_fd()]).unwrap();
            }
            _ => send_packet(
                &held,
                handoff.canonical_bytes(),
                &[service_peer.as_fd(), pidfd.as_fd()],
            )
            .unwrap(),
        }
        held_control = Some(held);
        send_packet(&control, &frame(b"HOF2", pid), &[sent.as_fd()]).unwrap();
        if consuming == Some(ConsumingCase::Ready) {
            let (request, []) =
                receive_packet::<0>(&control, Instant::now() + LIFECYCLE_TIMEOUT).unwrap();
            assert_eq!(&request[..4], b"PUB2");
            let issuer_pid = u32::from_le_bytes(request[4..].try_into().unwrap());
            let (published, []) = receive_sized_packet::<READY_BYTES, 0>(
                held_control.as_ref().unwrap(),
                Instant::now() + IO_TIMEOUT,
            )
            .unwrap();
            budget.reserve_storage(published.len()).unwrap();
            let (ready, delta) = Ready::decode(&published, &mut budget).unwrap();
            budget.reserve_storage(delta.additional_storage()).unwrap();
            assert!(
                ready
                    .matches_launch(issuer_pid, handoff.launch_manifest(), &policy, &mut budget)
                    .unwrap()
            );
            // Return the packet actually received from the public publication
            // path; the supervisor compares these bytes with its native owner.
            send_packet(&control, &published, &[]).unwrap();
            let (request, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
            assert_eq!(request, frame(b"FIN2", issuer_pid));
            send_packet(&client_control, &frame(b"FIN2", pid), &[]).unwrap();
            let (completed, []) =
                receive_packet::<0>(&client_control, Instant::now() + IO_TIMEOUT).unwrap();
            assert_eq!(completed, frame(b"FIN2", pid));
            send_packet(&control, &frame(b"FIN2", issuer_pid), &[]).unwrap();
        }
    }
    drop(held_control);
    send_packet(&client_control, &frame(b"STOP", pid), &[]).unwrap();
    assert!(
        child
            .wait_until(Instant::now() + IO_TIMEOUT)
            .unwrap()
            .success()
    );
    send_packet(&control, &frame(b"DONE", pid), &[]).unwrap();
}

#[test]
#[ignore = "private live client role, spawned by the isolated submitter fixture"]
fn client_process_helper() {
    require_child_credentials("client", UID);
    let control = inherited_control();
    let (service_peer, held_peer) = pair();
    let pidfd = rustix::process::pidfd_open(
        rustix::process::getpid(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap();
    let pid = std::process::id();
    send_packet(
        &control,
        &frame(b"CLI2", pid),
        &[service_peer.as_fd(), pidfd.as_fd()],
    )
    .unwrap();
    drop((service_peer, pidfd));
    let (mut request, []) =
        receive_packet::<0>(&control, Instant::now() + Duration::from_secs(45)).unwrap();
    if request == frame(b"FIN2", pid) {
        // Inert fixture stop byte, sent only after the submitter verified the
        // actual readiness publication. The issuer's service peer is fd 4.
        assert_eq!(
            rustix::net::send(
                &held_peer,
                &[0x01],
                rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
            )
            .unwrap(),
            1
        );
        send_packet(&control, &frame(b"FIN2", pid), &[]).unwrap();
        (request, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    }
    assert_eq!(request, frame(b"STOP", pid));
    drop(held_peer);
}

#[allow(unsafe_code)]
fn enable_pidfd(fd: &std::os::fd::OwnedFd) {
    use std::os::fd::AsRawFd;
    let enabled: libc::c_int = 1;
    // SAFETY: the socket is borrowed and the fixed integer input has the Linux ABI size.
    let result = unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PASSPIDFD,
            (&enabled as *const libc::c_int).cast(),
            std::mem::size_of_val(&enabled) as libc::socklen_t,
        )
    };
    assert_eq!(
        result,
        0,
        "SO_PASSPIDFD coverage requires kernel support: {}",
        std::io::Error::last_os_error()
    );
}

pub(crate) fn send_excess_rights(
    control: &std::os::fd::OwnedFd,
    payload: &[u8],
    source: &std::os::fd::OwnedFd,
) {
    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
    let rights = [source.as_fd(); 32];
    assert!(
        std::mem::size_of::<libc::cmsghdr>() + 32 * std::mem::size_of::<libc::c_int>()
            > crate::handoff_v2_io::ANCILLARY_BYTES
    );
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(32))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(SendAncillaryMessage::ScmRights(&rights)));
    assert_eq!(
        sendmsg(
            control,
            &[std::io::IoSlice::new(payload)],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL
        )
        .unwrap(),
        payload.len()
    );
}
