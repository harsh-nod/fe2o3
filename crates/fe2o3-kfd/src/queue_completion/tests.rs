use super::*;
use fe2o3_aql::{
    AMD_SIGNAL_VALUE_COMPLETE_V1, AMD_SIGNAL_VALUE_PENDING_V1, AqlBarrierAndPacketV1,
    AqlBarrierAndPublicationTargetV1, AqlPacketBatchPublicationTargetV1,
    classify_acquired_completion_value_v1, encode_pending_completion_signal_bytes_v1,
};
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
    MemoryAllocationKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1, VmIdV1,
    VmKeyV1,
};
use sha2::{Digest, Sha256};

#[repr(C, align(64))]
struct AlignedArena([u8; COMPLETION_SIGNAL_ARENA_BYTES_V1]);

struct MockBackend {
    values: [i64; COMPLETION_SIGNAL_CAPACITY_V1],
    trace: Vec<&'static str>,
    currentness_calls: usize,
    fail_currentness_at: Option<usize>,
    observe_calls: usize,
    fail_observe_at: Option<usize>,
    extra_batch_observation: Option<AqlCompletionObservationV1>,
    reset_calls: usize,
    fail_reset_at: Option<usize>,
}

#[derive(Default)]
struct PacketCapture {
    signals: Vec<u64>,
    headers: Vec<u16>,
}

#[derive(Default)]
struct BarrierCapture {
    bytes: Option<[u8; 64]>,
    header: Option<u16>,
}

impl AqlBarrierAndPublicationTargetV1 for BarrierCapture {
    type Error = ();

    fn write_unpublished_barrier(
        &mut self,
        packet: &AqlBarrierAndPacketV1,
    ) -> Result<(), Self::Error> {
        self.bytes = Some(packet.encode_unpublished_le());
        Ok(())
    }

    fn publish_barrier_release_header(&mut self, header: u16) -> Result<(), Self::Error> {
        self.header = Some(header);
        Ok(())
    }
}

impl AqlPacketBatchPublicationTargetV1 for PacketCapture {
    type Error = ();

    fn write_unpublished(
        &mut self,
        _batch_index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error> {
        self.signals.push(packet.completion_signal());
        Ok(())
    }

    fn publish_release_header(
        &mut self,
        _batch_index: u32,
        header: u16,
    ) -> Result<(), Self::Error> {
        self.headers.push(header);
        Ok(())
    }
}

impl MockBackend {
    fn pending() -> Self {
        Self {
            values: [AMD_SIGNAL_VALUE_PENDING_V1; COMPLETION_SIGNAL_CAPACITY_V1],
            trace: Vec::new(),
            currentness_calls: 0,
            fail_currentness_at: None,
            observe_calls: 0,
            fail_observe_at: None,
            extra_batch_observation: None,
            reset_calls: 0,
            fail_reset_at: None,
        }
    }
}

impl NativeCompletionSignalBackendV1 for MockBackend {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        self.trace.push("currentness");
        self.currentness_calls += 1;
        if self.fail_currentness_at == Some(self.currentness_calls) {
            Err(Gfx942CompletionErrorV1::Currentness)
        } else {
            Ok(())
        }
    }

    fn observe_one_acquire_in_current_scope(
        &mut self,
        slot_index: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        self.trace.push("acquire");
        self.observe_calls += 1;
        if self.fail_observe_at == Some(self.observe_calls) {
            return Err(Gfx942CompletionErrorV1::Observation);
        }
        Ok(classify_acquired_completion_value_v1(
            self.values[slot_index as usize],
        ))
    }

    fn observe_batch_acquire_in_current_scope(
        &mut self,
        slot_indices: &[u32],
    ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        self.trace.push("acquire");
        let mut observations = Vec::with_capacity(slot_indices.len());
        for &slot_index in slot_indices {
            self.observe_calls += 1;
            if self.fail_observe_at == Some(self.observe_calls) {
                return Err(Gfx942CompletionErrorV1::Observation);
            }
            observations.push(classify_acquired_completion_value_v1(
                self.values[slot_index as usize],
            ));
        }
        observations.extend(self.extra_batch_observation);
        Ok(observations)
    }

    fn reset_pending_release(&mut self, slot_index: u32) -> Result<(), Gfx942CompletionErrorV1> {
        self.trace.push("reset");
        self.reset_calls += 1;
        if self.fail_reset_at == Some(self.reset_calls) {
            return Err(Gfx942CompletionErrorV1::Recycle);
        }
        self.values[slot_index as usize] = AMD_SIGNAL_VALUE_PENDING_V1;
        Ok(())
    }
}

fn vm(device_generation: u64, vm_id: u64) -> VmKeyV1 {
    VmKeyV1 {
        device: DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(7),
            generation: DeviceGenerationV1(device_generation),
        },
        id: VmIdV1(vm_id),
    }
}

fn mapping(vm: VmKeyV1, id: u64, generation: u64) -> MemoryMappingKeyV1 {
    MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 {
            vm,
            id: AllocationIdV1(id),
            generation: AllocationGenerationV1(generation),
        },
        id: MappingIdV1(id),
    }
}

fn queue() -> QueueKeyV1 {
    QueueKeyV1 {
        vm: vm(3, 11),
        id: QueueInstanceIdV1(19),
        generation: QueueGenerationV1(5),
    }
}

fn owner() -> CompletionSignalArenaOwnerV1 {
    CompletionSignalArenaOwnerV1 {
        queue: queue(),
        signal_mapping: mapping(queue().vm, 23, 7),
        gpu_base: 0x20_0000,
        next_batch_id: 1,
        slots: allocate_completion_slot_records_v1().unwrap(),
        dependency_ledger: Box::new(CompletionDependencyLedgerV1::new()),
        phase: CompletionOwnerPhaseV1::Ready,
    }
}

#[test]
fn completion_owner_retains_exact_heap_cardinality_with_bounded_inline_state() {
    let owner = owner();
    assert_eq!(owner.slots.len(), COMPLETION_SIGNAL_CAPACITY_V1);
    assert!(core::mem::size_of::<CompletionSignalArenaOwnerV1>() <= 128);
}

#[test]
fn maximum_dispatch_roster_validation_is_one_visit_per_packet() {
    let binding = CompletionDispatchGenerationBindingV1::new(
        queue(),
        mapping(queue().vm, 30, 1),
        mapping(queue().vm, 31, 2),
        4,
    );
    let bindings = vec![binding; COMPLETION_SIGNAL_CAPACITY_V1];
    let (roster, visits) = completion_dispatch_roster_with_visits_v1(&bindings).unwrap();
    assert_eq!(roster.packet_count, COMPLETION_SIGNAL_CAPACITY_V1);
    assert_eq!(visits, COMPLETION_SIGNAL_CAPACITY_V1);
}

fn template(index: u64) -> CompletionPacketTemplateV1 {
    CompletionPacketTemplateV1::new(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        AqlDispatchOrderingV1::WaitForPrior,
        0,
        0,
        ObservedGpuAddressV1::new(0x40_0000).unwrap(),
        ObservedGpuAddressV1::new(0x50_0000 + index * 16).unwrap(),
        16,
        CompletionDispatchGenerationBindingV1::new(
            queue(),
            mapping(queue().vm, 30, 1),
            mapping(queue().vm, 31 + index * 2, 2),
            4,
        ),
    )
}

fn publish<const N: usize>(
    owner: &mut CompletionSignalArenaOwnerV1,
    templates: [CompletionPacketTemplateV1; N],
) -> Gfx942CompletionBatchV1<N> {
    let bound = owner.bind_batch(templates).unwrap();
    let (_, retention) = bound.into_parts();
    owner.validate_bound(&retention).unwrap();
    owner.mark_published(retention, 99).unwrap()
}

fn publish_barrier(owner: &mut CompletionSignalArenaOwnerV1) -> Gfx942BarrierProbeV1 {
    let bound = owner.bind_barrier_probe().unwrap();
    let (_, retention) = bound.into_parts();
    owner.mark_barrier_probe_published(retention, 41).unwrap()
}

#[test]
fn barrier_probe_binds_only_queue_and_signal_then_recycles() {
    let mut owner = owner();
    let bound = owner.bind_barrier_probe().unwrap();
    assert!(matches!(
        owner.bind_batch([template(0)]),
        Err(Gfx942CompletionErrorV1::Poisoned)
    ));
    let (packet, retention) = bound.into_parts();
    let mut capture = BarrierCapture::default();
    packet.publish_with(&mut capture).unwrap();
    let bytes = capture.bytes.unwrap();
    assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 1);
    assert!(bytes[8..48].iter().all(|byte| *byte == 0));
    assert_eq!(
        u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
        0x20_0000
    );
    assert_eq!(capture.header, Some(0x1403));

    let probe = owner.mark_barrier_probe_published(retention, 41).unwrap();
    let mut backend = MockBackend::pending();
    let probe = match owner
        .observe_barrier_probe_once(probe, &mut backend)
        .unwrap()
    {
        Gfx942BarrierProbePollV1::Pending { probe, progress } => {
            assert_eq!(progress.packet_count(), 1);
            assert_eq!(progress.signal(), Gfx942TimeoutSignalObservationV1::Pending);
            probe
        }
        Gfx942BarrierProbePollV1::Ready { .. } => panic!("pending probe reported ready"),
    };
    backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    let completed = match owner
        .observe_barrier_probe_once(probe, &mut backend)
        .unwrap()
    {
        Gfx942BarrierProbePollV1::Ready {
            completed,
            progress,
        } => {
            assert_eq!(
                progress.signal(),
                Gfx942TimeoutSignalObservationV1::Completed
            );
            completed
        }
        Gfx942BarrierProbePollV1::Pending { .. } => panic!("completed probe remained pending"),
    };
    assert_eq!(
        owner.recycle_barrier_probe(completed, &mut backend),
        Ok(Gfx942BarrierProbeRecycleObservationV1)
    );
    assert_eq!(backend.values[0], AMD_SIGNAL_VALUE_PENDING_V1);
    owner.ensure_releasable().unwrap();
    assert!(owner.bind_batch([template(0)]).is_ok());
}

#[test]
fn barrier_probe_rejects_missing_packet_and_zero_identity() {
    let mut missing_packet = owner();
    let bound = missing_packet.bind_barrier_probe().unwrap();
    let (_, retention) = bound.into_parts();
    missing_packet.slots[0].phase = CompletionSlotPhaseV1::Published {
        batch_id: retention.probe_id,
    };
    let mut backend = MockBackend::pending();
    assert!(matches!(
        missing_packet.observe_barrier_probe_once(Gfx942BarrierProbeV1 { retention }, &mut backend),
        Err(Gfx942CompletionErrorV1::StaleBatchGeneration)
    ));

    let mut zero_identity = owner();
    let probe = publish_barrier(&mut zero_identity);
    let mut retention = probe.retention;
    zero_identity.slots[0].phase = CompletionSlotPhaseV1::Published { batch_id: 0 };
    retention.probe_id = 0;
    assert!(matches!(
        zero_identity.observe_barrier_probe_once(Gfx942BarrierProbeV1 { retention }, &mut backend),
        Err(Gfx942CompletionErrorV1::StaleBatchGeneration)
    ));
}

#[test]
fn barrier_probe_generation_exhaustion_terminally_poisons_owner() {
    let mut owner = owner();
    let probe = publish_barrier(&mut owner);
    let mut backend = MockBackend::pending();
    backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    let Gfx942BarrierProbePollV1::Ready { mut completed, .. } = owner
        .observe_barrier_probe_once(probe, &mut backend)
        .unwrap()
    else {
        panic!("completed probe remained pending");
    };
    owner.slots[0].generation = u64::MAX;
    completed.retention.slot.generation = u64::MAX;
    assert_eq!(
        owner.recycle_barrier_probe(completed, &mut backend),
        Err(Gfx942CompletionErrorV1::SignalGenerationExhausted)
    );
    assert!(matches!(
        owner.bind_barrier_probe(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    ));
}

#[test]
fn barrier_probe_cancel_restores_release_and_dispatch_admission() {
    let mut owner = owner();
    let bound = owner.bind_barrier_probe().unwrap();
    let (_, retention) = bound.into_parts();
    assert_eq!(
        owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    );
    owner.cancel_bound_barrier_probe(retention).unwrap();
    owner.ensure_releasable().unwrap();
    assert!(owner.bind_batch([template(0)]).is_ok());
}

#[test]
fn barrier_probe_timeout_retains_custody_until_outer_quarantine() {
    let mut owner = owner();
    let probe = publish_barrier(&mut owner);
    let mut backend = MockBackend::pending();
    let failure = owner
        .wait_barrier_probe_bounded(probe, 2, &mut backend)
        .unwrap_err();
    let Gfx942BarrierProbeWaitFailureV1::Timeout { probe, polls } = failure else {
        panic!("pending probe did not time out");
    };
    assert_eq!(polls, 2);
    assert_eq!(probe.packet_and_signal_slot().unwrap(), (41, 0));
    assert_eq!(backend.observe_calls, 2);
    assert_eq!(
        owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    );
}

#[test]
fn barrier_probe_currentness_fault_and_reset_failures_poison() {
    let mut currentness = owner();
    let probe = publish_barrier(&mut currentness);
    let mut backend = MockBackend::pending();
    backend.fail_currentness_at = Some(1);
    assert!(matches!(
        currentness.observe_barrier_probe_once(probe, &mut backend),
        Err(Gfx942CompletionErrorV1::Currentness)
    ));
    assert!(matches!(
        currentness.bind_barrier_probe(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    ));

    let mut fault = owner();
    let probe = publish_barrier(&mut fault);
    let mut backend = MockBackend::pending();
    backend.values[0] = -7;
    assert!(matches!(
        fault.observe_barrier_probe_once(probe, &mut backend),
        Err(Gfx942CompletionErrorV1::Fault { slot: 0, value: -7 })
    ));
    assert!(matches!(
        fault.bind_barrier_probe(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    ));

    let mut reset = owner();
    let probe = publish_barrier(&mut reset);
    let mut backend = MockBackend::pending();
    backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    let Gfx942BarrierProbePollV1::Ready { completed, .. } = reset
        .observe_barrier_probe_once(probe, &mut backend)
        .unwrap()
    else {
        panic!("completed probe remained pending");
    };
    backend.fail_reset_at = Some(1);
    assert_eq!(
        reset.recycle_barrier_probe(completed, &mut backend),
        Err(Gfx942CompletionErrorV1::Recycle)
    );
    assert!(matches!(
        reset.bind_barrier_probe(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    ));
}

#[test]
fn exact_arena_initialization_matches_every_frozen_signal_image() {
    let mut arena = AlignedArena([0xaa; COMPLETION_SIGNAL_ARENA_BYTES_V1]);
    initialize_pending_completion_signal_arena(&mut arena.0).unwrap();
    for signal in arena.0.chunks_exact(AMD_SIGNAL_BYTES_V1) {
        assert_eq!(signal, encode_pending_completion_signal_bytes_v1());
    }
    assert_eq!(
        initialize_pending_completion_signal_arena(&mut arena.0[..AMD_SIGNAL_BYTES_V1]),
        Err(Gfx942CompletionErrorV1::Initialization)
    );
    let mut misaligned = [0_u8; COMPLETION_SIGNAL_ARENA_BYTES_V1 + 1];
    assert_eq!(
        initialize_pending_completion_signal_arena(&mut misaligned[1..]),
        Err(Gfx942CompletionErrorV1::Initialization)
    );
}

#[test]
fn completion_manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_AQL_COMPLETION_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(rendered, GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1);
}

#[test]
fn boundary_batches_bind_distinct_wrap_free_signal_slots() {
    for count in [1_usize, 2, 4, 16, 256, 8192] {
        let mut owner = owner();
        let templates: Vec<_> = (0..count).map(|index| template(index as u64)).collect();
        match count {
            1 => assert!(owner.bind_batch([templates[0]]).is_ok()),
            2 => assert!(owner.bind_batch([templates[0], templates[1]]).is_ok()),
            4 => assert!(
                owner
                    .bind_batch([templates[0], templates[1], templates[2], templates[3]])
                    .is_ok()
            ),
            16 => {
                let values: [CompletionPacketTemplateV1; 16] = templates.try_into().unwrap();
                assert!(owner.bind_batch(values).is_ok());
            }
            256 => {
                let values: [CompletionPacketTemplateV1; 256] = templates.try_into().unwrap();
                assert!(owner.bind_batch(values).is_ok());
            }
            8192 => {
                let values: CompletionPacketTemplatesV1<8192> =
                    CompletionPacketTemplatesV1::try_from_vec(templates).unwrap();
                assert!(owner.bind_fixed_batch(values).is_ok());
            }
            _ => unreachable!(),
        }
    }
    let mut zero = owner();
    assert!(matches!(
        zero.bind_batch([]),
        Err(Gfx942CompletionErrorV1::ZeroPacketCount)
    ));
    let mut over = owner();
    let over_values: CompletionPacketTemplatesV1<8193> = CompletionPacketTemplatesV1::try_from_vec(
        (0..8193).map(|index| template(index as u64)).collect(),
    )
    .unwrap();
    assert!(matches!(
        over.bind_fixed_batch(over_values),
        Err(Gfx942CompletionErrorV1::PacketCountExceedsMaximum { .. })
    ));

    let mut exact = owner();
    let bound = exact
        .bind_batch([template(0), template(1), template(2), template(3)])
        .unwrap();
    let (packets, _) = bound.into_parts();
    let mut capture = PacketCapture::default();
    packets.publish_with(&mut capture).unwrap();
    assert_eq!(
        capture.signals,
        vec![0x20_0000, 0x20_0040, 0x20_0080, 0x20_00c0]
    );
    assert_eq!(capture.headers, vec![0x1502; 4]);
}

#[test]
fn completion_binding_preserves_mixed_packet_ordering() {
    let mut owner = owner();
    let mut independent = template(0);
    independent.ordering = AqlDispatchOrderingV1::Independent;
    let bound = owner.bind_batch([independent, template(1)]).unwrap();
    let (packets, _) = bound.into_parts();
    let mut capture = PacketCapture::default();
    packets.publish_with(&mut capture).unwrap();
    assert_eq!(capture.headers, vec![0x1402, 0x1502]);
}

#[test]
fn binding_rejects_wrong_queue_vm_and_packet_without_mutation() {
    let mut owner = owner();
    let mut wrong_queue = template(0);
    wrong_queue.generations.queue.generation = QueueGenerationV1(6);
    assert!(matches!(
        owner.bind_batch([wrong_queue]),
        Err(Gfx942CompletionErrorV1::WrongQueueGeneration)
    ));
    let mut wrong_vm = template(0);
    wrong_vm.generations.code.allocation.vm = vm(3, 12);
    assert!(matches!(
        owner.bind_batch([wrong_vm]),
        Err(Gfx942CompletionErrorV1::WrongVmGeneration)
    ));
    let mut invalid_packet = template(0);
    invalid_packet.kernarg_alignment = 3;
    assert!(matches!(
        owner.bind_batch([invalid_packet]),
        Err(Gfx942CompletionErrorV1::PacketBinding(_))
    ));
    let mut invalid_second = template(1);
    invalid_second.kernarg_alignment = 3;
    assert!(matches!(
        owner.bind_batch([template(0), invalid_second]),
        Err(Gfx942CompletionErrorV1::PacketBinding(_))
    ));
    assert!(owner.bind_batch([template(0), template(1)]).is_ok());
}

#[test]
fn pending_ready_and_recycle_are_exact_for_unique_signals() {
    let mut owner = owner();
    let batch = publish(
        &mut owner,
        [template(0), template(1), template(2), template(3)],
    );
    let mut backend = MockBackend::pending();
    let batch = match owner.observe_once(batch, &mut backend).unwrap() {
        Gfx942CompletionPollV1::Pending(batch) => batch,
        Gfx942CompletionPollV1::Ready(_) => panic!("pending batch reported ready"),
    };
    backend.values[..4].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
    let completed = match owner.observe_once(batch, &mut backend).unwrap() {
        Gfx942CompletionPollV1::Ready(completed) => completed,
        Gfx942CompletionPollV1::Pending(_) => panic!("completed batch reported pending"),
    };
    let observation = owner.recycle(completed, &mut backend).unwrap();
    assert_eq!(observation.packet_count(), 4);
    assert_eq!(backend.values[..4], [AMD_SIGNAL_VALUE_PENDING_V1; 4]);
    assert!(owner.ensure_releasable().is_ok());
    assert!(owner.bind_batch([template(4); 4]).is_ok());
}

#[test]
fn progress_uses_one_currentness_envelope_for_the_exact_batch_scan() {
    let mut owner = owner();
    let batch = publish(
        &mut owner,
        [template(0), template(1), template(2), template(3)],
    );
    let mut backend = MockBackend::pending();
    backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    backend.values[2] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    let batch = match owner
        .observe_once_with_progress(batch, &mut backend)
        .unwrap()
    {
        Gfx942CompletionPollWithProgressV1::Pending { batch, progress } => {
            assert_eq!(progress.packet_count(), 4);
            assert_eq!(progress.completed_count(), 2);
            assert_eq!(progress.pending_count(), 2);
            assert_eq!(progress.first_pending_batch_index(), Some(1));
            batch
        }
        Gfx942CompletionPollWithProgressV1::Ready { .. } => {
            panic!("partially completed batch reported ready")
        }
    };
    assert_eq!(backend.currentness_calls, 2);
    assert_eq!(backend.observe_calls, 4);

    backend.values[..4].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
    match owner
        .observe_once_with_progress(batch, &mut backend)
        .unwrap()
    {
        Gfx942CompletionPollWithProgressV1::Ready { progress, .. } => {
            assert_eq!(progress.packet_count(), 4);
            assert_eq!(progress.completed_count(), 4);
            assert_eq!(progress.pending_count(), 0);
            assert_eq!(progress.first_pending_batch_index(), None);
        }
        Gfx942CompletionPollWithProgressV1::Pending { .. } => {
            panic!("completed batch reported pending")
        }
    }
    assert_eq!(backend.currentness_calls, 4);
    assert_eq!(backend.observe_calls, 8);
}

#[test]
fn ready_currentness_handoff_removes_exactly_one_recycle_opening_check() {
    let mut fused_owner = owner();
    let fused_batch = publish(&mut fused_owner, [template(0)]);
    let mut fused_backend = MockBackend::pending();
    fused_backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    let handoff = match fused_owner
        .observe_one_with_progress_current_handoff_retaining(fused_batch, &mut fused_backend)
        .unwrap()
    {
        CompletionPollWithCurrentnessHandoffV1::Ready { handoff, progress } => {
            assert_eq!(progress.completed_count(), 1);
            handoff
        }
        CompletionPollWithCurrentnessHandoffV1::Pending { .. } => {
            panic!("completed batch remained pending")
        }
    };
    fused_backend.trace.push("dispatch-completed");
    fused_backend.trace.push("allocation-completed");
    let recycled = fused_owner
        .recycle_current_handoff_retaining(handoff, &mut fused_backend)
        .unwrap();
    fused_backend.trace.push("dispatch-recycled");
    fused_backend.trace.push("attachment-recycled");
    assert_eq!(recycled.packet_count(), 1);
    assert_eq!(
        fused_backend.trace,
        [
            "currentness",
            "acquire",
            "currentness",
            "dispatch-completed",
            "allocation-completed",
            "reset",
            "currentness",
            "dispatch-recycled",
            "attachment-recycled",
        ]
    );
    assert_eq!(fused_backend.currentness_calls, 3);
    assert_eq!(fused_backend.observe_calls, 1);
    assert_eq!(fused_backend.reset_calls, 1);
    fused_owner.ensure_releasable().unwrap();

    let mut split_owner = owner();
    let split_batch = publish(&mut split_owner, [template(0)]);
    let mut split_backend = MockBackend::pending();
    split_backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    let completed = match split_owner
        .observe_once_with_progress_retaining(split_batch, &mut split_backend)
        .unwrap()
    {
        Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => completed,
        Gfx942CompletionPollWithProgressV1::Pending { .. } => {
            panic!("completed split batch remained pending")
        }
    };
    split_owner
        .recycle_retaining(completed, &mut split_backend)
        .unwrap();
    assert_eq!(split_backend.currentness_calls, 4);
    assert_eq!(split_backend.observe_calls, 1);
    assert_eq!(split_backend.reset_calls, 1);
}

#[test]
fn pending_currentness_handoff_preserves_the_two_check_no_reset_path() {
    let mut owner = owner();
    let batch = publish(&mut owner, [template(0)]);
    let mut backend = MockBackend::pending();
    let pending = owner
        .observe_one_with_progress_current_handoff_retaining(batch, &mut backend)
        .unwrap();
    assert!(matches!(
        pending,
        CompletionPollWithCurrentnessHandoffV1::Pending { .. }
    ));
    assert_eq!(backend.trace, ["currentness", "acquire", "currentness"]);
    assert_eq!(backend.currentness_calls, 2);
    assert_eq!(backend.observe_calls, 1);
    assert_eq!(backend.reset_calls, 0);
}

#[test]
fn pending_with_false_closing_currentness_is_terminal_not_retryable() {
    let mut owner = owner();
    let batch = publish(&mut owner, [template(0)]);
    let mut backend = MockBackend::pending();
    backend.fail_currentness_at = Some(2);

    let failure =
        match owner.observe_one_with_progress_current_handoff_retaining(batch, &mut backend) {
            Err(failure) => failure,
            Ok(_) => panic!("closing currentness must precede pending classification"),
        };
    assert!(matches!(failure.0, Gfx942CompletionErrorV1::Currentness));
    assert_eq!(backend.trace, ["currentness", "acquire", "currentness"]);
    assert_eq!(backend.currentness_calls, 2);
    assert_eq!(backend.observe_calls, 1);
    assert_eq!(backend.reset_calls, 0);
    assert!(matches!(
        owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    ));
}

#[test]
fn currentness_handoff_failures_never_report_false_recycle() {
    for fail_currentness_at in [1_usize, 2] {
        let mut owner = owner();
        let batch = publish(&mut owner, [template(0)]);
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        backend.fail_currentness_at = Some(fail_currentness_at);
        assert!(matches!(
            owner.observe_one_with_progress_current_handoff_retaining(batch, &mut backend),
            Err((Gfx942CompletionErrorV1::Currentness, _))
        ));
        assert_eq!(backend.reset_calls, 0);
        assert!(matches!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        ));
    }

    for (fail_reset_at, fail_currentness_at, expected, expected_trace) in [
        (
            Some(1_usize),
            None,
            Gfx942CompletionErrorV1::Recycle,
            &["currentness", "acquire", "currentness", "reset"][..],
        ),
        (
            None,
            Some(3_usize),
            Gfx942CompletionErrorV1::Currentness,
            &[
                "currentness",
                "acquire",
                "currentness",
                "reset",
                "currentness",
            ][..],
        ),
    ] {
        let mut owner = owner();
        let batch = publish(&mut owner, [template(0)]);
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let handoff = match owner
            .observe_one_with_progress_current_handoff_retaining(batch, &mut backend)
            .unwrap()
        {
            CompletionPollWithCurrentnessHandoffV1::Ready { handoff, .. } => handoff,
            CompletionPollWithCurrentnessHandoffV1::Pending { .. } => unreachable!(),
        };
        backend.fail_reset_at = fail_reset_at;
        backend.fail_currentness_at = fail_currentness_at;
        let (error, handoff) = owner
            .recycle_current_handoff_retaining(handoff, &mut backend)
            .unwrap_err();
        assert_eq!(error, expected);
        assert_eq!(handoff.into_completed().retention.batch_id, 1);
        assert_eq!(backend.trace, expected_trace);
        assert_eq!(backend.reset_calls, 1);
        assert!(matches!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        ));
    }
}

#[test]
fn substituted_handoff_identity_is_rejected_before_any_reset() {
    for substitution in 0..4 {
        let mut owner = owner();
        let mut batch = publish(&mut owner, [template(0)]);
        match substitution {
            0 => batch.retention.queue.generation = QueueGenerationV1(6),
            1 => batch.retention.signal_mapping.id = MappingIdV1(99),
            2 => batch.retention.batch_id = 99,
            3 => batch.retention.slots[0].generation += 1,
            _ => unreachable!(),
        }
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        assert!(matches!(
            owner.observe_one_with_progress_current_handoff_retaining(batch, &mut backend),
            Err((Gfx942CompletionErrorV1::StaleBatchGeneration, _))
        ));
        assert_eq!(backend.currentness_calls, 0);
        assert_eq!(backend.observe_calls, 0);
        assert_eq!(backend.reset_calls, 0);
    }
}

#[test]
fn completion_currentness_handoff_is_private_and_move_only() {
    let production = include_str!("../queue_completion.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let handoff = production
        .split("pub(super) struct CompletionCurrentnessHandoffV1")
        .nth(1)
        .unwrap()
        .split("pub(super) enum CompletionPollWithCurrentnessHandoffV1")
        .next()
        .unwrap();
    assert!(!handoff.contains("derive(Clone"));
    assert!(!handoff.contains("derive(Copy"));
    assert!(!production.contains("pub struct CompletionCurrentnessHandoffV1"));

    let specialized = production
        .split("fn observe_one_with_progress_current_handoff_retaining")
        .nth(1)
        .unwrap()
        .split("pub(super) fn recycle_current_handoff_retaining")
        .next()
        .unwrap();
    assert!(specialized.contains("backend.observe_one_acquire("));
    assert!(specialized.contains("self.validate_observation_preflight(&batch)"));
    assert!(
        specialized.contains(
            "self.classify_completion_observations(batch, core::iter::once(observation))"
        )
    );
    assert!(!specialized.contains("Vec<"));
    assert!(!specialized.contains("Vec::"));
    assert!(!specialized.contains("collect()"));
}

#[test]
fn fault_timeout_and_ambiguous_observation_poison() {
    let mut fault_owner = owner();
    let fault_batch = publish(&mut fault_owner, [template(0), template(1)]);
    let mut fault_backend = MockBackend::pending();
    fault_backend.values[1] = -7;
    assert!(matches!(
        fault_owner.observe_once(fault_batch, &mut fault_backend),
        Err(Gfx942CompletionErrorV1::Fault { slot: 1, value: -7 })
    ));
    assert_eq!(
        fault_owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    );

    let mut malformed_owner = owner();
    let malformed_batch = publish(&mut malformed_owner, [template(0)]);
    let mut malformed_backend = MockBackend::pending();
    malformed_backend.values[0] = -7;
    malformed_backend.extra_batch_observation = Some(AqlCompletionObservationV1::Completed);
    assert!(matches!(
        malformed_owner.observe_once(malformed_batch, &mut malformed_backend),
        Err(Gfx942CompletionErrorV1::Observation)
    ));
    assert_eq!(
        malformed_owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    );

    let mut timeout_owner = owner();
    let timeout_batch = publish(&mut timeout_owner, [template(0)]);
    let mut timeout_backend = MockBackend::pending();
    let timeout = timeout_owner
        .wait_bounded(timeout_batch, 3, &mut timeout_backend)
        .unwrap_err();
    let Gfx942CompletionWaitFailureV1::Timeout { batch, polls } = timeout else {
        panic!("pending exhaustion did not preserve timeout custody")
    };
    assert_eq!(polls, 3);
    assert_eq!(batch.first_packet_and_signal_slot().unwrap(), (99, 0));
    assert_eq!(timeout_backend.observe_calls, 3);
    timeout_owner.poison_owner();

    let mut observe_owner = owner();
    let observe_batch = publish(&mut observe_owner, [template(0)]);
    let mut observe_backend = MockBackend::pending();
    observe_backend.fail_observe_at = Some(1);
    assert!(matches!(
        observe_owner.observe_once(observe_batch, &mut observe_backend),
        Err(Gfx942CompletionErrorV1::Observation)
    ));
}

#[test]
fn zero_poll_timeout_preserves_locator_without_scanning_signals() {
    let mut owner = owner();
    let first = publish(&mut owner, [template(0), template(1)]);
    let mut backend = MockBackend::pending();
    backend.values[..2].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
    let completed = owner.wait_bounded(first, 1, &mut backend).unwrap();
    owner.recycle(completed, &mut backend).unwrap();

    let second = publish(&mut owner, [template(2), template(3), template(4)]);
    let scans_before = backend.observe_calls;
    let timeout = owner.wait_bounded(second, 0, &mut backend).unwrap_err();
    let Gfx942CompletionWaitFailureV1::Timeout { batch, polls } = timeout else {
        panic!("zero poll did not retain timeout custody")
    };
    assert_eq!(polls, 0);
    assert_eq!(batch.first_packet_and_signal_slot().unwrap(), (97, 0));
    assert_eq!(backend.observe_calls, scans_before);
    owner.poison_owner();
}

#[test]
fn expired_deadline_preserves_custody_without_scanning_signals() {
    let mut owner = owner();
    let batch = publish(&mut owner, [template(0)]);
    let mut backend = MockBackend::pending();
    let timeout = owner
        .wait_until(batch, Instant::now(), &mut backend)
        .unwrap_err();
    let Gfx942CompletionWaitFailureV1::Timeout { batch, polls } = timeout else {
        panic!("expired deadline did not retain timeout custody")
    };
    assert_eq!(polls, 0);
    assert_eq!(backend.observe_calls, 0);
    assert_eq!(batch.first_packet_and_signal_slot().unwrap(), (99, 0));
    owner.poison_owner();
}

#[test]
fn timeout_observation_is_addressless_and_preserves_exact_values() {
    let observation = Gfx942TimeoutExecutionObservationV1::new(
        545,
        545,
        0,
        0x1502,
        3,
        fe2o3_aql::AMD_SIGNAL_KIND_USER_V1,
        Gfx942TimeoutSignalObservationV1::Pending,
        0,
    );
    assert_eq!(observation.packet_count(), 545);
    assert_eq!(observation.write_counter(), 545);
    assert_eq!(observation.read_counter(), 0);
    assert_eq!(observation.first_packet_header(), 0x1502);
    assert_eq!(observation.first_packet_setup(), 3);
    assert_eq!(
        observation.first_signal_kind(),
        fe2o3_aql::AMD_SIGNAL_KIND_USER_V1
    );
    assert_eq!(
        observation.first_signal(),
        Gfx942TimeoutSignalObservationV1::Pending
    );
    assert_eq!(observation.first_signal().value(), 1);
    assert_eq!(observation.queue_exception_reason_mask(), 0);
    assert!(observation.currentness_confirmed());
    let rendered = format!("{observation:?}");
    for forbidden in ["address", "handle", "queue_id", "packet_id", "slot_index"] {
        assert!(!rendered.contains(forbidden));
    }
}

#[test]
fn every_currentness_and_recycle_boundary_fails_closed() {
    for fail_at in [1_usize, 2] {
        let mut owner = owner();
        let batch = publish(&mut owner, [template(0)]);
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        backend.fail_currentness_at = Some(fail_at);
        assert!(matches!(
            owner.observe_once(batch, &mut backend),
            Err(Gfx942CompletionErrorV1::Currentness)
        ));
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }

    for fail_reset_at in 1..=4 {
        let mut owner = owner();
        let batch = publish(
            &mut owner,
            [template(0), template(1), template(2), template(3)],
        );
        let mut backend = MockBackend::pending();
        backend.values[..4].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
        let completed = owner.wait_bounded(batch, 1, &mut backend).unwrap();
        backend.fail_reset_at = Some(fail_reset_at);
        assert_eq!(
            owner.recycle(completed, &mut backend),
            Err(Gfx942CompletionErrorV1::Recycle)
        );
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }

    for fail_currentness_at in [3_usize, 4] {
        let mut owner = owner();
        let batch = publish(&mut owner, [template(0)]);
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let completed = owner.wait_bounded(batch, 1, &mut backend).unwrap();
        backend.fail_currentness_at = Some(fail_currentness_at);
        assert_eq!(
            owner.recycle(completed, &mut backend),
            Err(Gfx942CompletionErrorV1::Currentness)
        );
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }
}

#[test]
fn observation_failure_at_every_batch_slot_is_terminal() {
    for fail_observe_at in 1..=4 {
        let mut owner = owner();
        let batch = publish(
            &mut owner,
            [template(0), template(1), template(2), template(3)],
        );
        let mut backend = MockBackend::pending();
        backend.fail_observe_at = Some(fail_observe_at);
        assert!(matches!(
            owner.observe_once(batch, &mut backend),
            Err(Gfx942CompletionErrorV1::Observation)
        ));
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }
}

#[test]
fn stale_generation_and_live_batches_prevent_release() {
    let mut owner = owner();
    let bound = owner.bind_batch([template(0)]).unwrap();
    let (_, mut retention) = bound.into_parts();
    assert_eq!(
        owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::BatchStillRetained)
    );
    retention.slots[0].generation += 1;
    assert_eq!(
        owner.validate_bound(&retention),
        Err(Gfx942CompletionErrorV1::StaleBatchGeneration)
    );
}

#[test]
fn capacity_and_identity_exhaustion_are_preflighted() {
    let mut full = owner();
    let all: CompletionPacketTemplatesV1<8192> = CompletionPacketTemplatesV1::try_from_vec(
        (0..8192).map(|index| template(index as u64)).collect(),
    )
    .unwrap();
    assert!(full.bind_fixed_batch(all).is_ok());
    assert!(matches!(
        full.bind_batch([template(300)]),
        Err(Gfx942CompletionErrorV1::InsufficientSignals)
    ));

    let mut batch_ids = owner();
    batch_ids.next_batch_id = u64::MAX;
    assert!(matches!(
        batch_ids.bind_batch([template(0)]),
        Err(Gfx942CompletionErrorV1::BatchIdentityExhausted)
    ));
    assert!(batch_ids.ensure_releasable().is_ok());

    let mut generations = owner();
    generations.slots[0].generation = u64::MAX;
    let batch = publish(&mut generations, [template(0)]);
    let mut backend = MockBackend::pending();
    backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
    let completed = generations.wait_bounded(batch, 1, &mut backend).unwrap();
    assert_eq!(
        generations.recycle(completed, &mut backend),
        Err(Gfx942CompletionErrorV1::SignalGenerationExhausted)
    );
    assert_eq!(backend.reset_calls, 0);
    assert_eq!(
        generations.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    );
}

#[test]
fn oversized_poll_bound_is_terminal_before_observation() {
    let mut owner = owner();
    let batch = publish(&mut owner, [template(0)]);
    let mut backend = MockBackend::pending();
    assert!(matches!(
        owner.wait_bounded(batch, MAX_COMPLETION_POLL_ATTEMPTS_V1 + 1, &mut backend),
        Err(Gfx942CompletionWaitFailureV1::Terminal(
            Gfx942CompletionErrorV1::InvalidPollBound {
                requested,
                maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
            }
        )) if requested == MAX_COMPLETION_POLL_ATTEMPTS_V1 + 1
    ));
    assert_eq!(backend.observe_calls, 0);
    assert_eq!(
        owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    );
}
