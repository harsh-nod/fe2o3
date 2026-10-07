include!("../event_release_tests.rs");

use super::*;
use fe2o3_aql::{
    AMD_SIGNAL_ALIGNMENT_V1, AqlCompletionObservationV1, AqlDispatchGeometryV1,
    AqlDispatchOrderingV1, ObservedGpuAddressV1,
};
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
    MemoryAllocationKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1, VmIdV1,
    VmKeyV1,
};
use sha2::{Digest, Sha256};

use crate::queue::completion::{
    CompletionDispatchGenerationBindingV1, CompletionPacketTemplateV1, Gfx942CompletedBatchV1,
    Gfx942CompletionPollV1, NativeCompletionSignalBackendV1,
};

const SESSION: u64 = 17;
const SOURCE_EPOCH: u64 = 23;
const DEPENDENT_EPOCH: u64 = 29;

struct CompletedBackend {
    reset_calls: usize,
}

impl NativeCompletionSignalBackendV1 for CompletedBackend {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        Ok(())
    }

    fn observe_one_acquire_in_current_scope(
        &mut self,
        _slot_index: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        Ok(AqlCompletionObservationV1::Completed)
    }

    fn observe_batch_acquire_in_current_scope(
        &mut self,
        slot_indices: &[u32],
    ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        Ok(vec![
            AqlCompletionObservationV1::Completed;
            slot_indices.len()
        ])
    }

    fn reset_pending_release(&mut self, _slot_index: u32) -> Result<(), Gfx942CompletionErrorV1> {
        self.reset_calls += 1;
        Ok(())
    }
}

fn queue() -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(7),
                generation: DeviceGenerationV1(3),
            },
            id: VmIdV1(11),
        },
        id: QueueInstanceIdV1(19),
        generation: QueueGenerationV1(5),
    }
}

fn mapping(id: u64) -> MemoryMappingKeyV1 {
    MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 {
            vm: queue().vm,
            id: AllocationIdV1(id),
            generation: AllocationGenerationV1(1),
        },
        id: MappingIdV1(id),
    }
}

fn template() -> CompletionPacketTemplateV1 {
    CompletionPacketTemplateV1::new(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        AqlDispatchOrderingV1::WaitForPrior,
        0,
        0,
        ObservedGpuAddressV1::new(0x40_0000).unwrap(),
        ObservedGpuAddressV1::new(0x50_0000).unwrap(),
        16,
        CompletionDispatchGenerationBindingV1::new(queue(), mapping(30), mapping(31), 41),
    )
}

fn owner() -> CompletionSignalArenaOwnerV1 {
    CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue())
}

fn unbound(
    owner: &mut CompletionSignalArenaOwnerV1,
) -> (
    CompletionBatchRetentionV1<1>,
    Gfx942ComputeEventOccurrenceV1,
) {
    let bound = owner.bind_batch([template()]).unwrap();
    let (_, retention) = bound.into_parts();
    let event = owner
        .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0)
        .unwrap();
    (retention, event)
}

fn published(
    owner: &mut CompletionSignalArenaOwnerV1,
) -> (Gfx942CompletionBatchV1<1>, Gfx942ComputeEventOccurrenceV1) {
    let (retention, event) = unbound(owner);
    let batch = owner.mark_published(retention, 101).unwrap();
    let event = owner
        .bind_compute_event_after_publication(event, &batch, 0)
        .unwrap();
    (batch, event)
}

fn complete<const N: usize>(
    owner: &mut CompletionSignalArenaOwnerV1,
    batch: Gfx942CompletionBatchV1<N>,
    backend: &mut CompletedBackend,
) -> Gfx942CompletedBatchV1<N> {
    match owner.observe_once(batch, backend).unwrap() {
        Gfx942CompletionPollV1::Ready(completed) => completed,
        Gfx942CompletionPollV1::Pending(_) => panic!("completed backend reported pending"),
    }
}

fn duplicate_event(event: &Gfx942ComputeEventOccurrenceV1) -> Gfx942ComputeEventOccurrenceV1 {
    Gfx942ComputeEventOccurrenceV1 {
        event_id: event.event_id,
        exact: event.exact,
    }
}

fn duplicate_reader(
    lease: &Gfx942ComputeDependencyReaderLeaseV1,
) -> Gfx942ComputeDependencyReaderLeaseV1 {
    Gfx942ComputeDependencyReaderLeaseV1 {
        lease_id: lease.lease_id,
        event_id: lease.event_id,
        dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
        source: lease.source,
    }
}

#[test]
fn unbound_event_pins_cancel_and_binds_exactly_once() {
    let mut owner = owner();
    let (retention, event) = unbound(&mut owner);
    assert_eq!(
        event.binding_state(),
        Gfx942ComputeEventBindingStateV1::Unbound
    );
    let retained = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
        Err((Gfx942CompletionErrorV1::EventNotPublished, event)) => event,
        other => panic!("unexpected prepublication result: {other:?}"),
    };
    let retention = match owner.cancel_bound_retaining(retention) {
        Err((
            Gfx942CompletionErrorV1::SignalPinned {
                event_pins: 1,
                native_reader_pins: 0,
                ..
            },
            retention,
        )) => retention,
        other => panic!("unexpected pinned cancellation result: {other:?}"),
    };
    owner.release_compute_event(retained).unwrap();
    owner.cancel_bound_retaining(retention).unwrap();

    let (retention, event) = unbound(&mut owner);
    let batch = owner.mark_published(retention, 101).unwrap();
    let event = owner
        .bind_compute_event_after_publication(event, &batch, 0)
        .unwrap();
    assert_eq!(
        event.binding_state(),
        Gfx942ComputeEventBindingStateV1::Bound
    );
    let event = match owner.bind_compute_event_after_publication(event, &batch, 0) {
        Err((Gfx942CompletionErrorV1::EventAlreadyBound, event)) => event,
        other => panic!("unexpected second bind result: {other:?}"),
    };
    owner.release_compute_event(event).unwrap();
}

#[test]
fn event_release_does_not_release_reader_and_both_block_recycle() {
    let mut owner = owner();
    let (batch, event) = published(&mut owner);
    let (event, lease) = owner
        .retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
        .unwrap();
    let mut backend = CompletedBackend { reset_calls: 0 };
    let handoff = match owner
        .observe_once_with_progress_current_handoff_retaining(batch, &mut backend)
        .unwrap()
    {
        super::super::CompletionPollWithCurrentnessHandoffV1::Ready { handoff, .. } => handoff,
        super::super::CompletionPollWithCurrentnessHandoffV1::Pending { .. } => {
            panic!("completed backend reported pending")
        }
    };
    let handoff = match owner.recycle_current_handoff_retaining(handoff, &mut backend) {
        Err((
            Gfx942CompletionErrorV1::SignalPinned {
                event_pins: 1,
                native_reader_pins: 1,
                ..
            },
            handoff,
        )) => handoff,
        other => panic!("unexpected pinned recycle result: {other:?}"),
    };
    assert_eq!(backend.reset_calls, 0);

    owner.release_compute_event(event).unwrap();
    let handoff = match owner.recycle_current_handoff_retaining(handoff, &mut backend) {
        Err((
            Gfx942CompletionErrorV1::SignalPinned {
                event_pins: 0,
                native_reader_pins: 1,
                ..
            },
            handoff,
        )) => handoff,
        other => panic!("unexpected reader-pinned recycle result: {other:?}"),
    };
    assert_eq!(backend.reset_calls, 0);
    owner.release_compute_dependency_reader(lease).unwrap();
    owner
        .recycle_current_handoff_retaining(handoff, &mut backend)
        .unwrap();
    assert_eq!(backend.reset_calls, 1);
}

#[test]
fn dependency_admission_rejects_cross_session_cycles_and_duplicates_without_mutation() {
    let mut owner = owner();
    let (_batch, event) = published(&mut owner);
    let event = match owner.retain_compute_dependency_reader(event, SESSION + 1, DEPENDENT_EPOCH) {
        Err((Gfx942CompletionErrorV1::CrossSessionEvent, event)) => event,
        other => panic!("unexpected cross-session result: {other:?}"),
    };
    let event = match owner.retain_compute_dependency_reader(event, SESSION, SOURCE_EPOCH) {
        Err((Gfx942CompletionErrorV1::SelfDependency, event)) => event,
        other => panic!("unexpected self-dependency result: {other:?}"),
    };
    let event = match owner.retain_compute_dependency_reader(event, SESSION, SOURCE_EPOCH - 1) {
        Err((Gfx942CompletionErrorV1::DependencyCycle, event)) => event,
        other => panic!("unexpected cycle result: {other:?}"),
    };
    let (event, lease) = owner
        .retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
        .unwrap();
    let event = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
        Err((Gfx942CompletionErrorV1::DuplicateDependency, event)) => event,
        other => panic!("unexpected duplicate result: {other:?}"),
    };
    assert_eq!(
        owner.slots[event.exact.slot.index as usize].native_reader_pins,
        1
    );
    owner.release_compute_event(event).unwrap();
    owner.release_compute_dependency_reader(lease).unwrap();
}

#[test]
fn stale_generation_and_checked_overflow_return_linear_owners() {
    let mut owner = owner();
    let (_batch, event) = published(&mut owner);
    let mut substituted = duplicate_event(&event);
    substituted.exact.slot.generation += 1;
    let substituted =
        match owner.retain_compute_dependency_reader(substituted, SESSION, DEPENDENT_EPOCH) {
            Err((Gfx942CompletionErrorV1::StaleEventOccurrence, event)) => event,
            other => panic!("unexpected generation-substitution result: {other:?}"),
        };
    assert_eq!(
        substituted.exact.slot.generation,
        event.exact.slot.generation + 1
    );
    assert_eq!(
        owner.slots[event.exact.slot.index as usize].native_reader_pins,
        0
    );

    owner.slots[event.exact.slot.index as usize].native_reader_pins = u32::MAX;
    let event = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
        Err((Gfx942CompletionErrorV1::SignalPinCountExhausted, event)) => event,
        other => panic!("unexpected reader-pin overflow result: {other:?}"),
    };
    assert!(owner.dependency_ledger.readers.is_empty());
    owner.slots[event.exact.slot.index as usize].native_reader_pins = 0;
    owner.release_compute_event(event).unwrap();

    let bound = owner.bind_batch([template()]).unwrap();
    let (_, retention) = bound.into_parts();
    let slot = retention.slots[0].index as usize;
    owner.slots[slot].event_pins = u32::MAX;
    assert!(matches!(
        owner.record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0),
        Err(Gfx942CompletionErrorV1::SignalPinCountExhausted)
    ));
    assert!(owner.dependency_ledger.events.is_empty());
}

#[test]
fn checked_identity_exhaustion_precedes_pin_or_ledger_mutation() {
    let mut owner = owner();
    let bound = owner.bind_batch([template()]).unwrap();
    let (_, retention) = bound.into_parts();
    owner.dependency_ledger.next_event_id = u64::MAX;
    assert!(matches!(
        owner.record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0),
        Err(Gfx942CompletionErrorV1::EventIdentityExhausted)
    ));
    assert_eq!(owner.slots[retention.slots[0].index as usize].event_pins, 0);
    assert!(owner.dependency_ledger.events.is_empty());
    owner.cancel_bound_retaining(retention).unwrap();

    owner.dependency_ledger.next_event_id = 1;
    let (_batch, event) = published(&mut owner);
    owner.dependency_ledger.next_reader_lease_id = u64::MAX;
    let event = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
        Err((Gfx942CompletionErrorV1::DependencyReaderIdentityExhausted, event)) => event,
        other => panic!("unexpected reader-identity exhaustion result: {other:?}"),
    };
    assert_eq!(
        owner.slots[event.exact.slot.index as usize].native_reader_pins,
        0
    );
    assert!(owner.dependency_ledger.readers.is_empty());
    owner.release_compute_event(event).unwrap();
}

#[test]
fn bounded_ledgers_reject_capacity_before_pin_mutation() {
    let mut event_owner = owner();
    let bound = event_owner.bind_batch([template()]).unwrap();
    let (_, retention) = bound.into_parts();
    let exact = exact_occurrence(SESSION, SOURCE_EPOCH, &retention, 0, None).unwrap();
    for event_id in 1..=GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1 as u64 {
        assert!(
            event_owner
                .dependency_ledger
                .events
                .insert(event_id, exact)
                .is_none()
        );
    }
    assert!(matches!(
        event_owner.record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0),
        Err(Gfx942CompletionErrorV1::EventCapacityExhausted)
    ));
    assert_eq!(event_owner.slots[exact.slot.index as usize].event_pins, 0);

    let mut reader_owner = owner();
    let (_batch, event) = published(&mut reader_owner);
    for dependent_acceptance_epoch in 1..=GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1 as u64 {
        let key = DependencyReaderUseKeyV1 {
            event_id: event.event_id + dependent_acceptance_epoch,
            dependent_acceptance_epoch,
        };
        assert!(
            reader_owner
                .dependency_ledger
                .readers
                .insert(
                    key,
                    ActiveDependencyReaderV1 {
                        lease_id: dependent_acceptance_epoch,
                        source: event.exact,
                    },
                )
                .is_none()
        );
    }
    let event = match reader_owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
    {
        Err((Gfx942CompletionErrorV1::DependencyReaderCapacityExhausted, event)) => event,
        other => panic!("unexpected reader-capacity result: {other:?}"),
    };
    assert_eq!(
        reader_owner.slots[event.exact.slot.index as usize].native_reader_pins,
        0
    );
    reader_owner.release_compute_event(event).unwrap();
}

#[test]
fn dependency_reader_release_is_exactly_once_and_addressless() {
    let mut owner = owner();
    let (batch, event) = published(&mut owner);
    let (event, lease) = owner
        .retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
        .unwrap();
    let duplicate = duplicate_reader(&lease);
    let mut backend = CompletedBackend { reset_calls: 0 };
    let completed = complete(&mut owner, batch, &mut backend);
    owner.release_compute_event(event).unwrap();
    owner.release_compute_dependency_reader(lease).unwrap();
    let duplicate = match owner.release_compute_dependency_reader(duplicate) {
        Err((Gfx942CompletionErrorV1::StaleDependencyReader, lease)) => lease,
        other => panic!("unexpected duplicate release result: {other:?}"),
    };
    assert_eq!(
        owner.slots[duplicate.source.slot.index as usize].native_reader_pins,
        0
    );
    owner.recycle_retaining(completed, &mut backend).unwrap();
    assert_eq!(backend.reset_calls, 1);
    assert!(!format!("{duplicate:?}").contains("0x"));
}

#[test]
fn batch_duplicate_rejection_is_atomic_and_exact_release_succeeds_once() {
    let mut owner = owner();
    let bound = owner
        .bind_batch(core::array::from_fn::<_, 2, _>(|_| template()))
        .unwrap();
    let (_packets, retention) = bound.into_parts();
    let events = owner
        .record_unbound_compute_event_batch(SESSION, SOURCE_EPOCH, &retention)
        .unwrap();
    let batch = owner.mark_published(retention, 202).unwrap();
    let mut events = owner
        .bind_compute_event_batch_after_publication(events, &batch)
        .unwrap();
    let second = events.pop().unwrap();
    let first = events.pop().unwrap();
    let duplicate = duplicate_event(&first);
    let mut returned = match owner.retain_compute_dependency_reader_batch(
        vec![first, duplicate],
        SESSION,
        DEPENDENT_EPOCH,
    ) {
        Err((Gfx942CompletionErrorV1::DuplicateDependency, events)) => events,
        other => panic!("unexpected duplicate batch admission: {other:?}"),
    };
    assert!(owner.dependency_ledger.readers.is_empty());
    let forged = returned.pop().unwrap();
    let first = returned.pop().unwrap();
    drop(forged);

    let mut retained = owner
        .retain_compute_dependency_reader_batch(vec![first, second], SESSION, DEPENDENT_EPOCH)
        .unwrap();
    let second_pair = retained.pop().unwrap();
    let first_pair = retained.pop().unwrap();
    let hostile = (
        duplicate_event(&first_pair.0),
        duplicate_reader(&first_pair.1),
    );
    let mut returned =
        match owner.release_compute_dependency_reader_event_batch(vec![first_pair, hostile]) {
            Err((Gfx942CompletionErrorV1::DuplicateDependency, retained)) => retained,
            other => panic!("unexpected duplicate batch release: {other:?}"),
        };
    assert_eq!(owner.dependency_ledger.readers.len(), 2);
    assert_eq!(owner.dependency_ledger.events.len(), 2);
    let forged = returned.pop().unwrap();
    let first_pair = returned.pop().unwrap();
    drop(forged);
    assert_eq!(
        owner
            .release_compute_dependency_reader_event_batch(vec![first_pair, second_pair])
            .unwrap(),
        2
    );
    assert!(owner.dependency_ledger.readers.is_empty());
    assert!(owner.dependency_ledger.events.is_empty());

    let mut backend = CompletedBackend { reset_calls: 0 };
    let completed = complete(&mut owner, batch, &mut backend);
    owner.recycle_retaining(completed, &mut backend).unwrap();
    assert_eq!(backend.reset_calls, 2);
}

#[test]
fn parent_bridge_projects_exact_identity_and_signal_without_observation() {
    let mut owner = owner();
    let (batch, event) = published(&mut owner);
    let source = owner.dependency_source_identity_v1(&event).unwrap();
    assert_eq!(source.session_occurrence, SESSION);
    assert_eq!(source.acceptance_epoch, SOURCE_EPOCH);
    assert_eq!(source.queue, queue());
    assert_eq!(source.packet_id, Some(101));

    let (event, lease) = owner
        .retain_dependency_reader_v1(event, SESSION, DEPENDENT_EPOCH)
        .unwrap();
    let signal = owner
        .native_dependency_signal_observation_v1(&lease)
        .unwrap();
    assert_eq!(signal.raw(), AMD_SIGNAL_ALIGNMENT_V1 as u64);

    let mut backend = CompletedBackend { reset_calls: 0 };
    let completed = complete(&mut owner, batch, &mut backend);
    assert_eq!(backend.reset_calls, 0);
    owner.release_dependency_event_v1(event).unwrap();
    owner.release_dependency_reader_v1(lease).unwrap();
    owner.recycle_retaining(completed, &mut backend).unwrap();
    assert_eq!(backend.reset_calls, 1);
}

#[test]
fn target_identity_bridge_binds_only_after_publication() {
    let mut owner = owner();
    let bound = owner.bind_batch([template()]).unwrap();
    let (_, retention) = bound.into_parts();
    let prepared = owner
        .bound_dependency_target_identity_v1(SESSION, DEPENDENT_EPOCH, &retention)
        .unwrap();
    assert_eq!(prepared.session_occurrence, SESSION);
    assert_eq!(prepared.acceptance_epoch, DEPENDENT_EPOCH);
    assert_eq!(prepared.packet_id, None);
    let batch = owner.mark_published(retention, 401).unwrap();
    let published = owner
        .published_dependency_target_identity_v1(SESSION, DEPENDENT_EPOCH, &batch)
        .unwrap();
    assert_eq!(published.packet_id, Some(401));
    assert_eq!(published.queue, prepared.queue);
    assert_eq!(published.signal_mapping, prepared.signal_mapping);
    assert_eq!(published.slot_index, prepared.slot_index);
    assert_eq!(published.slot_generation, prepared.slot_generation);
    assert_eq!(published.dispatch_generation, prepared.dispatch_generation);
}

#[test]
fn dropping_tokens_is_inert_and_never_hides_a_reset() {
    let mut owner = owner();
    let (batch, event) = published(&mut owner);
    let slot = event.exact.slot.index as usize;
    drop(event);
    assert_eq!(owner.slots[slot].event_pins, 1);
    let mut backend = CompletedBackend { reset_calls: 0 };
    let completed = complete(&mut owner, batch, &mut backend);
    drop(completed);
    assert_eq!(owner.slots[slot].event_pins, 1);
    assert_eq!(backend.reset_calls, 0);
    assert!(matches!(
        owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::BatchStillRetained)
    ));
}

#[test]
fn event_custody_manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(rendered, GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_SHA256_V1);
}
