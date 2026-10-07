mod complete_batch_tests {
    use super::*;

    fn facts(events: &[Gfx942ComputeEventOccurrenceV1]) -> Vec<(u64, ExactCompletionOccurrenceV1)> {
        events
            .iter()
            .map(|event| (event.event_id, event.exact))
            .collect()
    }

    #[test]
    fn batch_empty_and_single_preserve_phase_order_and_exact_custody() {
        for phase in [
            CompletionOwnerPhaseV1::Ready,
            CompletionOwnerPhaseV1::ProbeActive,
            CompletionOwnerPhaseV1::Poisoned,
        ] {
            for populated in [false, true] {
                let mut owner = owner();
                let (retention, event) = unbound(&mut owner);
                let mut events = Vec::with_capacity(7);
                let spare = if populated {
                    events.push(event);
                    None
                } else {
                    Some(event)
                };
                let supplied = facts(&events);
                let storage = (events.as_ptr(), events.capacity());
                owner.phase = phase;
                let before = snapshot(&owner);
                let result = owner.release_compute_event_batch(events);
                if phase == CompletionOwnerPhaseV1::Ready {
                    assert_eq!(result.unwrap(), usize::from(populated));
                    let mut expected = before;
                    if populated {
                        expected.events.remove(&supplied[0].0);
                        expected.custody.slots[supplied[0].1.slot.index as usize].event_pins -= 1;
                    }
                    assert_eq!(snapshot(&owner), expected);
                } else {
                    let (error, events) = result.unwrap_err();
                    assert_eq!(error, Gfx942CompletionErrorV1::Poisoned);
                    assert_eq!(facts(&events), supplied);
                    assert_eq!((events.as_ptr(), events.capacity()), storage);
                    assert_eq!(snapshot(&owner), before);
                    owner.phase = CompletionOwnerPhaseV1::Ready;
                    owner.release_compute_event_batch(events).unwrap();
                }
                if let Some(event) = spare {
                    owner.release_compute_event(event).unwrap();
                }
                owner.cancel_bound_retaining(retention).unwrap();
                owner.ensure_releasable().unwrap();
            }
        }
    }

    #[test]
    fn batch_ordered_late_validation_refusal_preserves_full_roster_and_owner() {
        for case in 0..8 {
            let mut owner = owner();
            let (neighbor, neighbor_event) = unbound(&mut owner);
            let (a, first) = unbound(&mut owner);
            let (b, second) = unbound(&mut owner);
            let (c, last) = unbound(&mut owner);
            let mut events = Vec::with_capacity(11);
            events.extend([first, second, last]);
            let original = facts(&events);
            let last_index = events[2].exact.slot.index as usize;
            let record = owner.slots[last_index];
            match case {
                0 => events[2].exact.source_acceptance_epoch += 1,
                1 => owner.slots[last_index].generation += 1,
                2 => owner.slots[last_index].phase = CompletionSlotPhaseV1::Available,
                3 => owner.slots[last_index].event_pins = 0,
                4 => {
                    events[2].exact.slot.index = 8192;
                    owner
                        .dependency_ledger
                        .events
                        .insert(events[2].event_id, events[2].exact);
                }
                5 => {
                    events[0].exact.source_acceptance_epoch += 1;
                    events[2].event_id = events[0].event_id;
                }
                6 => {
                    events[2].event_id = events[0].event_id;
                    events[2].exact.slot.index = u32::MAX;
                }
                7 => {
                    events[2].exact.queue.generation.0 += 1;
                    owner
                        .dependency_ledger
                        .events
                        .insert(events[2].event_id, events[2].exact);
                }
                _ => unreachable!(),
            }
            let supplied = facts(&events);
            let storage = (events.as_ptr(), events.capacity());
            let before = snapshot(&owner);
            let (error, mut returned) = owner.release_compute_event_batch(events).unwrap_err();
            assert_eq!(
                error,
                if case == 6 {
                    Gfx942CompletionErrorV1::DuplicateDependency
                } else {
                    Gfx942CompletionErrorV1::StaleEventOccurrence
                },
                "case {case}"
            );
            assert_eq!(facts(&returned), supplied, "case {case}");
            assert_eq!(
                (returned.as_ptr(), returned.capacity()),
                storage,
                "case {case}"
            );
            assert_eq!(snapshot(&owner), before, "case {case}");
            for (event, (id, exact)) in returned.iter_mut().zip(original) {
                event.event_id = id;
                event.exact = exact;
                owner.dependency_ledger.events.insert(id, exact);
            }
            owner.slots[last_index] = record;
            let mut expected = snapshot(&owner);
            for event in &returned {
                expected.events.remove(&event.event_id);
                expected.custody.slots[event.exact.slot.index as usize].event_pins -= 1;
            }
            assert_eq!(owner.release_compute_event_batch(returned).unwrap(), 3);
            assert_eq!(snapshot(&owner), expected);
            for retention in [a, b, c] {
                owner.cancel_bound_retaining(retention).unwrap();
            }
            owner.release_compute_event(neighbor_event).unwrap();
            owner.cancel_bound_retaining(neighbor).unwrap();
            owner.ensure_releasable().unwrap();
        }
    }

    #[test]
    fn batch_mixed_phases_and_interleaved_aliases_preserve_surviving_events_and_readers() {
        let mut owner = owner();
        let (neighbor, neighbor_event) = unbound(&mut owner);
        let (alias_retention, first) = unbound(&mut owner);
        let second = owner
            .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &alias_retention, 0)
            .unwrap();
        let survivor = owner
            .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &alias_retention, 0)
            .unwrap();
        let alias_batch = owner.mark_published(alias_retention, 101).unwrap();
        let mut aliases = Vec::new();
        for event in [first, second, survivor] {
            let event = owner
                .bind_compute_event_after_publication(event, &alias_batch, 0)
                .unwrap();
            aliases.push(
                owner
                    .retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
                    .unwrap(),
            );
        }
        let (survivor, survivor_reader) = aliases.pop().unwrap();
        let (second, second_reader) = aliases.pop().unwrap();
        let (first, first_reader) = aliases.pop().unwrap();
        let (bound, bound_event) = unbound(&mut owner);
        let (completed_batch, completed_event) = published(&mut owner);
        let (completed_event, completed_reader) = owner
            .retain_compute_dependency_reader(completed_event, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        let mut backend = CompletedBackend { reset_calls: 0 };
        let completed = complete(&mut owner, completed_batch, &mut backend);
        let events = vec![first, bound_event, completed_event, second];
        let mut expected = snapshot(&owner);
        for event in &events {
            expected.events.remove(&event.event_id);
            expected.custody.slots[event.exact.slot.index as usize].event_pins -= 1;
        }
        assert_eq!(owner.release_compute_event_batch(events).unwrap(), 4);
        assert_eq!(snapshot(&owner), expected);
        assert_eq!(
            owner.slots[survivor.exact.slot.index as usize].event_pins,
            1
        );
        for reader in [
            first_reader,
            second_reader,
            survivor_reader,
            completed_reader,
        ] {
            owner.release_compute_dependency_reader(reader).unwrap();
        }
        owner.release_compute_event(survivor).unwrap();
        owner.cancel_bound_retaining(bound).unwrap();
        owner.release_compute_event(neighbor_event).unwrap();
        owner.cancel_bound_retaining(neighbor).unwrap();
        owner.recycle(completed, &mut backend).unwrap();
        assert_eq!(backend.reset_calls, 1);
        finish(&mut owner, alias_batch);
    }
}
