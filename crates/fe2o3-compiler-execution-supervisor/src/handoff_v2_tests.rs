use super::*;
use crate::authority_v2_test_process::{IO_TIMEOUT, frame, pair, receive_packet, send_packet};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::{fd::AsFd, unix::fs::MetadataExt};

const WORK: usize = 10_000_000_000;
const STORAGE: usize = 10_000_000;
const EXTRA: usize = 19;

fn references(snapshot: Snapshot) -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (snapshot.0, snapshot.1))
        .count()
}
fn pidfd_references(pid: u32) -> usize {
    std::fs::read_dir("/proc/self/fdinfo")
        .unwrap()
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
        .filter(|record| {
            record.lines().any(|line| {
                line.strip_prefix("Pid:\t")
                    .and_then(|n| n.parse::<u32>().ok())
                    == Some(pid)
            })
        })
        .count()
}
fn resource(mut error: &(dyn Error + 'static)) -> Resource {
    loop {
        if let Some(error) = error.downcast_ref::<Resource>() {
            return *error;
        }
        error = error.source().expect("expected typed resource refusal");
    }
}
fn request(submitter: &OwnedFd, case: u32) -> (OwnedFd, u32) {
    send_packet(submitter, &frame(b"HOF2", case), &[]).unwrap();
    let (payload, [control]) = receive_packet::<1>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&payload[..4], b"HOF2");
    (
        control,
        u32::from_le_bytes(payload[4..].try_into().unwrap()),
    )
}

/// Only the explicitly opted-in real distinct-UID process fixture calls this.
pub(crate) fn exercise(peer: &OwnedFd, pidfd: &OwnedFd, submitter: &OwnedFd) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(EXTRA).unwrap();
    let (fixture, supervisor) = crate::authority_v2::tests::bound_fixture(peer, pidfd, &mut budget);
    let supervisor_storage = supervisor.retained_storage();
    assert_eq!(budget.storage(), EXTRA + supervisor_storage);
    send_packet(
        submitter,
        &frame(b"ANC2", supervisor.external_anchor_process().pid()),
        &[],
    )
    .unwrap();

    // One ledger spans policy/program/supervisor/frame/pidfd admission and release.
    let (control, pid) = request(submitter, 0);
    let control_snapshot = checks::snapshot(&control).unwrap();
    let pidfd_count = pidfd_references(pid);
    budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    let input_floor = budget.storage();
    let start_work = budget.work();
    let (accepted, delta) = supervisor
        .accept_handoff(control, IO_TIMEOUT, &mut budget)
        .unwrap();
    let accept_work = budget.work() - start_work;
    let supervisor_work = Supervisor::WORK + crate::authority_v2::tests::nested_work(&fixture);
    let frame_work =
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V2;
    let manifest_work =
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2;
    assert_eq!(
        accept_work,
        Accepted::WORK
            + 3 * supervisor_work
            + frame_work
            + 2 * manifest_work
            + LiveClient::ADMISSION_WORK
            + LiveClient::REVALIDATION_WORK
    );
    assert_eq!(budget.storage(), input_floor);
    assert_eq!(
        accepted.retained_storage(),
        Accepted::CONTROL_STORAGE + delta.additional_storage()
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(
        budget.storage(),
        EXTRA + supervisor_storage + accepted.retained_storage()
    );
    assert_eq!(
        pidfd_references(pid),
        pidfd_count + 1,
        "retain exactly one client pidfd"
    );
    assert_eq!(accepted.manifest().client().pid(), pid);
    assert_eq!(accepted.submitter().uid(), 65_532);
    assert_ne!(accepted.submitter().pid(), pid);
    let submitter_pid = accepted.submitter().pid();
    let service_snapshot = accepted.service_snapshot;
    let start_work = budget.work();
    accepted.revalidate(&supervisor, &mut budget).unwrap();
    let revalidation_work = budget.work() - start_work;
    assert_eq!(
        revalidation_work,
        Accepted::WORK + supervisor_work + manifest_work + LiveClient::REVALIDATION_WORK
    );
    assert!(accept_work > revalidation_work);
    let debug = format!("{accepted:?}");
    assert!(debug.contains("session-custody-only"));
    assert!(!debug.contains("OwnedFd"));
    let retained = accepted.retained_storage();
    drop(accepted);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA + supervisor_storage);
    assert_eq!(references(control_snapshot), 0);
    assert_eq!(pidfd_references(pid), pidfd_count);

    let floor = supervisor_storage + Accepted::CONTROL_STORAGE;
    let mut peak = 0;
    for case in 0..6 {
        let (control, pid) = request(submitter, 0);
        let object = checks::snapshot(&control).unwrap();
        let pidfds = pidfd_references(pid);
        let prepaid = if case == 3 { floor - 1 } else { floor + EXTRA };
        let mut work = Work::new(match case {
            1 => accept_work - 1,
            5 => ENTRY - 1,
            _ => WORK,
        });
        let mut budget = Budget::new(
            &mut work,
            match case {
                2 => peak - 1,
                4 => peak,
                _ => STORAGE,
            },
        );
        budget.reserve_storage(prepaid).unwrap();
        let result = supervisor.accept_handoff(control, IO_TIMEOUT, &mut budget);
        assert_eq!(budget.storage(), prepaid, "accept case {case}");
        match case {
            0 | 4 => {
                let (owner, delta) = result.unwrap();
                assert_eq!(
                    owner.retained_storage(),
                    Accepted::CONTROL_STORAGE + delta.additional_storage()
                );
                assert_eq!(budget.work(), accept_work);
                if case == 0 {
                    peak = budget.peak_storage();
                }
                assert_eq!(budget.peak_storage(), peak);
                drop(owner);
            }
            _ => {
                let error = result.unwrap_err();
                let _ = resource(&error);
                if case == 1 {
                    assert!(matches!(error, ProtectedIssuerHandoffErrorV2::Pidfd(_)));
                    assert_eq!(
                        budget.work(),
                        accept_work - LiveClient::REVALIDATION_WORK + ENTRY
                    );
                }
            }
        }
        assert_eq!(references(object), 0);
        assert_eq!(pidfd_references(pid), pidfds);
        assert_eq!(
            work.failed_work(),
            match case {
                1 => Some(accept_work),
                5 => Some(ENTRY),
                _ => None,
            }
        );
    }

    let (control, _) = request(submitter, 0);
    budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    let (mut owner, delta) = supervisor
        .accept_handoff(control, IO_TIMEOUT, &mut budget)
        .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let retained = owner.retained_storage();
    let floor = supervisor_storage + retained;
    let mut peak = 0;
    for case in 0..6 {
        let prepaid = if case == 3 { floor - 1 } else { floor + EXTRA };
        let mut work = Work::new(match case {
            1 => revalidation_work - 1,
            5 => ENTRY - 1,
            _ => WORK,
        });
        let mut budget = Budget::new(
            &mut work,
            match case {
                2 => peak - 1,
                4 => peak,
                _ => STORAGE,
            },
        );
        budget.reserve_storage(prepaid).unwrap();
        let result = owner.revalidate(&supervisor, &mut budget);
        assert_eq!(budget.storage(), prepaid);
        match case {
            0 | 4 => {
                result.unwrap();
                assert_eq!(budget.work(), revalidation_work);
                if case == 0 {
                    peak = budget.peak_storage();
                }
                assert_eq!(budget.peak_storage(), peak);
            }
            _ => {
                let error = result.unwrap_err();
                let _ = resource(&error);
                if case == 1 {
                    assert!(matches!(error, ProtectedIssuerHandoffErrorV2::Pidfd(_)));
                }
            }
        }
        assert_eq!(
            work.failed_work(),
            match case {
                1 => Some(revalidation_work),
                5 => Some(ENTRY),
                _ => None,
            }
        );
    }
    rustix::io::fcntl_setfd(&owner.control, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        owner.revalidate(&supervisor, &mut budget),
        Err(ProtectedIssuerHandoffErrorV2::InvalidControl(_))
    ));
    rustix::io::fcntl_setfd(&owner.control, rustix::io::FdFlags::CLOEXEC).unwrap();
    rustix::io::fcntl_setfd(&owner.service_peer, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        owner.revalidate(&supervisor, &mut budget),
        Err(ProtectedIssuerHandoffErrorV2::InvalidServicePeer)
    ));
    rustix::io::fcntl_setfd(&owner.service_peer, rustix::io::FdFlags::CLOEXEC).unwrap();
    std::mem::swap(&mut owner.control, &mut owner.service_peer);
    assert!(matches!(
        owner.revalidate(&supervisor, &mut budget),
        Err(ProtectedIssuerHandoffErrorV2::DescriptorChanged)
    ));
    std::mem::swap(&mut owner.control, &mut owner.service_peer);
    owner.pidfd_snapshot.1 ^= 1;
    assert!(matches!(
        owner.revalidate(&supervisor, &mut budget),
        Err(ProtectedIssuerHandoffErrorV2::DescriptorChanged)
    ));
    owner.pidfd_snapshot.1 ^= 1;
    owner.revalidate(&supervisor, &mut budget).unwrap();
    drop(owner);
    budget.release_storage(retained).unwrap();

    for case in 1..=14 {
        let (control, pid) = request(submitter, case);
        let object = checks::snapshot(&control).unwrap();
        let pidfds = pidfd_references(pid);
        let submitter_pidfds = pidfd_references(submitter_pid);
        let service_fds = references(service_snapshot);
        let auxiliary_closes = crate::handoff_ancillary::auxiliary_closes();
        budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
        let error = supervisor
            .accept_handoff(control, IO_TIMEOUT, &mut budget)
            .unwrap_err();
        assert!(
            matches!(
                (case, &error),
                (1, ProtectedIssuerHandoffErrorV2::PolicyMismatch)
                    | (
                        2,
                        ProtectedIssuerHandoffErrorV2::ExternalAnchorServiceMismatch
                    )
                    | (
                        3,
                        ProtectedIssuerHandoffErrorV2::SubmitterCredentialsMismatch
                    )
                    | (
                        4,
                        ProtectedIssuerHandoffErrorV2::ServicePeerCredentialsMismatch
                    )
                    | (5, ProtectedIssuerHandoffErrorV2::InvalidServicePeer)
                    | (6, ProtectedIssuerHandoffErrorV2::DescriptorAlias)
                    | (7 | 8, ProtectedIssuerHandoffErrorV2::MalformedTransfer)
                    | (
                        9,
                        ProtectedIssuerHandoffErrorV2::ClientAndExternalAnchorProcessMatch
                    )
                    | (10, ProtectedIssuerHandoffErrorV2::Pidfd(_))
                    | (11..=13, ProtectedIssuerHandoffErrorV2::MalformedTransfer)
                    | (14, ProtectedIssuerHandoffErrorV2::CanonicalHandoff(_))
            ),
            "case {case}: {error:?}"
        );
        budget.release_storage(Accepted::CONTROL_STORAGE).unwrap();
        assert_eq!(references(object), 0);
        assert_eq!(pidfd_references(pid), pidfds);
        assert_eq!(pidfd_references(submitter_pid), submitter_pidfds);
        assert_eq!(references(service_snapshot), service_fds);
        if (11..=12).contains(&case) {
            assert_eq!(
                crate::handoff_ancillary::auxiliary_closes(),
                auxiliary_closes + 1,
                "kernel must actually deliver the unexpected pidfd in case {case}"
            );
        }
        assert_eq!(budget.storage(), supervisor_storage + EXTRA);
    }
    let (control, _held) = pair();
    budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    assert!(matches!(
        supervisor.accept_handoff(control, IO_TIMEOUT, &mut budget),
        Err(ProtectedIssuerHandoffErrorV2::ClientAndSupervisorUidMatch)
    ));
    budget.release_storage(Accepted::CONTROL_STORAGE).unwrap();

    for timeout in [Duration::ZERO, Duration::MAX] {
        let (control, _) = request(submitter, 0);
        let object = checks::snapshot(&control).unwrap();
        budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
        let start = budget.work();
        let error = supervisor
            .accept_handoff(control, timeout, &mut budget)
            .unwrap_err();
        if timeout.is_zero() {
            assert!(matches!(
                error,
                ProtectedIssuerHandoffErrorV2::InvalidTimeout
            ));
        } else {
            assert!(matches!(
                error,
                ProtectedIssuerHandoffErrorV2::DeadlineOverflow
            ));
        }
        assert_eq!(budget.work() - start, Accepted::WORK);
        assert_eq!(references(object), 0);
        budget.release_storage(Accepted::CONTROL_STORAGE).unwrap();
    }

    let (control, _) = request(submitter, 0);
    let mut work = Work::new(WORK);
    let mut denied = Budget::new(&mut work, STORAGE);
    assert!(denied.charge_work(WORK + 1).is_err());
    assert!(denied.reserve_storage(STORAGE + 1).is_err());
    denied
        .reserve_storage(EXTRA + supervisor_storage + Accepted::CONTROL_STORAGE)
        .unwrap();
    let (owner, delta) = supervisor
        .accept_handoff(control, IO_TIMEOUT, &mut denied)
        .unwrap();
    denied.reserve_storage(delta.additional_storage()).unwrap();
    owner.revalidate(&supervisor, &mut denied).unwrap();
    let retained = owner.retained_storage();
    drop(owner);
    denied.release_storage(retained).unwrap();
    assert_eq!(denied.storage(), EXTRA + supervisor_storage);
    assert_eq!(denied.failed_storage(), Some(STORAGE + 1));
    assert_eq!(work.failed_work(), Some(WORK + 1));

    send_packet(submitter, &frame(b"STOP", 0), &[]).unwrap();
    let (completed, []) = receive_packet::<0>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&completed[..4], b"DONE");
    drop(supervisor);
    budget.release_storage(supervisor_storage).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    drop(fixture);
}

#[test]
fn finite_receive_preserves_order_and_cloexec() {
    let (send, receive) = pair();
    let (first, second) = pair();
    send_packet(&send, &[7; BYTES], &[first.as_fd(), second.as_fd()]).unwrap();
    let (payload, rights) = transport::receive(&receive, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(payload, [7; BYTES]);
    for (received, original) in rights.iter().zip([&first, &second]) {
        assert_eq!(
            checks::snapshot(received).unwrap(),
            checks::snapshot(original).unwrap()
        );
        assert!(
            rustix::io::fcntl_getfd(received)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
}

#[test]
fn finite_receive_drains_bad_packets_and_obeys_deadline() {
    let (source, _held) = pair();
    let object = checks::snapshot(&source).unwrap();
    let before = references(object);
    for (length, count) in [
        (BYTES - 1, 2),
        (BYTES + 1, 2),
        (BYTES, 0),
        (BYTES, 1),
        (BYTES, 3),
    ] {
        let (send, receive) = pair();
        send_packet(&send, &vec![0; length], &vec![source.as_fd(); count]).unwrap();
        assert!(matches!(
            transport::receive(&receive, Instant::now() + IO_TIMEOUT),
            Err(ProtectedIssuerHandoffErrorV2::MalformedTransfer)
        ));
        assert_eq!(references(object), before);
    }
    let (send, receive) = pair();
    crate::handoff_v2_test_process::send_excess_rights(&send, &[0; BYTES], &source);
    assert!(matches!(
        transport::receive(&receive, Instant::now() + IO_TIMEOUT),
        Err(ProtectedIssuerHandoffErrorV2::MalformedTransfer)
    ));
    assert_eq!(references(object), before);
    let (send, receive) = pair();
    assert!(matches!(
        transport::receive(&receive, Instant::now()),
        Err(ProtectedIssuerHandoffErrorV2::Timeout)
    ));
    drop(send);
    assert!(transport::receive(&receive, Instant::now() + IO_TIMEOUT).is_err());
}

#[test]
fn shared_socket_failures_preserve_legacy_and_native_categories() {
    use crate::ProtectedIssuerHandoffErrorV1 as Legacy;
    let errors = [
        checks::Failure::InvalidControl("test"),
        checks::Failure::SubmitterCredentialsMismatch,
        checks::Failure::InvalidServicePeer,
        checks::Failure::ServicePeerCredentialsMismatch,
        checks::Failure::DescriptorChanged,
        checks::Failure::DescriptorAlias,
        checks::Failure::Io(rustix::io::Errno::BADF),
    ];
    for error in errors {
        let legacy = Legacy::from(error);
        let native = ProtectedIssuerHandoffErrorV2::from(error);
        // Both adapters retain the same original diagnostics, including errno text.
        assert_eq!(legacy.to_string(), native.to_string());
        assert_eq!(legacy.source().is_some(), native.source().is_some());
    }
    let (control, _held) = pair();
    checks::control_shape(&control).unwrap();
    let client = checks::control_peer(&control).unwrap();
    checks::service_peer(&control, client).unwrap();
    let snapshot = checks::snapshot(&control).unwrap();
    assert!(matches!(
        checks::distinct(snapshot, snapshot, Snapshot(1, 2, 3)),
        Err(checks::Failure::DescriptorAlias)
    ));
    rustix::io::fcntl_setfd(&control, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        checks::control_shape(&control),
        Err(checks::Failure::InvalidControl(
            "control descriptor is inheritable"
        ))
    ));
    assert!(matches!(
        checks::service_peer(&control, client),
        Err(checks::Failure::InvalidServicePeer)
    ));
}
