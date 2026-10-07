use super::*;
use crate::topology::tests::count_allocations_for_test;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    next: u64,
    occurrence: u64,
    queue: Option<QueueKeyV1>,
    profile: FixedDispatchCapacityProfileV1,
    pointer: usize,
    slots: Vec<DispatchEpochSlotV1>,
    recycled: Option<u64>,
    predecessor: Option<u64>,
    poisoned: bool,
}

fn snapshot(owner: &DispatchGenerationOwnerV1) -> Snapshot {
    Snapshot {
        next: owner.next_generation,
        occurrence: owner.recipe_occurrence,
        queue: owner.recipe_queue,
        profile: owner.capacity_profile,
        pointer: owner.slots.as_ptr() as usize,
        slots: owner.slots.to_vec(),
        recycled: owner.recycled_generation,
        predecessor: owner.predecessor_detached_generation,
        poisoned: owner.poisoned,
    }
}

fn occupied(generation: u64) -> DispatchEpochSlotV1 {
    DispatchEpochSlotV1 {
        slot_generation: generation,
        phase: DispatchEpochPhaseV1::Published {
            dispatch_generation: generation,
            completion: test_completion_occurrence_v1(generation),
        },
    }
}

#[test]
fn reserve_scans_exhausted_vacancies_and_burns_both_generations_without_allocating() {
    for profile in [
        FixedDispatchCapacityProfileV1::Default64,
        FixedDispatchCapacityProfileV1::Qualification1024,
    ] {
        let bytes =
            host_metadata_table_payload_bytes_v1::<DispatchEpochSlotV1>(profile.slots()).unwrap();
        let account = ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
            1,
        )
        .unwrap();
        let mut owner =
            DispatchGenerationOwnerV1::with_capacity(1, profile, Some(&account)).unwrap();
        let mut ids = Vec::new();
        for generation in 1..=profile.slots() as u64 {
            ids.push(
                owner
                    .reserve(
                        test_dispatch_queue_v1(),
                        test_completion_roster_v1(generation),
                    )
                    .unwrap(),
            );
        }
        let target = profile.slots() - 1;
        owner.cancel_epoch(ids[0]).unwrap();
        owner.cancel_epoch(ids[target]).unwrap();
        owner.slots[0].slot_generation = u64::MAX;
        let usage = account.usage();
        for pass in 1..=2 {
            let before = snapshot(&owner);
            let generation = owner.next_generation;
            let (result, allocations) = count_allocations_for_test(|| {
                owner.reserve(
                    test_dispatch_queue_v1(),
                    test_completion_roster_v1(generation),
                )
            });
            assert_eq!(allocations, 0);
            let id = result.unwrap();
            assert_eq!(id.slot_index as usize, target);
            assert_eq!(id.slot_generation, 1 + pass);
            assert_eq!(id.dispatch_generation, generation);
            let mut expected = before;
            expected.next += 1;
            expected.slots[target] = DispatchEpochSlotV1 {
                slot_generation: 1 + pass,
                phase: DispatchEpochPhaseV1::Reserved {
                    dispatch_generation: generation,
                    expected_roster: test_completion_roster_v1(generation),
                },
            };
            assert_eq!(snapshot(&owner), expected);
            owner.cancel_epoch(id).unwrap();
            expected.slots[target].phase = DispatchEpochPhaseV1::Vacant;
            assert_eq!(snapshot(&owner), expected);
            assert!(owner.cancel_epoch(ids[target]).is_err());
            assert!(owner.cancel_epoch(id).is_err());
            assert_eq!(snapshot(&owner), expected);
            assert_eq!(account.usage(), usage);
        }
        drop(owner);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn reserve_raw_state_precedence_matches_independent_small_table_oracle() {
    let queue = test_dispatch_queue_v1();
    let mut wrong = queue;
    wrong.generation.0 += 1;
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    // Deliberately malformed table length: leaf behavior has no constructor premise.
    owner.slots = HostMetadataTableV1::try_new(3, None, || DispatchEpochSlotV1::VACANT).unwrap();
    for pattern in 0..64 {
        for next in [0, 9, u64::MAX] {
            for bound in [None, Some(queue), Some(wrong)] {
                for poisoned in [false, true] {
                    for roster_kind in 0..4 {
                        owner.next_generation = next;
                        owner.recipe_queue = bound;
                        owner.poisoned = poisoned;
                        owner.recipe_occurrence = 0;
                        owner.recycled_generation = Some(7);
                        owner.predecessor_detached_generation = Some(u64::MAX);
                        for (i, slot) in owner.slots.iter_mut().enumerate() {
                            *slot = match (pattern >> (i * 2)) & 3 {
                                0 => DispatchEpochSlotV1::VACANT,
                                1 => DispatchEpochSlotV1 {
                                    slot_generation: u64::MAX,
                                    phase: DispatchEpochPhaseV1::Vacant,
                                },
                                2 => occupied(71 + i as u64),
                                _ => DispatchEpochSlotV1 {
                                    slot_generation: u64::MAX - 1,
                                    phase: DispatchEpochPhaseV1::Vacant,
                                },
                            };
                        }
                        let mut roster = test_completion_roster_v1(next);
                        match roster_kind {
                            1 => roster.queue = wrong,
                            2 => roster.dispatch_generation = next.wrapping_add(1),
                            3 => roster.packet_count = 0,
                            _ => {}
                        }
                        let before = snapshot(&owner);
                        let candidates: Vec<_> = before
                            .slots
                            .iter()
                            .enumerate()
                            .filter(|(_, slot)| {
                                slot.phase == DispatchEpochPhaseV1::Vacant
                                    && slot.slot_generation != u64::MAX
                            })
                            .map(|(index, _)| index)
                            .collect();
                        let expected_error = if poisoned {
                            Some("Poisoned")
                        } else if bound == Some(wrong) {
                            Some("WrongQueueGeneration")
                        } else if candidates.is_empty() {
                            Some(
                                if before
                                    .slots
                                    .iter()
                                    .any(|slot| slot.phase == DispatchEpochPhaseV1::Vacant)
                                {
                                    "GenerationExhausted"
                                } else {
                                    "DispatchEpochCapacity { maximum: 64 }"
                                },
                            )
                        } else if next == u64::MAX {
                            Some("GenerationExhausted")
                        } else if roster_kind != 0 {
                            Some("StaleDispatchGeneration")
                        } else {
                            None
                        };
                        match (owner.reserve(queue, roster), expected_error) {
                            (Err(error), Some(expected)) => {
                                assert_eq!(error.to_string(), expected);
                                assert_eq!(snapshot(&owner), before);
                            }
                            (Ok(id), None) => {
                                let i = candidates[0];
                                assert_eq!(
                                    id,
                                    DispatchEpochIdentityV1 {
                                        queue,
                                        recipe_occurrence: 0,
                                        slot_index: i as u16,
                                        slot_generation: before.slots[i].slot_generation + 1,
                                        dispatch_generation: next,
                                    }
                                );
                                let mut expected = before;
                                expected.next = next + 1;
                                expected.queue = Some(queue);
                                expected.slots[i] = DispatchEpochSlotV1 {
                                    slot_generation: id.slot_generation,
                                    phase: DispatchEpochPhaseV1::Reserved {
                                        dispatch_generation: next,
                                        expected_roster: roster,
                                    },
                                };
                                assert_eq!(snapshot(&owner), expected);
                            }
                            (result, error) => {
                                panic!("pattern {pattern}: {result:?}, expected {error:?}")
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn reserve_preserves_leaf_width_and_roster_boundaries() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    owner.slots =
        HostMetadataTableV1::try_new(usize::from(u16::MAX) + 2, None, || occupied(4)).unwrap();
    let last = owner.slots.len() - 1;
    owner.slots[last].phase = DispatchEpochPhaseV1::Vacant;
    let before = snapshot(&owner);
    assert!(matches!(
        owner.reserve(queue, test_completion_roster_v1(1)),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(snapshot(&owner), before);
    let mut roster = test_completion_roster_v1(1);
    roster.packet_count = 0;
    assert!(matches!(
        owner.reserve(queue, roster),
        Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
    ));
    assert_eq!(snapshot(&owner), before);
    owner.slots[last - 1].phase = DispatchEpochPhaseV1::Vacant;
    roster.packet_count = usize::MAX;
    let id = owner.reserve(queue, roster).unwrap();
    assert_eq!(id.slot_index, u16::MAX);
    owner.cancel_epoch(id).unwrap();
    owner.capacity_profile = FixedDispatchCapacityProfileV1::Qualification1024;
    roster.dispatch_generation = 2;
    for count in [0, 2, usize::MAX] {
        roster.packet_count = count;
        let before = snapshot(&owner);
        assert!(matches!(
            owner.reserve(queue, roster),
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
        ));
        assert_eq!(snapshot(&owner), before);
    }
    roster.packet_count = 1;
    assert_eq!(owner.reserve(queue, roster).unwrap().slot_index, u16::MAX);
}
