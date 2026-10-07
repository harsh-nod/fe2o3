#[test]
fn event_batch_issuance_frames_neighbors_and_selects_exact_high_slots_and_ids() {
    for (selected, high_ids) in [([1, 2, 3], false), ([63, 64, 8191], true)] {
        let mut owner = owner();
        let (neighbor_batch, neighbor) = published(&mut owner);
        let (neighbor, reader) = owner
            .retain_compute_dependency_reader(neighbor, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        for (index, slot) in owner.slots.iter_mut().enumerate() {
            if slot.phase == CompletionSlotPhaseV1::Available && !selected.contains(&index) {
                slot.phase = CompletionSlotPhaseV1::Bound { batch_id: u64::MAX };
            }
        }
        let (_, retention) = owner.bind_batch([template(); 3]).unwrap().into_parts();
        for slot in owner.slots.iter_mut() {
            if slot.phase == (CompletionSlotPhaseV1::Bound { batch_id: u64::MAX }) {
                slot.phase = CompletionSlotPhaseV1::Available;
            }
        }
        assert_eq!(retention.slots.map(|slot| slot.index as usize), selected);
        let aliases = [0, 2].map(|index| {
            owner
                .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, index)
                .unwrap()
        });
        if high_ids {
            owner.dependency_ledger.next_event_id = u64::MAX - 3;
        }
        let mut expected = snapshot(&owner);
        let first = expected.next_event_id;
        expected.next_event_id += 3;
        let expected_events = selected.map(|slot| {
            let index = selected.iter().position(|other| *other == slot).unwrap();
            (
                first + index as u64,
                ExactCompletionOccurrenceV1 {
                    session_occurrence: SESSION,
                    source_acceptance_epoch: SOURCE_EPOCH,
                    batch_id: retention.batch_id,
                    queue: retention.queue,
                    signal_mapping: retention.signal_mapping,
                    slot: retention.slots[index],
                    dispatch_generation: retention.dispatches[index].dispatch_generation,
                    packet_id: None,
                },
            )
        });
        for (id, exact) in expected_events {
            assert!(expected.events.insert(id, exact).is_none());
            expected.custody.slots[exact.slot.index as usize].event_pins += 1;
        }
        let events = owner
            .record_unbound_compute_event_batch(SESSION, SOURCE_EPOCH, &retention)
            .unwrap();
        assert_eq!(bind_event_facts(&events), expected_events);
        assert_eq!(snapshot(&owner), expected);
        if high_ids {
            let before = snapshot(&owner);
            assert_eq!(
                owner
                    .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0)
                    .unwrap_err(),
                Gfx942CompletionErrorV1::EventIdentityExhausted
            );
            assert_eq!(snapshot(&owner), before);
        }
        owner.release_compute_event_batch(events).unwrap();
        for event in aliases {
            owner.release_compute_event(event).unwrap();
        }
        owner.cancel_bound_retaining(retention).unwrap();
        owner.release_compute_dependency_reader(reader).unwrap();
        owner.release_compute_event(neighbor).unwrap();
        finish(&mut owner, neighbor_batch);
    }
}

#[test]
fn event_batch_issuance_has_two_allocations_and_both_refusals_are_atomic() {
    for refusal in [0, 1, 2] {
        let mut owner = owner();
        let (_, retention) = owner.bind_batch([template(); 3]).unwrap().into_parts();
        let before = snapshot(&owner);
        let events = if refusal == 0 {
            let (result, allocations) = crate::topology::tests::count_allocations_for_test(|| {
                owner.record_unbound_compute_event_batch(SESSION, SOURCE_EPOCH, &retention)
            });
            assert_eq!(allocations, 2);
            result.unwrap()
        } else {
            let (result, failed) =
                crate::topology::tests::fail_allocation_for_test(refusal, || {
                    owner.record_unbound_compute_event_batch(SESSION, SOURCE_EPOCH, &retention)
                });
            assert!(failed);
            assert_eq!(
                result.unwrap_err(),
                Gfx942CompletionErrorV1::DependencyLedgerAllocation
            );
            assert_eq!(snapshot(&owner), before);
            if refusal == 2 {
                assert!(owner.dependency_ledger.events.capacity() >= 3);
            }
            owner
                .record_unbound_compute_event_batch(SESSION, SOURCE_EPOCH, &retention)
                .unwrap()
        };
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        owner.release_compute_event_batch(events).unwrap();
        owner.cancel_bound_retaining(retention).unwrap();
        assert_eq!(owner.dependency_ledger.next_event_id, 4);
        owner.ensure_releasable().unwrap();
    }
    let mut owner = owner();
    let (_, retention) = owner.bind_batch([template(); 3]).unwrap().into_parts();
    let before = snapshot(&owner);
    let (result, failed) = crate::topology::tests::fail_allocation_for_test(1, || {
        owner.record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 2)
    });
    assert!(failed);
    assert_eq!(
        result.unwrap_err(),
        Gfx942CompletionErrorV1::DependencyLedgerAllocation
    );
    assert_eq!(snapshot(&owner), before);
    owner.cancel_bound_retaining(retention).unwrap();
    owner.ensure_releasable().unwrap();
}

#[test]
fn event_batch_issuance_preflight_errors_do_not_allocate_or_partially_pin() {
    for fault in 0..8 {
        let mut owner = owner();
        let (_, mut retention) = owner.bind_batch([template(); 3]).unwrap().into_parts();
        let original = retention.slots.clone();
        let mut session = SESSION;
        let mut epoch = SOURCE_EPOCH;
        let expected_error = match fault {
            0 => {
                owner.phase = CompletionOwnerPhaseV1::Poisoned;
                retention.slots[2].generation += 1;
                Gfx942CompletionErrorV1::Poisoned
            }
            1 => {
                retention.slots[2].generation += 1;
                owner.slots[0].event_pins = u32::MAX;
                session = 0;
                Gfx942CompletionErrorV1::StaleBatchGeneration
            }
            2 => {
                session = 0;
                epoch = 0;
                Gfx942CompletionErrorV1::InvalidSessionOccurrence
            }
            3 => {
                epoch = 0;
                Gfx942CompletionErrorV1::InvalidAcceptanceEpoch
            }
            4 => {
                owner.dependency_ledger.next_event_id = u64::MAX - 2;
                Gfx942CompletionErrorV1::EventIdentityExhausted
            }
            5 => {
                owner.slots[2].event_pins = u32::MAX;
                Gfx942CompletionErrorV1::SignalPinCountExhausted
            }
            6 | 7 => {
                let exact = exact_occurrence(SESSION, SOURCE_EPOCH, &retention, 0, None).unwrap();
                for id in 1..=8190 {
                    owner.dependency_ledger.events.insert(id, exact);
                }
                owner.dependency_ledger.next_event_id = if fault == 7 { u64::MAX } else { 8191 };
                Gfx942CompletionErrorV1::EventCapacityExhausted
            }
            _ => unreachable!(),
        };
        let before = snapshot(&owner);
        let (result, allocations) = crate::topology::tests::count_allocations_for_test(|| {
            owner.record_unbound_compute_event_batch(session, epoch, &retention)
        });
        assert_eq!(result.unwrap_err(), expected_error, "fault {fault}");
        assert_eq!(allocations, 0, "fault {fault}");
        assert_eq!(snapshot(&owner), before, "fault {fault}");
        owner.phase = CompletionOwnerPhaseV1::Ready;
        retention.slots = original;
        owner.slots[0].event_pins = 0;
        owner.slots[2].event_pins = 0;
        if fault >= 6 {
            owner.dependency_ledger.events.clear();
        }
        owner.cancel_bound_retaining(retention).unwrap();
        owner.ensure_releasable().unwrap();
    }
}
