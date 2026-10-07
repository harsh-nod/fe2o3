use super::*;
use std::os::fd::FromRawFd;

fn pidfd_for(pid: u32) -> OwnedFd {
    // SAFETY: scalar arguments; success returns one exclusively owned descriptor.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    assert!(raw >= 0, "pidfd_open: {}", io::Error::last_os_error());
    // SAFETY: the successful syscall returned a new owned descriptor.
    unsafe { OwnedFd::from_raw_fd(raw as i32) }
}

#[test]
fn native_received_owner_revalidates_original_process_and_exact_fallback() {
    let pid = std::process::id();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(97).unwrap();
    let (owner, charge) =
        NativeReceivedProcessPidfdV1::admit_received(pidfd_for(pid), pid, &mut budget).unwrap();
    assert_eq!(budget.storage(), 97);
    assert!(budget.work() > 0);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let floor = budget.storage();
    owner.revalidate(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(owner.pid(), pid);
    assert_eq!(
        owner.start_time_ticks(),
        current_process_start_time_ticks_v1().unwrap()
    );
    let target = inspect_fdinfo(&owner.pidfd, &mut budget).unwrap();
    assert_eq!(
        target,
        PidfdTargetObservationV1 {
            pid,
            source: PidfdIdentitySourceV1::ProcfsFdinfo
        }
    );
    assert!(!format!("{owner:?}").contains("raw_fd"));
}

#[test]
fn native_admission_rejects_wrong_target_non_pidfd_and_non_cloexec() {
    let pid = std::process::id();
    let not_cloexec = pidfd_for(pid);
    rustix::io::fcntl_setfd(&not_cloexec, rustix::io::FdFlags::empty()).unwrap();
    for (fd, expected, kind) in [
        (pidfd_for(pid), 0, PidfdObservationErrorKindV1::ExpectedPid),
        (
            pidfd_for(pid),
            pid + 1,
            PidfdObservationErrorKindV1::TargetMismatch,
        ),
        (
            File::open("/dev/null").unwrap().into(),
            pid,
            PidfdObservationErrorKindV1::InspectPidfd,
        ),
        (not_cloexec, pid, PidfdObservationErrorKindV1::CloseOnExec),
    ] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(97).unwrap();
        let error =
            NativeReceivedProcessPidfdV1::admit_received(fd, expected, &mut budget).unwrap_err();
        assert!(
            matches!(error, NativeReceivedProcessPidfdErrorV1::Observation(error) if error.kind() == kind)
        );
        assert_eq!(budget.storage(), 97 + SCRATCH);
    }
}

#[test]
fn native_owner_rejects_foreign_ledger_before_any_charge_or_syscall() {
    let pid = std::process::id();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let (owner, charge) =
        NativeReceivedProcessPidfdV1::admit_received(pidfd_for(pid), pid, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let mut foreign_work = Work::new(1_000_000);
    let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
    foreign
        .reserve_storage(charge.additional_storage())
        .unwrap();
    assert!(matches!(
        owner.revalidate(&mut foreign),
        Err(NativeReceivedProcessPidfdErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), charge.additional_storage());
    owner.revalidate(&mut budget).unwrap();
}

#[test]
fn native_owner_requires_prepaid_retained_storage_and_preserves_failure_prefix() {
    let pid = std::process::id();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let (owner, charge) =
        NativeReceivedProcessPidfdV1::admit_received(pidfd_for(pid), pid, &mut budget).unwrap();
    let accepted = budget.work();
    assert!(matches!(
        owner.revalidate(&mut budget),
        Err(NativeReceivedProcessPidfdErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(budget.work(), accepted);
    assert_eq!(budget.storage(), 0);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    rustix::io::fcntl_setfd(&owner.pidfd, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(owner.revalidate(&mut budget),
        Err(NativeReceivedProcessPidfdErrorV1::Observation(error))
            if error.kind() == PidfdObservationErrorKindV1::CloseOnExec));
    assert_eq!(budget.storage(), charge.additional_storage() + SCRATCH);
}

#[test]
fn native_read_attempts_fail_eintr_without_retry_and_charge_before_io() {
    for failure_at in [1, 2] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 0);
        let mut bytes = [0; RECORD_BYTES + 1];
        let mut calls = 0;
        let result = read_record_with(
            &mut bytes,
            PidfdObservationErrorKindV1::InspectPidfd,
            &mut budget,
            |buffer, offset| {
                calls += 1;
                if calls == failure_at {
                    return Err(rustix::io::Errno::INTR);
                }
                assert_eq!(offset, 0);
                buffer[..3].copy_from_slice(b"abc");
                Ok(3)
            },
        );
        assert_eq!(calls, failure_at);
        assert_eq!(budget.work(), RECORD_BYTES + 1 + calls);
        let NativeReceivedProcessPidfdErrorV1::Observation(error) = result.unwrap_err() else {
            panic!("unexpected refusal")
        };
        assert_eq!(
            error.into_parts().2.unwrap().raw_os_error(),
            Some(libc::EINTR)
        );
    }
    for allowed in [0, RECORD_BYTES + 1] {
        let mut work = Work::new(allowed);
        let mut budget = Budget::new(&mut work, 0);
        let mut bytes = [0; RECORD_BYTES + 1];
        let result = read_record_with(
            &mut bytes,
            PidfdObservationErrorKindV1::InspectPidfd,
            &mut budget,
            |_, _| panic!("work denial must precede I/O"),
        );
        assert!(matches!(
            result,
            Err(NativeReceivedProcessPidfdErrorV1::Resource(Resource::Work(
                _
            )))
        ));
        assert_eq!(budget.work(), allowed);
        assert!(budget.failed_work().is_some());
    }
}

#[test]
fn native_read_rejects_empty_oversized_and_incomplete_records() {
    for (length, trailing) in [(0, 0), (RECORD_BYTES + 1, 0), (3, 1)] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 0);
        let mut bytes = [0; RECORD_BYTES + 1];
        let mut calls = 0;
        let result = read_record_with(
            &mut bytes,
            PidfdObservationErrorKindV1::InspectPidfd,
            &mut budget,
            |_, offset| {
                calls += 1;
                Ok(if offset == 0 { length } else { trailing })
            },
        );
        assert!(matches!(
            result,
            Err(NativeReceivedProcessPidfdErrorV1::Observation(_))
        ));
        assert_eq!(calls, if length == 3 { 2 } else { 1 });
    }
}

#[test]
fn native_owner_detects_all_retained_identity_substitutions() {
    let pid = std::process::id();
    for field in 0..9 {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (mut owner, charge) =
            NativeReceivedProcessPidfdV1::admit_received(pidfd_for(pid), pid, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        match field {
            0 => owner.object.device ^= 1,
            1 => owner.object.inode ^= 1,
            2 => owner.object.mode ^= 1,
            3 => owner.object.uid ^= 1,
            4 => owner.object.gid ^= 1,
            5 => owner.object.links ^= 1,
            6 => owner.target.pid += 1,
            7 => {
                owner.target.source = match owner.target.source {
                    PidfdIdentitySourceV1::KernelIoctl => PidfdIdentitySourceV1::ProcfsFdinfo,
                    PidfdIdentitySourceV1::ProcfsFdinfo => PidfdIdentitySourceV1::KernelIoctl,
                }
            }
            _ => owner.start_time_ticks += 1,
        }
        let error = owner.revalidate(&mut budget).unwrap_err();
        let expected = if field == 8 {
            PidfdObservationErrorKindV1::StartTimeChanged
        } else {
            PidfdObservationErrorKindV1::IdentityChanged
        };
        assert!(
            matches!(error, NativeReceivedProcessPidfdErrorV1::Observation(error) if error.kind() == expected)
        );
    }
}
