mod release_tests {
    use super::super::super::{CompletionCustodySnapshotV1, CompletionOwnerPhaseV1};
    use super::*;

    include!("bound_cancel_tests.rs");
    include!("batch_event_release_tests.rs");
    include!("event_bind_tests.rs");

    #[derive(Debug, Eq, PartialEq)]
    struct Snapshot {
        custody: CompletionCustodySnapshotV1,
        phase: CompletionOwnerPhaseV1,
        next_event_id: u64,
        next_reader_lease_id: u64,
        events: HashMap<u64, ExactCompletionOccurrenceV1>,
        readers: HashMap<DependencyReaderUseKeyV1, ActiveDependencyReaderV1>,
    }

    fn snapshot(owner: &CompletionSignalArenaOwnerV1) -> Snapshot {
        Snapshot {
            custody: owner.custody_snapshot_for_test(),
            phase: owner.phase,
            next_event_id: owner.dependency_ledger.next_event_id,
            next_reader_lease_id: owner.dependency_ledger.next_reader_lease_id,
            events: owner.dependency_ledger.events.clone(),
            readers: owner.dependency_ledger.readers.clone(),
        }
    }

    fn finish(owner: &mut CompletionSignalArenaOwnerV1, batch: Gfx942CompletionBatchV1<1>) {
        let mut backend = CompletedBackend { reset_calls: 0 };
        let completed = complete(owner, batch, &mut backend);
        owner.recycle(completed, &mut backend).unwrap();
        assert_eq!(backend.reset_calls, 1);
        owner.ensure_releasable().unwrap();
    }

    struct AliasedRoster {
        owner: CompletionSignalArenaOwnerV1,
        neighbor: CompletionBatchRetentionV1<1>,
        neighbor_event: Gfx942ComputeEventOccurrenceV1,
        batch: Gfx942CompletionBatchV1<1>,
        retained: Gfx942ComputeDependencyReaderBatchV1,
    }

    fn aliased_roster() -> AliasedRoster {
        let mut owner = owner();
        let (neighbor, neighbor_event) = unbound(&mut owner);
        let (retention, first) = unbound(&mut owner);
        let second = owner
            .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0)
            .unwrap();
        let batch = owner.mark_published(retention, 101).unwrap();
        let first = owner
            .bind_compute_event_after_publication(first, &batch, 0)
            .unwrap();
        let second = owner
            .bind_compute_event_after_publication(second, &batch, 0)
            .unwrap();
        let first = owner
            .retain_compute_dependency_reader(first, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        let second = owner
            .retain_compute_dependency_reader(second, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        assert_ne!(first.0.event_id, second.0.event_id);
        assert_ne!(first.1.lease_id, second.1.lease_id);
        assert_eq!(first.0.exact.slot, second.0.exact.slot);
        assert_eq!(first.0.exact.slot.index, 1);
        assert_eq!(owner.slots[1].event_pins, 2);
        assert_eq!(owner.slots[1].native_reader_pins, 2);
        AliasedRoster {
            owner,
            neighbor,
            neighbor_event,
            batch,
            retained: vec![second, first],
        }
    }

    type PairFacts = (
        u64,
        ExactCompletionOccurrenceV1,
        u64,
        u64,
        u64,
        ExactCompletionOccurrenceV1,
    );

    fn pair_facts(retained: &Gfx942ComputeDependencyReaderBatchV1) -> Vec<PairFacts> {
        retained
            .iter()
            .map(|(event, lease)| {
                (
                    event.event_id,
                    event.exact,
                    lease.lease_id,
                    lease.event_id,
                    lease.dependent_acceptance_epoch,
                    lease.source,
                )
            })
            .collect()
    }

    fn remove_expected_reader(
        expected: &mut Snapshot,
        lease: &Gfx942ComputeDependencyReaderLeaseV1,
    ) {
        expected.readers.remove(&DependencyReaderUseKeyV1 {
            event_id: lease.event_id,
            dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
        });
        expected.custody.slots[lease.source.slot.index as usize].native_reader_pins -= 1;
    }

    fn finish_aliases(
        owner: &mut CompletionSignalArenaOwnerV1,
        neighbor: CompletionBatchRetentionV1<1>,
        neighbor_event: Gfx942ComputeEventOccurrenceV1,
        batch: Gfx942CompletionBatchV1<1>,
    ) {
        owner.release_compute_event(neighbor_event).unwrap();
        owner.cancel_bound_retaining(neighbor).unwrap();
        finish(owner, batch);
    }

    #[test]
    fn batch_release_aggregate_refusal_is_atomic_and_valid_aliases_succeed() {
        for case in 0..4 {
            let AliasedRoster {
                mut owner,
                neighbor,
                neighbor_event,
                batch,
                retained,
            } = aliased_roster();
            if matches!(case, 0 | 2) {
                owner.slots[1].event_pins = 1;
            } else {
                owner.slots[1].native_reader_pins = 1;
            }
            let before = snapshot(&owner);
            if case == 0 {
                let (events, readers): (Vec<_>, Vec<_>) = retained.into_iter().unzip();
                let facts = events
                    .iter()
                    .map(|e| (e.event_id, e.exact))
                    .collect::<Vec<_>>();
                let storage = (events.as_ptr(), events.capacity());
                let (error, events) = owner.release_compute_event_batch(events).unwrap_err();
                assert_eq!(error, Gfx942CompletionErrorV1::StaleEventOccurrence);
                assert_eq!((events.as_ptr(), events.capacity()), storage);
                assert_eq!(
                    events
                        .iter()
                        .map(|e| (e.event_id, e.exact))
                        .collect::<Vec<_>>(),
                    facts
                );
                assert_eq!(snapshot(&owner), before);
                owner.slots[1].event_pins = 2;
                let mut expected = snapshot(&owner);
                for event in &events {
                    expected.events.remove(&event.event_id);
                }
                expected.custody.slots[1].event_pins = 0;
                assert_eq!(owner.release_compute_event_batch(events).unwrap(), 2);
                assert_eq!(snapshot(&owner), expected);
                for reader in readers {
                    owner.release_compute_dependency_reader(reader).unwrap();
                }
            } else {
                let facts = pair_facts(&retained);
                let storage = (retained.as_ptr(), retained.capacity());
                let (error, retained) = if case == 1 {
                    owner
                        .release_compute_dependency_reader_batch(retained)
                        .unwrap_err()
                } else {
                    owner
                        .release_compute_dependency_reader_event_batch(retained)
                        .unwrap_err()
                };
                assert_eq!(error, Gfx942CompletionErrorV1::StaleDependencyReader);
                assert_eq!((retained.as_ptr(), retained.capacity()), storage);
                assert_eq!(pair_facts(&retained), facts);
                assert_eq!(snapshot(&owner), before);
                owner.slots[1].event_pins = 2;
                owner.slots[1].native_reader_pins = 2;
                let mut expected = snapshot(&owner);
                for (event, lease) in &retained {
                    remove_expected_reader(&mut expected, lease);
                    if case != 1 {
                        expected.events.remove(&event.event_id);
                        expected.custody.slots[1].event_pins -= 1;
                    }
                }
                if case == 1 {
                    let events = owner
                        .release_compute_dependency_reader_batch(retained)
                        .unwrap();
                    assert_eq!(
                        events
                            .iter()
                            .map(|e| (e.event_id, e.exact))
                            .collect::<Vec<_>>(),
                        facts.iter().map(|f| (f.0, f.1)).collect::<Vec<_>>()
                    );
                    assert_eq!(snapshot(&owner), expected);
                    assert_eq!(owner.release_compute_event_batch(events).unwrap(), 2);
                } else {
                    assert_eq!(
                        owner
                            .release_compute_dependency_reader_event_batch(retained)
                            .unwrap(),
                        2
                    );
                    assert_eq!(snapshot(&owner), expected);
                }
            }
            finish_aliases(&mut owner, neighbor, neighbor_event, batch);
        }
    }

    #[test]
    fn batch_release_legacy_errors_precede_aggregate_deficits() {
        for operation in 0..3 {
            for stale in [false, true] {
                let AliasedRoster {
                    mut owner,
                    neighbor,
                    neighbor_event,
                    batch,
                    mut retained,
                } = aliased_roster();
                let mut hostile = (
                    duplicate_event(&retained[0].0),
                    duplicate_reader(&retained[0].1),
                );
                if stale {
                    hostile.0.event_id += 100;
                    hostile.1.lease_id += 100;
                }
                retained.push(hostile);
                owner.slots[1].event_pins = 1;
                owner.slots[1].native_reader_pins = 1;
                let before = snapshot(&owner);
                let expected_error = if stale {
                    Gfx942CompletionErrorV1::StaleEventOccurrence
                } else {
                    Gfx942CompletionErrorV1::DuplicateDependency
                };
                let mut retained = if operation == 0 {
                    let (events, readers): (Vec<_>, Vec<_>) = retained.into_iter().unzip();
                    let facts = events
                        .iter()
                        .map(|e| (e.event_id, e.exact))
                        .collect::<Vec<_>>();
                    let storage = (events.as_ptr(), events.capacity());
                    let (error, events) = owner.release_compute_event_batch(events).unwrap_err();
                    assert_eq!(error, expected_error);
                    assert_eq!((events.as_ptr(), events.capacity()), storage);
                    assert_eq!(
                        events
                            .iter()
                            .map(|e| (e.event_id, e.exact))
                            .collect::<Vec<_>>(),
                        facts
                    );
                    events.into_iter().zip(readers).collect()
                } else {
                    let facts = pair_facts(&retained);
                    let storage = (retained.as_ptr(), retained.capacity());
                    let (error, retained) = if operation == 1 {
                        owner
                            .release_compute_dependency_reader_batch(retained)
                            .unwrap_err()
                    } else {
                        owner
                            .release_compute_dependency_reader_event_batch(retained)
                            .unwrap_err()
                    };
                    assert_eq!(error, expected_error);
                    assert_eq!((retained.as_ptr(), retained.capacity()), storage);
                    assert_eq!(pair_facts(&retained), facts);
                    retained
                };
                assert_eq!(snapshot(&owner), before);
                drop(retained.pop().unwrap());
                owner.slots[1].event_pins = 2;
                owner.slots[1].native_reader_pins = 2;
                owner
                    .release_compute_dependency_reader_event_batch(retained)
                    .unwrap();
                finish_aliases(&mut owner, neighbor, neighbor_event, batch);
            }
        }
    }

    #[test]
    fn release_budget_empty_single_and_multi_paths_match_counts() {
        for (budgets, success) in [
            (vec![], true),
            (vec![(7, 0)], false),
            (vec![(7, 1)], true),
            (vec![(7, u32::MAX)], true),
            (vec![(7, 1), (7, 1)], false),
            (vec![(7, 2), (7, 2)], true),
            (vec![(7, 1), (8, 1)], true),
            (vec![(7, 2), (8, 1), (7, 2)], true),
            // The raw helper retains the first budget for each slot key.
            (vec![(7, 2), (7, 0)], true),
            (vec![(7, 1), (7, 9)], false),
            (vec![(7, 2), (8, 1), (7, 0)], true),
        ] {
            assert_eq!(
                validate_release_pin_budgets(
                    budgets.into_iter(),
                    Gfx942CompletionErrorV1::StaleEventOccurrence
                ),
                if success {
                    Ok(())
                } else {
                    Err(Gfx942CompletionErrorV1::StaleEventOccurrence)
                }
            );
        }
    }

    #[test]
    fn release_authenticates_every_occurrence_field_without_mutation() {
        for case in 0..18 {
            let mut owner = owner();
            let (batch, event) = published(&mut owner);
            let original = (event.event_id, event.exact);
            let mut altered = event.exact;
            match case {
                0 => altered.session_occurrence += 1,
                1 => altered.source_acceptance_epoch += 1,
                2 => altered.batch_id += 1,
                3 => altered.queue.vm.device.physical.0 += 1,
                4 => altered.queue.vm.device.generation.0 += 1,
                5 => altered.queue.vm.id.0 += 1,
                6 => altered.queue.id.0 += 1,
                7 => altered.queue.generation.0 += 1,
                8 => altered.signal_mapping.allocation.vm.device.physical.0 += 1,
                9 => altered.signal_mapping.allocation.vm.device.generation.0 += 1,
                10 => altered.signal_mapping.allocation.vm.id.0 += 1,
                11 => altered.signal_mapping.allocation.id.0 += 1,
                12 => altered.signal_mapping.allocation.generation.0 += 1,
                13 => altered.signal_mapping.id.0 += 1,
                14 => altered.slot.index += 1,
                15 => altered.slot.generation += 1,
                16 => altered.dispatch_generation += 1,
                17 => altered.packet_id = None,
                _ => unreachable!(),
            }
            owner
                .dependency_ledger
                .events
                .insert(event.event_id, altered);
            let before = snapshot(&owner);
            let (error, returned) = owner.release_compute_event(event).unwrap_err();
            assert_eq!(
                error,
                Gfx942CompletionErrorV1::StaleEventOccurrence,
                "case {case}"
            );
            assert_eq!((returned.event_id, returned.exact), original);
            assert_eq!(snapshot(&owner), before, "case {case}");
            owner
                .dependency_ledger
                .events
                .insert(original.0, original.1);
            owner.release_compute_event(returned).unwrap();
            finish(&mut owner, batch);
        }
    }

    #[test]
    fn release_refusal_preserves_token_and_complete_owner() {
        for case in 0..12 {
            let mut owner = owner();
            let (batch, mut event) = published(&mut owner);
            let original = (event.event_id, event.exact);
            let index = event.exact.slot.index as usize;
            let record = owner.slots[index];
            let queue = owner.queue;
            let mapping = owner.signal_mapping;
            match case {
                0 => owner.phase = CompletionOwnerPhaseV1::ProbeActive,
                1 => owner.phase = CompletionOwnerPhaseV1::Poisoned,
                2 => {
                    owner.dependency_ledger.events.remove(&event.event_id);
                }
                3 => owner.queue.generation.0 += 1,
                4 => owner.signal_mapping.id.0 += 1,
                5 => owner.slots[index].generation += 1,
                6 => owner.slots[index].phase = CompletionSlotPhaseV1::Available,
                7 => {
                    owner.slots[index].phase = CompletionSlotPhaseV1::Published {
                        batch_id: event.exact.batch_id + 1,
                    }
                }
                8 => owner.slots[index].event_pins = 0,
                // Deliberately corrupt both metadata copies to reach the bounds check.
                9 => {
                    event.exact.slot.index = COMPLETION_SIGNAL_CAPACITY_V1 as u32;
                    owner
                        .dependency_ledger
                        .events
                        .insert(event.event_id, event.exact);
                }
                10 => event.event_id += 1,
                11 => {
                    owner.phase = CompletionOwnerPhaseV1::ProbeActive;
                    owner.dependency_ledger.events.remove(&event.event_id);
                    owner.slots[index].event_pins = 0;
                }
                _ => unreachable!(),
            }
            let presented = (event.event_id, event.exact);
            let before = snapshot(&owner);
            let (error, mut returned) = owner.release_compute_event(event).unwrap_err();
            assert_eq!(
                error,
                if matches!(case, 0 | 1 | 11) {
                    Gfx942CompletionErrorV1::Poisoned
                } else {
                    Gfx942CompletionErrorV1::StaleEventOccurrence
                },
                "case {case}"
            );
            assert_eq!((returned.event_id, returned.exact), presented);
            assert_eq!(snapshot(&owner), before, "case {case}");
            // Undo only the CPU corruption, then consume the genuine original ownership.
            owner.phase = CompletionOwnerPhaseV1::Ready;
            owner.queue = queue;
            owner.signal_mapping = mapping;
            owner.slots[index] = record;
            owner
                .dependency_ledger
                .events
                .insert(original.0, original.1);
            returned.event_id = original.0;
            returned.exact = original.1;
            owner.release_compute_event(returned).unwrap();
            finish(&mut owner, batch);
        }
    }

    #[test]
    fn release_raw_acceptance_does_not_assume_creation_provenance() {
        for phase in 0..3 {
            for pins in [1, u32::MAX] {
                let mut owner = owner();
                let (retention, mut event) = unbound(&mut owner);
                let index = event.exact.slot.index as usize;
                let record = owner.slots[index];
                // Coherent but deliberately unreachable metadata exercises the raw contract.
                event.exact.session_occurrence = 0;
                event.exact.source_acceptance_epoch = 0;
                event.exact.dispatch_generation = 0;
                owner
                    .dependency_ledger
                    .events
                    .insert(event.event_id, event.exact);
                owner.slots[index].phase = match phase {
                    0 => CompletionSlotPhaseV1::Bound {
                        batch_id: event.exact.batch_id,
                    },
                    1 => CompletionSlotPhaseV1::Published {
                        batch_id: event.exact.batch_id,
                    },
                    _ => CompletionSlotPhaseV1::Completed {
                        batch_id: event.exact.batch_id,
                    },
                };
                owner.slots[index].event_pins = pins;
                owner.slots[index].native_reader_pins = u32::MAX;
                let mut expected = snapshot(&owner);
                expected.events.remove(&event.event_id);
                expected.custody.slots[index].event_pins -= 1;
                owner.release_compute_event(event).unwrap();
                assert_eq!(snapshot(&owner), expected);
                owner.slots[index] = super::super::super::CompletionSlotRecordV1 {
                    event_pins: 0,
                    ..record
                };
                owner.cancel_bound_retaining(retention).unwrap();
                owner.ensure_releasable().unwrap();
            }
        }
    }

    #[test]
    fn release_live_phases_frame_same_slot_event_reader_and_neighbor() {
        for phase in 0..3 {
            let mut owner = owner();
            let (neighbor, neighbor_event) = unbound(&mut owner);
            let (retention, first) = unbound(&mut owner);
            let second = owner
                .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0)
                .unwrap();
            let (first, second, batch, completed, reader, bound) = if phase == 0 {
                (first, second, None, None, None, Some(retention))
            } else {
                let batch = owner.mark_published(retention, 101).unwrap();
                let first = owner
                    .bind_compute_event_after_publication(first, &batch, 0)
                    .unwrap();
                let second = owner
                    .bind_compute_event_after_publication(second, &batch, 0)
                    .unwrap();
                let (first, reader) = owner
                    .retain_compute_dependency_reader(first, SESSION, DEPENDENT_EPOCH)
                    .unwrap();
                if phase == 1 {
                    (first, second, Some(batch), None, Some(reader), None)
                } else {
                    let completed =
                        complete(&mut owner, batch, &mut CompletedBackend { reset_calls: 0 });
                    (first, second, None, Some(completed), Some(reader), None)
                }
            };
            let index = first.exact.slot.index as usize;
            assert_eq!(index, 1);
            let mut expected = snapshot(&owner);
            expected.events.remove(&first.event_id);
            expected.custody.slots[index].event_pins -= 1;
            owner.release_compute_event(first).unwrap();
            assert_eq!(snapshot(&owner), expected);
            expected.events.remove(&second.event_id);
            expected.custody.slots[index].event_pins -= 1;
            owner.release_compute_event(second).unwrap();
            assert_eq!(snapshot(&owner), expected);
            if let Some(reader) = reader {
                let mut backend = CompletedBackend { reset_calls: 0 };
                let completed =
                    completed.unwrap_or_else(|| complete(&mut owner, batch.unwrap(), &mut backend));
                let (error, completed) = owner
                    .recycle_retaining(completed, &mut backend)
                    .unwrap_err();
                assert!(matches!(
                    error,
                    Gfx942CompletionErrorV1::SignalPinned {
                        event_pins: 0,
                        native_reader_pins: 1,
                        ..
                    }
                ));
                assert_eq!(backend.reset_calls, 0);
                owner.release_compute_dependency_reader(reader).unwrap();
                owner.recycle(completed, &mut backend).unwrap();
                assert_eq!(backend.reset_calls, 1);
            }
            owner.release_compute_event(neighbor_event).unwrap();
            owner.cancel_bound_retaining(neighbor).unwrap();
            if let Some(bound) = bound {
                owner.cancel_bound_retaining(bound).unwrap();
            }
            owner.ensure_releasable().unwrap();
        }
    }
}
