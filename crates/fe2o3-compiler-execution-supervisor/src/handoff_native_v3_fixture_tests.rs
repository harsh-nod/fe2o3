//! Opt-in custody fixture only: no issuer launch, readiness, signing or activation.
use super::*;
use crate::authority_v2_test_process::*;
use crate::program_v3::tests::{SEED, new_program, policy, revalidation_work};
use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceAdmissionV2 as Anchor;
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Key;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as MANIFEST_WORK,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V3 as FRAME_WORK,
    CompilerExecutionExternalAnchorServiceIdentityV1 as AnchorIdentity,
    sealed_static_issuer_runtime_measurement_v1,
};
use std::{
    fs::File,
    process::{Command, Stdio},
};

const WORK: usize = 10_000_000_000;
const STORAGE: usize = 10_000_000;
const PREFIX: &str = "handoff_v3::tests::fixture::";
const OPT_IN: &str = "FE2O3_RUN_PRIVILEGED_HANDOFF_V3_TEST";

// This private, single-test child has no concurrent descriptor-producing work.
// The directory iterator contributes the same one descriptor to every census.
fn open_descriptor_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .map(|entry| entry.unwrap())
        .count()
}

pub(crate) fn supervisor(
    peer: &OwnedFd,
    pidfd: &OwnedFd,
    b: &mut Budget<'_>,
) -> (crate::tests::Fixture, Supervisor) {
    let fixture = crate::tests::Fixture::new("v3-handoff-supervisor");
    let program = new_program(&fixture, b);
    b.reserve_storage(SEED.len()).unwrap();
    let mut seed = SEED;
    let (key, delta) = Key::create_and_zeroize(&mut seed, program.policy(), b).unwrap();
    assert_eq!(seed, [0; 32]);
    b.reserve_storage(delta.additional_storage()).unwrap();
    b.release_storage(SEED.len()).unwrap();
    b.reserve_storage(Anchor::PAIR_STORAGE).unwrap();
    let (anchor, delta) = Anchor::admit(
        rustix::io::fcntl_dupfd_cloexec(peer, 3).unwrap(),
        rustix::io::fcntl_dupfd_cloexec(pidfd, 3).unwrap(),
        AnchorIdentity::new(65_534, 65_534).unwrap(),
        b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    b.reserve_storage(Supervisor::ROOT_FILE_STORAGE).unwrap();
    let credentials = crate::IssuerServiceCredentialProfileV1::new(65_533, 65_533).unwrap();
    let (supervisor, delta) = Supervisor::bind(
        program,
        credentials,
        File::open(&fixture.root).unwrap(),
        key,
        anchor,
        b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    (fixture, supervisor)
}

pub(crate) fn request(submitter: &OwnedFd, case: u32) -> OwnedFd {
    send_packet(submitter, &frame(b"HOF3", case), &[]).unwrap();
    let (payload, [control]) = receive_packet::<1>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(payload, frame(b"HOF3", case));
    // The sender closes its source before this acknowledgment, avoiding an
    // observation race when checking that the consumed control owner closed.
    let (ack, []) = receive_packet::<0>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(ack, frame(b"SENT", case));
    control
}

#[test]
#[ignore = "private distinct-UID V3 handoff supervisor, run only by the root coordinator"]
fn supervisor_process_helper() {
    require_child_credentials("handoff-supervisor-v3", 65_533);
    let control = inherited_control();
    let (payload, [peer, pidfd, submitter]) =
        receive_packet::<3>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&payload[..4], b"ANC2");
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let account = b.work_ledger_identity_v1();
    b.reserve_storage(19).unwrap();
    let (fixture, supervisor) = supervisor(&peer, &pidfd, &mut b);
    let supervisor_storage = supervisor.retained_storage();
    let supervisor_work =
        Supervisor::WORK + revalidation_work(&fixture) + Key::IO_WORK + Anchor::REVALIDATION_WORK;
    let accept_work = Accepted::WORK
        + 3 * supervisor_work
        + FRAME_WORK
        + 2 * MANIFEST_WORK
        + LiveClient::ADMISSION_WORK
        + LiveClient::REVALIDATION_WORK;
    let descriptors = open_descriptor_count();
    let input = request(&submitter, 0);
    let object = checks::snapshot(&input).unwrap();
    b.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    let floor = b.storage();
    let start = b.work();
    let (mut accepted, delta) = supervisor
        .accept_handoff(input, IO_TIMEOUT, &mut b)
        .unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work() - start, accept_work);
    assert_eq!(
        accepted.retained_storage(),
        Accepted::CONTROL_STORAGE + delta.additional_storage()
    );
    b.reserve_storage(delta.additional_storage()).unwrap();
    let _: &Manifest = accepted.manifest();
    assert!(
        accepted
            .manifest()
            .matches_policy(supervisor.policy(), &mut b)
            .unwrap()
    );
    assert_ne!(
        accepted.submitter().pid(),
        accepted.manifest().client().pid()
    );
    let start = b.work();
    accepted.revalidate(&supervisor, &mut b).unwrap();
    assert_eq!(
        b.work() - start,
        Accepted::WORK + supervisor_work + MANIFEST_WORK + LiveClient::REVALIDATION_WORK
    );
    b.release_storage(20).unwrap();
    let start = b.work();
    assert!(matches!(
        accepted.revalidate(&supervisor, &mut b),
        Err(HandoffError::Resource(Resource::Accounting))
    ));
    assert_eq!(b.work() - start, ENTRY);
    b.reserve_storage(20).unwrap();
    accepted.pidfd_snapshot.1 ^= 1;
    assert!(matches!(
        accepted.revalidate(&supervisor, &mut b),
        Err(HandoffError::DescriptorChanged)
    ));
    accepted.pidfd_snapshot.1 ^= 1;
    rustix::io::fcntl_setfd(&accepted.service_peer, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        accepted.revalidate(&supervisor, &mut b),
        Err(HandoffError::InvalidServicePeer)
    ));
    rustix::io::fcntl_setfd(&accepted.service_peer, rustix::io::FdFlags::CLOEXEC).unwrap();
    accepted.revalidate(&supervisor, &mut b).unwrap();
    let retained = accepted.retained_storage();
    drop(accepted);
    b.release_storage(retained).unwrap();
    assert_eq!(references(object), 0);
    assert_eq!(open_descriptor_count(), descriptors);
    assert_eq!(b.storage(), 19 + supervisor_storage);

    for case in 1..=7 {
        let descriptors = open_descriptor_count();
        let input = request(&submitter, case);
        let object = checks::snapshot(&input).unwrap();
        b.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
        let floor = b.storage();
        let start = b.work();
        let error = supervisor
            .accept_handoff(input, IO_TIMEOUT, &mut b)
            .unwrap_err();
        assert!(
            matches!(
                (case, &error),
                (1 | 2, HandoffError::PolicyMismatch)
                    | (3, HandoffError::ExternalAnchorServiceMismatch)
                    | (4, HandoffError::InvalidServicePeer)
                    | (5, HandoffError::DescriptorAlias)
                    | (6, HandoffError::MalformedTransfer)
                    | (7, HandoffError::CanonicalHandoff(FrameError::Framing(_)))
            ),
            "case {case}: {error:?}"
        );
        if case <= 3 {
            assert_eq!(
                b.work() - start,
                Accepted::WORK + supervisor_work + FRAME_WORK + MANIFEST_WORK
            );
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(references(object), 0);
        assert_eq!(open_descriptor_count(), descriptors, "semantic case {case}");
        b.release_storage(Accepted::CONTROL_STORAGE).unwrap();
    }
    for case in 0..5 {
        let descriptors = open_descriptor_count();
        let input = request(&submitter, 0);
        let object = checks::snapshot(&input).unwrap();
        b.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
        let before = b.storage();
        if case == 0 {
            b.release_storage(20).unwrap();
        }
        if case == 2 {
            b.reserve_storage(STORAGE - b.storage() - Accepted::SCRATCH + 1)
                .unwrap();
        }
        if case == 3 {
            b.charge_work(WORK - b.work() - (accept_work - 1)).unwrap();
        }
        if case == 4 {
            b.charge_work(WORK - b.work() - (ENTRY - 1)).unwrap();
        }
        let floor = b.storage();
        let start = b.work();
        let timeout = if case == 1 {
            Duration::ZERO
        } else {
            IO_TIMEOUT
        };
        let error = supervisor
            .accept_handoff(input, timeout, &mut b)
            .unwrap_err();
        assert_eq!(b.storage(), floor);
        assert_eq!(references(object), 0);
        assert_eq!(open_descriptor_count(), descriptors, "resource case {case}");
        match case {
            0 => {
                assert!(matches!(
                    error,
                    HandoffError::Resource(Resource::Accounting)
                ));
                assert_eq!(b.work() - start, ENTRY);
                b.reserve_storage(20).unwrap();
            }
            1 => {
                assert!(matches!(error, HandoffError::InvalidTimeout));
                assert_eq!(b.work() - start, Accepted::WORK);
            }
            2 => {
                assert!(matches!(
                    error,
                    HandoffError::Resource(Resource::Storage(_))
                ));
                assert_eq!(b.work() - start, Accepted::WORK);
                assert_eq!(b.failed_storage(), Some(STORAGE + 1));
                b.release_storage(floor - before).unwrap();
            }
            3 => {
                assert!(matches!(error, HandoffError::Pidfd(_)));
                assert_eq!(
                    b.work() - start,
                    accept_work - LiveClient::REVALIDATION_WORK + ENTRY
                );
            }
            _ => {
                assert!(matches!(error, HandoffError::Resource(Resource::Work(_))));
                assert_eq!(b.work(), start);
            }
        }
        assert_eq!(b.storage(), before);
        b.release_storage(Accepted::CONTROL_STORAGE).unwrap();
    }
    assert!(b.work_ledger_identity_v1() == account);
    assert_eq!(b.failed_work(), Some(WORK + 1));
    assert_eq!(b.failed_storage(), Some(STORAGE + 1));
    drop(supervisor);
    b.release_storage(supervisor_storage).unwrap();
    assert_eq!(b.storage(), 19);
    send_packet(&submitter, &frame(b"STOP", 0), &[]).unwrap();
    let (done, []) = receive_packet::<0>(&submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&done[..4], b"DONE");
    send_packet(&control, &payload, &[]).unwrap();
}

#[test]
#[ignore = "private V3 submitter, run only by the isolated root coordinator"]
fn submitter_process_helper() {
    require_child_credentials("handoff-submitter-v3", 65_532);
    let control = inherited_control();
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
    let mut child = ChildGuard(
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap(),
    );
    drop(command);
    let (payload, [service, pidfd]) =
        receive_packet::<2>(&client_control, Instant::now() + IO_TIMEOUT).unwrap();
    let pid = child.0.id();
    assert_eq!(payload, frame(b"CLI2", pid));
    let fixture = crate::tests::Fixture::new("v3-handoff-submitter");
    let mut held = None;
    let mut stopped = false;
    for _ in 0..16 {
        let (request, []) =
            receive_packet::<0>(&control, Instant::now() + Duration::from_secs(40)).unwrap();
        if request == frame(b"STOP", 0) {
            stopped = true;
            break;
        }
        assert_eq!(&request[..4], b"HOF3");
        let case = u32::from_le_bytes(request[4..].try_into().unwrap());
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        let p = policy(
            fixture.issuer_measurement(),
            sealed_static_issuer_runtime_measurement_v1(),
            if case == 2 { 8 } else { 7 },
            &mut b,
        );
        let client = Client::new(pid, 65_532, 65_532).unwrap();
        let submitter = Client::new(std::process::id(), 65_532, 65_532).unwrap();
        let anchor = AnchorIdentity::new(if case == 3 { 65_530 } else { 65_534 }, 65_534).unwrap();
        let bytes = if case == 1 {
            use ed25519_dalek::SigningKey;
            use fe2o3_compiler_execution_protocol::{
                CompilerExecutionIssuerPolicyV2 as P,
                CompilerExecutionServiceLaunchManifestV2 as M,
                CompilerExecutionSupervisorHandoffV2 as F,
            };
            let (other, delta) = P::new(
                7,
                fixture.issuer_measurement(),
                sealed_static_issuer_runtime_measurement_v1(),
                SigningKey::from_bytes(&SEED).verifying_key().to_bytes(),
                SigningKey::from_bytes(&[0x52; 32])
                    .verifying_key()
                    .to_bytes(),
                &mut b,
            )
            .unwrap();
            b.reserve_storage(delta.additional_storage()).unwrap();
            let (m, delta) = M::new(client, anchor, &other, &mut b).unwrap();
            b.reserve_storage(delta.additional_storage()).unwrap();
            let (f, delta) = F::new(submitter, m, &mut b).unwrap();
            b.reserve_storage(delta.additional_storage()).unwrap();
            *f.canonical_bytes()
        } else {
            let (m, delta) = Manifest::new(client, anchor, &p, &mut b).unwrap();
            b.reserve_storage(delta.additional_storage()).unwrap();
            let (f, delta) = Frame::new(submitter, m, &mut b).unwrap();
            b.reserve_storage(delta.additional_storage()).unwrap();
            *f.canonical_bytes()
        };
        let (sent, retained) = pair();
        match case {
            4 => send_packet(&retained, &bytes, &[pidfd.as_fd(), service.as_fd()]).unwrap(),
            5 => send_packet(&retained, &bytes, &[service.as_fd(), service.as_fd()]).unwrap(),
            6 => send_packet(&retained, &bytes[..1], &[service.as_fd(), pidfd.as_fd()]).unwrap(),
            7 => {
                let mut bad = bytes;
                bad[0] ^= 1;
                send_packet(&retained, &bad, &[service.as_fd(), pidfd.as_fd()]).unwrap();
            }
            _ => send_packet(&retained, &bytes, &[service.as_fd(), pidfd.as_fd()]).unwrap(),
        }
        held = Some(retained);
        send_packet(&control, &request, &[sent.as_fd()]).unwrap();
        drop(sent);
        send_packet(&control, &frame(b"SENT", case), &[]).unwrap();
    }
    assert!(
        stopped,
        "V3 handoff fixture exceeded its fixed request bound"
    );
    drop(held);
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
#[ignore = "requires explicit disposable-container root opt-in; handoff custody only"]
fn native_distinct_uid_handoff_v3_fixture() {
    run_fixture(OPT_IN, &format!("{PREFIX}supervisor_process_helper"));
}

// Preparation reuses the same bounded submitter/client/anchor processes.
pub(crate) fn run_fixture(opt_in: &str, supervisor_test: &str) {
    assert_eq!(std::env::var(opt_in).as_deref(), Ok("1"));
    assert_eq!(
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw()
        ),
        (0, 0)
    );
    assert!(
        std::path::Path::new("/.dockerenv").is_file(),
        "isolated Docker fixture only"
    );
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let caps = u64::from_str_radix(
        status
            .lines()
            .find_map(|line| line.strip_prefix("CapEff:\t"))
            .unwrap(),
        16,
    )
    .unwrap();
    let required = (1 << 5) | (1 << 6) | (1 << 7);
    assert_eq!(
        caps & required,
        required,
        "root fixture lacks cleanup/credential capabilities"
    );
    let (anchor_control, child) = pair();
    let mut anchor = spawn_role(
        "authority_v2_test_process::anchor_process_helper",
        "anchor",
        65_534,
        child,
    );
    let pid = anchor.0.id();
    let process = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
    let pidfd = rustix::process::pidfd_open(process, rustix::process::PidfdFlags::empty()).unwrap();
    let (payload, [peer]) =
        receive_packet::<1>(&anchor_control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(payload, frame(b"ANC2", pid));
    let (submitter_control, child) = pair();
    let mut submitter = spawn_role(
        &format!("{PREFIX}submitter_process_helper"),
        "handoff-submitter-v3",
        65_532,
        child,
    );
    let (supervisor_control, child) = pair();
    let mut supervisor = spawn_role(supervisor_test, "handoff-supervisor-v3", 65_533, child);
    let deadline = Instant::now() + Duration::from_secs(30);
    send_packet(
        &supervisor_control,
        &payload,
        &[peer.as_fd(), pidfd.as_fd(), submitter_control.as_fd()],
    )
    .unwrap();
    drop((peer, pidfd, submitter_control));
    let (done, []) = receive_packet::<0>(&supervisor_control, deadline).unwrap();
    assert_eq!(done, payload);
    let supervisor_status = supervisor.wait_until(deadline).unwrap();
    let submitter_status = submitter.wait_until(deadline).unwrap();
    send_packet(&anchor_control, &frame(b"STOP", pid), &[]).unwrap();
    let anchor_status = anchor.wait_until(Instant::now() + IO_TIMEOUT).unwrap();
    assert!(
        supervisor_status.success(),
        "V3 handoff supervisor failed: {supervisor_status}"
    );
    assert!(
        submitter_status.success(),
        "V3 handoff submitter failed: {submitter_status}"
    );
    assert!(
        anchor_status.success(),
        "anchor fixture failed: {anchor_status}"
    );
}
