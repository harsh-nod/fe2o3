//! Constructed-engine composition of the production insertion sequencer.
//! Concrete Linux session forwarding and facade transport have separate guards.

use super::*;
use crate::queue::dispatch_binding::preparation::PreparationOwnerRefsV1;
use crate::queue::live::data_insertion::{
    DataInsertionContextV1, DataInsertionIndexV1, DetachedInsertionLedgerV1,
    SettledDataInsertionV1, reserve_identity_capacity_v1, settle_data_insertion_v1,
};
use crate::shared_memory::{
    DeviceInitializationCustodyV1, DeviceInitializationSnapshotV1, DeviceInsertionMemorySnapshotV1,
    DeviceInsertionPrefixV1, SharedMemorySessionPhaseV1,
};
use fe2o3_kfd_uapi::KfdAllocMemoryFlags;

#[path = "integration_coherent_insertion_tests.rs"]
mod coherent_cases;

#[path = "integration_uninitialized_device_insertion_tests.rs"]
mod allocation_cases;

#[path = "integration_data_release_tests.rs"]
mod release_cases;

#[derive(Default)]
struct PrimaryDetached {
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    count: usize,
    next: Option<usize>,
    continuation: Option<UnpublishedDispatchContinuationV1>,
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

impl DataInsertionContextV1<DeviceInitializationCustodyV1, u64> for Context<'_> {
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
        reserve_identity_capacity_v1(
            ledger.identities,
            "detached initialized-device identity ledger",
        )?;
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
                abort = Some(dispatch.take().unwrap().begin_unpublished_abort_v1(buffers));
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
    ) -> SettledDataInsertionV1 {
        settle_data_insertion_v1(
            &mut self.context(),
            DeviceInitializationCustodyV1::new(bytes, content),
            index.map_or(
                DataInsertionIndexV1::HoleOrAppend,
                DataInsertionIndexV1::Explicit,
            ),
            alignment,
        )
    }

    fn assert_partition(&self, attempted_id: u64, bytes: usize) {
        let layout = crate::shared_memory::device_memory_layout(
            bytes as u64,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();
        self.assert_partition_with_layout(attempted_id, layout);
    }

    fn assert_partition_with_layout(
        &self,
        attempted_id: u64,
        layout: crate::Gfx942DeviceMemoryLayoutV1,
    ) {
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
        if layout.uapi_flags() == KfdAllocMemoryFlags::DEVICE_LOCAL.bits() {
            self.memory().insertion_assert_allocation_partition_v1(
                &refs.device_leases,
                &refs.device_authorities,
                None,
                attempted_id,
                layout,
                &self.released,
            );
        } else {
            self.memory().insertion_assert_device_partition_v1(
                &refs.device_leases,
                &refs.device_authorities,
                None,
                attempted_id,
                layout,
                &self.released,
            );
        }
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

#[path = "integration_insertion_tests/insertion_tests.rs"]
mod insertion_tests;
