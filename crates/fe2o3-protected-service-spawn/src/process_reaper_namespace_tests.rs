use super::*;

impl Service {
    // Dedicated reexec fixture only: intentionally quarantined inert records
    // retain their harmless pipes until subprocess exit. There is no real child.
    pub(crate) fn check_namespace_pool_retention_fixture() {
        use crate::RetainedResourcesV2 as Retained;
        use crate::native_cgroup::NativeCgroupDomainV1 as Domain;
        use crate::native_user_namespace::NativeUserNamespaceV1 as Namespace;
        use rustix::io::{Errno, read};
        use rustix::pipe::{PipeFlags, pipe_with};
        use std::os::fd::OwnedFd;

        const INPUT: usize = std::mem::size_of::<OwnedFd>() + std::mem::size_of::<usize>();
        let payload = Retained::<OwnedFd>::payload_storage(INPUT).unwrap();
        let request = Retained::<OwnedFd>::storage_for(INPUT).unwrap();
        let retain_work = Service::retained_launch_work::<OwnedFd>(INPUT).unwrap();
        let limit = Service::ADMISSION_WORK
            + retain_work
            + MIN_TURN_WORK
            + Service::RECOVERY_WORK
            + SHUTDOWN_WORK;
        let reaper = Box::leak(Box::new(DeferredReaperV1::new()));
        let mut service = Service::admit_at(
            reaper,
            Account::new(Work::new(limit), Service::STORAGE + payload),
        )
        .unwrap();
        let mut work = Work::new(retain_work);
        let mut budget = Budget::new(
            &mut work,
            INPUT + Service::retained_launch_scratch::<OwnedFd>(INPUT).unwrap(),
        );
        budget.reserve_storage(INPUT).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let (input_reader, input_writer) =
            pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
        let (slot, view, growth) = service
            .reserve_retaining(input_writer, INPUT, &mut budget)
            .unwrap();
        budget.reserve_storage(growth.additional_storage()).unwrap();
        let (namespace_reader, namespace_writer) =
            pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
        let mut child = ChildCleanupV1::new_with_domain_and_namespace(
            None,
            synthetic_pid(0),
            None,
            Domain::quarantined_fixture_for_cleanup(),
            Namespace::poisoned_fixture_for_cleanup(namespace_writer),
        );
        child.terminal_reaped(); // Inert state marker only; domain remains pending.
        slot.into_slot().defer(child);
        drop(view);
        budget.release_storage(request).unwrap();
        assert!(identity == budget.work_ledger_identity_v1());
        assert_eq!((budget.work(), budget.storage()), (retain_work, 0));
        assert_eq!(reaper.cells[0].state.load(Ordering::Acquire), DEFERRED);
        assert_eq!(
            service.report().unwrap().storage,
            Service::STORAGE + payload
        );
        assert_eq!(read(&input_reader, &mut [0]), Err(Errno::AGAIN));
        assert_eq!(read(&namespace_reader, &mut [0]), Err(Errno::AGAIN));
        let report = service.pump(1).unwrap();
        assert_eq!(
            report.work,
            Service::ADMISSION_WORK + retain_work + MIN_TURN_WORK
        );
        assert_eq!(report.storage, Service::STORAGE + payload);
        assert_record(reaper, 0, QUARANTINED);
        drop(service);
        let mut service = Service::recover_at(reaper).unwrap();
        assert_eq!(
            service.report().unwrap().work,
            report.work + Service::RECOVERY_WORK
        );
        assert!(matches!(service.shutdown(), Err(Failure::Busy)));
        assert!(matches!(service.shutdown(), Err(Failure::Busy)));
        let retained = service.report().unwrap();
        assert_eq!((retained.work, retained.work_limit), (limit, limit));
        assert_eq!(retained.storage, Service::STORAGE + payload);
        assert_eq!(read(&input_reader, &mut [0]), Err(Errno::AGAIN));
        assert_eq!(read(&namespace_reader, &mut [0]), Err(Errno::AGAIN));
        assert_record(reaper, 0, QUARANTINED);
    }
}
