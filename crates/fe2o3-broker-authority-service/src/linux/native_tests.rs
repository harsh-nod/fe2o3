use super::super::tests::{
    external_anchor_service_identity as service, nonblocking_seqpacket as pair, pidfd_for,
};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::unix::fs::MetadataExt;

const EXTRA: usize = 19;

fn run<T>(
    floor: usize,
    work: usize,
    storage: usize,
    op: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> (Result<T>, usize, usize, usize) {
    let mut meter = Work::new(work);
    let mut budget = Budget::new(&mut meter, storage);
    budget.reserve_storage(floor).unwrap();
    let result = op(&mut budget);
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

fn fixture() -> (Anchor, OwnedFd) {
    let (peer, other) = pair();
    let (result, _, _, _) = run(Anchor::PAIR_STORAGE, Anchor::ADMISSION_WORK, 100_000, |b| {
        Anchor::admit_inner::<false>(peer, pidfd_for(std::process::id()), service(), b)
    });
    (result.unwrap().0, other)
}

fn references(fd: &OwnedFd) -> usize {
    // Only count uniquely owned objects; self pidfds share an inode across tests.
    let stat = rustix::fs::fstat(fd).unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|meta| (meta.dev(), meta.ino()) == (stat.st_dev, stat.st_ino))
        .count()
}

fn process_references(pid: u32) -> usize {
    // Old kernels can share one anon-inode across pidfds. The live test child
    // gives us a unique target even there, without counting other tests' pidfds.
    std::fs::read_dir("/proc/self/fdinfo")
        .unwrap()
        .filter_map(|entry| {
            let record = std::fs::read_to_string(entry.ok()?.path()).ok()?;
            super::super::checks::parse_pidfd_fdinfo(&record).ok()
        })
        .filter(|actual| *actual == pid)
        .count()
}

fn error<T>(result: Result<T>) -> Error {
    match result {
        Ok(_) => panic!("expected refusal"),
        Err(e) => e,
    }
}

#[test]
fn production_entry_and_native_revalidation_never_inherit_legacy_same_uid_bypass() {
    let (peer, other) = pair();
    let (result, used, live, _) = run(
        Anchor::PAIR_STORAGE + EXTRA,
        Anchor::ADMISSION_WORK,
        100_000,
        |b| Anchor::admit(peer, pidfd_for(std::process::id()), service(), b),
    );
    assert_eq!(
        error(result).kind(),
        Some(AdmissionErrorKindV1::SameUidExternalAnchorService)
    );
    assert_eq!(used, Anchor::ADMISSION_WORK);
    assert_eq!(live, Anchor::PAIR_STORAGE + EXTRA);
    drop(other);

    let (mut anchor, _other) = fixture();
    assert!(anchor.state.non_authoritative_same_uid_test);
    anchor.same_uid_fixture = false;
    let (result, _, _, _) = run(
        anchor.retained_storage(),
        Anchor::REVALIDATION_WORK,
        100_000,
        |b| anchor.validate_continuity(b),
    );
    assert_eq!(
        error(result).kind(),
        Some(AdmissionErrorKindV1::SameUidExternalAnchorService)
    );
}

#[test]
fn bounded_fixture_transfer_chain_uses_one_ledger_and_retires_full_charges() {
    let (peer, other) = pair();
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget
        .reserve_storage(EXTRA + Anchor::PAIR_STORAGE)
        .unwrap();
    let (anchor, delta) =
        Anchor::admit_inner::<false>(peer, pidfd_for(std::process::id()), service(), &mut budget)
            .unwrap();
    assert_eq!(
        delta.additional_storage(),
        anchor.retained_storage() - Anchor::PAIR_STORAGE
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let retained = anchor.retained_storage();
    assert_eq!(budget.storage(), EXTRA + retained);
    assert_eq!(anchor.service_identity(), service());
    assert_eq!(anchor.service_process_identity().pid(), std::process::id());
    let before_peer = references(&anchor.state.peer);
    let (peer, pidfd, delta) = anchor.try_clone_for_transfer(&mut budget).unwrap();
    assert_eq!(delta.additional_storage(), Anchor::PAIR_STORAGE);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(references(&peer), before_peer + 1);
    for fd in [&peer, &pidfd] {
        assert!(
            rustix::io::fcntl_getfd(fd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        assert!(fd.as_raw_fd() >= 3);
    }
    anchor
        .validate_transfer(peer.as_fd(), pidfd.as_fd(), &mut budget)
        .unwrap();
    assert_eq!(references(&peer), before_peer + 1);
    anchor.validate_continuity(&mut budget).unwrap();
    assert_eq!(
        budget.work(),
        Anchor::ADMISSION_WORK
            + Anchor::CLONE_TRANSFER_WORK
            + Anchor::VALIDATE_TRANSFER_WORK
            + Anchor::REVALIDATION_WORK
    );
    drop((peer, pidfd));
    budget.release_storage(Anchor::PAIR_STORAGE).unwrap();
    assert_eq!(references(&anchor.state.peer), before_peer);
    drop(anchor);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    drop(other);
}

fn boundary<T>(
    floor: usize,
    work: usize,
    case: usize,
    op: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) {
    let prepaid = if case == 0 { floor - 1 } else { floor + EXTRA };
    let work_limit = match case {
        1 => ENTRY_WORK - 1,
        2 => work - 1,
        _ => work,
    };
    let storage_limit = prepaid + Anchor::IO_STORAGE - usize::from(case == 3);
    let mut meter = Work::new(work_limit);
    let mut budget = Budget::new(&mut meter, storage_limit);
    budget.reserve_storage(prepaid).unwrap();
    let result = op(&mut budget);
    assert_eq!(budget.storage(), prepaid);
    match case {
        0 => {
            assert_eq!(error(result).resource(), Some(Resource::Accounting));
            assert_eq!(budget.work(), ENTRY_WORK);
        }
        1 | 2 => {
            assert!(matches!(error(result).resource(), Some(Resource::Work(_))));
            assert_eq!(budget.work(), if case == 1 { 0 } else { ENTRY_WORK });
        }
        3 => {
            assert!(matches!(
                error(result).resource(),
                Some(Resource::Storage(_))
            ));
            assert_eq!(budget.work(), work);
            assert_eq!(budget.failed_storage(), Some(prepaid + Anchor::IO_STORAGE));
        }
        _ => {
            assert!(result.is_ok());
            assert_eq!(budget.work(), work);
            assert_eq!(budget.peak_storage(), storage_limit);
        }
    }
    if case < 4 {
        assert_eq!(budget.peak_storage(), prepaid);
    }
    assert_eq!(
        meter.failed_work(),
        match case {
            1 => Some(ENTRY_WORK),
            2 => Some(work),
            _ => None,
        }
    );
}

#[test]
fn every_native_operation_enforces_full_input_and_exact_work_and_scratch_floors() {
    let (anchor, _other) = fixture();
    for case in 0..5 {
        let peer = rustix::io::fcntl_dupfd_cloexec(&anchor.state.peer, 3).unwrap();
        let pidfd = rustix::io::fcntl_dupfd_cloexec(&anchor.state.live_service.pidfd, 3).unwrap();
        let peers = references(&anchor.state.peer);
        boundary(Anchor::PAIR_STORAGE, Anchor::ADMISSION_WORK, case, |b| {
            Anchor::admit_inner::<false>(peer, pidfd, service(), b)
        });
        assert_eq!(references(&anchor.state.peer), peers - 1);
        boundary(
            anchor.retained_storage(),
            Anchor::REVALIDATION_WORK,
            case,
            |b| anchor.validate_continuity(b),
        );
        let peers = references(&anchor.state.peer);
        boundary(
            anchor.retained_storage(),
            Anchor::CLONE_TRANSFER_WORK,
            case,
            |b| anchor.try_clone_for_transfer(b),
        );
        assert_eq!(references(&anchor.state.peer), peers);
        boundary(
            anchor.retained_storage() + Anchor::PAIR_STORAGE,
            Anchor::VALIDATE_TRANSFER_WORK,
            case,
            |b| {
                anchor.validate_transfer(
                    anchor.state.peer.as_fd(),
                    anchor.state.live_service.pidfd.as_fd(),
                    b,
                )
            },
        );
        assert_eq!(references(&anchor.state.peer), peers);
    }
}

#[test]
fn native_custody_rejects_retained_state_changes_and_peer_substitution() {
    for change in 0..6 {
        let (mut anchor, _other) = fixture();
        let expected = match change {
            0 => {
                anchor.state.issuer_uid ^= 1;
                AdmissionErrorKindV1::ServiceIdentityChanged
            }
            1 => {
                rustix::fs::fcntl_setfl(&anchor.state.peer, OFlags::RDWR).unwrap();
                AdmissionErrorKindV1::PeerStatusFlags
            }
            2 => {
                anchor.state.live_service.start_time_ticks += 1;
                AdmissionErrorKindV1::ClientStartTimeChanged
            }
            3 => {
                anchor.state.live_service.descriptor_identity.inode ^= 1;
                AdmissionErrorKindV1::ClientPidfdIdentityChanged
            }
            4 => {
                anchor.state.live_service.expected_client.uid ^= 1;
                AdmissionErrorKindV1::ExternalAnchorServiceCredentialsMismatch
            }
            _ => {
                anchor.state.live_service.identity_source =
                    match anchor.state.live_service.identity_source {
                        PidfdIdentitySourceV1::KernelIoctl => PidfdIdentitySourceV1::ProcfsFdinfo,
                        PidfdIdentitySourceV1::ProcfsFdinfo => PidfdIdentitySourceV1::KernelIoctl,
                    };
                AdmissionErrorKindV1::ClientPidfdIdentityChanged
            }
        };
        let (result, _, live, _) = run(
            anchor.retained_storage(),
            Anchor::REVALIDATION_WORK,
            100_000,
            |b| anchor.validate_continuity(b),
        );
        assert_eq!(error(result).kind(), Some(expected));
        assert_eq!(live, anchor.retained_storage());
    }
    let (anchor, _other) = fixture();
    let (replacement, _replacement_other) = pair();
    let pidfd = pidfd_for(std::process::id());
    let peer_count = references(&replacement);
    let (result, _, _, _) = run(
        anchor.retained_storage() + Anchor::PAIR_STORAGE,
        Anchor::VALIDATE_TRANSFER_WORK,
        100_000,
        |b| anchor.validate_transfer(replacement.as_fd(), pidfd.as_fd(), b),
    );
    assert_eq!(
        error(result).kind(),
        Some(AdmissionErrorKindV1::PeerIdentityChanged)
    );
    assert_eq!(references(&replacement), peer_count);
}

#[test]
fn refusals_close_consumed_and_temporary_pidfds_without_touching_borrowed_inputs() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut command = std::process::Command::new("/bin/sleep");
    command.arg("30");
    let child = Child(crate::test_process_execution::spawn(&mut command).unwrap());
    let pidfd = pidfd_for(child.0.id());
    let count = process_references(child.0.id());
    assert!(count >= 1);
    let (anchor, _other) = fixture();
    let peers = references(&anchor.state.peer);
    for case in 0..4 {
        let peer = rustix::io::fcntl_dupfd_cloexec(&anchor.state.peer, 3).unwrap();
        let input = rustix::io::fcntl_dupfd_cloexec(&pidfd, 3).unwrap();
        assert_eq!(process_references(child.0.id()), count + 1);
        boundary(Anchor::PAIR_STORAGE, Anchor::ADMISSION_WORK, case, |b| {
            Anchor::admit_inner::<false>(peer, input, service(), b)
        });
        assert_eq!(process_references(child.0.id()), count);
        assert_eq!(references(&anchor.state.peer), peers);
    }
    // Full inspection rejects the different process after privately duplicating
    // the borrowed pair. Both private duplicates must close on that refusal.
    let (result, _, _, _) = run(
        anchor.retained_storage() + Anchor::PAIR_STORAGE,
        Anchor::VALIDATE_TRANSFER_WORK,
        100_000,
        |b| anchor.validate_transfer(anchor.state.peer.as_fd(), pidfd.as_fd(), b),
    );
    assert_eq!(
        error(result).kind(),
        Some(AdmissionErrorKindV1::ClientPidfdTargetMismatch)
    );
    assert_eq!(process_references(child.0.id()), count);
    assert_eq!(references(&anchor.state.peer), peers);
}

#[test]
fn unwind_restores_storage_without_refunding_the_prepaid_inspection_schedule() {
    let mut work = Work::new(Anchor::ADMISSION_WORK);
    let mut budget = Budget::new(&mut work, EXTRA + Anchor::PAIR_STORAGE + Anchor::IO_STORAGE);
    budget
        .reserve_storage(EXTRA + Anchor::PAIR_STORAGE)
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Anchor::scope::<()>(
            &mut budget,
            Anchor::PAIR_STORAGE,
            Anchor::ADMISSION_WORK,
            |_| panic!("inspection unwound"),
        )
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), EXTRA + Anchor::PAIR_STORAGE);
    assert_eq!(budget.work(), Anchor::ADMISSION_WORK);
}

#[test]
fn debug_is_inert_and_contains_no_raw_descriptors_or_procfs_path() {
    let (anchor, _other) = fixture();
    let debug = format!("{anchor:?}");
    assert!(debug.contains("authority: \"none\""));
    for text in ["/proc/", "OwnedFd", "peer_identity", "start_time_ticks"] {
        assert!(!debug.contains(text));
    }
}
