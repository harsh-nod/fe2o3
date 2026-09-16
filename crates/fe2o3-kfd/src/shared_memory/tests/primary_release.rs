//! Permanent restoration and teardown on the original constructor memory fixture.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

impl PreparationMemoryFixtureV1 {
    pub(crate) fn primary_quarantine_release_v1(&mut self) {
        let _: Result<(), MemorySessionError> =
            self.fixture
                .engine
                .quarantine(MemorySessionError::KernelResultMalformed(
                    "terminal directional SDMA release",
                ));
    }

    pub(crate) fn primary_is_quarantined_v1(&self) -> bool {
        self.fixture.engine.phase == SharedMemorySessionPhaseV1::Quarantined
    }

    pub(crate) fn primary_release_sdma_resources_v1(
        &mut self,
        resources: &mut SdmaResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        assert!(matches!(
            self.fixture.ownership.phase,
            QueueModelOwnershipPhaseV1::SessionOwned
        ));
        let before = resources.observation();
        let f = &mut self.fixture;
        let mut projection = control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm);
        let result = catch_unwind(AssertUnwindSafe(|| {
            queue_cleanup::release_v1(&mut f.engine, &mut projection, resources, || {
                self.control_release_process_poisoned += 1;
            })
        }));
        if !before.started {
            for (index, control) in before.controls.iter().enumerate() {
                let record = f
                    .engine
                    .allocations
                    .iter()
                    .find(|r| r.id == control.identity.id)
                    .unwrap();
                assert_eq!(record.generation, control.identity.generation);
                if record.phase == SharedAllocationPhaseV1::Released {
                    if index == 0 {
                        assert_eq!(record.profile, SharedGttProfileV1::HostVisibleCoherent);
                        self.disposed_host_data.push(control.identity);
                    } else {
                        self.disposed_queue_resources
                            .push((control.identity, record.profile));
                    }
                }
            }
        }
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
    pub(crate) fn primary_fail_currentness_v1(&mut self, offset: usize, panic: bool) {
        let backend = &mut self.fixture.engine.backend;
        let at = backend.currentness_calls + offset;
        if panic {
            backend.panic_currentness_at = Some(at);
        } else {
            backend.fail_currentness_at = Some(at);
        }
    }
    pub(crate) fn primary_fail_cleanup_call_v1(
        &mut self,
        ordinal: usize,
        operation: &'static str,
        panic: bool,
    ) {
        let backend = &mut self.fixture.engine.backend;
        backend.cleanup_fault = Some((backend.cleanup_calls.len() + ordinal, operation, panic));
    }
    pub(crate) fn primary_restore_foundation_v1(
        &mut self,
        queue: &mut QueueModelFoundationV1,
        foreign_vm: bool,
    ) -> Result<(), MemorySessionError> {
        let f = &mut self.fixture;
        let mut vm = f.vm;
        if foreign_vm {
            vm.id.0 += 1;
        }
        let snapshot = |q: &QueueModelFoundationV1| {
            (
                q.identity().clone(),
                q.memory().clone(),
                q.certificate_snapshot_for_test(),
            )
        };
        let before = foreign_vm.then(|| {
            (
                snapshot(queue),
                snapshot(&f.foundation),
                f.ownership.phase,
                f.ownership.next_live_loan_generation,
            )
        });
        let result =
            f.ownership
                .restore_foundation(&mut f.engine, &mut f.foundation, queue, f.device, vm);
        if let Some(before) = before {
            assert!(result.is_err());
            assert_eq!(
                (
                    snapshot(queue),
                    snapshot(&f.foundation),
                    f.ownership.phase,
                    f.ownership.next_live_loan_generation
                ),
                before
            );
            assert_eq!(f.engine.phase, SharedMemorySessionPhaseV1::Quarantined);
        }
        result
    }

    pub(crate) fn primary_release_queue_resources_v1(
        &mut self,
        resources: &mut QueueResourceCleanupCustodyV1,
        fault: Option<(usize, control_cleanup::CleanupStageV1, bool)>,
    ) -> Result<(), MemorySessionError> {
        assert!(matches!(
            self.fixture.ownership.phase,
            QueueModelOwnershipPhaseV1::SessionOwned
        ));
        let before = resources.observation();
        let f = &mut self.fixture;
        let mut projection = control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm);
        if let Some((index, stage, panic)) = fault {
            projection.fault = Some((
                stage,
                if panic {
                    adapter::ProjectionFaultV1::Panic
                } else {
                    adapter::ProjectionFaultV1::Error
                },
            ));
            projection.skip_fault_matches = index;
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            queue_cleanup::release_v1(&mut f.engine, &mut projection, resources, || {
                self.control_release_process_poisoned += 1
            })
        }));
        if !before.started {
            for (index, control) in before.controls.iter().enumerate() {
                let profile = [
                    AqlQueueGttV1::PROFILE,
                    UserptrAqlControlGttV1::PROFILE,
                    ExecutableGttV1::PROFILE,
                    ExecutableGttV1::PROFILE,
                ][index];
                assert_eq!(
                    control.profile_type,
                    [
                        std::any::TypeId::of::<AqlQueueGttV1>(),
                        std::any::TypeId::of::<UserptrAqlControlGttV1>(),
                        std::any::TypeId::of::<ExecutableGttV1>(),
                        std::any::TypeId::of::<ExecutableGttV1>()
                    ][index]
                );
                let record = f
                    .engine
                    .allocations
                    .iter()
                    .find(|r| r.id == control.identity.id)
                    .unwrap();
                assert_eq!(record.generation, control.identity.generation);
                assert_eq!(record.profile, profile);
                if record.phase == SharedAllocationPhaseV1::Released {
                    self.disposed_queue_resources
                        .push((control.identity, profile));
                }
            }
        }
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }

    pub(crate) fn primary_release_signals_v1(
        &mut self,
        signals: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        assert!(matches!(
            self.fixture.ownership.phase,
            QueueModelOwnershipPhaseV1::SessionOwned
        ));
        let before = signals.observation();
        let f = &mut self.fixture;
        let mut projection = control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm);
        let result = catch_unwind(AssertUnwindSafe(|| {
            control_cleanup::release_v1(&mut f.engine, &mut projection, signals, || {
                self.control_release_process_poisoned += 1
            })
        }));
        if !before.started {
            let record = f
                .engine
                .allocations
                .iter()
                .find(|r| r.id == before.identity.id)
                .unwrap();
            assert_eq!(record.generation, before.identity.generation);
            assert_eq!(record.profile, SharedGttProfileV1::HostVisibleCoherent);
            if record.phase == SharedAllocationPhaseV1::Released {
                self.disposed_host_data.push(before.identity);
            }
        }
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }

    pub(crate) fn primary_assert_permanently_restored_v1(&self, queue: &QueueModelFoundationV1) {
        assert!(matches!(
            self.fixture.ownership.phase,
            QueueModelOwnershipPhaseV1::SessionOwned
        ));
        assert!(!queue.is_certified_for_test());
        assert!(!self.fixture.foundation.is_certified_for_test());
        assert!(queue.memory().allocations().is_empty());
    }

    pub(crate) fn primary_assert_all_released_v1(&self) {
        self.assert_disposed_controls_v1();
        let e = &self.fixture.engine;
        assert!(
            e.allocations
                .iter()
                .all(|r| r.phase == SharedAllocationPhaseV1::Released)
        );
        assert!(e.device_memory.iter().all(|r| r.is_fully_released()));
        assert!(e.allocation_record_slots.is_empty());
        assert!(e.device_memory_record_slots.is_empty());
        assert_eq!(e.retained_gpu_va_bytes, 0);
        assert_eq!(e.retained_device_memory_bytes, 0);
        let host = e.host_backing_account.as_ref().unwrap().usage();
        let device = self.fixture.usage().unwrap();
        assert_eq!(
            (host.used_backing_bytes, host.used_allocation_records),
            (0, 0)
        );
        assert_eq!(
            (device.used_backing_bytes, device.used_allocation_records),
            (0, 0)
        );
        assert_eq!(
            (
                host.reserved_records,
                host.retained_records,
                host.quarantined_records,
                host.poisoned
            ),
            (0, 0, 0, false)
        );
        assert_eq!(
            (
                device.reserved_records,
                device.retained_records,
                device.quarantined_records,
                device.poisoned
            ),
            (0, 0, 0, false)
        );
    }
}
