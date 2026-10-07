//! Allocation-only composition through the original constructed memory engine.
use super::*;
use crate::queue::live::data_insertion::DataInsertionRootV1;
use crate::shared_memory::{DeviceAllocationCustodyV1, DeviceAllocationSnapshotV1};

#[derive(Default)]
struct AllocationTrace {
    before_retake: Option<DeviceAllocationSnapshotV1>,
    before_failure: Option<DeviceAllocationSnapshotV1>,
}

struct AllocationContext<'a> {
    base: Context<'a>,
    trace: &'a mut AllocationTrace,
}

impl DataInsertionContextV1<DeviceAllocationCustodyV1, (u64, u64)> for AllocationContext<'_> {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.base.require_unbound()
    }
    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_> {
        self.base.ledger()
    }
    fn reserve(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.base.trace.reserve_calls += 1;
        outcome("insertion-reserve", self.base.trace.reserve)?;
        let ledger = self.base.ledger();
        reserve_identity_capacity_v1(
            ledger.identities,
            DeviceAllocationCustodyV1::LEDGER_OPERATION,
        )?;
        self.base.trace.reserved_storage = Some((
            ledger.identities.as_ptr() as usize,
            ledger.identities.capacity(),
        ));
        Ok(())
    }
    fn prepare(
        &mut self,
        root: &mut DeviceAllocationCustodyV1,
        (bytes, alignment): (u64, u64),
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.base.trace.prepare_calls += 1;
        let before = self.base.ledger_snapshot();
        let ledger = self.base.ledger();
        assert!(ledger.identities.capacity() >= 16);
        let storage = (ledger.identities.as_ptr(), ledger.identities.capacity());
        let skip = self.base.trace.skip_prepare;
        let trace = &mut self.trace;
        let result = self.base.parent.with_preparation_custody(|memory| {
            if skip {
                return Ok(());
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                memory.primary_prepare_device_allocation_v1(root, bytes, alignment)
            }));
            trace.before_retake = Some(root.insertion_snapshot_for_test());
            match result {
                Ok(result) => result.map_err(Into::into),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        });
        assert_eq!(
            self.base.ledger_snapshot(),
            before,
            "no allocation metadata commit before retake"
        );
        let ledger = self.base.ledger();
        assert_eq!(
            (ledger.identities.as_ptr(), ledger.identities.capacity()),
            storage
        );
        let (operation, retake) = result?;
        retake?;
        operation?;
        self.base.trace.commit_ready = true;
        Ok(())
    }
    fn fail(&mut self, root: DeviceAllocationCustodyV1, panicked: bool) {
        self.base.trace.fail_calls += 1;
        self.base.trace.panic_fail |= panicked;
        self.trace.before_failure = Some(root.insertion_snapshot_for_test());
        if let Some(lane) = self.base.lane.as_mut() {
            lane.completion_owner.poison_owner();
            lane.submission.as_mut().unwrap().poison();
            lane.unpublished_dispatch.continuation = None;
        } else {
            self.base.primary.continuation = None;
        }
        self.base.parent.poison();
        if panicked {
            Fixture::poison();
        }
        if root.requires_retention() {
            self.base
                .parent
                .original
                .as_mut()
                .unwrap()
                .primary
                .completed
                .as_mut()
                .unwrap()
                .engine
                .backend
                .session
                .primary_retain_device_allocation_v1(root);
        }
    }
}

struct AllocationFixture {
    base: InsertionFixture,
    trace: AllocationTrace,
    model: fe2o3_runtime_model::MemoryLifecycleStateV1,
}

fn layout(bytes: u64) -> crate::Gfx942DeviceMemoryLayoutV1 {
    crate::shared_memory::device_memory_layout(bytes, 4096, KfdAllocMemoryFlags::DEVICE_LOCAL)
        .unwrap()
}

impl AllocationFixture {
    fn new(ordinal: usize) -> Self {
        let base = InsertionFixture::new(ordinal);
        let engine = &base.scope.primary.completed.as_ref().unwrap().engine;
        let model = engine
            .backend
            .session
            .insertion_model_snapshot_v1(&engine.foundation);
        Self {
            base,
            trace: AllocationTrace::default(),
            model,
        }
    }
    fn insert(&mut self, bytes: u64, alignment: u64) -> SettledDataInsertionV1 {
        settle_data_insertion_v1(
            &mut AllocationContext {
                base: self.base.context(),
                trace: &mut self.trace,
            },
            DeviceAllocationCustodyV1::new(),
            DataInsertionIndexV1::HoleOrAppend,
            (bytes, alignment),
        )
    }
    fn assert_model(&self) {
        let engine = &self.base.scope.primary.completed.as_ref().unwrap().engine;
        assert_eq!(
            engine
                .backend
                .session
                .insertion_model_snapshot_v1(&engine.foundation),
            self.model
        );
    }
    fn assert_partition(&self, id: u64, bytes: u64) {
        self.base.assert_partition_with_layout(id, layout(bytes));
        self.assert_model();
    }
    fn finish(&mut self, transport: bool, id: u64, bytes: u64) {
        let native = self.base.memory().insertion_memory_snapshot_v1();
        self.base.restore_and_transport(transport);
        assert_eq!(self.base.memory().insertion_memory_snapshot_v1(), native);
        self.assert_partition(id, bytes);
    }
    fn assert_retry(&mut self) {
        assert!(self.base.scope.parent.poisoned);
        assert!(
            self.base
                .scope
                .primary
                .completed
                .as_ref()
                .unwrap()
                .completion_owner
                .is_poisoned_for_test()
        );
        if let Some(lane) = &self.base.lane {
            assert!(lane.completion_owner.is_poisoned_for_test());
            assert!(lane.submission.as_ref().unwrap().is_poisoned_for_test());
        }
        self.base.memory_mut().insertion_clear_faults_v1();
        self.base.scope.parent.faults = Faults::default();
        let native = self.base.memory().insertion_memory_snapshot_v1();
        let observation = self.base.memory().observation();
        let ledger = self.base.context().ledger_snapshot();
        let counts = (
            self.base.trace.reserve_calls,
            self.base.trace.prepare_calls,
            self.base.trace.fail_calls,
        );
        let retry = self.insert(17, 4096);
        assert!(!retry.transport);
        assert!(matches!(
            retry.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            )))
        ));
        assert_eq!(self.base.memory().insertion_memory_snapshot_v1(), native);
        assert_eq!(self.base.memory().observation(), observation);
        assert_eq!(self.base.context().ledger_snapshot(), ledger);
        assert_eq!(
            (
                self.base.trace.reserve_calls,
                self.base.trace.prepare_calls,
                self.base.trace.fail_calls
            ),
            counts
        );
        self.assert_model();
    }
    fn assert_prefix(
        &self,
        before: &DeviceInsertionMemorySnapshotV1,
        bytes: u64,
        calls: [usize; 5],
        phase: Option<&'static str>,
        handle: bool,
    ) {
        self.base
            .memory()
            .insertion_assert_native_prefix_with_layout_v1(
                before,
                DeviceInsertionPrefixV1 {
                    calls,
                    phase,
                    handle,
                    cpu_writable: None,
                    written: false,
                    operations: if calls[4] == 1 { &["map_gpu"] } else { &[] },
                },
                layout(bytes),
                None,
            );
        let after = self.base.memory().insertion_memory_snapshot_v1();
        assert_eq!(after.account, before.account);
        assert_eq!(after.terminal_storage, before.terminal_storage);
    }
}

#[test]
fn device_allocation_insertion_constructed_append_preserves_exact_uninitialized_identity() {
    for ordinal in 0..3 {
        for bytes in [1, 17, 4097] {
            let mut f = AllocationFixture::new(ordinal);
            let native = f.base.memory().insertion_memory_snapshot_v1();
            let loan = f.base.loan_state();
            let (mut identities, count, next) = f.base.context().ledger_snapshot();
            assert_eq!(next, None);
            let settled = f.insert(bytes, 4096);
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            assert!(!data.is_fully_initialized());
            assert_eq!(data.initialized_content(), None);
            assert_eq!(
                data.layout().kind(),
                crate::Gfx942FixedDispatchDataKindV1::DeviceLocal
            );
            assert_eq!(data.layout().requested_bytes(), bytes);
            assert_eq!(data.layout().alignment(), 4096);
            let Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(identity) =
                data.storage_identity()
            else {
                panic!("allocation cannot grant initialized authority");
            };
            let root = f.trace.before_retake.as_ref().unwrap();
            assert!(root.started() && root.native_started() && !root.failed());
            assert_eq!(root.lease(), Some((identity, layout(bytes), true)));
            assert_eq!(root.progress(), (true, Some(true), Some(1)));
            identities.push(data.storage_identity());
            f.base.data.push(data);
            assert_eq!(
                f.base.context().ledger_snapshot(),
                (identities, count + 1, None)
            );
            assert_eq!(
                (
                    f.base.trace.reserve_calls,
                    f.base.trace.prepare_calls,
                    f.base.trace.fail_calls
                ),
                (1, 1, 0)
            );
            assert!(f.trace.before_failure.is_none());
            assert!(
                !f.base
                    .memory()
                    .insertion_memory_snapshot_v1()
                    .terminal_occupied
            );
            assert_eq!(f.base.loan_state(), (loan.0, None, loan.2 + 1));
            f.assert_prefix(&native, bytes, [4, 1, 1, 0, 1], Some("Mapped"), true);
            f.finish(false, native.next_id, bytes);
        }
    }
}

#[test]
fn device_allocation_insertion_uses_genuine_released_hole_before_append() {
    for ordinal in 0..3 {
        let mut f = AllocationFixture::new(ordinal);
        let old = f.base.data[2].storage_identity();
        let mut data = Some(f.base.data.remove(2));
        let mut released = None;
        let (operation, retake) = f
            .base
            .scope
            .parent
            .with_preparation_custody(|memory| {
                released = Some(memory.insertion_release_device_v1(data.take().unwrap())?);
                Ok(())
            })
            .unwrap();
        retake.unwrap();
        operation.unwrap();
        f.base.released.push(released.unwrap());
        {
            let mut context = f.base.context();
            let ledger = context.ledger();
            assert_eq!(ledger.identities.remove(2), old);
            *ledger.count -= 1;
            *ledger.next = Some(2);
        }
        let native = f.base.memory().insertion_memory_snapshot_v1();
        let (mut expected, count, next) = f.base.context().ledger_snapshot();
        assert_eq!((count, next), (3, Some(2)));
        let settled = f.insert(4097, 4096);
        assert!(!settled.transport);
        let output = settled.into_result().unwrap();
        expected.insert(2, output.storage_identity());
        f.base.data.push(output);
        assert_eq!(f.base.context().ledger_snapshot(), (expected, 4, None));
        f.assert_prefix(&native, 4097, [4, 1, 1, 0, 1], Some("Mapped"), true);
        f.finish(false, native.next_id, 4097);
    }
}

#[test]
fn device_allocation_insertion_complete_survives_retake_and_commit_failures() {
    for ordinal in 0..3 {
        for closing in 0..6 {
            let mut f = AllocationFixture::new(ordinal);
            let native = f.base.memory().insertion_memory_snapshot_v1();
            let ledger = f.base.context().ledger_snapshot();
            let loan = f.base.loan_state();
            let trace_offset = trace().borrow().calls.len();
            let occurrence = |name| {
                trace()
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&s| s == name)
                    .count()
                    + 1
            };
            let pre_occurrence = occurrence("auxiliary-retake");
            let post_occurrence = occurrence("auxiliary-retake-complete");
            match closing {
                0 => f.base.scope.parent.faults.reclaim_before = Outcome::Error,
                1 => f.base.scope.parent.faults.reclaim_before = Outcome::Panic,
                2 => f.base.scope.parent.faults.reclaim_after = Outcome::Error,
                3 => f.base.scope.parent.faults.reclaim_after = Outcome::Panic,
                4 => f.base.scope.parent.faults.regress_revision = true,
                _ => f.base.trace.commit_panic = true,
            }
            let settled = f.insert(4097, 4096);
            assert!(settled.transport);
            assert_eq!(settled.result.is_err(), matches!(closing, 1 | 3 | 5));
            match &settled.result {
                Err(payload) if closing == 5 => assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"insertion commit ledger access")
                ),
                Err(payload) => assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(
                        if closing == 1 {
                            "auxiliary-retake"
                        } else {
                            "auxiliary-retake-complete"
                        },
                        if closing == 1 {
                            pre_occurrence
                        } else {
                            post_occurrence
                        }
                    ))
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name)))
                    if matches!(closing, 0 | 2) =>
                {
                    assert_eq!(
                        *name,
                        if closing == 0 {
                            "auxiliary-retake"
                        } else {
                            "auxiliary-retake-complete"
                        }
                    )
                }
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(name))))
                    if closing == 4 =>
                {
                    assert_eq!(*name, "fixture live foundation reclaim")
                }
                _ => panic!("unexpected allocation closing result"),
            }
            for name in ["auxiliary-loan", "auxiliary-retake"] {
                assert_eq!(
                    trace().borrow().calls[trace_offset..]
                        .iter()
                        .filter(|&&s| s == name)
                        .count(),
                    1
                );
            }
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            let after = f.base.memory().insertion_memory_snapshot_v1();
            assert!(after.terminal_occupied && after.terminal.is_none());
            let root = after.allocation_terminal.as_ref().unwrap();
            assert_eq!(
                root,
                &f.trace
                    .before_retake
                    .as_ref()
                    .unwrap()
                    .with_failure_for_test()
            );
            assert_eq!(f.trace.before_failure, f.trace.before_retake);
            assert!(root.started() && root.failed() && root.native_started());
            assert!(root.lease().unwrap().2);
            assert_eq!(root.lease().unwrap().1, layout(4097));
            assert_eq!(root.progress(), (true, Some(true), Some(1)));
            assert!(f.base.memory().insertion_terminal_output_unavailable_v1());
            assert_eq!(
                f.base.loan_state(),
                (
                    loan.0,
                    (!matches!(closing, 2 | 3 | 5)).then_some(loan.2),
                    loan.2 + 1
                )
            );
            f.assert_prefix(&native, 4097, [4, 1, 1, 0, 1], Some("Mapped"), true);
            f.assert_partition(native.next_id, 4097);
            f.assert_retry();
            f.finish(true, native.next_id, 4097);
        }
    }
}

#[test]
fn device_allocation_insertion_real_fifteen_to_sixteen_capacity_is_pre_effect_bounded() {
    for ordinal in 0..3 {
        let mut f = AllocationFixture::new(ordinal);
        assert!(f.base.context().ledger().identities.capacity() < 16);
        while f.base.data.len() < 15 {
            let settled = f.insert(17, 4096);
            assert!(!settled.transport);
            f.base.data.push(settled.into_result().unwrap());
        }
        let (mut identities, count, _) = f.base.context().ledger_snapshot();
        assert_eq!(count, 15);
        let settled = f.insert(4097, 4096);
        assert!(!settled.transport);
        let output = settled.into_result().unwrap();
        identities.push(output.storage_identity());
        f.base.data.push(output);
        assert_eq!(f.base.context().ledger_snapshot(), (identities, 16, None));
        let native = f.base.memory().insertion_memory_snapshot_v1();
        let observation = f.base.memory().observation();
        let ledger = f.base.context().ledger_snapshot();
        let counts = (
            f.base.trace.reserve_calls,
            f.base.trace.prepare_calls,
            f.base.trace.fail_calls,
        );
        for (bytes, alignment) in [(17, 4096), (0, 4096), (17, 0), (0, 0)] {
            let settled = f.insert(bytes, alignment);
            assert!(!settled.transport);
            assert!(matches!(
                settled.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::DataLeaseCount {
                        requested: 17,
                        maximum: 16
                    }
                )))
            ));
            assert_eq!(f.base.memory().insertion_memory_snapshot_v1(), native);
            assert_eq!(f.base.memory().observation(), observation);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            assert_eq!(
                (
                    f.base.trace.reserve_calls,
                    f.base.trace.prepare_calls,
                    f.base.trace.fail_calls
                ),
                counts
            );
        }
        f.finish(false, native.next_id, 4097);
    }
}

#[test]
fn device_allocation_insertion_reservation_and_opening_failures_never_enter_native_calls() {
    for ordinal in 0..3 {
        for mode in 0..5 {
            let mut f = AllocationFixture::new(ordinal);
            match mode {
                0 => f.base.trace.reserve = Outcome::Error,
                1 => f.base.trace.reserve = Outcome::Panic,
                2 => f.base.scope.parent.faults.loan = Outcome::Error,
                3 => f.base.scope.parent.faults.loan = Outcome::Panic,
                _ => f.base.memory_mut().primary_expire_loan_generation_v1(),
            }
            let native = f.base.memory().insertion_memory_snapshot_v1();
            let observation = f.base.memory().observation();
            let loan = f.base.loan_state();
            let ledger = f.base.context().ledger_snapshot();
            let calls = trace().borrow().calls.len();
            let settled = f.insert(17, 4096);
            assert_eq!(settled.transport, mode != 0);
            assert_eq!(settled.result.is_err(), matches!(mode, 1 | 3));
            assert!(!matches!(settled.result, Ok(Ok(_))));
            assert_eq!(f.base.trace.prepare_calls, usize::from(mode >= 2));
            assert_eq!(f.base.trace.fail_calls, usize::from(mode != 0));
            assert!(f.trace.before_retake.is_none());
            assert_eq!(f.trace.before_failure.is_some(), mode != 0);
            if let Some(root) = &f.trace.before_failure {
                assert!(!root.started() && !root.failed() && !root.native_started());
                assert!(root.lease().is_none());
                assert_eq!(root.progress(), (false, None, None));
            }
            assert_eq!(f.base.memory().insertion_memory_snapshot_v1(), native);
            assert_eq!(f.base.memory().observation(), observation);
            assert_eq!(f.base.loan_state(), loan);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            assert!(!trace().borrow().calls[calls..].contains(&"auxiliary-retake"));
            if settled.transport {
                f.assert_retry();
            }
            f.finish(settled.transport, native.next_id, 17);
        }
    }
}

#[test]
fn device_allocation_insertion_success_without_complete_cannot_commit() {
    for ordinal in 0..3 {
        let mut f = AllocationFixture::new(ordinal);
        f.base.trace.skip_prepare = true;
        let native = f.base.memory().insertion_memory_snapshot_v1();
        let ledger = f.base.context().ledger_snapshot();
        let settled = f.insert(17, 4096);
        assert!(settled.transport);
        assert!(matches!(
            settled.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                MemorySessionError::InvalidDeviceMemoryAuthority
            )))
        ));
        assert_eq!(f.base.context().ledger_snapshot(), ledger);
        assert_eq!(f.base.memory().insertion_memory_snapshot_v1(), native);
        assert!(!f.base.trace.commit_panic);
        assert!(f.trace.before_failure.as_ref().unwrap().lease().is_none());
        f.assert_retry();
        f.finish(true, native.next_id, 17);
    }
}

#[test]
fn device_allocation_insertion_native_prefixes_remain_in_original_terminal_custody() {
    for ordinal in 0..3 {
        for panic in [false, true] {
            for operation in ["reserve_va", "alloc", "map_gpu"] {
                let mut f = AllocationFixture::new(ordinal);
                f.base.memory_mut().primary_arm_native(operation, panic);
                let native = f.base.memory().insertion_memory_snapshot_v1();
                let ledger = f.base.context().ledger_snapshot();
                let settled = f.insert(4097, 4096);
                assert!(settled.transport);
                if panic {
                    assert_eq!(
                        settled.result.err().unwrap().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", operation))
                    );
                } else {
                    assert!(
                        matches!(settled.result, Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(actual)))) if actual == operation)
                    );
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let after = f.base.memory().insertion_memory_snapshot_v1();
                let root = after.allocation_terminal.as_ref().unwrap();
                assert_eq!(Some(root), f.trace.before_failure.as_ref());
                assert_eq!(f.trace.before_failure, f.trace.before_retake);
                assert!(root.started() && root.failed() && root.native_started());
                assert_eq!(root.lease().is_some(), operation == "map_gpu");
                if let Some((_, actual_layout, mapped)) = root.lease() {
                    assert_eq!(actual_layout, layout(4097));
                    assert!(!mapped);
                }
                assert_eq!(
                    root.progress(),
                    (
                        operation == "map_gpu",
                        (operation == "map_gpu" && !panic).then_some(false),
                        (operation == "map_gpu" && !panic).then_some(1)
                    )
                );
                f.assert_prefix(
                    &native,
                    4097,
                    [
                        if operation == "map_gpu" { 3 } else { 1 },
                        1,
                        usize::from(operation != "reserve_va"),
                        0,
                        usize::from(operation == "map_gpu"),
                    ],
                    (operation != "reserve_va").then_some("Ambiguous"),
                    !(operation == "reserve_va" || operation == "alloc" && panic),
                );
                assert!(f.base.memory().insertion_terminal_output_unavailable_v1());
                f.assert_partition(native.next_id, 4097);
                f.assert_retry();
                f.finish(true, native.next_id, 4097);
            }
        }
    }
}

#[test]
fn device_allocation_insertion_currentness_and_map_prefixes_never_fabricate_output() {
    for ordinal in 0..3 {
        for fault in 0..13 {
            let mut f = AllocationFixture::new(ordinal);
            let native = f.base.memory().insertion_memory_snapshot_v1();
            let ledger = f.base.context().ledger_snapshot();
            let currentness = if fault < 8 { fault % 4 + 1 } else { 3 };
            let panic = (4..8).contains(&fault);
            let (prefix, errno) = match fault {
                8 => (0, false),
                9 => (0, true),
                10 => (1, true),
                11 => (2, false),
                _ => (2, true),
            };
            if fault < 8 {
                f.base
                    .memory_mut()
                    .insertion_arm_currentness_v1(currentness, panic);
            } else {
                f.base.memory_mut().insertion_arm_map_v1(prefix, errno);
            }
            let settled = f.insert(4097, 4096);
            assert!(settled.transport);
            if panic {
                assert_eq!(
                    settled.result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            } else if fault < 8 {
                assert!(matches!(
                    settled.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Injected("currentness")
                    )))
                ));
            } else if errno && prefix != 2 {
                assert!(matches!(
                    settled.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Injected("map_gpu")
                    )))
                ));
            } else {
                let expected = if prefix == 2 {
                    "device-memory MAP_MEMORY_TO_GPU cumulative n_success"
                } else {
                    "device-memory MAP_MEMORY_TO_GPU full prefix"
                };
                assert!(
                    matches!(settled.result, Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::KernelResultMalformed(actual)))) if actual == expected)
                );
            }
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            let root = f.trace.before_failure.as_ref().unwrap();
            assert_eq!(f.trace.before_failure, f.trace.before_retake);
            assert!(root.started() && root.failed());
            assert_eq!(root.native_started(), currentness > 1);
            assert_eq!(root.lease().is_some(), currentness >= 3);
            if let Some((_, actual_layout, mapped)) = root.lease() {
                assert_eq!(actual_layout, layout(4097));
                assert!(!mapped);
            }
            let gpu_called = fault >= 8 || currentness == 4;
            assert_eq!(
                root.progress(),
                (
                    gpu_called,
                    gpu_called.then_some(fault < 8 || !errno),
                    gpu_called.then_some(if fault < 8 { 1 } else { prefix })
                )
            );
            let after = f.base.memory().insertion_memory_snapshot_v1();
            assert_eq!(
                after.allocation_terminal.as_ref(),
                (currentness > 1).then_some(root)
            );
            assert_eq!(after.terminal_occupied, currentness > 1);
            assert!(after.terminal.is_none());
            let phase = if currentness == 1 {
                None
            } else if currentness == 3 && fault < 8 {
                Some("Unmapped")
            } else {
                Some("Ambiguous")
            };
            f.assert_prefix(
                &native,
                4097,
                [
                    currentness,
                    usize::from(currentness > 1),
                    usize::from(currentness > 1),
                    0,
                    usize::from(gpu_called),
                ],
                phase,
                currentness > 1,
            );
            f.assert_partition(native.next_id, 4097);
            f.assert_retry();
            f.finish(true, native.next_id, 4097);
        }
    }
}

#[test]
fn device_allocation_insertion_operation_panic_wins_each_secondary_closing_failure() {
    for ordinal in 0..3 {
        for panic in [false, true] {
            for after in [false, true] {
                for closing in [Outcome::Success, Outcome::Error, Outcome::Panic] {
                    let mut f = AllocationFixture::new(ordinal);
                    f.base.memory_mut().primary_arm_native("map_gpu", panic);
                    if after {
                        f.base.scope.parent.faults.reclaim_after = closing;
                    } else {
                        f.base.scope.parent.faults.reclaim_before = closing;
                    }
                    let native = f.base.memory().insertion_memory_snapshot_v1();
                    let ledger = f.base.context().ledger_snapshot();
                    let calls = trace().borrow().calls.len();
                    let name = if after {
                        "auxiliary-retake-complete"
                    } else {
                        "auxiliary-retake"
                    };
                    let occurrence = trace()
                        .borrow()
                        .calls
                        .iter()
                        .filter(|&&s| s == name)
                        .count()
                        + 1;
                    let settled = f.insert(4097, 4096);
                    assert!(settled.transport);
                    if panic {
                        assert_eq!(
                            settled.result.err().unwrap().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", "map_gpu"))
                        );
                    } else if closing == Outcome::Panic {
                        assert_eq!(
                            settled
                                .result
                                .err()
                                .unwrap()
                                .downcast_ref::<(&str, usize)>(),
                            Some(&(name, occurrence))
                        );
                    } else if closing == Outcome::Error {
                        assert!(
                            matches!(settled.result, Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(actual))) if actual == name)
                        );
                    } else {
                        assert!(matches!(
                            settled.result,
                            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                                MemorySessionError::Injected("map_gpu")
                            )))
                        ));
                    }
                    assert_eq!(
                        trace().borrow().calls[calls..]
                            .iter()
                            .filter(|&&call| call == name)
                            .count(),
                        1
                    );
                    assert_eq!(f.base.context().ledger_snapshot(), ledger);
                    assert_eq!(f.base.trace.panic_fail, panic || closing == Outcome::Panic);
                    f.assert_prefix(&native, 4097, [3, 1, 1, 0, 1], Some("Ambiguous"), true);
                    f.assert_partition(native.next_id, 4097);
                    f.assert_retry();
                    f.finish(true, native.next_id, 4097);
                }
            }
        }
    }
}

#[test]
fn device_allocation_insertion_invalid_layout_retakes_without_native_custody() {
    for ordinal in 0..3 {
        for (bytes, alignment) in [(0, 4096), (17, 0), (17, 3)] {
            let mut f = AllocationFixture::new(ordinal);
            let native = f.base.memory().insertion_memory_snapshot_v1();
            let observation = f.base.memory().observation();
            let ledger = f.base.context().ledger_snapshot();
            let loan = f.base.loan_state();
            let calls = trace().borrow().calls.len();
            let settled = f.insert(bytes, alignment);
            assert!(settled.transport);
            if bytes == 0 {
                assert!(matches!(
                    settled.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::InvalidDeviceMemorySize
                    )))
                ));
            } else {
                assert!(matches!(
                    settled.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::InvalidDeviceMemoryAlignment
                    )))
                ));
            }
            assert_eq!(
                (
                    f.base.trace.reserve_calls,
                    f.base.trace.prepare_calls,
                    f.base.trace.fail_calls
                ),
                (1, 1, 1)
            );
            for name in [
                "auxiliary-loan",
                "auxiliary-retake",
                "auxiliary-retake-complete",
            ] {
                assert_eq!(
                    trace().borrow().calls[calls..]
                        .iter()
                        .filter(|&&call| call == name)
                        .count(),
                    1
                );
            }
            let root = f.trace.before_failure.as_ref().unwrap();
            assert_eq!(Some(root), f.trace.before_retake.as_ref());
            assert!(root.started() && root.failed() && !root.native_started());
            assert_eq!(root.lease(), None);
            assert_eq!(root.progress(), (false, None, None));
            assert_eq!(f.base.memory().insertion_memory_snapshot_v1(), native);
            assert_eq!(f.base.memory().observation(), observation);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            assert_eq!(f.base.loan_state(), (loan.0, None, loan.2 + 1));
            f.assert_retry();
            f.finish(true, native.next_id, 17);
        }
    }
}
