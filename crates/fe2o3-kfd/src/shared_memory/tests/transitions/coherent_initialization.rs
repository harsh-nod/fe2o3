use super::*;
use crate::shared_memory::coherent_initialization::{
    CoherentInitializationV1, CpuAllocation, MappedAllocation, initialize_v1,
};

const SOURCE: [u8; 17] = [0x5a; 17];

struct Initializer {
    memory: BackingConstructorFixture,
    anchor: Option<MappedAllocation>,
    anchor_identity: Option<TokenSnapshot>,
    anchor_native: (u64, Option<u64>, u64),
    anchor_bytes: Vec<u8>,
    stages: [usize; 3],
    source: Option<(usize, usize)>,
    allocated: Option<TokenSnapshot>,
    after_allocation: Option<MemoryLifecycleStateV1>,
    projection_fault: Option<(Stage, Fault)>,
    panic_after_copy_bytes: Option<usize>,
    poison_calls: usize,
}

impl Initializer {
    fn new(configured: bool) -> Self {
        Self::with_fixture(fixture(configured))
    }

    fn with_fixture(memory: BackingConstructorFixture) -> Self {
        let mut this = Self {
            memory,
            anchor: None,
            anchor_identity: None,
            anchor_native: (0, None, 0),
            anchor_bytes: Vec::new(),
            stages: [0; 3],
            source: None,
            allocated: None,
            after_allocation: None,
            projection_fault: None,
            panic_after_copy_bytes: None,
            poison_calls: 0,
        };
        let anchor = initialize_v1(&mut this, &[7; 17]).unwrap().into_token();
        this.anchor_identity = Some(TokenSnapshot::new(&anchor));
        let record = &this.memory.engine.allocations[0];
        this.anchor_native = (record.gpu_va, record.handle, record.mmap_offset);
        this.anchor_bytes = record.mapping.as_ref().unwrap().bytes.clone();
        this.anchor = Some(anchor);
        this.stages = [0; 3];
        this.source = None;
        this.allocated = None;
        this.after_allocation = None;
        this
    }

    fn assert_anchor(&self) {
        let engine = &self.memory.engine;
        assert_eq!(
            TokenSnapshot::new(self.anchor.as_ref().unwrap()),
            self.anchor_identity.unwrap()
        );
        let record = &engine.allocations[0];
        assert_eq!(
            (record.gpu_va, record.handle, record.mmap_offset),
            self.anchor_native
        );
        assert_eq!(record.phase, SharedAllocationPhaseV1::GpuAccessibleMutable);
        let mapping = record.mapping.as_ref().unwrap();
        assert_eq!(mapping.bytes, self.anchor_bytes);
        assert_eq!(mapping.address, record.gpu_va);
        assert!(mapping.active && mapping.writable);
        for record in &engine.allocations {
            assert_complete_record(&self.memory, record.id);
            assert!(engine.shared_host_backing_charge_matches(record));
        }
        if let Some(usage) = self.memory.usage() {
            assert_eq!(usage.used_backing_bytes, 0);
            assert_eq!(usage.used_allocation_records, 0);
        }
        assert_eq!(engine.backend.unmap_gpu_calls, 0);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }

    fn assert_debit(&self, bytes: u64, records: u64, quarantined: usize) {
        if let Some(account) = &self.memory.engine.host_backing_account {
            let usage = account.usage();
            assert_eq!(usage.used_backing_bytes, bytes);
            assert_eq!(usage.used_allocation_records, records);
            assert_eq!(usage.reserved_records, 0);
            assert_eq!(usage.quarantined_records, quarantined);
            assert_eq!(usage.retained_records, records as usize - quarantined);
        }
    }

    fn assert_terminal(&self, stage: Stage, mapped: bool) {
        let terminal = self.memory.engine.terminal_transition.as_ref().unwrap();
        assert_eq!(terminal.stage, stage);
        let actual = if mapped {
            assert!(terminal.input.is_none());
            terminal.output.as_ref().unwrap()
        } else {
            assert!(terminal.output.is_none());
            terminal.input.as_ref().unwrap()
        };
        let mut expected = self.allocated.expect("actual returned CPU token");
        if mapped {
            expected.state = TypeId::of::<GttGpuAccessibleMutableV1>();
        }
        expected.assert_terminal(actual);
        assert_eq!(self.memory.engine.allocations.len(), 2);
        assert!(self.memory.engine.pending_allocation.is_none());
        assert_eq!(
            self.memory.engine.allocations[1].phase,
            if mapped {
                SharedAllocationPhaseV1::GpuAccessibleMutable
            } else {
                SharedAllocationPhaseV1::CpuWritable
            }
        );
        assert_eq!(
            self.memory.foundation.memory(),
            self.after_allocation.as_ref().unwrap()
        );
        self.assert_anchor();
        self.assert_debit(8192, 2, 0);
    }

    fn assert_no_retry(&mut self) {
        let before = calls(&self.memory.engine);
        let records = self.memory.engine.allocations.len();
        let host = self
            .memory
            .engine
            .host_backing_account
            .as_ref()
            .map(|a| a.usage());
        assert!(matches!(
            initialize_v1(self, &SOURCE),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
        assert_eq!(calls(&self.memory.engine), before);
        assert_eq!(self.memory.engine.allocations.len(), records);
        assert_eq!(
            self.memory
                .engine
                .host_backing_account
                .as_ref()
                .map(|a| a.usage()),
            host
        );
        assert_closed(&mut self.memory);
        self.assert_anchor();
    }

    fn assert_pending_details(&self, original_va: u64, writable: bool) {
        let engine = &self.memory.engine;
        let pending = engine.pending_allocation.as_ref().unwrap();
        let layout = profile_layout::<HostVisibleCoherentGttV1>(SOURCE.len()).unwrap();
        assert_eq!(pending.layout, layout);
        assert_eq!(pending.profile, SharedGttProfileV1::HostVisibleCoherent);
        assert_eq!((pending.id, pending.record_slot), (2, 1));
        assert!(!engine.allocation_record_slots.contains_key(&pending.id));
        if let Some(reservation) = pending.reservation {
            assert_eq!(reservation, (original_va, layout.gpu_va_bytes as usize));
        }
        if let Some(output) = pending.allocation_output {
            assert_eq!(Some(output), engine.backend.last_allocation_output);
            assert_eq!(output.va_addr, original_va);
            assert_eq!(output.size, layout.gpu_va_bytes);
            assert_eq!(
                output.flags,
                KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT.bits()
            );
        }
        if let Some(mapping) = &pending.mapping {
            assert_eq!(mapping.address, original_va);
            assert_eq!(mapping.byte_offset, 0);
            assert_eq!(mapping.bytes.len(), layout.cpu_mapping_bytes);
            assert!(mapping.bytes.iter().all(|b| *b == 0));
            assert!(mapping.active);
            assert_eq!(mapping.writable, writable);
            assert_eq!(mapping.readback_calls.get(), 0);
        }
    }
}

impl CoherentInitializationV1 for Initializer {
    fn allocate(&mut self, length: usize) -> Result<CpuAllocation, MemorySessionError> {
        self.stages[0] += 1;
        let memory = &mut self.memory;
        let mut projection =
            adapter::ProjectionV1::new(&mut memory.foundation, memory.device, memory.vm);
        projection.fault = self.projection_fault;
        let token = adapter::allocate_v1(&mut memory.engine, &mut projection, length, || {
            self.poison_calls += 1
        })?;
        self.allocated = Some(TokenSnapshot::new(&token));
        self.after_allocation = Some(memory.foundation.memory().clone());
        Ok(token)
    }

    fn copy(
        &mut self,
        token: CpuAllocation,
        source: &[u8],
    ) -> Result<CpuAllocation, MemorySessionError> {
        self.stages[1] += 1;
        self.source = Some((source.as_ptr() as usize, source.len()));
        if let Some(length) = self.panic_after_copy_bytes {
            adapter::with_owned_coherent_bytes_v1(&mut self.memory.engine, token, |destination| {
                destination[..length].copy_from_slice(&source[..length]);
                std::panic::panic_any(("coherent copy panic", length));
            })
        } else {
            adapter::copy_coherent_v1(&mut self.memory.engine, token, source)
        }
    }

    fn map(&mut self, token: CpuAllocation) -> Result<MappedAllocation, MemorySessionError> {
        self.stages[2] += 1;
        let memory = &mut self.memory;
        let mut projection =
            adapter::ProjectionV1::new(&mut memory.foundation, memory.device, memory.vm);
        projection.fault = self.projection_fault;
        adapter::map_mutable_v1(&mut memory.engine, &mut projection, token, || {
            self.poison_calls += 1
        })
    }
}

#[test]
fn coherent_initializer_success_preserves_exact_storage_source_and_padded_charge() {
    for configured in [false, true] {
        for length in [1usize, 4096, 4097] {
            let mut memory = Initializer::new(configured);
            let mut source = (0..length).map(|i| (i % 251) as u8).collect::<Vec<_>>();
            let original = (source.as_ptr() as usize, source.len(), source.capacity());
            let before = calls(&memory.memory.engine);
            let initialized = initialize_v1(&mut memory, &source).unwrap();
            assert_eq!(memory.stages, [1; 3]);
            assert_eq!(memory.source, Some((original.0, original.1)));
            assert_eq!(
                (source.as_ptr() as usize, source.len(), source.capacity()),
                original
            );
            assert!(
                source
                    .iter()
                    .enumerate()
                    .all(|(i, b)| *b == (i % 251) as u8)
            );
            let mut expected = memory.allocated.unwrap();
            expected.state = TypeId::of::<GttGpuAccessibleMutableV1>();
            let token = initialized.into_token();
            assert_eq!(TokenSnapshot::new(&token), expected);
            source.fill(0xa5);
            drop(source);
            let record = &memory.memory.engine.allocations[1];
            let mapping = record.mapping.as_ref().unwrap();
            assert_eq!(record.phase, SharedAllocationPhaseV1::GpuAccessibleMutable);
            assert!(
                mapping.bytes[..length]
                    .iter()
                    .enumerate()
                    .all(|(i, b)| *b == (i % 251) as u8)
            );
            assert!(mapping.bytes[length..].iter().all(|b| *b == 0));
            assert_eq!(memory.memory.engine.backend.map_gpu_calls, before[4] + 1);
            assert_eq!(
                memory.memory.engine.backend.currentness_calls,
                before[0] + 7
            );
            assert!(memory.memory.engine.terminal_transition.is_none());
            let (_, _, mapping_key) =
                model_keys(memory.memory.vm, expected.id, expected.generation);
            let expected_model = project_map(
                memory.after_allocation.as_ref().unwrap(),
                mapping_key,
                memory.memory.device,
            )
            .unwrap();
            assert_eq!(memory.memory.foundation.memory(), &expected_model);
            memory.assert_debit(4096 + length.div_ceil(4096) as u64 * 4096, 2, 0);
            memory.assert_anchor();
        }
    }
}

#[test]
fn coherent_initializer_empty_capacity_and_revision_reject_before_native_effects() {
    for configured in [false, true] {
        let mut memory = Initializer::new(configured);
        let before = calls(&memory.memory.engine);
        assert!(matches!(
            initialize_v1(&mut memory, &[]),
            Err(MemorySessionError::InvalidRequestedSize)
        ));
        assert_eq!(memory.stages, [0; 3]);
        assert_eq!(calls(&memory.memory.engine), before);
        memory.assert_debit(4096, 1, 0);
        memory.assert_anchor();
        certify(&mut memory.memory, u64::MAX - 1);
        assert!(matches!(
            initialize_v1(&mut memory, &SOURCE),
            Err(MemorySessionError::Model(
                "queue foundation certificate revision exhausted"
            ))
        ));
        assert_eq!(memory.poison_calls, 1);
        assert_eq!(memory.stages, [1, 0, 0]);
        assert_eq!(calls(&memory.memory.engine), before);
        assert!(memory.memory.engine.terminal_transition.is_none());
        memory.assert_debit(4096, 1, 0);
        memory.assert_anchor();
        memory.assert_no_retry();
    }
    for (bytes, records) in [(4096, 2), (8192, 1)] {
        let mut base = fixture(false);
        base.engine
            .configure_host_visible_backing_budget_v1(
                base.device.model_key(),
                base.vm,
                Gfx942HostVisibleBackingBudgetV1::new(bytes, records).unwrap(),
            )
            .unwrap();
        let mut memory = Initializer::with_fixture(base);
        let before = calls(&memory.memory.engine);
        assert!(matches!(
            initialize_v1(&mut memory, &SOURCE),
            Err(MemorySessionError::HostVisibleBackingCredits(_))
        ));
        assert_eq!(memory.stages, [1, 0, 0]);
        assert_eq!(calls(&memory.memory.engine), before);
        assert_eq!(
            memory.memory.engine.phase(),
            SharedMemorySessionPhaseV1::Active
        );
        memory.assert_debit(4096, 1, 0);
        memory.assert_anchor();
    }
}

#[test]
fn coherent_initializer_copy_currentness_keeps_actual_cpu_owner_and_written_prefix() {
    for configured in [false, true] {
        for step in [4, 5] {
            for panic in [false, true] {
                let mut memory = Initializer::new(configured);
                let before = memory.memory.engine.backend.currentness_calls;
                if panic {
                    memory.memory.engine.backend.panic_currentness_at = Some(before + step);
                } else {
                    memory.memory.engine.backend.fail_currentness_at = Some(before + step);
                }
                let result = catch_unwind(AssertUnwindSafe(|| initialize_v1(&mut memory, &SOURCE)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Injected("currentness"))
                    ));
                }
                assert_eq!(memory.stages, [1, 1, 0]);
                assert_eq!(
                    memory.memory.engine.backend.currentness_calls,
                    before + step
                );
                assert_eq!(memory.memory.engine.backend.map_gpu_calls, 1);
                let bytes = &memory.memory.engine.allocations[1]
                    .mapping
                    .as_ref()
                    .unwrap()
                    .bytes;
                assert_eq!(
                    &bytes[..SOURCE.len()],
                    if step == 4 { &[0; 17] } else { &SOURCE }
                );
                memory.assert_terminal(Stage::Copy, false);
                memory.assert_no_retry();
                memory.assert_terminal(Stage::Copy, false);
            }
        }
    }
}

#[test]
fn coherent_initializer_copy_panics_preserve_first_payload_without_closing_check() {
    for configured in [false, true] {
        for copied in [None, Some(0), Some(1), Some(SOURCE.len())] {
            for closing in 0..3 {
                let mut memory = Initializer::new(configured);
                let before = memory.memory.engine.backend.currentness_calls;
                memory.panic_after_copy_bytes = copied;
                if copied.is_none() {
                    memory.memory.engine.backend.panic_operation = Some("with_bytes_mut");
                }
                if closing == 1 {
                    memory.memory.engine.backend.fail_currentness_at = Some(before + 5);
                }
                if closing == 2 {
                    memory.memory.engine.backend.panic_currentness_at = Some(before + 5);
                }
                let result = catch_unwind(AssertUnwindSafe(|| initialize_v1(&mut memory, &SOURCE)));
                let payload = result.unwrap_err();
                if let Some(length) = copied {
                    assert_eq!(
                        payload.downcast_ref::<(&str, usize)>(),
                        Some(&("coherent copy panic", length))
                    );
                } else {
                    assert_eq!(
                        payload.downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "with_bytes_mut"))
                    );
                }
                assert_eq!(memory.memory.engine.backend.currentness_calls, before + 4);
                assert_eq!(memory.stages, [1, 1, 0]);
                assert_eq!(memory.memory.engine.backend.map_gpu_calls, 1);
                let bytes = &memory.memory.engine.allocations[1]
                    .mapping
                    .as_ref()
                    .unwrap()
                    .bytes;
                let length = copied.unwrap_or(0);
                assert_eq!(&bytes[..length], &SOURCE[..length]);
                assert!(bytes[length..].iter().all(|b| *b == 0));
                memory.assert_terminal(Stage::Copy, false);
                memory.assert_no_retry();
                memory.assert_terminal(Stage::Copy, false);
            }
        }
    }
}

#[test]
fn coherent_initializer_projection_failures_keep_exact_allocation_or_mapped_successor() {
    for configured in [false, true] {
        for stage in [
            Stage::AllocationEvidence,
            Stage::AllocationProjection,
            Stage::AllocationCommit,
            Stage::MappingEvidence,
            Stage::Map,
            Stage::MapProjection,
            Stage::MapCommit,
        ] {
            for fault in [Fault::Error, Fault::Panic, Fault::ExhaustRevision] {
                if matches!(fault, Fault::ExhaustRevision)
                    && !matches!(stage, Stage::AllocationCommit | Stage::MapCommit)
                {
                    continue;
                }
                let mut memory = Initializer::new(configured);
                certify(&mut memory.memory, 0);
                let before = memory.memory.foundation.memory().clone();
                memory.projection_fault = Some((stage, fault));
                let result = catch_unwind(AssertUnwindSafe(|| initialize_v1(&mut memory, &SOURCE)));
                if matches!(fault, Fault::Panic) {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                        Some(&("session projection", stage))
                    );
                } else {
                    assert!(result.unwrap().is_err());
                }
                let allocation = matches!(
                    stage,
                    Stage::AllocationEvidence
                        | Stage::AllocationProjection
                        | Stage::AllocationCommit
                );
                let mapped = matches!(stage, Stage::MapProjection | Stage::MapCommit);
                assert_eq!(memory.stages, if allocation { [1, 0, 0] } else { [1; 3] });
                assert_eq!(
                    memory.memory.engine.backend.map_gpu_calls,
                    1 + usize::from(mapped)
                );
                if allocation {
                    let terminal = memory.memory.engine.terminal_transition.as_ref().unwrap();
                    assert_eq!(terminal.stage, stage);
                    assert!(terminal.input.is_none());
                    let output = terminal.output.as_ref().unwrap();
                    assert_eq!(
                        (output.session_id, output.id, output.generation),
                        (memory.memory.engine.session_id, 2, 1)
                    );
                    assert_eq!(
                        output.profile_type,
                        TypeId::of::<HostVisibleCoherentGttV1>()
                    );
                    assert_eq!(output.state_type, TypeId::of::<GttCpuWritableV1>());
                    assert_eq!(
                        output.layout,
                        profile_layout::<HostVisibleCoherentGttV1>(SOURCE.len()).unwrap()
                    );
                    assert_eq!(memory.memory.foundation.memory(), &before);
                    memory.assert_anchor();
                    memory.assert_debit(8192, 2, 0);
                } else {
                    memory.assert_terminal(stage, mapped);
                }
                memory.assert_no_retry();
                assert_eq!(
                    memory
                        .memory
                        .engine
                        .terminal_transition
                        .as_ref()
                        .unwrap()
                        .stage,
                    stage
                );
            }
        }
    }
}

#[test]
fn coherent_initializer_native_allocation_failures_retain_original_pending_prefix() {
    for configured in [false, true] {
        for operation in ["reserve_va", "alloc", "map_cpu", "prepare_cpu_mapping"] {
            for panic in [false, true] {
                let mut memory = Initializer::new(configured);
                let original_va = memory.memory.engine.backend.next_va;
                if panic {
                    memory.memory.engine.backend.panic_operation = Some(operation);
                } else {
                    memory.memory.engine.backend.fail_operation = Some(operation);
                }
                let result = catch_unwind(AssertUnwindSafe(|| initialize_v1(&mut memory, &SOURCE)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", operation))
                    );
                } else {
                    assert!(
                        matches!(result.unwrap(), Err(MemorySessionError::Injected(op)) if op == operation)
                    );
                }
                assert_eq!(memory.stages, [1, 0, 0]);
                assert!(memory.memory.engine.terminal_transition.is_none());
                let pending = memory.memory.engine.pending_allocation.as_ref().unwrap();
                assert_eq!(pending.id, 2);
                assert_eq!(pending.record_slot, 1);
                assert_eq!(pending.reservation.is_some(), operation != "reserve_va");
                assert_eq!(
                    pending.allocation_output.is_some(),
                    matches!(operation, "map_cpu" | "prepare_cpu_mapping")
                        || operation == "alloc" && !panic
                );
                assert_eq!(
                    pending.mapping.is_some(),
                    operation == "prepare_cpu_mapping"
                );
                assert_eq!(memory.memory.engine.allocations.len(), 1);
                memory.assert_pending_details(original_va, false);
                memory.assert_debit(8192, 2, 1);
                memory.assert_anchor();
                memory.assert_no_retry();
                memory.assert_pending_details(original_va, false);
            }
        }
    }
}

#[test]
fn coherent_initializer_map_failures_retain_cpu_authority_without_duplicate_custody() {
    for configured in [false, true] {
        for case in 0..11 {
            let mut memory = Initializer::new(configured);
            let before = memory.memory.engine.backend.currentness_calls;
            match case {
                0 => {
                    memory.memory.engine.backend.map_progress = 0;
                }
                1 => {
                    memory.memory.engine.backend.map_progress = 0;
                    memory.memory.engine.backend.map_errno = true;
                }
                2 => {
                    memory.memory.engine.backend.map_errno = true;
                }
                3 => {
                    memory.memory.engine.backend.map_progress = 2;
                }
                4 => {
                    memory.memory.engine.backend.map_progress = 2;
                    memory.memory.engine.backend.map_errno = true;
                }
                5 => memory.memory.engine.backend.fail_operation = Some("map_gpu"),
                6 => memory.memory.engine.backend.panic_operation = Some("map_gpu"),
                7 => memory.memory.engine.backend.fail_currentness_at = Some(before + 6),
                8 => memory.memory.engine.backend.panic_currentness_at = Some(before + 6),
                9 => memory.memory.engine.backend.fail_currentness_at = Some(before + 7),
                10 => memory.memory.engine.backend.panic_currentness_at = Some(before + 7),
                _ => unreachable!(),
            }
            let result = catch_unwind(AssertUnwindSafe(|| initialize_v1(&mut memory, &SOURCE)));
            if matches!(case, 6 | 8 | 10) {
                let operation = if case == 6 { "map_gpu" } else { "currentness" };
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(memory.stages, [1; 3]);
            assert_eq!(
                &memory.memory.engine.allocations[1]
                    .mapping
                    .as_ref()
                    .unwrap()
                    .bytes[..SOURCE.len()],
                &SOURCE
            );
            let progress = memory
                .memory
                .engine
                .terminal_transition
                .as_ref()
                .unwrap()
                .progress;
            assert_eq!(progress.attempted, !matches!(case, 7 | 8));
            assert_eq!(
                progress.returned_map_prefix,
                match case {
                    0 | 1 => Some(0),
                    2 | 5 | 9 | 10 => Some(1),
                    3 | 4 => Some(2),
                    _ => None,
                }
            );
            memory.assert_terminal(Stage::Map, false);
            memory.assert_no_retry();
            memory.assert_terminal(Stage::Map, false);
        }
    }
}

#[test]
fn coherent_initializer_preserves_revision_headroom_and_rejects_map_exhaustion() {
    for configured in [false, true] {
        let mut memory = Initializer::new(configured);
        certify(&mut memory.memory, u64::MAX - 3);
        let initialized = initialize_v1(&mut memory, &SOURCE).unwrap();
        assert_eq!(initialized.layout().requested_bytes(), SOURCE.len());
        assert_eq!(memory.poison_calls, 0);
        assert!(
            memory
                .memory
                .foundation
                .preflight_memory_transition_revisions(1)
                .is_err()
        );
        memory.assert_anchor();

        let mut memory = Initializer::new(configured);
        certify(&mut memory.memory, u64::MAX - 2);
        let before = memory.memory.engine.backend.currentness_calls;
        assert!(initialize_v1(&mut memory, &SOURCE).is_err());
        assert_eq!(memory.poison_calls, 1);
        assert_eq!(memory.memory.engine.backend.currentness_calls, before + 5);
        assert_eq!(memory.stages, [1; 3]);
        memory.assert_terminal(Stage::Preflight, false);
        memory.assert_no_retry();
    }
}

#[test]
fn coherent_initializer_allocation_currentness_preserves_preflight_and_pending_policy() {
    for configured in [false, true] {
        for step in 1..=3 {
            for panic in [false, true] {
                let mut memory = Initializer::new(configured);
                let original_va = memory.memory.engine.backend.next_va;
                let before = calls(&memory.memory.engine);
                if panic {
                    memory.memory.engine.backend.panic_currentness_at = Some(before[0] + step);
                } else {
                    memory.memory.engine.backend.fail_currentness_at = Some(before[0] + step);
                }
                let result = catch_unwind(AssertUnwindSafe(|| initialize_v1(&mut memory, &SOURCE)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Injected("currentness"))
                    ));
                }
                assert_eq!(memory.stages, [1, 0, 0]);
                assert_eq!(
                    memory.memory.engine.backend.currentness_calls,
                    before[0] + step
                );
                assert!(memory.memory.engine.terminal_transition.is_none());
                assert_eq!(memory.memory.engine.allocations.len(), 1);
                if step == 1 {
                    assert!(memory.memory.engine.pending_allocation.is_none());
                    assert_eq!(&calls(&memory.memory.engine)[1..], &before[1..]);
                    memory.assert_debit(4096, 1, 0);
                } else {
                    let pending = memory.memory.engine.pending_allocation.as_ref().unwrap();
                    assert_eq!((pending.id, pending.record_slot), (2, 1));
                    assert!(pending.reservation.is_some());
                    assert!(pending.allocation_output.is_some());
                    assert_eq!(pending.mapping.is_some(), step == 3);
                    memory.assert_pending_details(original_va, step == 3);
                    memory.assert_debit(8192, 2, 1);
                }
                memory.assert_anchor();
                if step == 1 && panic && !configured {
                    assert_eq!(
                        memory.memory.engine.phase(),
                        SharedMemorySessionPhaseV1::Active
                    );
                    memory.memory.engine.backend.panic_currentness_at = None;
                    let retried = initialize_v1(&mut memory, &SOURCE).unwrap();
                    assert_eq!(retried.layout().requested_bytes(), SOURCE.len());
                    assert_eq!(
                        &memory.memory.engine.allocations[1]
                            .mapping
                            .as_ref()
                            .unwrap()
                            .bytes[..SOURCE.len()],
                        &SOURCE
                    );
                    assert_eq!(memory.memory.engine.backend.reserve_va_calls, before[1] + 1);
                    assert!(memory.memory.engine.terminal_transition.is_none());
                    memory.assert_anchor();
                } else {
                    memory.assert_no_retry();
                }
            }
        }
    }
}

#[test]
fn coherent_copy_invalid_private_inputs_reject_before_currentness_without_poison() {
    for case in 0..5 {
        let mut memory = Initializer::new(true);
        let mut token = memory.allocate(SOURCE.len()).unwrap();
        let mut removed_charge = None;
        match case {
            0 => token.session_id += 1,
            1 => token.id += 1,
            2 => token.generation += 1,
            3 => token.layout.requested_bytes += 1,
            4 => {
                removed_charge = memory.memory.engine.allocations[1]
                    .host_backing_charge
                    .take()
            }
            _ => unreachable!(),
        }
        let before = calls(&memory.memory.engine);
        assert!(matches!(
            adapter::copy_coherent_v1(&mut memory.memory.engine, token, &SOURCE),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(calls(&memory.memory.engine), before);
        assert_eq!(
            memory.memory.engine.phase(),
            SharedMemorySessionPhaseV1::Active
        );
        assert!(memory.memory.engine.terminal_transition.is_none());
        if case == 4 {
            memory.memory.engine.allocations[1].host_backing_charge = removed_charge;
        }
        memory.assert_debit(8192, 2, 0);
        memory.assert_anchor();
        assert!(
            memory.memory.engine.allocations[1]
                .mapping
                .as_ref()
                .unwrap()
                .bytes
                .iter()
                .all(|b| *b == 0)
        );
    }
}
