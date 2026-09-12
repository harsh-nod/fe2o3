//! Observations over the original constructed engine, not a replacement backend.

use super::preparation::PreparationMemoryFixtureV1;
use super::*;
use crate::shared_memory::allocation::PendingAllocationStageV1;
use crate::shared_memory::coherent_initialization::{
    CoherentInitializationV1, CpuAllocation, MappedAllocation,
};
use crate::shared_memory::transitions::{
    self as adapter, NativeTransitionProgressV1, TerminalTokenV1,
};
use std::any::TypeId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoherentTokenSnapshotV1 {
    pub(crate) identity: SharedGttAllocationIdentityV1,
    layout: SharedGttAllocationLayoutV1,
    profile: TypeId,
    state: TypeId,
    userptr: bool,
}

#[test]
fn coherent_insertion_root_is_one_shot_and_extracts_once() {
    for configured in [false, true] {
        let mut memory = PreparationMemoryFixtureV1::new(configured);
        let mut root = CoherentInitializationCustodyV1::new();
        let mut trace = CoherentPreparationTraceV1::default();
        memory
            .primary_prepare_coherent_initialization_v1(
                &mut root,
                &[0x5a; 17],
                CoherentInsertionFaultV1::None,
                &mut trace,
            )
            .unwrap();
        let identity = root.completed().unwrap().storage_identity();
        let before = memory.coherent_insertion_snapshot_v1();
        let accounting = memory.observation();
        let source = trace.source;
        assert!(matches!(
            memory.primary_prepare_coherent_initialization_v1(
                &mut root,
                &[0xff; 9],
                CoherentInsertionFaultV1::None,
                &mut trace
            ),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(root.completed().unwrap().storage_identity(), identity);
        let output = root.take_complete().unwrap();
        assert_eq!(output.storage_identity(), identity);
        assert!(matches!(
            root.take_complete(),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert!(matches!(
            memory.primary_prepare_coherent_initialization_v1(
                &mut root,
                &[1],
                CoherentInsertionFaultV1::None,
                &mut trace
            ),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(trace.stages, [1; 3]);
        assert_eq!(trace.source, source);
        memory.primary_retain_coherent_initialization_v1(root);
        assert_eq!(memory.coherent_insertion_snapshot_v1(), before);
        assert_eq!(memory.observation(), accounting);
        assert_eq!(output.storage_identity(), identity);
    }
}

#[test]
fn coherent_insertion_empty_root_retention_and_rejected_attempt_have_no_effects() {
    for configured in [false, true] {
        let mut memory = PreparationMemoryFixtureV1::new(configured);
        let before = memory.coherent_insertion_snapshot_v1();
        let accounting = memory.observation();
        memory.primary_retain_coherent_initialization_v1(CoherentInitializationCustodyV1::new());
        assert_eq!(memory.coherent_insertion_snapshot_v1(), before);
        let mut root = CoherentInitializationCustodyV1::new();
        let mut trace = CoherentPreparationTraceV1::default();
        assert!(matches!(
            memory.primary_prepare_coherent_initialization_v1(
                &mut root,
                &[],
                CoherentInsertionFaultV1::None,
                &mut trace
            ),
            Err(MemorySessionError::InvalidRequestedSize)
        ));
        assert!(matches!(
            memory.primary_prepare_coherent_initialization_v1(
                &mut root,
                &[1],
                CoherentInsertionFaultV1::None,
                &mut trace
            ),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(trace.stages, [0; 3]);
        assert!(!root.requires_retention());
        memory.primary_retain_coherent_initialization_v1(root);
        assert_eq!(memory.coherent_insertion_snapshot_v1(), before);
        assert_eq!(memory.observation(), accounting);
    }
}

#[test]
fn coherent_insertion_retention_preserves_earlier_lower_failure() {
    for configured in [false, true] {
        let mut memory = PreparationMemoryFixtureV1::new(configured);
        let mut first = CoherentInitializationCustodyV1::new();
        let mut first_trace = CoherentPreparationTraceV1::default();
        memory
            .primary_prepare_coherent_initialization_v1(
                &mut first,
                &[0x5a; 17],
                CoherentInsertionFaultV1::None,
                &mut first_trace,
            )
            .unwrap();
        let mut second = CoherentInitializationCustodyV1::new();
        let mut trace = CoherentPreparationTraceV1::default();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            memory.primary_prepare_coherent_initialization_v1(
                &mut second,
                &[0x6b; 17],
                CoherentInsertionFaultV1::CopyPanic(3),
                &mut trace,
            )
        }));
        assert_eq!(
            failure.unwrap_err().downcast_ref::<(&str, usize)>(),
            Some(&("coherent insertion copy", 3))
        );
        let token = trace.allocated.as_ref().unwrap();
        memory.coherent_assert_terminal_v1(token, "Copy", false, (false, None, None));
        let before = memory.coherent_insertion_snapshot_v1();
        let accounting = memory.observation();
        let model = memory.fixture.foundation.memory().clone();
        assert!(!second.requires_retention());
        assert!(first.requires_retention());
        for root in [second, first] {
            memory.primary_retain_coherent_initialization_v1(root);
            memory.coherent_assert_terminal_v1(token, "Copy", false, (false, None, None));
            assert_eq!(memory.coherent_insertion_snapshot_v1(), before);
            assert_eq!(memory.observation(), accounting);
            assert_eq!(memory.fixture.foundation.memory(), &model);
        }
    }
}

impl CoherentTokenSnapshotV1 {
    pub(crate) fn assert_id(&self, id: u64) {
        assert_eq!(self.identity.id, id);
    }
    fn new<S: GttAllocationStateV1>(
        token: &SharedGttAllocationV1<HostVisibleCoherentGttV1, S>,
    ) -> Self {
        Self {
            identity: token.storage_identity(),
            layout: token.layout,
            profile: TypeId::of::<HostVisibleCoherentGttV1>(),
            state: TypeId::of::<S>(),
            userptr: false,
        }
    }

    pub(crate) fn mapped(&self) -> Self {
        Self {
            state: TypeId::of::<GttGpuAccessibleMutableV1>(),
            ..self.clone()
        }
    }

    fn terminal(&self, token: &TerminalTokenV1) {
        assert_eq!(
            (token.session_id, token.id, token.generation),
            (
                self.identity.session_id,
                self.identity.id,
                self.identity.generation
            )
        );
        assert_eq!(token.layout, self.layout);
        assert_eq!(
            (token.profile_type, token.state_type, token.userptr),
            (self.profile, self.state, self.userptr)
        );
        assert_eq!(token.profile, SharedGttProfileV1::HostVisibleCoherent);
        assert_eq!(
            token.flags,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT.bits()
        );
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) enum CoherentInsertionFaultV1 {
    #[default]
    None,
    Currentness(usize, bool),
    Native(&'static str, bool),
    Map(u32, bool),
    CopyPanic(usize),
    Projection(bool, PrimaryProjectionCaseV1),
}

#[derive(Default)]
pub(crate) struct CoherentPreparationTraceV1 {
    pub(crate) stages: [usize; 3],
    pub(crate) source: Option<(usize, usize)>,
    pub(crate) allocated: Option<CoherentTokenSnapshotV1>,
    before_model: Option<MemoryLifecycleStateV1>,
    expected_model: Option<MemoryLifecycleStateV1>,
}

struct Initializer<'a> {
    memory: &'a mut PreparationMemoryFixtureV1,
    trace: &'a mut CoherentPreparationTraceV1,
    fault: CoherentInsertionFaultV1,
}

impl CoherentInitializationV1 for Initializer<'_> {
    fn allocate(&mut self, length: usize) -> Result<CpuAllocation, MemorySessionError> {
        self.trace.stages[0] += 1;
        match self.fault {
            CoherentInsertionFaultV1::Currentness(check, panic) => {
                assert!((1..=7).contains(&check));
                let b = &mut self.memory.fixture.engine.backend;
                let at = b.currentness_calls + check;
                if panic {
                    b.panic_currentness_at = Some(at);
                } else {
                    b.fail_currentness_at = Some(at);
                }
            }
            CoherentInsertionFaultV1::Native(operation, panic) => {
                self.memory.primary_arm_native(operation, panic)
            }
            CoherentInsertionFaultV1::Map(prefix, errno) => {
                self.memory.insertion_arm_map_v1(prefix, errno)
            }
            CoherentInsertionFaultV1::Projection(true, case) => {
                self.memory.primary_arm_projection_v1(case)
            }
            _ => {}
        }
        let token = self.memory.allocate(length)?;
        self.trace.allocated = Some(CoherentTokenSnapshotV1::new(&token));
        Ok(token)
    }

    fn copy(
        &mut self,
        token: CpuAllocation,
        source: &[u8],
    ) -> Result<CpuAllocation, MemorySessionError> {
        self.trace.stages[1] += 1;
        self.trace.source = Some((source.as_ptr() as usize, source.len()));
        let engine = &mut self.memory.fixture.engine;
        if let CoherentInsertionFaultV1::CopyPanic(length) = self.fault {
            adapter::with_owned_coherent_bytes_v1(engine, token, |destination| {
                destination[..length].copy_from_slice(&source[..length]);
                std::panic::panic_any(("coherent insertion copy", length));
            })
        } else {
            adapter::copy_coherent_v1(engine, token, source)
        }
    }

    fn map(&mut self, token: CpuAllocation) -> Result<MappedAllocation, MemorySessionError> {
        self.trace.stages[2] += 1;
        if let CoherentInsertionFaultV1::Projection(false, case) = self.fault {
            self.memory.primary_arm_projection_v1(case);
        }
        self.memory.map(token)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MappingSnapshot {
    address: u64,
    offset: usize,
    bytes: Vec<u8>,
    active: bool,
    writable: bool,
    readbacks: usize,
}

fn mapping_snapshot(mapping: &FakeMapping) -> MappingSnapshot {
    MappingSnapshot {
        address: mapping.address,
        offset: mapping.byte_offset,
        bytes: mapping.bytes.clone(),
        active: mapping.active,
        writable: mapping.writable,
        readbacks: mapping.readback_calls.get(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordSnapshot {
    id: u64,
    generation: u64,
    profile: SharedGttProfileV1,
    userptr: bool,
    layout: SharedGttAllocationLayoutV1,
    phase: SharedAllocationPhaseV1,
    va: u64,
    mmap_offset: u64,
    reservation: Option<(u64, usize)>,
    handle: Option<u64>,
    mapping: Option<MappingSnapshot>,
    free_attempted: bool,
    charged: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalTokenSnapshot {
    coordinates: (u64, u64, u64),
    layout: SharedGttAllocationLayoutV1,
    types: (TypeId, TypeId),
    profile: SharedGttProfileV1,
    flags: u32,
    userptr: bool,
}

fn terminal_token_snapshot(t: &TerminalTokenV1) -> TerminalTokenSnapshot {
    TerminalTokenSnapshot {
        coordinates: (t.session_id, t.id, t.generation),
        layout: t.layout,
        types: (t.profile_type, t.state_type),
        profile: t.profile,
        flags: t.flags,
        userptr: t.userptr,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalSnapshot {
    stage: adapter::TransitionStageV1,
    progress: NativeTransitionProgressV1,
    input: Option<TerminalTokenSnapshot>,
    output: Option<TerminalTokenSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingSnapshot {
    id: u64,
    profile: SharedGttProfileV1,
    layout: SharedGttAllocationLayoutV1,
    slot: usize,
    stage: PendingAllocationStageV1,
    reservation: Option<(u64, usize)>,
    output: Option<KfdIoctlAllocMemoryOfGpuArgs>,
    mapping: Option<MappingSnapshot>,
    charged: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoherentInsertionSnapshotV1 {
    pub(crate) next_id: u64,
    pub(crate) calls: [usize; 8],
    pub(crate) operations: Vec<&'static str>,
    pub(crate) phase: SharedMemorySessionPhaseV1,
    next_va: u64,
    retained_va: u64,
    terminal: Option<TerminalSnapshot>,
    pending: Option<PendingSnapshot>,
    records: Vec<RecordSnapshot>,
    cpu_inputs: Vec<((u64, usize), u64, usize)>,
    gpu_inputs: Vec<(u64, u32)>,
}

pub(crate) struct CoherentInsertionPrefixV1 {
    pub(crate) calls: [usize; 5],
    pub(crate) operations: Vec<&'static str>,
    pub(crate) copied: usize,
    pub(crate) record_phase: Option<&'static str>,
    pub(crate) pending: Option<(&'static str, bool, bool, Option<bool>)>,
}

impl CoherentInitializationCustodyV1 {
    pub(crate) fn coherent_snapshot_for_test(&self) -> Option<CoherentTokenSnapshotV1> {
        self.completed
            .as_ref()
            .map(|memory| CoherentTokenSnapshotV1::new(&memory.token))
    }
}

impl PreparationMemoryFixtureV1 {
    pub(crate) fn coherent_expect_unchanged_model_v1(
        &self,
        queue: &QueueModelFoundationV1,
        trace: &mut CoherentPreparationTraceV1,
    ) {
        trace.expected_model = Some(self.coherent_active_foundation_v1(queue).memory().clone());
    }
    fn coherent_active_foundation_v1<'a>(
        &'a self,
        queue: &'a QueueModelFoundationV1,
    ) -> &'a QueueModelFoundationV1 {
        self.primary_loan_state_v1(queue);
        match self.fixture.ownership.phase {
            QueueModelOwnershipPhaseV1::QueueOwned { .. } => queue,
            QueueModelOwnershipPhaseV1::SessionOwnedLiveLoan { .. } => &self.fixture.foundation,
            QueueModelOwnershipPhaseV1::SessionOwned => {
                panic!("constructed queue must retain the original foundation")
            }
        }
    }

    pub(crate) fn coherent_assert_model_v1(
        &self,
        queue: &QueueModelFoundationV1,
        trace: &mut CoherentPreparationTraceV1,
        source: &[u8],
        committed: u8,
    ) {
        assert!(committed <= 2);
        let mut expected = trace
            .before_model
            .as_ref()
            .unwrap()
            .checkpoint_released()
            .unwrap();
        if committed > 0 {
            let token = trace.allocated.as_ref().unwrap();
            let id = token.identity.id;
            let vm = self.fixture.vm;
            let raw = self.fixture.engine.backend.last_allocation_output.unwrap();
            let layout = profile_layout::<HostVisibleCoherentGttV1>(source.len()).unwrap();
            let reservation = VaReservationKeyV1 {
                vm,
                id: VaReservationIdV1(id),
            };
            let allocation = MemoryAllocationKeyV1 {
                vm,
                id: AllocationIdV1(id),
                generation: AllocationGenerationV1(1),
            };
            let mapping = MemoryMappingKeyV1 {
                allocation,
                id: MappingIdV1(id),
            };
            expected = expected
                .next(MemoryTransitionV1::ReserveVa {
                    key: reservation,
                    range: GpuVaRangeV1 {
                        base: raw.va_addr,
                        byte_len: layout.gpu_va_bytes(),
                    },
                    alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
                })
                .unwrap()
                .next(MemoryTransitionV1::Allocate {
                    key: allocation,
                    reservation,
                    handle: UntrustedAllocationHandleObservationV1(raw.handle),
                    spec: MemoryAllocationSpecV1 {
                        byte_len: layout.gpu_va_bytes(),
                        alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
                        kind: MemoryKindV1::HostVisibleCoherent,
                        coherence: MemoryCoherenceV1::HostCoherent,
                    },
                })
                .unwrap();
            if committed == 2 {
                expected = expected
                    .next(MemoryTransitionV1::BeginMap {
                        key: mapping,
                        target_devices: vec![self.fixture.device.model_key()],
                        access: MemoryAccessV1::ReadWrite,
                    })
                    .unwrap()
                    .next(MemoryTransitionV1::ObserveMap {
                        key: mapping,
                        progress: PartialProgressObservationV1 {
                            n_success: 1,
                            status: PartialOperationStatusV1::Succeeded,
                        },
                    })
                    .unwrap();
            }
        }
        assert_eq!(
            self.coherent_active_foundation_v1(queue).memory(),
            &expected,
            "independent lifecycle projection at original loan boundary"
        );
        trace.expected_model = Some(expected);
    }

    pub(crate) fn coherent_assert_model_retained_v1(
        &self,
        queue: &QueueModelFoundationV1,
        trace: &CoherentPreparationTraceV1,
    ) {
        if let Some(expected) = &trace.expected_model {
            assert_eq!(self.coherent_active_foundation_v1(queue).memory(), expected);
        }
    }

    pub(crate) fn coherent_assert_projection_v1(&self, queue: &QueueModelFoundationV1) {
        self.primary_assert_coherent_projection_with_foundation_v1(
            self.coherent_active_foundation_v1(queue),
        );
    }

    pub(crate) fn primary_prepare_coherent_initialization_v1(
        &mut self,
        root: &mut CoherentInitializationCustodyV1,
        source: &[u8],
        fault: CoherentInsertionFaultV1,
        trace: &mut CoherentPreparationTraceV1,
    ) -> Result<(), MemorySessionError> {
        if trace.before_model.is_none() {
            trace.before_model = Some(self.fixture.foundation.memory().clone());
        }
        root.prepare_with_memory(
            &mut Initializer {
                memory: self,
                trace,
                fault,
            },
            source,
        )
    }

    pub(crate) fn primary_retain_coherent_initialization_v1(
        &mut self,
        root: CoherentInitializationCustodyV1,
    ) {
        root.retain_with_engine(&mut self.fixture.engine);
    }

    pub(crate) fn coherent_insertion_snapshot_v1(&self) -> CoherentInsertionSnapshotV1 {
        let e = &self.fixture.engine;
        let b = &e.backend;
        CoherentInsertionSnapshotV1 {
            next_id: e.next_id,
            calls: [
                b.currentness_calls,
                b.reserve_va_calls,
                b.alloc_calls,
                b.map_cpu_calls,
                b.map_gpu_calls,
                b.unmap_gpu_calls,
                b.free_calls,
                b.release_va_calls,
            ],
            operations: b.operations.clone(),
            phase: e.phase,
            next_va: b.fixed_va.unwrap_or(b.next_va),
            retained_va: e.retained_gpu_va_bytes,
            terminal: e.terminal_transition.as_ref().map(|t| TerminalSnapshot {
                stage: t.stage,
                progress: t.progress,
                input: t.input.as_ref().map(terminal_token_snapshot),
                output: t.output.as_ref().map(terminal_token_snapshot),
            }),
            pending: e.pending_allocation.as_ref().map(|p| PendingSnapshot {
                id: p.id,
                profile: p.profile,
                layout: p.layout,
                slot: p.record_slot,
                stage: p.stage,
                reservation: p.reservation,
                output: p.allocation_output,
                mapping: p.mapping.as_ref().map(mapping_snapshot),
                charged: p.host_backing_charge.is_some(),
            }),
            records: e
                .allocations
                .iter()
                .map(|r| RecordSnapshot {
                    id: r.id,
                    generation: r.generation,
                    profile: r.profile,
                    userptr: r.userptr,
                    layout: r.layout,
                    phase: r.phase,
                    va: r.gpu_va,
                    mmap_offset: r.mmap_offset,
                    reservation: r.reservation,
                    handle: r.handle,
                    mapping: r.mapping.as_ref().map(mapping_snapshot),
                    free_attempted: r.free_attempted,
                    charged: r.host_backing_charge.is_some(),
                })
                .collect(),
            cpu_inputs: b.map_cpu_inputs.clone(),
            gpu_inputs: b.map_gpu_inputs.clone(),
        }
    }

    pub(crate) fn coherent_assert_prefix_v1(
        &self,
        before: &CoherentInsertionSnapshotV1,
        source: &[u8],
        expected: CoherentInsertionPrefixV1,
    ) {
        let CoherentInsertionPrefixV1 {
            calls,
            operations,
            copied,
            record_phase,
            pending,
        } = expected;
        let after = self.coherent_insertion_snapshot_v1();
        assert_eq!(
            core::array::from_fn::<_, 5, _>(|i| after.calls[i] - before.calls[i]),
            calls,
            "exact coherent native/currentness prefix"
        );
        assert_eq!(
            &after.calls[5..],
            &before.calls[5..],
            "no disposal of possibly live coherent storage"
        );
        assert_eq!(
            &after.operations[..before.operations.len()],
            before.operations
        );
        assert_eq!(&after.operations[before.operations.len()..], operations);
        assert_eq!(
            &after.records[..before.records.len()],
            before.records,
            "all earlier shared records unchanged"
        );
        assert_eq!(
            after.records.len(),
            before.records.len() + usize::from(record_phase.is_some())
        );
        assert_eq!(
            &after.cpu_inputs[..before.cpu_inputs.len()],
            before.cpu_inputs
        );
        assert_eq!(
            &after.gpu_inputs[..before.gpu_inputs.len()],
            before.gpu_inputs
        );
        assert_eq!(after.cpu_inputs.len() - before.cpu_inputs.len(), calls[3]);
        assert_eq!(after.gpu_inputs.len() - before.gpu_inputs.len(), calls[4]);
        let e = &self.fixture.engine;
        assert_eq!(e.pending_allocation.is_some(), pending.is_some());
        let bytes = profile_layout::<HostVisibleCoherentGttV1>(source.len())
            .unwrap()
            .gpu_va_bytes();
        assert_eq!(
            after.retained_va,
            before.retained_va + if calls[1] != 0 { bytes } else { 0 }
        );
        if let Some(phase) = record_phase {
            let r = e.allocations.last().unwrap();
            assert_eq!((r.id, r.generation), (before.next_id, 1));
            assert_eq!(r.profile, SharedGttProfileV1::HostVisibleCoherent);
            assert!(!r.userptr);
            assert_eq!(after.next_id, before.next_id + 1);
            assert_eq!(
                r.layout,
                profile_layout::<HostVisibleCoherentGttV1>(source.len()).unwrap()
            );
            assert_eq!(format!("{:?}", r.phase), phase);
            assert_eq!(
                r.reservation,
                Some((r.gpu_va, r.layout.gpu_va_bytes as usize))
            );
            let raw = e.backend.last_allocation_output.unwrap();
            assert_eq!(
                (r.handle, r.gpu_va, r.mmap_offset),
                (Some(raw.handle), raw.va_addr, raw.mmap_offset)
            );
            assert!(e.shared_host_backing_charge_matches(r));
            assert!(!r.free_attempted);
            let mapping = r.mapping.as_ref().unwrap();
            assert!(mapping.active && mapping.writable);
            assert_eq!(
                (mapping.address, mapping.byte_offset, mapping.bytes.len()),
                (r.gpu_va, 0, r.layout.cpu_mapping_bytes)
            );
            assert_eq!(&mapping.bytes[..copied], &source[..copied]);
            assert!(
                mapping.bytes[copied..].iter().all(|&b| b == 0),
                "exact untouched logical suffix and padding"
            );
            if calls[3] == 1 {
                assert_eq!(
                    after.cpu_inputs.last(),
                    Some(&(
                        r.reservation.unwrap(),
                        r.mmap_offset,
                        r.layout.cpu_mapping_bytes
                    ))
                );
            }
            if calls[4] == 1 {
                assert_eq!(after.gpu_inputs.last(), Some(&(r.handle.unwrap(), 0)));
            }
        } else if let Some(p) = &e.pending_allocation {
            let (stage, reserved, returned, writable) = pending.unwrap();
            assert_eq!(format!("{:?}", p.stage), stage);
            assert_eq!(p.reservation.is_some(), reserved);
            assert_eq!(
                p.reservation,
                reserved.then_some((before.next_va, p.layout.gpu_va_bytes as usize))
            );
            assert_eq!(p.allocation_output.is_some(), returned);
            assert_eq!(p.mapping.as_ref().map(|m| m.writable), writable);
            assert_eq!(p.id, before.next_id);
            assert_eq!(p.record_slot, before.records.len());
            assert_eq!(after.next_id, before.next_id + 1);
            assert!(!e.allocation_record_slots.contains_key(&p.id));
            assert_eq!(p.profile, SharedGttProfileV1::HostVisibleCoherent);
            assert_eq!(
                p.layout,
                profile_layout::<HostVisibleCoherentGttV1>(source.len()).unwrap()
            );
            assert!(
                p.host_backing_charge.is_none(),
                "pending charge moved to quarantined credits"
            );
            if let Some(raw) = p.allocation_output {
                assert_eq!(Some(raw), e.backend.last_allocation_output);
                assert_eq!(raw.va_addr, p.reservation.unwrap().0);
                assert_eq!(raw.size, p.layout.gpu_va_bytes);
                assert_eq!(raw.flags, KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT.bits());
                if calls[3] == 1 {
                    assert_eq!(
                        after.cpu_inputs.last(),
                        Some(&(
                            p.reservation.unwrap(),
                            raw.mmap_offset,
                            p.layout.cpu_mapping_bytes
                        ))
                    );
                }
            }
            if let Some(mapping) = &p.mapping {
                assert_eq!(mapping.address, p.reservation.unwrap().0);
                assert_eq!(mapping.byte_offset, 0);
                assert_eq!(mapping.bytes.len(), p.layout.cpu_mapping_bytes);
                assert!(mapping.active);
                assert!(mapping.bytes.iter().all(|&b| b == 0));
            }
        } else {
            assert_eq!(after.next_id, before.next_id);
        }
    }

    pub(crate) fn coherent_assert_terminal_v1(
        &self,
        expected: &CoherentTokenSnapshotV1,
        stage: &str,
        output: bool,
        progress: (bool, Option<bool>, Option<u32>),
    ) {
        let terminal = self
            .fixture
            .engine
            .terminal_transition
            .as_ref()
            .expect("coherent owner retained after failed settlement");
        assert_eq!(format!("{:?}", terminal.stage), stage);
        assert_eq!(terminal.input.is_some(), !output);
        assert_eq!(terminal.output.is_some(), output);
        expected.terminal(if output {
            terminal.output.as_ref().unwrap()
        } else {
            terminal.input.as_ref().unwrap()
        });
        assert_eq!(
            terminal.progress,
            NativeTransitionProgressV1 {
                attempted: progress.0,
                returned_success: progress.1,
                returned_map_prefix: progress.2
            }
        );
        assert_eq!(
            self.fixture.engine.phase,
            SharedMemorySessionPhaseV1::Quarantined
        );
    }
}
