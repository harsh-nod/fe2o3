use alloc::vec::Vec;

use super::r42_compute_event_signal_custody::*;

fn source() -> R42SignalOccurrenceIdentityV1 {
    R42SignalOccurrenceIdentityV1 {
        session_occurrence: 7,
        source_acceptance_epoch: 11,
        queue_key: 13,
        signal_mapping: 17,
        slot: 19,
        slot_generation: 23,
        dispatch_generation: 29,
    }
}

fn registry() -> R42SignalCustodyRegistryV1 {
    R42SignalCustodyRegistryV1::new_model_only(source()).unwrap()
}

fn premise(epoch: u64) -> R42TargetEpochUniquenessPremiseV1 {
    R42TargetEpochUniquenessPremiseV1::new_model_only(7, epoch, true)
}

fn bound_event(registry: &mut R42SignalCustodyRegistryV1) -> R42ComputeEventOccurrenceV1 {
    let event = registry.record_event_model_only().unwrap();
    registry.publish_source_model_only(31).unwrap();
    registry.bind_event_model_only(event).unwrap()
}

#[test]
fn unbound_event_binds_once_and_failures_return_it_without_mutation() {
    let mut registry = registry();
    let event = registry.record_event_model_only().unwrap();
    assert_eq!(
        event.binding_state_model_only(),
        R42EventBindingStateV1::Unbound
    );
    let before = registry.snapshot_model_only();
    let event = match registry.bind_event_model_only(event) {
        Err((R42SignalCustodyErrorV1::EventNotPublished, event)) => event,
        other => panic!("unexpected bind result: {other:?}"),
    };
    assert_eq!(registry.snapshot_model_only(), before);
    registry.publish_source_model_only(31).unwrap();
    let event = registry.bind_event_model_only(event).unwrap();
    assert_eq!(
        event.binding_state_model_only(),
        R42EventBindingStateV1::Bound
    );
    let before = registry.snapshot_model_only();
    let _event = match registry.bind_event_model_only(event) {
        Err((R42SignalCustodyErrorV1::EventAlreadyBound, event)) => event,
        other => panic!("unexpected second bind result: {other:?}"),
    };
    assert_eq!(registry.snapshot_model_only(), before);
}

#[test]
fn independent_event_and_reader_pins_block_completed_recycle() {
    let mut registry = registry();
    let event = bound_event(&mut registry);
    let (event, reader) = registry
        .retain_reader_model_only(event, premise(37))
        .unwrap();
    registry.complete_source_model_only().unwrap();
    assert_eq!(
        registry.recycle_model_only(),
        Err(R42SignalCustodyErrorV1::SignalPinned)
    );
    registry.release_event_model_only(event).unwrap();
    let snapshot = registry.snapshot_model_only();
    assert_eq!(snapshot.event_pins, 0);
    assert_eq!(snapshot.native_reader_pins, 1);
    assert_eq!(
        registry.recycle_model_only(),
        Err(R42SignalCustodyErrorV1::SignalPinned)
    );
    registry.release_reader_model_only(reader).unwrap();
    let observation = registry.recycle_model_only().unwrap();
    assert_eq!(observation.prior_generation, 23);
    assert_eq!(observation.next_generation, 24);
}

#[test]
fn stale_duplicate_cross_session_cycle_and_missing_premise_do_not_mutate() {
    let mut stale_registry = registry();
    let event = bound_event(&mut stale_registry);
    let stale = event.substitute_slot_generation_model_only(99);
    let before = stale_registry.snapshot_model_only();
    let _stale = match stale_registry.retain_reader_model_only(stale, premise(37)) {
        Err((R42SignalCustodyErrorV1::StaleEvent, event)) => event,
        other => panic!("unexpected stale result: {other:?}"),
    };
    assert_eq!(stale_registry.snapshot_model_only(), before);

    // A second registry gives one intact event for all owner-returning cases.
    let mut registry = registry();
    let event = bound_event(&mut registry);
    let cases = [
        R42TargetEpochUniquenessPremiseV1::new_model_only(8, 37, true),
        premise(11),
        premise(10),
        R42TargetEpochUniquenessPremiseV1::new_model_only(7, 37, false),
    ];
    let expected = [
        R42SignalCustodyErrorV1::CrossSession,
        R42SignalCustodyErrorV1::SelfDependency,
        R42SignalCustodyErrorV1::DependencyCycle,
        R42SignalCustodyErrorV1::MissingTargetEpochUniquenessPremise,
    ];
    let mut event = event;
    for (case, expected) in cases.into_iter().zip(expected) {
        let before = registry.snapshot_model_only();
        event = match registry.retain_reader_model_only(event, case) {
            Err((error, event)) if error == expected => event,
            other => panic!("unexpected admission result: {other:?}"),
        };
        assert_eq!(registry.snapshot_model_only(), before);
    }
    let (event, reader) = registry
        .retain_reader_model_only(event, premise(37))
        .unwrap();
    let before = registry.snapshot_model_only();
    let event = match registry.retain_reader_model_only(event, premise(37)) {
        Err((R42SignalCustodyErrorV1::DuplicateDependency, event)) => event,
        other => panic!("unexpected duplicate result: {other:?}"),
    };
    assert_eq!(registry.snapshot_model_only(), before);
    registry.release_event_model_only(event).unwrap();
    registry.release_reader_model_only(reader).unwrap();
}

#[test]
fn event_and_reader_capacity_failures_preserve_exact_snapshots() {
    let mut event_registry = registry();
    let mut events = Vec::with_capacity(R42_EVENT_CAPACITY_V1);
    for _ in 0..R42_EVENT_CAPACITY_V1 {
        events.push(event_registry.record_event_model_only().unwrap());
    }
    let before = event_registry.snapshot_model_only();
    assert_eq!(
        event_registry.record_event_model_only(),
        Err(R42SignalCustodyErrorV1::EventCapacity)
    );
    assert_eq!(event_registry.snapshot_model_only(), before);
    assert_eq!(before.event_pins as usize, R42_EVENT_CAPACITY_V1);

    let mut reader_registry = registry();
    let event = bound_event(&mut reader_registry);
    let mut readers = Vec::with_capacity(R42_NATIVE_READER_CAPACITY_V1);
    let mut event = event;
    for offset in 0..R42_NATIVE_READER_CAPACITY_V1 as u64 {
        let retained = reader_registry
            .retain_reader_model_only(event, premise(37 + offset))
            .unwrap();
        event = retained.0;
        readers.push(retained.1);
    }
    let before = reader_registry.snapshot_model_only();
    let _event = match reader_registry.retain_reader_model_only(event, premise(20_000)) {
        Err((R42SignalCustodyErrorV1::ReaderCapacity, event)) => event,
        other => panic!("unexpected reader capacity result: {other:?}"),
    };
    assert_eq!(reader_registry.snapshot_model_only(), before);
    assert_eq!(
        before.native_reader_pins as usize,
        R42_NATIVE_READER_CAPACITY_V1
    );
}

#[test]
fn completion_and_failed_recycle_do_not_hide_generation_advance() {
    let mut registry = registry();
    let event = bound_event(&mut registry);
    registry.complete_source_model_only().unwrap();
    let before = registry.snapshot_model_only();
    assert_eq!(
        registry.recycle_model_only(),
        Err(R42SignalCustodyErrorV1::SignalPinned)
    );
    assert_eq!(registry.snapshot_model_only(), before);
    registry.release_event_model_only(event).unwrap();
    registry.recycle_model_only().unwrap();
    let after = registry.snapshot_model_only();
    assert_eq!(after.phase, R42CompletionSlotPhaseV1::Available);
    assert_eq!(after.source.slot_generation, 24);
}
