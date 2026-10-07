use super::*;

#[test]
fn epoch_cancel_refusals_preserve_all_metadata_and_poison_precedes_stale() {
    for poisoned in [false, true] {
        for fault in 0..10 {
            let mut owner = DispatchGenerationOwnerV1::new().unwrap();
            let mut identity = owner
                .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1))
                .unwrap();
            match fault {
                0 => identity.recipe_occurrence += 1,
                1 => identity.queue.generation.0 += 1,
                2 => identity.slot_index = owner.slots.len() as u16,
                3 => identity.slot_index = u16::MAX,
                4 => identity.slot_generation += 1,
                5 => identity.dispatch_generation += 1,
                6 => owner.slots[0].phase = DispatchEpochPhaseV1::Vacant,
                7 => {
                    owner.slots[0].phase = DispatchEpochPhaseV1::Published {
                        dispatch_generation: 1,
                        completion: test_completion_occurrence_v1(1),
                    }
                }
                8 => {
                    owner.slots[0].phase = DispatchEpochPhaseV1::Completed {
                        dispatch_generation: 1,
                        completion: test_completion_occurrence_v1(1),
                    }
                }
                9 => owner.recipe_queue = None,
                _ => unreachable!(),
            }
            owner.poisoned = poisoned;
            let before = owner.clone();
            let storage = owner.slots.as_ptr();
            let error = owner.cancel_epoch(identity).unwrap_err();
            assert!(if poisoned {
                matches!(error, Gfx942DispatchBindingErrorV1::Poisoned)
            } else {
                matches!(error, Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
            });
            assert_eq!(owner, before, "fault {fault}, poisoned {poisoned}");
            assert_eq!(owner.slots.as_ptr(), storage);
        }
    }
}

#[test]
fn epoch_cancel_only_vacates_selected_slot_even_with_malformed_neighbors_and_roster() {
    for index in [0, 1, 63] {
        let mut owner = DispatchGenerationOwnerV1::new().unwrap();
        owner.recipe_queue = Some(test_dispatch_queue_v1());
        owner.next_generation = u64::MAX;
        owner.recipe_occurrence = 0;
        owner.recycled_generation = Some(0);
        owner.predecessor_detached_generation = Some(u64::MAX);
        for (i, slot) in owner.slots.iter_mut().enumerate() {
            *slot = DispatchEpochSlotV1 {
                slot_generation: i as u64,
                phase: DispatchEpochPhaseV1::Completed {
                    dispatch_generation: 0,
                    completion: test_completion_occurrence_v1(u64::MAX),
                },
            };
        }
        let mut roster = test_completion_roster_v1(u64::MAX);
        roster.packet_count = 0;
        roster.queue.generation.0 += 1;
        owner.slots[index].phase = DispatchEpochPhaseV1::Reserved {
            dispatch_generation: 0,
            expected_roster: roster,
        };
        let identity = DispatchEpochIdentityV1 {
            queue: test_dispatch_queue_v1(),
            recipe_occurrence: 0,
            slot_index: index as u16,
            slot_generation: index as u64,
            dispatch_generation: 0,
        };
        let mut expected = owner.clone();
        expected.slots[index].phase = DispatchEpochPhaseV1::Vacant;
        let storage = owner.slots.as_ptr();
        owner.cancel_epoch(identity).unwrap();
        assert_eq!(owner, expected);
        assert_eq!(owner.slots.as_ptr(), storage);
        assert!(matches!(
            owner.cancel_epoch(identity),
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
        ));
        assert_eq!(owner, expected);
    }
}
