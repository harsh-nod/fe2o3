use alloc::vec::Vec;

use super::*;

fn device(generation: u64) -> R12DeviceKeyV1 {
    R12DeviceKeyV1 {
        device_id: 7,
        generation,
    }
}

fn capability(device: R12DeviceKeyV1) -> R12MultiQueueCapabilityV1 {
    R12MultiQueueCapabilityV1 {
        device,
        stable: true,
        multi_queue_compute: true,
        max_compute_queues: 4,
        max_slots_per_queue: 8,
    }
}

fn stream(device: R12DeviceKeyV1, stream_id: u64) -> R12StreamKeyV1 {
    R12StreamKeyV1 {
        device,
        stream_id,
        generation: 1,
    }
}

fn submission(stream: R12StreamKeyV1, sequence: u64) -> R12SubmissionKeyV1 {
    R12SubmissionKeyV1 { stream, sequence }
}

fn resource(device: R12DeviceKeyV1, resource_id: u64) -> R12ResourceKeyV1 {
    R12ResourceKeyV1 {
        device,
        resource_id,
        generation: 1,
    }
}

fn model() -> R12NativeConcurrencyModelV1 {
    let device = device(1);
    R12NativeConcurrencyModelV1::new_model_only(device, capability(device), 2, 2).unwrap()
}

#[test]
fn capability_admission_binds_exact_device_and_multi_queue_limits() {
    let key = device(1);
    let mut wrong_generation = capability(device(2));
    assert!(matches!(
        R12NativeConcurrencyModelV1::new_model_only(key, wrong_generation, 2, 2),
        Err(R12ConcurrencyModelErrorV1::InvalidCapability)
    ));

    wrong_generation.device = key;
    wrong_generation.multi_queue_compute = false;
    assert!(matches!(
        R12NativeConcurrencyModelV1::new_model_only(key, wrong_generation, 2, 2),
        Err(R12ConcurrencyModelErrorV1::Unsupported)
    ));

    let capability = capability(key);
    assert!(matches!(
        R12NativeConcurrencyModelV1::new_model_only(key, capability, 1, 2),
        Err(R12ConcurrencyModelErrorV1::CapacityExceeded)
    ));
    assert!(matches!(
        R12NativeConcurrencyModelV1::new_model_only(key, capability, 5, 2),
        Err(R12ConcurrencyModelErrorV1::CapacityExceeded)
    ));
}

#[test]
fn terminal_events_may_arrive_out_of_order_but_cannot_cross_queues() {
    let mut model = model();
    let queues: Vec<_> = model.queues().collect();
    let first = submission(stream(device(1), 1), 1);
    let second = submission(stream(device(1), 2), 1);
    let first_slot = model
        .reserve_model_only(first, queues[0], &[], &[])
        .unwrap();
    let second_slot = model
        .reserve_model_only(second, queues[1], &[], &[])
        .unwrap();
    model.publish_model_only(first, first_slot).unwrap();
    model.publish_model_only(second, second_slot).unwrap();

    assert_eq!(
        model.observe_terminal_model_only(first, second_slot, R12TerminalStatusV1::Succeeded),
        Err(R12ConcurrencyModelErrorV1::StaleIdentity)
    );
    model
        .observe_terminal_model_only(second, second_slot, R12TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(
        model.submission(first).unwrap().phase(),
        R12SubmissionPhaseV1::Published
    );
    model
        .observe_terminal_model_only(first, first_slot, R12TerminalStatusV1::Failed { code: -9 })
        .unwrap();
    model.validate_global_invariants().unwrap();
}

#[test]
fn dependency_publication_requires_success_and_retains_producer_until_consumed() {
    let mut model = model();
    let queues: Vec<_> = model.queues().collect();
    let producer = submission(stream(device(1), 1), 1);
    let consumer = submission(stream(device(1), 2), 1);
    let producer_slot = model
        .reserve_model_only(producer, queues[0], &[], &[])
        .unwrap();
    let consumer_slot = model
        .reserve_model_only(consumer, queues[1], &[producer], &[])
        .unwrap();
    model.publish_model_only(producer, producer_slot).unwrap();
    assert_eq!(
        model.publish_model_only(consumer, consumer_slot),
        Err(R12ConcurrencyModelErrorV1::DependencyNotReady)
    );
    model
        .observe_terminal_model_only(producer, producer_slot, R12TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(
        model.release_terminal_model_only(producer),
        Err(R12ConcurrencyModelErrorV1::ResourceBusy)
    );
    model.publish_model_only(consumer, consumer_slot).unwrap();
    model.release_terminal_model_only(producer).unwrap();
    model.validate_global_invariants().unwrap();
}

#[test]
fn failed_dependency_never_becomes_ready() {
    let mut model = model();
    let queues: Vec<_> = model.queues().collect();
    let producer = submission(stream(device(1), 1), 1);
    let consumer = submission(stream(device(1), 2), 1);
    let producer_slot = model
        .reserve_model_only(producer, queues[0], &[], &[])
        .unwrap();
    let consumer_slot = model
        .reserve_model_only(consumer, queues[1], &[producer], &[])
        .unwrap();
    model.publish_model_only(producer, producer_slot).unwrap();
    model
        .observe_terminal_model_only(
            producer,
            producer_slot,
            R12TerminalStatusV1::Failed { code: 5 },
        )
        .unwrap();
    assert_eq!(
        model.publish_model_only(consumer, consumer_slot),
        Err(R12ConcurrencyModelErrorV1::DependencyNotReady)
    );
}

#[test]
fn prepublication_cancel_releases_resource_and_advances_slot_generation() {
    let mut model = model();
    let queue = model.queues().next().unwrap();
    let key = submission(stream(device(1), 1), 1);
    let resource = resource(device(1), 1);
    model.register_resource_model_only(resource).unwrap();
    let old_slot = model
        .reserve_model_only(key, queue, &[], &[resource])
        .unwrap();
    model.cancel_model_only(key).unwrap();

    assert_eq!(model.resource_owner(resource), Some(None));
    assert_eq!(
        model.submission(key).unwrap().phase(),
        R12SubmissionPhaseV1::CancelledBeforePublication
    );
    assert!(model.slot(old_slot).is_none());
    let replacement = submission(stream(device(1), 1), 2);
    let new_slot = model
        .reserve_model_only(replacement, queue, &[], &[resource])
        .unwrap();
    assert_eq!(old_slot.slot_index, new_slot.slot_index);
    assert_eq!(old_slot.generation + 1, new_slot.generation);
    assert_eq!(
        model.observe_terminal_model_only(replacement, old_slot, R12TerminalStatusV1::Succeeded),
        Err(R12ConcurrencyModelErrorV1::StaleIdentity)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn published_cancellation_is_too_late_and_retains_custody() {
    let mut model = model();
    let queue = model.queues().next().unwrap();
    let key = submission(stream(device(1), 1), 1);
    let resource = resource(device(1), 1);
    model.register_resource_model_only(resource).unwrap();
    let slot = model
        .reserve_model_only(key, queue, &[], &[resource])
        .unwrap();
    model.publish_model_only(key, slot).unwrap();
    assert_eq!(
        model.cancel_model_only(key),
        Err(R12ConcurrencyModelErrorV1::TooLate)
    );
    assert_eq!(model.resource_owner(resource), Some(Some(key)));
    assert_eq!(
        model.slot(slot).unwrap().phase(),
        R12SlotPhaseV1::Published(key)
    );
}

#[test]
fn currentness_loss_cancels_reserved_and_quarantines_published() {
    let mut model = model();
    let queues: Vec<_> = model.queues().collect();
    let reserved = submission(stream(device(1), 1), 1);
    let published = submission(stream(device(1), 2), 1);
    let reserved_resource = resource(device(1), 1);
    let published_resource = resource(device(1), 2);
    model
        .register_resource_model_only(reserved_resource)
        .unwrap();
    model
        .register_resource_model_only(published_resource)
        .unwrap();
    model
        .reserve_model_only(reserved, queues[0], &[], &[reserved_resource])
        .unwrap();
    let published_slot = model
        .reserve_model_only(published, queues[1], &[], &[published_resource])
        .unwrap();
    model.publish_model_only(published, published_slot).unwrap();

    model.lose_currentness_model_only().unwrap();
    assert!(!model.current());
    assert_eq!(
        model.submission(reserved).unwrap().phase(),
        R12SubmissionPhaseV1::CancelledBeforePublication
    );
    assert_eq!(model.resource_owner(reserved_resource), Some(None));
    assert_eq!(
        model.submission(published).unwrap().phase(),
        R12SubmissionPhaseV1::Indeterminate
    );
    assert_eq!(
        model.resource_owner(published_resource),
        Some(Some(published))
    );
    assert_eq!(model.resource_quarantined(published_resource), Some(true));
    assert_eq!(
        model.release_terminal_model_only(published),
        Err(R12ConcurrencyModelErrorV1::IllegalTransition)
    );
    assert_eq!(
        model.drain_queue_model_only(queues[1]),
        Err(R12ConcurrencyModelErrorV1::NotDrained)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn drain_is_occurrence_bound_and_requires_released_terminal_custody() {
    let mut model = model();
    let queue = model.queues().next().unwrap();
    let key = submission(stream(device(1), 1), 1);
    let slot = model.reserve_model_only(key, queue, &[], &[]).unwrap();
    model.publish_model_only(key, slot).unwrap();
    model
        .observe_terminal_model_only(key, slot, R12TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(
        model.drain_queue_model_only(queue),
        Err(R12ConcurrencyModelErrorV1::NotDrained)
    );
    model.release_terminal_model_only(key).unwrap();
    model.drain_queue_model_only(queue).unwrap();
    assert_eq!(model.queue_drained(queue), Some(true));

    let stale_occurrence = R12QueueOccurrenceV1 {
        occurrence: queue.occurrence + 1,
        ..queue
    };
    assert_eq!(model.queue_drained(stale_occurrence), None);
    assert_eq!(
        model.drain_queue_model_only(stale_occurrence),
        Err(R12ConcurrencyModelErrorV1::UnknownQueue)
    );

    let recreated = model.recreate_drained_queue_model_only(queue).unwrap();
    assert_eq!(recreated.queue_id, queue.queue_id);
    assert_eq!(recreated.occurrence, queue.occurrence + 1);
    assert_eq!(model.queue_drained(queue), None);
    assert_eq!(model.queue_drained(recreated), Some(false));
    assert_eq!(
        model.reserve_model_only(submission(stream(device(1), 1), 2), queue, &[], &[]),
        Err(R12ConcurrencyModelErrorV1::UnknownQueue)
    );

    let next = submission(stream(device(1), 1), 2);
    let next_slot = model.reserve_model_only(next, recreated, &[], &[]).unwrap();
    assert_eq!(next_slot.queue, recreated);
    assert_eq!(next_slot.generation, 1);
    model.validate_global_invariants().unwrap();
}

#[test]
fn queue_recreation_requires_exact_current_drain() {
    let mut model = model();
    let queue = model.queues().next().unwrap();
    assert_eq!(
        model.recreate_drained_queue_model_only(queue),
        Err(R12ConcurrencyModelErrorV1::NotDrained)
    );
    model.drain_queue_model_only(queue).unwrap();
    model.lose_currentness_model_only().unwrap();
    assert_eq!(
        model.recreate_drained_queue_model_only(queue),
        Err(R12ConcurrencyModelErrorV1::NotCurrent)
    );
}
