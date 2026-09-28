mod release_tests {
    use super::super::super::{CompletionCustodySnapshotV1, CompletionOwnerPhaseV1};
    use super::*;

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
