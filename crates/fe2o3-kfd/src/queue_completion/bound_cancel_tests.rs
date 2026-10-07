mod cancel_tests {
    use super::*;

    #[derive(Debug, Eq, PartialEq)]
    struct RetentionFacts<const N: usize> {
        batch_id: u64,
        queue: QueueKeyV1,
        signal_mapping: MemoryMappingKeyV1,
        slots: [CompletionSlotLeaseV1; N],
        dispatches: [CompletionDispatchGenerationBindingV1; N],
        last_packet_id: Option<u64>,
        slot_storage: usize,
        dispatch_storage: usize,
    }

    fn retention_facts<const N: usize>(
        retention: &CompletionBatchRetentionV1<N>,
    ) -> RetentionFacts<N> {
        RetentionFacts {
            batch_id: retention.batch_id,
            queue: retention.queue,
            signal_mapping: retention.signal_mapping,
            slots: *retention.slots,
            dispatches: *retention.dispatches,
            last_packet_id: retention.last_packet_id,
            slot_storage: retention.slots.as_ptr() as usize,
            dispatch_storage: retention.dispatches.as_ptr() as usize,
        }
    }

    #[test]
    fn cancellation_refuses_late_faults_before_mutation_and_returns_exact_retention() {
        for case in 0..15 {
            let mut owner = owner();
            let (neighbor, neighbor_event) = unbound(&mut owner);
            let bound = owner.bind_batch([template(); 3]).unwrap();
            let events = owner
                .record_dependency_event_batch_for_bound_v1(SESSION, SOURCE_EPOCH, &bound)
                .unwrap();
            let (_, mut retention) = bound.into_parts();
            let mut kept = Vec::new();
            for (i, event) in events.into_iter().enumerate() {
                if (case == 0 && i == 2) || (case == 5 && i == 0) {
                    kept.push(event);
                } else {
                    owner.release_compute_event(event).unwrap();
                }
            }
            let original = retention_facts(&retention);
            let last = retention.slots[2].index as usize;
            let original_record = owner.slots[last];
            let expected = match case {
                0 => Gfx942CompletionErrorV1::SignalPinned {
                    slot: last as u32,
                    event_pins: 1,
                    native_reader_pins: 0,
                },
                1 => {
                    // Raw hostile metadata: bound sources cannot mint a reader.
                    owner.slots[last].native_reader_pins = u32::MAX;
                    Gfx942CompletionErrorV1::SignalPinned {
                        slot: last as u32,
                        event_pins: 0,
                        native_reader_pins: u32::MAX,
                    }
                }
                _ => Gfx942CompletionErrorV1::StaleBatchGeneration,
            };
            match case {
                2 => retention.slots[2].generation += 1,
                3 => {
                    owner.slots[last].phase = CompletionSlotPhaseV1::Published {
                        batch_id: retention.batch_id,
                    }
                }
                4 | 5 => retention.slots[2] = retention.slots[0],
                6 => retention.queue.generation.0 += 1,
                7 => retention.signal_mapping.id.0 += 1,
                8 => retention.dispatches[2].queue.generation.0 += 1,
                9 => retention.dispatches[2].dispatch_generation = 0,
                10 => retention.dispatches[2].code.allocation.vm.id.0 += 1,
                11 => retention.dispatches[2].kernarg.allocation.vm.id.0 += 1,
                12 => retention.last_packet_id = Some(123),
                13 => retention.slots[2].index = 8192,
                14 => {
                    owner.slots[last].phase = CompletionSlotPhaseV1::Bound {
                        batch_id: retention.batch_id + 1,
                    }
                }
                _ => {}
            }
            let before = snapshot(&owner);
            let supplied = retention_facts(&retention);
            let (error, mut returned) = owner.cancel_bound_retaining(retention).unwrap_err();
            assert_eq!(error, expected, "case {case}");
            assert_eq!(retention_facts(&returned), supplied, "case {case}");
            assert_eq!(snapshot(&owner), before, "case {case}");

            // Restore the injected fields, retaining the original Box allocations.
            returned.queue = original.queue;
            returned.signal_mapping = original.signal_mapping;
            *returned.slots = original.slots;
            *returned.dispatches = original.dispatches;
            returned.last_packet_id = original.last_packet_id;
            owner.slots[last] = original_record;
            assert_eq!(retention_facts(&returned), original);
            for event in kept {
                owner.release_compute_event(event).unwrap();
            }
            let mut expected = snapshot(&owner);
            for slot in returned.slots.iter() {
                expected.custody.slots[slot.index as usize].phase =
                    CompletionSlotPhaseV1::Available;
            }
            owner.cancel_bound_retaining(returned).unwrap();
            assert_eq!(snapshot(&owner), expected);
            owner.release_compute_event(neighbor_event).unwrap();
            owner.cancel_bound_retaining(neighbor).unwrap();
            owner.ensure_releasable().unwrap();
        }
    }

    #[test]
    fn cancellation_bitmap_crosses_words_without_aliasing_and_rejects_late_duplicates() {
        let mut owner = owner();
        let (_, prefix) = owner.bind_batch([template(); 63]).unwrap().into_parts();
        let (_, mut retention) = owner.bind_batch([template(); 3]).unwrap().into_parts();
        assert_eq!(retention.slots.map(|slot| slot.index), [63, 64, 65]);
        let last = retention.slots[2];
        retention.slots[2] = retention.slots[0];
        let before = snapshot(&owner);
        let supplied = retention_facts(&retention);
        let (error, mut retention) = owner.cancel_bound_retaining(retention).unwrap_err();
        assert_eq!(error, Gfx942CompletionErrorV1::StaleBatchGeneration);
        assert_eq!(retention_facts(&retention), supplied);
        assert_eq!(snapshot(&owner), before);
        retention.slots[2] = last;
        let mut expected = before;
        for index in 63..66 {
            expected.custody.slots[index].phase = CompletionSlotPhaseV1::Available;
        }
        owner.cancel_bound_retaining(retention).unwrap();
        assert_eq!(snapshot(&owner), expected);
        owner.cancel_bound_retaining(prefix).unwrap();
        owner.ensure_releasable().unwrap();
        let (_, same_bit) = owner.bind_batch([template(); 65]).unwrap().into_parts();
        assert_eq!(same_bit.slots[0].index, 0);
        assert_eq!(same_bit.slots[64].index, 64);
        let mut expected = snapshot(&owner);
        for index in 0..65 {
            expected.custody.slots[index].phase = CompletionSlotPhaseV1::Available;
        }
        owner.cancel_bound_retaining(same_bit).unwrap();
        assert_eq!(snapshot(&owner), expected);
        owner.ensure_releasable().unwrap();
    }

    #[test]
    fn cancellation_preserves_owner_phase_and_accepts_only_the_raw_dispatch_checks() {
        for phase in [
            CompletionOwnerPhaseV1::Ready,
            CompletionOwnerPhaseV1::ProbeActive,
            CompletionOwnerPhaseV1::Poisoned,
        ] {
            let mut owner = owner();
            let (_, mut retention) = owner.bind_batch([template(); 3]).unwrap().into_parts();
            // These identities are not authenticated by raw cancellation beyond VM/nonzero checks.
            retention.dispatches[2].dispatch_generation = u64::MAX;
            retention.dispatches[2].code.id.0 += 100;
            retention.dispatches[2].kernarg.allocation.generation.0 += 100;
            owner.phase = phase;
            let mut expected = snapshot(&owner);
            for slot in retention.slots.iter() {
                expected.custody.slots[slot.index as usize].phase =
                    CompletionSlotPhaseV1::Available;
            }
            owner.cancel_bound_retaining(retention).unwrap();
            assert_eq!(snapshot(&owner), expected);
            owner.phase = CompletionOwnerPhaseV1::Ready;
            owner.ensure_releasable().unwrap();
        }
    }
}
