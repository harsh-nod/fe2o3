//! Constructed-engine composition of the production insertion sequencer.
//! Concrete Linux session forwarding and facade transport have separate guards.

use super::*;
use crate::queue::dispatch_binding::preparation::PreparationOwnerRefsV1;
use crate::queue::live::data_insertion::{
    DetachedInsertionLedgerV1, DeviceInsertionContextV1, SettledDeviceInsertionV1,
    reserve_identity_capacity_v1, settle_device_insertion_v1,
};
use crate::shared_memory::{
    DeviceInitializationCustodyV1, DeviceInitializationSnapshotV1, DeviceInsertionMemorySnapshotV1,
    DeviceInsertionPrefixV1, SharedMemorySessionPhaseV1,
};
use fe2o3_kfd_uapi::KfdAllocMemoryFlags;

#[derive(Default)]
struct PrimaryDetached {
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    count: usize,
    next: Option<usize>,
    continuation: Option<PristineDispatchContinuationV1>,
}

#[derive(Default)]
struct InsertionTrace {
    reserve: Outcome,
    skip_prepare: bool,
    prepare_calls: usize,
    reserve_calls: usize,
    fail_calls: usize,
    panic_fail: bool,
    before_retake: Option<DeviceInitializationSnapshotV1>,
    before_failure: Option<DeviceInitializationSnapshotV1>,
    reserved_storage: Option<(usize, usize)>,
    commit_panic: bool,
    commit_ready: bool,
}

struct Context<'a> {
    parent: &'a mut Parent,
    lane: Option<&'a mut ComputeAqlQueueLaneStateV1<Fixture>>,
    primary: &'a mut PrimaryDetached,
    trace: &'a mut InsertionTrace,
}

impl Context<'_> {
    fn ledger_snapshot(
        &mut self,
    ) -> (
        Vec<Gfx942FixedDispatchStorageIdentityV1>,
        usize,
        Option<usize>,
    ) {
        let ledger = self.ledger();
        (ledger.identities.clone(), *ledger.count, *ledger.next)
    }
}

impl DeviceInsertionContextV1 for Context<'_> {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        // Fixture preflight uses genuine owners; the concrete session guard is
        // covered separately, not inferred from this test-only implementation.
        if self.parent.poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let (bound, detached, completion, count, identities, next) = match &self.lane {
            Some(lane) => (
                lane.dispatch.is_some(),
                lane.unpublished_dispatch.is_detached(),
                &*lane.completion_owner,
                lane.detached_data_count,
                &lane.detached_data_identities,
                lane.detached_next_insertion_index,
            ),
            None => {
                let primary = self
                    .parent
                    .original
                    .as_ref()
                    .unwrap()
                    .primary
                    .completed
                    .as_ref()
                    .unwrap();
                (
                    primary.dispatch.is_some(),
                    self.primary.continuation.is_some(),
                    &primary.completion_owner,
                    self.primary.count,
                    &self.primary.identities,
                    self.primary.next,
                )
            }
        };
        if bound || !detached {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if count > 16 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data ledger bound",
            ));
        }
        if count != identities.len() || next.is_some_and(|i| i > count) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data identity ledger",
            ));
        }
        completion.ensure_releasable()?;
        Ok(())
    }

    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_> {
        if self.trace.commit_ready && self.trace.commit_panic {
            self.trace.commit_panic = false;
            panic!("insertion commit ledger access");
        }
        if let Some(lane) = self.lane.as_mut() {
            DetachedInsertionLedgerV1 {
                identities: &mut lane.detached_data_identities,
                count: &mut lane.detached_data_count,
                next: &mut lane.detached_next_insertion_index,
            }
        } else {
            DetachedInsertionLedgerV1 {
                identities: &mut self.primary.identities,
                count: &mut self.primary.count,
                next: &mut self.primary.next,
            }
        }
    }

    fn reserve(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.trace.reserve_calls += 1;
        outcome("insertion-reserve", self.trace.reserve)?;
        let ledger = self.ledger();
        reserve_identity_capacity_v1(ledger.identities)?;
        self.trace.reserved_storage = Some((
            ledger.identities.as_ptr() as usize,
            ledger.identities.capacity(),
        ));
        Ok(())
    }

    fn prepare(
        &mut self,
        root: &mut DeviceInitializationCustodyV1,
        alignment: u64,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.trace.prepare_calls += 1;
        let before = self.ledger_snapshot();
        let ledger = self.ledger();
        assert!(
            ledger.identities.capacity() >= 16,
            "identity capacity reserved before native effects"
        );
        let storage = (ledger.identities.as_ptr(), ledger.identities.capacity());
        let skip = self.trace.skip_prepare;
        let before_retake = &mut self.trace.before_retake;
        let result = self.parent.with_preparation_custody(|memory| {
            if skip {
                return Ok(());
            }
            let prepared = catch_unwind(AssertUnwindSafe(|| {
                memory.primary_prepare_device_initialization_v1(root, alignment)
            }));
            *before_retake = Some(root.insertion_snapshot_for_test());
            match prepared {
                Ok(result) => result.map_err(Into::into),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        });
        assert_eq!(
            self.ledger_snapshot(),
            before,
            "no ledger commit before retake settlement"
        );
        let ledger = self.ledger();
        assert_eq!(
            (ledger.identities.as_ptr(), ledger.identities.capacity()),
            storage
        );
        let (operation, retake) = result?;
        retake?;
        operation?;
        self.trace.commit_ready = true;
        Ok(())
    }

    fn fail(&mut self, root: DeviceInitializationCustodyV1, panicked: bool) {
        self.trace.fail_calls += 1;
        self.trace.panic_fail |= panicked;
        self.trace.before_failure = Some(root.insertion_snapshot_for_test());
        if let Some(lane) = self.lane.as_mut() {
            lane.completion_owner.poison_owner();
            lane.submission.as_mut().unwrap().poison();
            lane.unpublished_dispatch.continuation = None;
        } else {
            self.primary.continuation = None;
        }
        self.parent.poison();
        if panicked {
            Fixture::poison();
        }
        if root.requires_retention() {
            self.parent
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
                .primary_retain_device_initialization_v1(root);
        }
    }
}

struct InsertionFixture {
    scope: Box<Scope>,
    slot: Option<usize>,
    lane: Option<Box<ComputeAqlQueueLaneStateV1<Fixture>>>,
    primary: PrimaryDetached,
    data: Vec<Gfx942FixedDispatchDataV1>,
    released: Vec<crate::shared_memory::Gfx942DeviceMemoryIdentityV1>,
    trace: InsertionTrace,
}

impl InsertionFixture {
    fn new(ordinal: usize) -> Self {
        let (mut scope, result, _) = prefix_case_with_probe(
            false,
            |_| {},
            |_, _| {},
            run_auxiliary,
            |scope, failed| {
                assert!(!failed);
                assert_pair(scope);
            },
            ordinal != 0,
        );
        assert!(result.is_ok());
        if ordinal == 2 {
            let original = scope.parent.original.as_mut().unwrap();
            let lane = original.lanes[0].state.take().unwrap();
            let generation = original.lanes[0].generation;
            original.lanes.push(AuxiliaryComputeLaneSlotV1 {
                generation,
                state: Some(lane),
            });
        }
        let slot = (ordinal != 0).then_some(ordinal.saturating_sub(1));
        let mut lane = slot.map(|index| {
            Box::new(
                scope.parent.original.as_mut().unwrap().lanes[index]
                    .state
                    .take()
                    .unwrap(),
            )
        });
        let mut dispatch = if let Some(lane) = lane.as_mut() {
            lane.completion_owner.ensure_releasable().unwrap();
            lane.dispatch.take()
        } else {
            let p = scope
                .parent
                .original
                .as_mut()
                .unwrap()
                .primary
                .completed
                .as_mut()
                .unwrap();
            p.completion_owner.ensure_releasable().unwrap();
            p.dispatch.take()
        };
        let buffers = dispatch
            .as_ref()
            .unwrap()
            .prepare_pristine_abort_v1()
            .unwrap();
        let mut abort = None;
        let (operation, retake) = scope
            .parent
            .with_preparation_custody(|memory| {
                abort = Some(dispatch.take().unwrap().begin_pristine_abort_v1(buffers));
                abort.as_mut().unwrap().release_controls(memory)?;
                Ok(())
            })
            .unwrap();
        retake.unwrap();
        operation.unwrap();
        let (continuation, data, identities) = abort.unwrap().into_detached();
        let mut primary = PrimaryDetached::default();
        if let Some(lane) = lane.as_mut() {
            lane.detached_data_count = data.len();
            lane.detached_data_identities = identities;
            lane.detached_dispatch_generation = None;
            lane.detached_next_insertion_index = None;
            lane.unpublished_dispatch.continuation = Some(continuation);
        } else {
            primary.count = data.len();
            primary.identities = identities;
            primary.continuation = Some(continuation);
        }
        Self {
            scope,
            slot,
            lane,
            primary,
            data,
            released: Vec::new(),
            trace: InsertionTrace::default(),
        }
    }

    fn context(&mut self) -> Context<'_> {
        Context {
            parent: &mut self.scope.parent,
            lane: self.lane.as_deref_mut(),
            primary: &mut self.primary,
            trace: &mut self.trace,
        }
    }

    fn memory(&self) -> &Memory {
        &self
            .scope
            .primary
            .completed
            .as_ref()
            .unwrap()
            .engine
            .backend
            .session
    }

    fn memory_mut(&mut self) -> &mut Memory {
        &mut self
            .scope
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
    }

    fn loan_state(&self) -> (u64, Option<u64>, u64) {
        let engine = &self.scope.primary.completed.as_ref().unwrap().engine;
        self.memory().primary_loan_state_v1(&engine.foundation)
    }

    fn assert_terminal_retry(&mut self, snapshot: &DeviceInsertionMemorySnapshotV1) {
        assert!(self.scope.parent.poisoned);
        assert!(
            self.scope
                .primary
                .completed
                .as_ref()
                .unwrap()
                .completion_owner
                .is_poisoned_for_test()
        );
        if let Some(lane) = &self.lane {
            assert!(lane.completion_owner.is_poisoned_for_test());
            assert!(lane.submission.as_ref().unwrap().is_poisoned_for_test());
        }
        self.memory_mut().insertion_clear_faults_v1();
        self.scope.parent.faults = Faults::default();
        let memory = self.memory().observation();
        let ledger = self.context().ledger_snapshot();
        let calls = (
            self.trace.reserve_calls,
            self.trace.prepare_calls,
            self.trace.fail_calls,
        );
        let (bytes, content) = input();
        let retry = self.insert(None, bytes, 4096, content);
        assert!(!retry.transport);
        assert!(matches!(
            retry.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            )))
        ));
        assert_eq!(
            (
                self.trace.reserve_calls,
                self.trace.prepare_calls,
                self.trace.fail_calls
            ),
            calls
        );
        assert_eq!(self.memory().observation(), memory);
        assert_eq!(&self.memory().insertion_memory_snapshot_v1(), snapshot);
        assert_eq!(self.context().ledger_snapshot(), ledger);
    }

    fn insert(
        &mut self,
        index: Option<usize>,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> SettledDeviceInsertionV1 {
        settle_device_insertion_v1(
            &mut self.context(),
            DeviceInitializationCustodyV1::new(bytes, content),
            index,
            alignment,
        )
    }

    fn assert_partition(&self, attempted_id: u64, bytes: usize) {
        let mut refs = PreparationOwnerRefsV1::default();
        if let Some(dispatch) = &self.scope.primary.completed.as_ref().unwrap().dispatch {
            refs.dispatch(dispatch);
        }
        for slot in &self.scope.lanes {
            if let Some(dispatch) = slot.state.as_ref().and_then(|l| l.dispatch.as_ref()) {
                refs.dispatch(dispatch);
            }
        }
        refs.data(&self.data);
        let layout = crate::shared_memory::device_memory_layout(
            bytes as u64,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();
        self.memory().insertion_assert_device_partition_v1(
            &refs.device_leases,
            &refs.device_authorities,
            None,
            attempted_id,
            layout,
            &self.released,
        );
        self.memory().primary_assert_shared_layouts_v1(&refs.shared);
        self.memory()
            .primary_assert_accounts_with_device_disposal_v1(
                trace().borrow().session,
                [4 + self.released.len(); 3],
                &self.released,
            );
        let installed = self
            .lane
            .as_deref()
            .or_else(|| self.scope.lanes.iter().find_map(|slot| slot.state.as_ref()));
        platform::assert_auxiliary_platform(
            &self.scope.primary,
            &self.scope.construction,
            installed,
        );
    }

    fn restore_and_transport(&mut self, transport: bool) {
        if let Some(storage) = self.trace.reserved_storage {
            let mut context = self.context();
            let ledger = context.ledger();
            assert_eq!(
                (
                    ledger.identities.as_ptr() as usize,
                    ledger.identities.capacity()
                ),
                storage,
                "reserved identity storage survives every returned error and panic"
            );
        }
        let stable = |scope: &Scope| {
            let p = scope.primary.completed.as_ref().unwrap();
            (
                &*scope.primary as *const Root as usize,
                &p.engine as *const _ as usize,
                p.key,
                p.observation,
                p.completion_owner.custody_snapshot_for_test(),
                p.dependency_owner.custody_snapshot_for_test(),
                platform::platform_identities(&scope.primary, None),
            )
        };
        let primary = stable(&self.scope);
        let lane_snapshot = |lane: &ComputeAqlQueueLaneStateV1<Fixture>| {
            (
                lane.key,
                lane.observation,
                lane.completion_owner.custody_snapshot_for_test(),
                lane.detached_data_count,
                lane.detached_dispatch_generation,
                lane.detached_next_insertion_index,
                lane.detached_data_identities.clone(),
                lane.detached_data_identities.as_ptr() as usize,
                lane.detached_data_identities.capacity(),
            )
        };
        let selected = self.lane.as_deref().map(lane_snapshot);
        let unselected = self
            .scope
            .lanes
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != self.slot)
            .map(|(i, s)| (i, s.generation, s.state.as_ref().map(lane_snapshot)))
            .collect::<Vec<_>>();
        let generation = self.slot.map(|i| self.scope.lanes[i].generation);
        if let Some(index) = self.slot {
            assert!(
                self.scope.parent.original.as_ref().unwrap().lanes[index]
                    .state
                    .is_none()
            );
            self.scope.parent.original.as_mut().unwrap().lanes[index].state =
                Some(*self.lane.take().unwrap());
        }
        if transport {
            if let Some(index) = self.slot {
                assert!(
                    self.scope.parent.original.as_ref().unwrap().lanes[index]
                        .state
                        .is_some(),
                    "lane restored before original parent transport"
                );
            }
            self.scope.terminal_parent = Some(self.scope.parent.take_terminal_parent());
        }
        assert_eq!(self.scope.terminal_parent.is_some(), transport);
        assert_eq!(stable(&self.scope), primary);
        if let Some(index) = self.slot {
            assert_eq!(Some(self.scope.lanes[index].generation), generation);
            assert_eq!(
                self.scope.lanes[index].state.as_ref().map(lane_snapshot),
                selected
            );
        }
        assert_eq!(
            self.scope
                .lanes
                .iter()
                .enumerate()
                .filter(|(i, _)| Some(*i) != self.slot)
                .map(|(i, s)| (i, s.generation, s.state.as_ref().map(lane_snapshot)))
                .collect::<Vec<_>>(),
            unselected
        );
        let installed = self.scope.lanes.iter().find_map(|slot| slot.state.as_ref());
        platform::assert_auxiliary_platform(
            &self.scope.primary,
            &self.scope.construction,
            installed,
        );
    }
}

fn input() -> (Box<[u8]>, Gfx942DeviceContentDescriptorV1) {
    let bytes: Box<[u8]> = (0..4103).map(|i| (i % 251) as u8).collect();
    let role = crate::Gfx942DeviceContentRoleV1::new([0x49; 32], 7).unwrap();
    let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &bytes).unwrap();
    (bytes, content)
}

#[test]
fn device_insertion_constructed_primary_and_auxiliary_preserve_success_identity_and_order() {
    for ordinal in 0..3 {
        for index in [Some(0), Some(2), Some(4), None] {
            let mut f = InsertionFixture::new(ordinal);
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let loan = f.loan_state();
            let (mut expected, count, _) = f.context().ledger_snapshot();
            assert_eq!(count, 4);
            let (bytes, content) = input();
            let pointer = bytes.as_ptr() as usize;
            let expected_bytes = bytes.to_vec();
            let settled = f.insert(index, bytes, 4096, content);
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            assert_eq!(data.initialized_content(), Some(content));
            expected.insert(index.unwrap_or(count), data.storage_identity());
            f.data.push(data);
            assert_eq!(f.context().ledger_snapshot(), (expected, count + 1, None));
            let after = f.memory().insertion_memory_snapshot_v1();
            assert_eq!(after.terminal, None);
            f.memory().insertion_assert_native_prefix_v1(
                &native,
                DeviceInsertionPrefixV1 {
                    calls: [6, 1, 1, 1, 1],
                    phase: Some("Mapped"),
                    handle: true,
                    cpu_writable: None,
                    written: true,
                    operations: &["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"],
                },
                &expected_bytes,
            );
            assert_eq!(after.account, native.account);
            assert_eq!(after.terminal_storage, native.terminal_storage);
            assert_eq!(
                after.initialized_bytes.as_ref().unwrap()[..expected_bytes.len()],
                expected_bytes
            );
            assert!(after.readback_calls > 0);
            f.trace.before_retake.as_ref().unwrap().assert_source(
                pointer,
                &expected_bytes,
                content,
            );
            assert_eq!(
                f.trace.before_retake.as_ref().unwrap().stage_name(),
                "Complete"
            );
            assert_eq!(f.loan_state(), (loan.0, None, loan.2 + 1));
            f.memory().assert_original_records_unchanged(&before);
            f.assert_partition(native.next_id, expected_bytes.len());
            f.restore_and_transport(false);
        }
    }
}

#[test]
fn device_insertion_completed_output_survives_each_failed_retake() {
    for ordinal in 0..3 {
        for closing in 0..5 {
            let mut f = InsertionFixture::new(ordinal);
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let ledger = f.context().ledger_snapshot();
            let loan = f.loan_state();
            let trace_offset = trace().borrow().calls.len();
            let pre_occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-retake")
                .count()
                + 1;
            let post_occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-retake-complete")
                .count()
                + 1;
            match closing {
                0 => f.scope.parent.faults.reclaim_before = Outcome::Error,
                1 => f.scope.parent.faults.reclaim_before = Outcome::Panic,
                2 => f.scope.parent.faults.reclaim_after = Outcome::Error,
                3 => f.scope.parent.faults.reclaim_after = Outcome::Panic,
                _ => f.scope.parent.faults.regress_revision = true,
            }
            let (bytes, content) = input();
            let pointer = bytes.as_ptr() as usize;
            let expected = bytes.to_vec();
            let settled = f.insert(Some(2), bytes, 4096, content);
            assert!(settled.transport);
            assert_eq!(settled.result.is_err(), matches!(closing, 1 | 3));
            assert!(!matches!(settled.result, Ok(Ok(_))));
            match &settled.result {
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
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name))) => assert_eq!(
                    *name,
                    if closing == 0 {
                        "auxiliary-retake"
                    } else {
                        "auxiliary-retake-complete"
                    }
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(name))))
                    if closing == 4 =>
                {
                    assert_eq!(*name, "fixture live foundation reclaim");
                }
                _ => panic!("unexpected retake failure provenance"),
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
            assert_eq!(
                (
                    f.trace.reserve_calls,
                    f.trace.prepare_calls,
                    f.trace.fail_calls
                ),
                (1, 1, 1)
            );
            assert_eq!(f.context().ledger_snapshot(), ledger);
            let after = f.memory().insertion_memory_snapshot_v1();
            let root = after.terminal.as_ref().unwrap();
            assert_eq!(
                root,
                &f.trace
                    .before_retake
                    .as_ref()
                    .unwrap()
                    .with_failure_for_test()
            );
            assert_eq!(f.trace.before_failure, f.trace.before_retake);
            root.assert_source(pointer, &expected, content);
            assert_eq!(root.stage_name(), "Complete");
            assert!(root.failed() && root.native_started());
            assert!(root.lease().unwrap().2);
            assert_eq!(root.progress(), (true, Some(true), Some(1)));
            f.memory().insertion_assert_native_prefix_v1(
                &native,
                DeviceInsertionPrefixV1 {
                    calls: [6, 1, 1, 1, 1],
                    phase: Some("Mapped"),
                    handle: true,
                    cpu_writable: None,
                    written: true,
                    operations: &["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"],
                },
                &expected,
            );
            assert!(f.memory().insertion_terminal_output_unavailable_v1());
            assert_eq!(after.account, native.account);
            assert_eq!(after.terminal_storage, native.terminal_storage);
            assert_eq!(
                f.loan_state(),
                (
                    loan.0,
                    (!matches!(closing, 2 | 3)).then_some(loan.2),
                    loan.2 + 1
                )
            );
            f.memory().assert_original_records_unchanged(&before);
            f.assert_partition(native.next_id, expected.len());
            assert_eq!(
                f.memory().observation().phase,
                SharedMemorySessionPhaseV1::Quarantined
            );
            f.assert_terminal_retry(&after);
            f.restore_and_transport(true);
        }
    }
}

#[test]
fn device_insertion_real_capacity_growth_and_fifteen_to_sixteen_are_pre_effect_bounded() {
    for ordinal in 0..3 {
        for index in [0, 7, 15] {
            let mut f = InsertionFixture::new(ordinal);
            assert!(f.context().ledger().identities.capacity() < 16);
            while f.data.len() < 15 {
                let (bytes, content) = input();
                let settled = f.insert(None, bytes, 4096, content);
                assert!(!settled.transport);
                f.data.push(settled.into_result().unwrap());
            }
            let (mut expected, count, _) = f.context().ledger_snapshot();
            assert_eq!(count, 15);
            let native = f.memory().insertion_memory_snapshot_v1();
            let (bytes, content) = input();
            let size = bytes.len();
            let settled = f.insert(Some(index), bytes, 4096, content);
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            expected.insert(index, data.storage_identity());
            f.data.push(data);
            assert_eq!(f.context().ledger_snapshot(), (expected, 16, None));
            f.assert_partition(native.next_id, size);
            let before = f.memory().observation();
            let ledger = f.context().ledger_snapshot();
            let counts = (
                f.trace.reserve_calls,
                f.trace.prepare_calls,
                f.trace.fail_calls,
            );
            for index in [16, 99] {
                let (bytes, content) = input();
                let settled = f.insert(Some(index), bytes, 4096, content);
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
                assert_eq!(f.memory().observation(), before);
                assert_eq!(f.context().ledger_snapshot(), ledger);
                assert_eq!(
                    (
                        f.trace.reserve_calls,
                        f.trace.prepare_calls,
                        f.trace.fail_calls
                    ),
                    counts
                );
            }
            f.restore_and_transport(false);
        }
    }
}

#[test]
fn device_insertion_invalid_ordinal_and_reservation_reject_before_loan() {
    for ordinal in 0..3 {
        for mode in 0..3 {
            let mut f = InsertionFixture::new(ordinal);
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let ledger = f.context().ledger_snapshot();
            let loan = f.loan_state();
            if mode > 0 {
                f.trace.reserve = if mode == 1 {
                    Outcome::Error
                } else {
                    Outcome::Panic
                };
            }
            let (bytes, content) = input();
            let settled = f.insert(Some(if mode == 0 { 5 } else { 2 }), bytes, 4096, content);
            assert_eq!(settled.transport, mode == 2);
            assert_eq!(settled.result.is_err(), mode == 2);
            assert!(!matches!(settled.result, Ok(Ok(_))));
            assert_eq!(f.trace.prepare_calls, 0);
            assert_eq!(f.trace.fail_calls, usize::from(mode == 2));
            assert_eq!(f.memory().observation(), before);
            assert_eq!(f.memory().insertion_memory_snapshot_v1(), native);
            assert_eq!(f.loan_state(), loan);
            assert_eq!(f.context().ledger_snapshot(), ledger);
            f.restore_and_transport(settled.transport);
        }
    }
}

#[test]
fn device_insertion_opening_failure_never_initializes_or_retakes() {
    for ordinal in 0..3 {
        for mode in 0..3 {
            let mut f = InsertionFixture::new(ordinal);
            if mode == 2 {
                f.memory_mut().primary_expire_loan_generation_v1();
            } else {
                f.scope.parent.faults.loan = if mode == 0 {
                    Outcome::Error
                } else {
                    Outcome::Panic
                };
            }
            let loan = f.loan_state();
            let before = f.memory().observation();
            let native = f.memory().insertion_memory_snapshot_v1();
            let ledger = f.context().ledger_snapshot();
            let trace_offset = trace().borrow().calls.len();
            let (bytes, content) = input();
            let settled = f.insert(Some(2), bytes, 4096, content);
            assert!(settled.transport);
            assert_eq!(settled.result.is_err(), mode == 1);
            assert!(!matches!(settled.result, Ok(Ok(_))));
            assert!(f.trace.before_retake.is_none());
            assert_eq!(
                f.trace.before_failure.as_ref().unwrap().stage_name(),
                "Source"
            );
            assert!(!f.trace.before_failure.as_ref().unwrap().native_started());
            assert_eq!(f.loan_state(), loan);
            assert_eq!(f.memory().observation(), before);
            assert_eq!(f.memory().insertion_memory_snapshot_v1(), native);
            assert_eq!(f.context().ledger_snapshot(), ledger);
            assert!(!trace().borrow().calls[trace_offset..].contains(&"auxiliary-retake"));
            f.assert_terminal_retry(&native);
            f.restore_and_transport(true);
        }
    }
}

#[test]
fn device_insertion_success_without_complete_cannot_commit_metadata() {
    let mut f = InsertionFixture::new(1);
    f.trace.skip_prepare = true;
    let before = f.memory().insertion_memory_snapshot_v1();
    let ledger = f.context().ledger_snapshot();
    let (bytes, content) = input();
    let settled = f.insert(Some(2), bytes, 4096, content);
    assert!(settled.transport);
    assert!(matches!(
        settled.result,
        Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
            MemorySessionError::InvalidDeviceMemoryAuthority
        )))
    ));
    assert_eq!(f.context().ledger_snapshot(), ledger);
    assert_eq!(f.memory().insertion_memory_snapshot_v1(), before);
    f.restore_and_transport(true);
}

#[derive(Clone, Copy, Debug)]
enum InitializerFault {
    Native(&'static str, bool, &'static str),
    Currentness(usize, bool, &'static str),
    Map(u32, bool),
    Readback,
    Layout,
    Source,
}

impl InitializerFault {
    fn prefix(self) -> DeviceInsertionPrefixV1<'static> {
        let calls = match self {
            Self::Source | Self::Layout => [0; 5],
            Self::Currentness(n, _, _) => [
                n,
                usize::from(n > 1),
                usize::from(n > 1),
                usize::from(n > 3),
                usize::from(n > 5),
            ],
            Self::Native("reserve_va", _, _) => [1, 1, 0, 0, 0],
            Self::Native("alloc", _, _) => [1, 1, 1, 0, 0],
            Self::Native("map_gpu", _, _) | Self::Map(..) => [5, 1, 1, 1, 1],
            _ => [3, 1, 1, 1, 0],
        };
        let phase = if !self.admitted() || matches!(self, Self::Native("reserve_va", _, _)) {
            None
        } else if matches!(
            self,
            Self::Native("alloc" | "map_gpu", _, _)
                | Self::Currentness(2 | 6, _, _)
                | Self::Map(..)
        ) {
            Some("Ambiguous")
        } else {
            Some("Unmapped")
        };
        let handle = phase.is_some() && !matches!(self, Self::Native("alloc", true, _));
        let cpu_writable = match self.stage() {
            "CpuPrepare" => Some(false),
            "CpuWrite" | "CpuVerify" | "CpuUnmap" => Some(true),
            _ => None,
        };
        let written = matches!(
            self.stage(),
            "CpuVerify" | "CpuUnmap" | "CpuClosingCurrentness" | "GpuMap"
        );
        const CPU: &[&str] = &["map_cpu", "prepare_cpu_mapping", "unmap_cpu"];
        const GPU: &[&str] = &["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"];
        let operations = match self.stage() {
            "Source" | "Allocate" | "CpuCurrentness" => &CPU[..0],
            "CpuMap" => &CPU[..1],
            "CpuPrepare" | "CpuWrite" | "CpuVerify" => &CPU[..2],
            _ if calls[4] == 0 => CPU,
            _ => GPU,
        };
        DeviceInsertionPrefixV1 {
            calls,
            phase,
            handle,
            cpu_writable,
            written,
            operations,
        }
    }

    fn arm(self, f: &mut InsertionFixture) {
        match self {
            Self::Native(operation, panic, _) => {
                f.memory_mut().primary_arm_native(operation, panic)
            }
            Self::Currentness(offset, panic, _) => {
                f.memory_mut().insertion_arm_currentness_v1(offset, panic)
            }
            Self::Map(prefix, errno) => f.memory_mut().insertion_arm_map_v1(prefix, errno),
            Self::Readback => f.memory_mut().insertion_corrupt_readback_v1(),
            Self::Layout | Self::Source => {}
        }
    }

    fn stage(self) -> &'static str {
        match self {
            Self::Native(_, _, stage) | Self::Currentness(_, _, stage) => stage,
            Self::Map(..) => "GpuMap",
            Self::Readback => "CpuVerify",
            Self::Layout => "Allocate",
            Self::Source => "Source",
        }
    }

    fn panics(self) -> bool {
        matches!(
            self,
            Self::Native(_, true, _) | Self::Currentness(_, true, _)
        )
    }
    fn admitted(self) -> bool {
        !matches!(
            self,
            Self::Currentness(1, _, _) | Self::Layout | Self::Source
        )
    }
}

#[test]
fn device_insertion_native_and_currentness_failures_retain_exact_prefixes() {
    let mut faults = vec![
        InitializerFault::Readback,
        InitializerFault::Layout,
        InitializerFault::Source,
    ];
    for panic in [false, true] {
        for (operation, stage) in [
            ("reserve_va", "Allocate"),
            ("alloc", "Allocate"),
            ("map_cpu", "CpuMap"),
            ("prepare_cpu_mapping", "CpuPrepare"),
            ("unmap_cpu", "CpuUnmap"),
            ("map_gpu", "GpuMap"),
        ] {
            faults.push(InitializerFault::Native(operation, panic, stage));
        }
        for (offset, stage) in [
            (1, "Allocate"),
            (2, "Allocate"),
            (3, "CpuCurrentness"),
            (4, "CpuClosingCurrentness"),
            (5, "GpuMap"),
            (6, "GpuMap"),
        ] {
            faults.push(InitializerFault::Currentness(offset, panic, stage));
        }
    }
    faults.extend([
        InitializerFault::Native("with_bytes_mut", true, "CpuWrite"),
        InitializerFault::Native("with_bytes", true, "CpuVerify"),
    ]);
    for (prefix, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
        faults.push(InitializerFault::Map(prefix, errno));
    }
    for ordinal in 0..3 {
        for &fault in &faults {
            let mut f = InsertionFixture::new(ordinal);
            let native = f.memory().insertion_memory_snapshot_v1();
            let before = f.memory().observation();
            let ledger = f.context().ledger_snapshot();
            let loan = f.loan_state();
            fault.arm(&mut f);
            let (bytes, mut content) = input();
            let pointer = bytes.as_ptr() as usize;
            let expected = bytes.to_vec();
            if matches!(fault, InitializerFault::Source) {
                content = Gfx942DeviceContentDescriptorV1::from_bytes(
                    crate::Gfx942DeviceContentRoleV1::new([0x49; 32], 7).unwrap(),
                    &[0; 4103],
                )
                .unwrap();
            }
            let settled = f.insert(
                Some(2),
                bytes,
                if matches!(fault, InitializerFault::Layout) {
                    3
                } else {
                    4096
                },
                content,
            );
            assert!(settled.transport, "{fault:?}");
            assert_eq!(settled.result.is_err(), fault.panics(), "{fault:?}");
            assert!(!matches!(settled.result, Ok(Ok(_))), "{fault:?}");
            assert_eq!(f.context().ledger_snapshot(), ledger);
            let root = f.trace.before_failure.as_ref().unwrap();
            root.assert_source(pointer, &expected, content);
            assert_eq!(root.stage_name(), fault.stage(), "{fault:?}");
            assert_eq!(root.native_started(), fault.admitted(), "{fault:?}");
            assert!(root.failed());
            let after = f.memory().insertion_memory_snapshot_v1();
            assert_eq!(after.terminal.is_some(), fault.admitted(), "{fault:?}");
            assert_eq!(
                root.lease().map(|lease| lease.2),
                (fault.admitted() && fault.stage() != "Allocate").then_some(false),
                "{fault:?}"
            );
            assert_eq!(
                f.memory().observation().phase,
                if matches!(fault, InitializerFault::Source | InitializerFault::Layout) {
                    SharedMemorySessionPhaseV1::Active
                } else {
                    SharedMemorySessionPhaseV1::Quarantined
                },
                "{fault:?}"
            );
            let progress = match fault {
                InitializerFault::Map(prefix, errno) => (true, Some(!errno), Some(prefix)),
                InitializerFault::Native("map_gpu", true, _) => (true, None, None),
                InitializerFault::Native("map_gpu", false, _) => (true, Some(false), Some(1)),
                InitializerFault::Currentness(6, _, _) => (true, Some(true), Some(1)),
                _ => (false, None, None),
            };
            assert_eq!(root.progress(), progress);
            if let Some(terminal) = &after.terminal {
                assert_eq!(terminal, root);
            }
            assert_eq!(after.account, native.account);
            assert_eq!(after.terminal_storage, native.terminal_storage);
            f.memory()
                .insertion_assert_native_prefix_v1(&native, fault.prefix(), &expected);
            assert_eq!(f.loan_state(), (loan.0, None, loan.2 + 1));
            f.memory().assert_original_records_unchanged(&before);
            f.assert_partition(native.next_id, expected.len());
            f.assert_terminal_retry(&after);
            f.restore_and_transport(true);
        }
    }
}

#[test]
fn device_insertion_complete_stays_rooted_until_commit_ledger_access() {
    let mut f = InsertionFixture::new(1);
    f.trace.commit_panic = true;
    let ledger = f.context().ledger_snapshot();
    let native = f.memory().insertion_memory_snapshot_v1();
    let (bytes, content) = input();
    let expected = bytes.to_vec();
    let pointer = bytes.as_ptr() as usize;
    let settled = f.insert(Some(2), bytes, 4096, content);
    assert!(settled.transport);
    assert_eq!(
        settled.result.err().unwrap().downcast_ref::<&str>(),
        Some(&"insertion commit ledger access")
    );
    assert_eq!(f.context().ledger_snapshot(), ledger);
    let after = f.memory().insertion_memory_snapshot_v1();
    let root = after.terminal.as_ref().unwrap();
    root.assert_source(pointer, &expected, content);
    assert_eq!(root.stage_name(), "Complete");
    assert!(root.failed() && root.lease().unwrap().2);
    assert_eq!(
        root,
        &f.trace
            .before_retake
            .as_ref()
            .unwrap()
            .with_failure_for_test()
    );
    f.assert_partition(native.next_id, expected.len());
    assert_eq!(
        f.memory().observation().phase,
        SharedMemorySessionPhaseV1::Quarantined
    );
    f.assert_terminal_retry(&after);
    f.restore_and_transport(true);
}

#[test]
fn device_insertion_uses_a_genuine_release_hole_and_explicit_index_overrides_it() {
    for ordinal in 0..3 {
        for index in [None, Some(1)] {
            let mut f = InsertionFixture::new(ordinal);
            let old_identity = f.data[2].storage_identity();
            let mut data = Some(f.data.remove(2));
            let mut released = None;
            let (operation, retake) = f
                .scope
                .parent
                .with_preparation_custody(|memory| {
                    released = Some(memory.insertion_release_device_v1(data.take().unwrap())?);
                    Ok(())
                })
                .unwrap();
            retake.unwrap();
            operation.unwrap();
            f.released.push(released.unwrap());
            {
                let mut context = f.context();
                let ledger = context.ledger();
                assert_eq!(ledger.identities.remove(2), old_identity);
                *ledger.count -= 1;
                *ledger.next = Some(2);
            }
            let native = f.memory().insertion_memory_snapshot_v1();
            let (mut expected, count, next) = f.context().ledger_snapshot();
            assert_eq!((count, next), (3, Some(2)));
            let (bytes, content) = input();
            let size = bytes.len();
            let settled = f.insert(index, bytes, 4096, content);
            assert!(!settled.transport);
            let output = settled.into_result().unwrap();
            expected.insert(index.unwrap_or(2), output.storage_identity());
            f.data.push(output);
            assert_eq!(f.context().ledger_snapshot(), (expected, 4, None));
            f.assert_partition(native.next_id, size);
            f.restore_and_transport(false);
        }
    }
}

#[test]
fn device_insertion_operation_panic_wins_secondary_retake_error_or_panic() {
    for panic in [false, true] {
        for closing in [Outcome::Success, Outcome::Error, Outcome::Panic] {
            let mut f = InsertionFixture::new(1);
            f.memory_mut().primary_arm_native("map_gpu", panic);
            f.scope.parent.faults.reclaim_before = closing;
            let (bytes, content) = input();
            let ledger = f.context().ledger_snapshot();
            let calls = trace().borrow().calls.len();
            let settled = f.insert(Some(2), bytes, 4096, content);
            assert!(settled.transport);
            if panic {
                let payload = settled.result.err().unwrap();
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "map_gpu"))
                );
            } else if closing == Outcome::Error {
                assert!(matches!(
                    settled.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "auxiliary-retake"
                    )))
                ));
            } else {
                assert_eq!(settled.result.is_err(), closing == Outcome::Panic);
            }
            assert_eq!(
                trace().borrow().calls[calls..]
                    .iter()
                    .filter(|&&call| call == "auxiliary-retake")
                    .count(),
                1
            );
            assert_eq!(f.context().ledger_snapshot(), ledger);
            assert_eq!(f.trace.panic_fail, panic || closing == Outcome::Panic);
            f.restore_and_transport(true);
        }
    }
}
