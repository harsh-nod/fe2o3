use super::super::{checks, continuity, tests::pidfd_for};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{cell::Cell, error::Error as _, fs::File, os::fd::AsFd, process::Command};

const EXTRA: usize = 19;

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture {
    witness: OwnedFd,
    child: Child,
}
impl Fixture {
    fn new() -> Self {
        let mut command = Command::new("/bin/sleep");
        command.arg("60");
        let child = Child(crate::test_process_execution::spawn(&mut command).unwrap());
        let witness = pidfd_for(child.0.id());
        Self { witness, child }
    }

    fn expected(&self) -> ExpectedClientProcessIdentityV1 {
        ExpectedClientProcessIdentityV1::new(
            self.child.0.id(),
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap()
    }

    fn input(&self) -> OwnedFd {
        rustix::io::fcntl_dupfd_cloexec(&self.witness, 3).unwrap()
    }

    fn references(&self) -> usize {
        // A unique live child also distinguishes pidfds on shared-anon-inode kernels.
        std::fs::read_dir("/proc/self/fdinfo")
            .unwrap()
            .filter_map(|entry| {
                let record = std::fs::read_to_string(entry.ok()?.path()).ok()?;
                checks::parse_pidfd_fdinfo(&record).ok()
            })
            .filter(|pid| *pid == self.child.0.id())
            .count()
    }

    fn admitted(&self) -> Client {
        let mut work = Work::new(Client::ADMISSION_WORK);
        let mut budget = Budget::new(&mut work, Client::FD_STORAGE + Client::IO_STORAGE);
        budget.reserve_storage(Client::FD_STORAGE).unwrap();
        let (client, delta) = Client::admit(self.input(), self.expected(), &mut budget).unwrap();
        budget.reserve_storage(delta.additional_storage()).unwrap();
        client
    }
}

fn revalidate(client: &Client) -> Result<()> {
    let mut work = Work::new(Client::REVALIDATION_WORK);
    let floor = client.retained_storage() + EXTRA;
    let mut budget = Budget::new(&mut work, floor + Client::IO_STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result = client.validate_liveness(&mut budget);
    assert_eq!(budget.work(), Client::REVALIDATION_WORK);
    assert_eq!(budget.storage(), floor);
    result
}

fn transfer(client: &Client, pidfd: Option<&OwnedFd>) -> Result<Option<(OwnedFd, Storage)>> {
    let work_limit = if pidfd.is_some() {
        Client::VALIDATE_TRANSFER_WORK
    } else {
        Client::CLONE_TRANSFER_WORK
    };
    let floor = client.retained_storage() + usize::from(pidfd.is_some()) * Client::FD_STORAGE;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, floor + Client::IO_STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result = match pidfd {
        Some(fd) => client.validate_transfer(fd, &mut budget).map(|()| None),
        None => client.try_clone_for_transfer(&mut budget).map(Some),
    };
    assert_eq!(budget.work(), work_limit);
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn quotas_have_independent_literal_work_and_fixed_storage_bounds() {
    assert_eq!(Client::ADMISSION_WORK, 26_482_792);
    assert_eq!(Client::REVALIDATION_WORK, 17_829_960);
    assert_eq!(Client::CLONE_TRANSFER_WORK, 52_441_288);
    assert_eq!(Client::VALIDATE_TRANSFER_WORK, 52_441_288);
    assert_eq!(CURRENT_PROCESS_START_TIME_WORK_V2, 9_177_128);
    assert_eq!(CURRENT_PROCESS_START_TIME_IO_STORAGE_V2, 16_450);
    assert_eq!(Client::FD_STORAGE, size_of::<(OwnedFd, Storage)>());
    assert_eq!(
        Client::IO_STORAGE,
        8 * size_of::<(Client, Storage)>() + 4 * Client::FD_STORAGE + 2 * 4097 + 8192
    );
}

#[test]
fn same_ledger_admission_transfer_revalidation_and_terminal_release_preserve_exact_custody() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    assert!(baseline >= 1);
    let stat = rustix::fs::fstat(&fixture.witness).unwrap();
    let total_work = Client::ADMISSION_WORK
        + Client::REVALIDATION_WORK
        + Client::CLONE_TRANSFER_WORK
        + Client::VALIDATE_TRANSFER_WORK;
    let mut work = Work::new(total_work);
    let floor = EXTRA + Client::FD_STORAGE;
    let mut budget = Budget::new(
        &mut work,
        EXTRA + Client::RETAINED + Client::FD_STORAGE + Client::IO_STORAGE,
    );
    budget.reserve_storage(floor).unwrap();
    let (client, delta) = Client::admit(fixture.input(), fixture.expected(), &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), Client::ADMISSION_WORK);
    assert_eq!(fixture.references(), baseline + 1);
    assert_eq!(
        delta.additional_storage(),
        Client::RETAINED - Client::FD_STORAGE
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(budget.storage(), EXTRA + client.retained_storage());
    assert_eq!(client.expected_client(), fixture.expected());
    assert_eq!(
        client.descriptor_identity(),
        (stat.st_dev, stat.st_ino, stat.st_mode)
    );
    client.validate_liveness(&mut budget).unwrap();
    assert_eq!(
        budget.work(),
        Client::ADMISSION_WORK + Client::REVALIDATION_WORK
    );
    assert_eq!(
        budget.peak_storage(),
        EXTRA + Client::RETAINED + Client::IO_STORAGE
    );
    assert_eq!(fixture.references(), baseline + 1);
    let (transferred, storage) = client.try_clone_for_transfer(&mut budget).unwrap();
    assert_eq!(storage.additional_storage(), Client::FD_STORAGE);
    assert_eq!(fixture.references(), baseline + 2);
    assert_eq!(budget.storage(), EXTRA + Client::RETAINED);
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    assert!(
        rustix::io::fcntl_getfd(&transferred)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_ne!(
        std::os::fd::AsRawFd::as_raw_fd(&transferred),
        std::os::fd::AsRawFd::as_raw_fd(&client.state.pidfd)
    );
    let transferred = File::from(transferred);
    client.validate_transfer(&transferred, &mut budget).unwrap();
    assert_eq!(budget.work(), total_work);
    assert_eq!(
        budget.peak_storage(),
        EXTRA + Client::RETAINED + Client::FD_STORAGE + Client::IO_STORAGE
    );
    assert_eq!(fixture.references(), baseline + 2);
    drop(transferred);
    budget.release_storage(Client::FD_STORAGE).unwrap();
    assert_eq!(fixture.references(), baseline + 1);
    let retained = client.retained_storage();
    drop(client);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    assert_eq!(fixture.references(), baseline);
}

fn limits(floor: usize, work: usize, case: usize) -> (usize, usize, usize) {
    let prepaid = if case == 0 { floor - 1 } else { floor + EXTRA };
    let work_limit = match case {
        1 => native::ENTRY_WORK - 1,
        2 => work - 1,
        _ => work,
    };
    (
        prepaid,
        work_limit,
        prepaid + Client::IO_STORAGE - usize::from(case == 3),
    )
}

fn assert_refusal(error: Error, budget: &Budget<'_>, work: usize, prepaid: usize, case: usize) {
    assert_eq!(error.kind(), None);
    assert_eq!(error.errno(), None);
    assert!(error.source().unwrap().downcast_ref::<Resource>().is_some());
    match case {
        0 => {
            assert_eq!(error.resource(), Some(Resource::Accounting));
            assert_eq!(budget.work(), native::ENTRY_WORK);
        }
        1 | 2 => {
            assert!(matches!(error.resource(), Some(Resource::Work(_))));
            assert_eq!(
                budget.work(),
                if case == 1 { 0 } else { native::ENTRY_WORK }
            );
        }
        3 => {
            assert!(matches!(error.resource(), Some(Resource::Storage(_))));
            assert_eq!(budget.work(), work);
            assert_eq!(budget.failed_storage(), Some(prepaid + Client::IO_STORAGE));
        }
        _ => panic!("not a refusal case"),
    }
    assert_eq!(budget.peak_storage(), prepaid);
    assert_eq!(budget.storage(), prepaid);
}

#[test]
fn admission_exact_and_one_short_boundaries_close_every_consumed_input() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    assert!(baseline >= 1);
    for case in 0..5 {
        let (prepaid, work_limit, storage_limit) =
            limits(Client::FD_STORAGE, Client::ADMISSION_WORK, case);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        let input = fixture.input();
        assert_eq!(fixture.references(), baseline + 1);
        let result = Client::admit(input, fixture.expected(), &mut budget);
        assert_eq!(budget.storage(), prepaid);
        if case == 4 {
            let (client, delta) = result.unwrap();
            assert_eq!(budget.work(), Client::ADMISSION_WORK);
            assert_eq!(budget.peak_storage(), storage_limit);
            budget.reserve_storage(delta.additional_storage()).unwrap();
            let retained = client.retained_storage();
            drop(client);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), EXTRA);
        } else {
            assert_refusal(
                result.unwrap_err(),
                &budget,
                Client::ADMISSION_WORK,
                prepaid,
                case,
            );
            budget.release_storage(prepaid).unwrap();
        }
        assert_eq!(fixture.references(), baseline);
        assert_eq!(
            work.failed_work(),
            match case {
                1 => Some(native::ENTRY_WORK),
                2 => Some(Client::ADMISSION_WORK),
                _ => None,
            }
        );
    }
}

#[test]
fn revalidation_exact_and_one_short_boundaries_preserve_the_borrowed_owner() {
    let fixture = Fixture::new();
    let client = fixture.admitted();
    let baseline = fixture.references();
    for case in 0..5 {
        let (prepaid, work_limit, storage_limit) =
            limits(client.retained_storage(), Client::REVALIDATION_WORK, case);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        let result = client.validate_liveness(&mut budget);
        assert_eq!(budget.storage(), prepaid);
        if case == 4 {
            result.unwrap();
            assert_eq!(budget.work(), Client::REVALIDATION_WORK);
            assert_eq!(budget.peak_storage(), storage_limit);
        } else {
            assert_refusal(
                result.unwrap_err(),
                &budget,
                Client::REVALIDATION_WORK,
                prepaid,
                case,
            );
        }
        assert_eq!(fixture.references(), baseline);
        assert_eq!(
            work.failed_work(),
            match case {
                1 => Some(native::ENTRY_WORK),
                2 => Some(Client::REVALIDATION_WORK),
                _ => None,
            }
        );
    }
    revalidate(&client).unwrap();
}

#[test]
fn transfer_exact_and_one_short_boundaries_charge_output_and_preserve_borrowed_inputs() {
    let fixture = Fixture::new();
    let client = fixture.admitted();
    let input = fixture.input();
    let baseline = fixture.references();
    for validating in [false, true] {
        let quota = if validating {
            Client::VALIDATE_TRANSFER_WORK
        } else {
            Client::CLONE_TRANSFER_WORK
        };
        let floor = client.retained_storage() + usize::from(validating) * Client::FD_STORAGE;
        for case in 0..5 {
            let (prepaid, work_limit, storage_limit) = limits(floor, quota, case);
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(prepaid).unwrap();
            let result = if validating {
                client.validate_transfer(&input, &mut budget).map(|()| None)
            } else {
                client.try_clone_for_transfer(&mut budget).map(Some)
            };
            assert_eq!(budget.storage(), prepaid);
            if case == 4 {
                let returned = result.unwrap();
                assert_eq!(budget.work(), quota);
                assert_eq!(budget.peak_storage(), storage_limit);
                if let Some((fd, charge)) = returned {
                    assert_eq!(fixture.references(), baseline + 1);
                    assert_eq!(charge.additional_storage(), Client::FD_STORAGE);
                    budget.reserve_storage(charge.additional_storage()).unwrap();
                    drop(fd);
                    budget.release_storage(charge.additional_storage()).unwrap();
                }
            } else {
                assert_refusal(result.unwrap_err(), &budget, quota, prepaid, case);
            }
            assert_eq!(budget.storage(), prepaid);
            assert_eq!(fixture.references(), baseline);
            assert_eq!(
                work.failed_work(),
                match case {
                    1 => Some(native::ENTRY_WORK),
                    2 => Some(quota),
                    _ => None,
                }
            );
        }
    }
    revalidate(&client).unwrap();
}

#[test]
fn transfer_work_and_storage_denials_stick_without_descriptor_growth() {
    let fixture = Fixture::new();
    let client = fixture.admitted();
    let input = fixture.input();
    let baseline = fixture.references();
    for validating in [false, true] {
        let quota = if validating {
            Client::VALIDATE_TRANSFER_WORK
        } else {
            Client::CLONE_TRANSFER_WORK
        };
        let floor = client.retained_storage() + usize::from(validating) * Client::FD_STORAGE;
        for storage_denial in [false, true] {
            let mut work = Work::new(if storage_denial { 2 * quota } else { quota - 1 });
            let mut budget = Budget::new(
                &mut work,
                floor + Client::IO_STORAGE - usize::from(storage_denial),
            );
            budget.reserve_storage(floor).unwrap();
            for attempt in 1..=2 {
                let error = if validating {
                    client.validate_transfer(&input, &mut budget).unwrap_err()
                } else {
                    client.try_clone_for_transfer(&mut budget).unwrap_err()
                };
                if storage_denial {
                    assert!(matches!(error.resource(), Some(Resource::Storage(_))));
                    assert_eq!(budget.failed_storage(), Some(floor + Client::IO_STORAGE));
                    assert_eq!(budget.work(), attempt * quota);
                } else {
                    assert!(matches!(error.resource(), Some(Resource::Work(_))));
                    assert_eq!(budget.work(), attempt * native::ENTRY_WORK);
                }
                assert_eq!(budget.storage(), floor);
                assert_eq!(fixture.references(), baseline);
            }
            assert_eq!(work.failed_work(), (!storage_denial).then_some(quota));
        }
    }
}

#[test]
fn first_work_and_storage_denials_are_sticky_across_revalidation_attempts() {
    let fixture = Fixture::new();
    let client = fixture.admitted();
    let floor = client.retained_storage();
    let mut work = Work::new(Client::REVALIDATION_WORK - 1);
    let mut budget = Budget::new(&mut work, floor + Client::IO_STORAGE);
    budget.reserve_storage(floor).unwrap();
    for attempt in 1..=2 {
        let error = client.validate_liveness(&mut budget).unwrap_err();
        assert!(matches!(error.resource(), Some(Resource::Work(_))));
        assert_eq!(budget.work(), attempt * native::ENTRY_WORK);
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(work.failed_work(), Some(Client::REVALIDATION_WORK));
    let mut work = Work::new(2 * Client::REVALIDATION_WORK);
    let mut budget = Budget::new(&mut work, floor + Client::IO_STORAGE - 1);
    budget.reserve_storage(floor).unwrap();
    for attempt in 1..=2 {
        let error = client.validate_liveness(&mut budget).unwrap_err();
        assert!(matches!(error.resource(), Some(Resource::Storage(_))));
        assert_eq!(budget.work(), attempt * Client::REVALIDATION_WORK);
        assert_eq!(budget.failed_storage(), Some(floor + Client::IO_STORAGE));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn target_mismatch_and_descriptor_refusals_keep_precedence_and_cleanup() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    let wrong = ExpectedClientProcessIdentityV1::new(
        std::process::id(),
        fixture.expected().uid(),
        fixture.expected().gid(),
    )
    .unwrap();
    for inheritable in [false, true] {
        let input = fixture.input();
        if inheritable {
            rustix::io::fcntl_setfd(&input, rustix::io::FdFlags::empty()).unwrap();
        }
        let mut work = Work::new(Client::ADMISSION_WORK);
        let mut budget = Budget::new(&mut work, Client::FD_STORAGE + Client::IO_STORAGE);
        budget.reserve_storage(Client::FD_STORAGE).unwrap();
        let error = Client::admit(input, wrong, &mut budget).unwrap_err();
        assert_eq!(
            error.kind(),
            Some(if inheritable {
                AdmissionErrorKindV1::ClientPidfdCloseOnExec
            } else {
                AdmissionErrorKindV1::ClientPidfdTargetMismatch
            })
        );
        assert_eq!(error.resource(), None);
        assert_eq!(error.errno(), None);
        if !inheritable {
            assert_eq!(
                error.to_string(),
                format!(
                    "client pidfd targets PID {}, expected exact PID {}",
                    fixture.child.0.id(),
                    std::process::id()
                )
            );
        }
        assert_eq!(budget.work(), Client::ADMISSION_WORK);
        assert_eq!(budget.storage(), Client::FD_STORAGE);
        assert_eq!(fixture.references(), baseline);
    }
    let mut work = Work::new(Client::ADMISSION_WORK);
    let mut budget = Budget::new(&mut work, Client::FD_STORAGE + Client::IO_STORAGE);
    budget.reserve_storage(Client::FD_STORAGE).unwrap();
    let error = Client::admit(
        File::open("/dev/null").unwrap().into(),
        fixture.expected(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.kind(), Some(AdmissionErrorKindV1::InspectClientPidfd));
}

#[test]
fn cached_facts_are_inert_and_supplied_credentials_are_not_independently_proven() {
    let fixture = Fixture::new();
    let expected = ExpectedClientProcessIdentityV1::new(
        fixture.child.0.id(),
        fixture.expected().uid() ^ 1,
        fixture.expected().gid() ^ 1,
    )
    .unwrap();
    let mut work = Work::new(Client::ADMISSION_WORK);
    let mut budget = Budget::new(&mut work, Client::FD_STORAGE + Client::IO_STORAGE);
    budget.reserve_storage(Client::FD_STORAGE).unwrap();
    let (client, delta) = Client::admit(fixture.input(), expected, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(client.expected_client(), expected);
    let snapshot = client.descriptor_identity();
    rustix::io::fcntl_setfd(&client.state.pidfd, rustix::io::FdFlags::empty()).unwrap();
    assert_eq!(client.descriptor_identity(), snapshot);
    assert_eq!(
        revalidate(&client).unwrap_err().kind(),
        Some(AdmissionErrorKindV1::ClientPidfdCloseOnExec)
    );
    for pidfd in [None, Some(&fixture.witness)] {
        assert_eq!(
            transfer(&client, pidfd).unwrap_err().kind(),
            Some(AdmissionErrorKindV1::ClientPidfdCloseOnExec)
        );
    }
    rustix::io::fcntl_setfd(&client.state.pidfd, rustix::io::FdFlags::CLOEXEC).unwrap();
    revalidate(&client).unwrap();
    let debug = format!("{client:?}");
    assert!(debug.contains("authority: \"none\""));
    for forbidden in [
        "OwnedFd",
        "/proc/",
        "descriptor_identity",
        "start_time_ticks",
    ] {
        assert!(!debug.contains(forbidden));
    }
    let retained = client.retained_storage();
    drop(client);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn retained_object_target_source_and_start_time_drift_are_rejected() {
    let fixture = Fixture::new();
    for change in 0..4 {
        let mut client = fixture.admitted();
        let expected = match change {
            0 => {
                client.state.descriptor_identity.inode ^= 1;
                AdmissionErrorKindV1::ClientPidfdIdentityChanged
            }
            1 => {
                client.state.start_time_ticks += 1;
                AdmissionErrorKindV1::ClientStartTimeChanged
            }
            2 => {
                client.state.expected_client = ExpectedClientProcessIdentityV1::new(
                    std::process::id(),
                    fixture.expected().uid(),
                    fixture.expected().gid(),
                )
                .unwrap();
                AdmissionErrorKindV1::ClientPidfdTargetMismatch
            }
            _ => {
                use super::super::PidfdIdentitySourceV1;
                client.state.identity_source = match client.state.identity_source {
                    PidfdIdentitySourceV1::KernelIoctl => PidfdIdentitySourceV1::ProcfsFdinfo,
                    PidfdIdentitySourceV1::ProcfsFdinfo => PidfdIdentitySourceV1::KernelIoctl,
                };
                AdmissionErrorKindV1::ClientPidfdIdentityChanged
            }
        };
        assert_eq!(revalidate(&client).unwrap_err().kind(), Some(expected));
        let input = fixture.input();
        for pidfd in [None, Some(&input)] {
            assert_eq!(transfer(&client, pidfd).unwrap_err().kind(), Some(expected));
        }
    }
    let mut client = fixture.admitted();
    client.state.pidfd = File::open("/dev/null").unwrap().into();
    assert_eq!(
        revalidate(&client).unwrap_err().kind(),
        Some(AdmissionErrorKindV1::ClientPidfdIdentityChanged)
    );
    assert_eq!(
        transfer(&client, None).unwrap_err().kind(),
        Some(AdmissionErrorKindV1::ClientPidfdIdentityChanged)
    );
}

#[test]
fn transfer_validates_actual_cloexec_object_and_process_without_retaining_a_duplicate() {
    let fixture = Fixture::new();
    let other = Fixture::new();
    let client = fixture.admitted();
    let input = fixture.input();
    let baseline = fixture.references();
    let other_baseline = other.references();
    rustix::io::fcntl_setfd(&input, rustix::io::FdFlags::empty()).unwrap();
    assert_eq!(
        transfer(&client, Some(&input)).unwrap_err().kind(),
        Some(AdmissionErrorKindV1::ClientPidfdCloseOnExec)
    );
    assert_eq!(fixture.references(), baseline);
    rustix::io::fcntl_setfd(&input, rustix::io::FdFlags::CLOEXEC).unwrap();
    transfer(&client, Some(&input)).unwrap();
    assert_eq!(fixture.references(), baseline);
    let non_pidfd = File::open("/dev/null").unwrap().into();
    assert_eq!(
        transfer(&client, Some(&non_pidfd)).unwrap_err().kind(),
        Some(AdmissionErrorKindV1::ClientPidfdIdentityChanged)
    );
    // Older kernels can share anon-inode metadata for different pidfd targets;
    // the same-object predicate is supplemented by the exact target predicate.
    let error = transfer(&client, Some(&other.witness)).unwrap_err();
    assert!(matches!(
        error.kind(),
        Some(
            AdmissionErrorKindV1::ClientPidfdIdentityChanged
                | AdmissionErrorKindV1::ClientPidfdTargetMismatch
        )
    ));
    assert_eq!(fixture.references(), baseline);
    assert_eq!(other.references(), other_baseline);
    revalidate(&client).unwrap();
}

fn wait_for_exit(pidfd: &OwnedFd) {
    let mut descriptors = [rustix::event::PollFd::new(
        pidfd,
        rustix::event::PollFlags::IN,
    )];
    let timeout = rustix::event::Timespec {
        tv_sec: 5,
        tv_nsec: 0,
    };
    assert_eq!(
        rustix::event::poll(&mut descriptors, Some(&timeout)).unwrap(),
        1
    );
    assert!(
        descriptors[0]
            .revents()
            .contains(rustix::event::PollFlags::IN)
    );
}

#[test]
fn exited_targets_fail_admission_and_revalidation_without_reaping() {
    for admitted in [false, true] {
        let mut fixture = Fixture::new();
        let client = admitted.then(|| fixture.admitted());
        fixture.child.0.kill().unwrap();
        wait_for_exit(&fixture.witness);
        let error = if let Some(client) = client {
            for pidfd in [None, Some(&fixture.witness)] {
                assert_eq!(
                    transfer(&client, pidfd).unwrap_err().kind(),
                    Some(AdmissionErrorKindV1::ClientAlreadyDead)
                );
            }
            revalidate(&client).unwrap_err()
        } else {
            let mut work = Work::new(Client::ADMISSION_WORK);
            let mut budget = Budget::new(&mut work, Client::FD_STORAGE + Client::IO_STORAGE);
            budget.reserve_storage(Client::FD_STORAGE).unwrap();
            Client::admit(fixture.input(), fixture.expected(), &mut budget).unwrap_err()
        };
        assert_eq!(error.kind(), Some(AdmissionErrorKindV1::ClientAlreadyDead));
        // std::process::Child caches wait results; inspect waitid directly first.
        let status = rustix::process::waitid(
            rustix::process::WaitId::PidFd(fixture.witness.as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOWAIT
                | rustix::process::WaitIdOptions::NOHANG,
        )
        .unwrap();
        assert!(status.is_some());
        assert!(!fixture.child.0.wait().unwrap().success());
    }
}

#[test]
fn unwind_closes_the_consumed_descriptor_and_restores_storage_without_work_refund() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    let input = fixture.input();
    let floor = EXTRA + Client::FD_STORAGE;
    let mut work = Work::new(Client::ADMISSION_WORK);
    let mut budget = Budget::new(&mut work, floor + Client::IO_STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Client::scope::<()>(
            &mut budget,
            Client::FD_STORAGE,
            Client::ADMISSION_WORK,
            |_| {
                let _owned = input;
                panic!("native client inspection unwound");
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(budget.work(), Client::ADMISSION_WORK);
    assert_eq!(budget.storage(), floor);
    assert_eq!(fixture.references(), baseline);
}

thread_local! {
    static COUNTS: Cell<(usize, usize, usize)> = const { Cell::new((0, 0, 0)) };
    static FAULT_AT_STAT: Cell<usize> = const { Cell::new(0) };
    static UNWIND_AT_FAULT: Cell<bool> = const { Cell::new(false) };
}
struct Counting;
impl continuity::InspectionIo for Counting {
    type Error = CheckError;
    const MIN_DUP_FD: i32 = 3;
    fn fdinfo(
        fd: &OwnedFd,
    ) -> std::result::Result<super::super::PidfdTargetObservationV1, CheckError> {
        COUNTS.set({
            let (a, b, c) = COUNTS.get();
            (a + 1, b, c)
        });
        <Native as continuity::InspectionIo>::fdinfo(fd)
    }
    fn start_time(pid: u32) -> std::result::Result<u64, CheckError> {
        COUNTS.set({
            let (a, b, c) = COUNTS.get();
            (a, b + 1, c)
        });
        if FAULT_AT_STAT.get() == COUNTS.get().1 {
            FAULT_AT_STAT.set(0);
            assert!(
                !UNWIND_AT_FAULT.replace(false),
                "injected native stat unwind"
            );
            return Err(CheckError::new(
                AdmissionErrorKindV1::InspectClientStartTime,
                "injected native stat refusal",
            ));
        }
        <Native as continuity::InspectionIo>::start_time(pid)
    }
    fn poll(fd: &OwnedFd) -> std::result::Result<(i32, i16), CheckError> {
        COUNTS.set({
            let (a, b, c) = COUNTS.get();
            (a, b, c + 1)
        });
        <Native as continuity::InspectionIo>::poll(fd)
    }
    fn io_error(
        kind: AdmissionErrorKindV1,
        message: &'static str,
        error: std::io::Error,
    ) -> CheckError {
        <Native as continuity::InspectionIo>::io_error(kind, message, error)
    }
}

#[test]
fn shared_schedule_has_three_then_two_target_stat_pairs_without_extra_admission() {
    let fixture = Fixture::new();
    COUNTS.set((0, 0, 0));
    let state =
        LiveClientPidfdIdentityV1::admit_with::<Counting>(fixture.input(), fixture.expected())
            .unwrap();
    let (fdinfo, stat, polls) = COUNTS.replace((0, 0, 0));
    assert!(fdinfo <= 3);
    assert_eq!((stat, polls), (3, 2));
    state.validate_liveness_with::<Counting>().unwrap();
    let (fdinfo, stat, polls) = COUNTS.replace((0, 0, 0));
    assert!(fdinfo <= 2);
    assert_eq!((stat, polls), (2, 2));
}

#[test]
fn transfer_reuses_six_pair_shared_schedule_for_returned_and_borrowed_descriptors() {
    let fixture = Fixture::new();
    let client = fixture.admitted();
    let transfer = fixture.input();
    let baseline = fixture.references();
    for input in [&client.state.pidfd, &transfer] {
        COUNTS.set((0, 0, 0));
        let fd = client.checked_duplicate_with::<Counting>(input).unwrap();
        let (fdinfo, stat, polls) = COUNTS.replace((0, 0, 0));
        assert!(fdinfo <= 6);
        assert_eq!((stat, polls), (6, 6));
        assert_eq!(fixture.references(), baseline + 1);
        drop(fd);
        assert_eq!(fixture.references(), baseline);
    }
}

#[test]
fn in_flight_transfer_refusal_and_unwind_close_temporary_pidfds_and_restore_storage() {
    let fixture = Fixture::new();
    let client = fixture.admitted();
    let input = fixture.input();
    let baseline = fixture.references();
    for (source, floor, quota) in [
        (
            &client.state.pidfd,
            Client::RETAINED,
            Client::CLONE_TRANSFER_WORK,
        ),
        (
            &input,
            Client::RETAINED + Client::FD_STORAGE,
            Client::VALIDATE_TRANSFER_WORK,
        ),
    ] {
        for stat in [3, 5] {
            for unwind in [false, true] {
                COUNTS.set((0, 0, 0));
                FAULT_AT_STAT.set(stat);
                UNWIND_AT_FAULT.set(unwind);
                let mut work = Work::new(quota);
                let mut budget = Budget::new(&mut work, floor + Client::IO_STORAGE);
                budget.reserve_storage(floor).unwrap();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Client::scope(&mut budget, floor, quota, |_| {
                        Ok(client.checked_duplicate_with::<Counting>(source)?)
                    })
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    let error = result.unwrap().unwrap_err();
                    assert_eq!(
                        error.kind(),
                        Some(AdmissionErrorKindV1::InspectClientStartTime)
                    );
                    assert_eq!(error.resource(), None);
                }
                assert_eq!(COUNTS.get().1, stat);
                assert_eq!(FAULT_AT_STAT.get(), 0);
                assert_eq!(budget.work(), quota);
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor + Client::IO_STORAGE);
                assert_eq!(fixture.references(), baseline);
            }
        }
    }
    revalidate(&client).unwrap();
}

#[test]
fn current_process_start_time_exact_and_short_budgets_preserve_the_caller_ledger() {
    let expected = super::super::current_process_start_time_ticks_v1().unwrap();
    for prepaid in [0, EXTRA] {
        for case in 0..4 {
            let quota = CURRENT_PROCESS_START_TIME_WORK_V2;
            let work_limit = match case {
                0 => native::ENTRY_WORK - 1,
                1 => quota - 1,
                _ => quota,
            };
            let storage_limit =
                prepaid + CURRENT_PROCESS_START_TIME_IO_STORAGE_V2 - usize::from(case == 2);
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(prepaid).unwrap();
            let result = current_process_start_time_ticks_v2(&mut budget);
            assert_eq!(budget.storage(), prepaid);
            match case {
                0 | 1 => {
                    assert!(matches!(
                        result.unwrap_err().resource(),
                        Some(Resource::Work(_))
                    ));
                    assert_eq!(
                        budget.work(),
                        if case == 0 { 0 } else { native::ENTRY_WORK }
                    );
                    assert_eq!(budget.peak_storage(), prepaid);
                }
                2 => {
                    assert!(matches!(
                        result.unwrap_err().resource(),
                        Some(Resource::Storage(_))
                    ));
                    assert_eq!(budget.work(), quota);
                    assert_eq!(budget.peak_storage(), prepaid);
                    assert_eq!(budget.failed_storage(), Some(storage_limit + 1));
                }
                _ => {
                    assert_eq!(result.unwrap(), expected);
                    assert_eq!(budget.work(), quota);
                    assert_eq!(budget.peak_storage(), storage_limit);
                }
            }
            assert_eq!(
                work.failed_work(),
                match case {
                    0 => Some(native::ENTRY_WORK),
                    1 => Some(quota),
                    _ => None,
                }
            );
        }
    }
}

#[test]
fn current_process_start_time_reuses_work_and_preserves_sticky_denials() {
    let quota = CURRENT_PROCESS_START_TIME_WORK_V2;
    let scratch = CURRENT_PROCESS_START_TIME_IO_STORAGE_V2;
    for storage_denial in [false, true] {
        let mut work = Work::new(if storage_denial { 2 * quota } else { quota - 1 });
        let mut budget = Budget::new(&mut work, EXTRA + scratch - usize::from(storage_denial));
        budget.reserve_storage(EXTRA).unwrap();
        for attempt in 1..=2 {
            let error = current_process_start_time_ticks_v2(&mut budget).unwrap_err();
            if storage_denial {
                assert!(matches!(error.resource(), Some(Resource::Storage(_))));
                assert_eq!(budget.work(), attempt * quota);
                assert_eq!(budget.failed_storage(), Some(EXTRA + scratch));
            } else {
                assert!(matches!(error.resource(), Some(Resource::Work(_))));
                assert_eq!(budget.work(), attempt * native::ENTRY_WORK);
            }
            assert_eq!(budget.storage(), EXTRA);
        }
        assert_eq!(work.failed_work(), (!storage_denial).then_some(quota));
    }
    let mut work = Work::new(2 * quota);
    let mut budget = Budget::new(&mut work, EXTRA + scratch);
    budget.reserve_storage(EXTRA).unwrap();
    let first = current_process_start_time_ticks_v2(&mut budget).unwrap();
    let second = current_process_start_time_ticks_v2(&mut budget).unwrap();
    assert_eq!(first, second);
    assert_eq!(budget.work(), 2 * quota);
    assert_eq!(budget.storage(), EXTRA);
    assert_eq!(budget.peak_storage(), EXTRA + scratch);
}
