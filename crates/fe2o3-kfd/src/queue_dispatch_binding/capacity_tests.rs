use super::*;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

fn account(bytes: u64) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        4,
    )
    .unwrap()
}

fn payload() -> u64 {
    host_metadata_table_payload_bytes_v1::<DispatchEpochSlotV1>(1024).unwrap()
}

fn scaled(account: &ResourceCreditAccountV1) -> DispatchGenerationOwnerV1 {
    DispatchGenerationOwnerV1::with_capacity(
        1,
        FixedDispatchCapacityProfileV1::Qualification1024,
        Some(account),
    )
    .unwrap()
}

#[test]
fn scaled_epoch_table_retains_1024_exact_epochs_and_rejects_1025_atomically() {
    let account = account(payload());
    let mut owner = scaled(&account);
    let pointer = owner.slots.as_ptr();
    let mut identities = Vec::new();
    for generation in 1..=1024 {
        let identity = owner
            .reserve(
                test_dispatch_queue_v1(),
                test_completion_roster_v1(generation),
            )
            .unwrap();
        assert_eq!(usize::from(identity.slot_index), generation as usize - 1);
        owner
            .mark_published(identity, test_completion_occurrence_v1(generation))
            .unwrap();
        identities.push(identity);
    }
    for index in [63, 64, 255, 256, 1023] {
        assert_eq!(usize::from(identities[index].slot_index), index);
        owner
            .validate_published(
                identities[index],
                test_completion_occurrence_v1(index as u64 + 1),
            )
            .unwrap();
    }
    let before = owner.slots.to_vec();
    let usage = account.usage();
    assert!(matches!(
        owner.reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1025)),
        Err(Gfx942DispatchBindingErrorV1::DispatchEpochCapacity { maximum: 1024 })
    ));
    assert_eq!(&*owner.slots, before);
    assert_eq!(owner.next_generation, 1025);
    assert_eq!(owner.slots.as_ptr(), pointer);
    assert_eq!(account.usage(), usage);
    for identity in identities.iter().rev().copied() {
        let completion = test_completion_occurrence_v1(identity.dispatch_generation);
        owner.complete_epoch(identity, completion).unwrap();
        owner.recycle_epoch(identity, completion).unwrap();
    }
    owner.ensure_prepared().unwrap();
    assert_eq!(owner.returned_generation().unwrap(), 1024);
    let replacement = owner
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1025))
        .unwrap();
    assert_eq!(replacement.slot_index, 0);
    assert_eq!(replacement.slot_generation, 2);
    assert!(owner.cancel_epoch(identities[0]).is_err());
    owner.cancel_epoch(replacement).unwrap();
    drop(owner);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn scaled_profile_requires_complete_metadata_credit_and_one_packet_epochs() {
    assert!(
        DispatchGenerationOwnerV1::with_capacity(
            1,
            FixedDispatchCapacityProfileV1::Qualification1024,
            None
        )
        .is_err()
    );
    let short = account(payload() - 1);
    let before = short.usage();
    assert!(matches!(
        DispatchGenerationOwnerV1::with_capacity(
            1,
            FixedDispatchCapacityProfileV1::Qualification1024,
            Some(&short)
        ),
        Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity { .. })
    ));
    assert_eq!(short.usage(), before);
    let account = account(payload());
    let mut owner = scaled(&account);
    let before = owner.slots.to_vec();
    let mut roster = test_completion_roster_v1(1);
    roster.packet_count = 2;
    assert!(owner.reserve(test_dispatch_queue_v1(), roster).is_err());
    assert_eq!(owner.next_generation, 1);
    assert_eq!(&*owner.slots, before);
    assert!(owner.recipe_queue.is_none());
}

#[test]
fn scaled_observation_has_separate_domain_and_full_width_slot_identity() {
    let account = account(payload());
    let mut owner = scaled(&account);
    let mut observations = Vec::new();
    for generation in 1..=1024 {
        let identity = owner
            .reserve(
                test_dispatch_queue_v1(),
                test_completion_roster_v1(generation),
            )
            .unwrap();
        let completion = test_completion_occurrence_v1(generation);
        owner.mark_published(identity, completion).unwrap();
        let digest =
            r66_retained_published_occurrence_observation_v1(&owner, identity, completion).unwrap();
        let mut expected = Sha256::new();
        expected.update(b"fe2o3.qualification1024.retained-dispatch.v1\0");
        expected.update(1024_u16.to_le_bytes());
        expected.update(completion.roster_sha256);
        expected.update(identity.recipe_occurrence.to_le_bytes());
        expected.update(identity.slot_index.to_le_bytes());
        expected.update(identity.slot_generation.to_le_bytes());
        expected.update(identity.dispatch_generation.to_le_bytes());
        assert_eq!(digest, <[u8; 32]>::from(expected.finalize()));
        observations.push(digest);
    }
    assert_eq!(
        observations
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1024
    );
}

#[test]
fn scaled_generation_and_slot_exhaustion_do_not_wrap_or_refund_live_storage() {
    let account = account(payload());
    let mut owner = DispatchGenerationOwnerV1::with_capacity(
        u64::MAX - 1,
        FixedDispatchCapacityProfileV1::Qualification1024,
        Some(&account),
    )
    .unwrap();
    let identity = owner
        .reserve(
            test_dispatch_queue_v1(),
            test_completion_roster_v1(u64::MAX - 1),
        )
        .unwrap();
    owner.cancel_epoch(identity).unwrap();
    let usage = account.usage();
    let before = owner.slots.to_vec();
    assert!(matches!(
        owner.reserve(
            test_dispatch_queue_v1(),
            test_completion_roster_v1(u64::MAX)
        ),
        Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
    ));
    assert_eq!(&*owner.slots, before);
    assert_eq!(account.usage(), usage);
    drop(owner);
    let mut owner = scaled(&account);
    for slot in owner.slots.iter_mut() {
        slot.slot_generation = u64::MAX;
    }
    assert!(matches!(
        owner.reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1)),
        Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
    ));
    assert_eq!(owner.next_generation, 1);
    assert_eq!(account.usage(), usage);
}

#[test]
fn scaled_replacement_reserves_independent_table_before_changing_predecessor() {
    let account = account(2 * payload());
    let mut predecessor = scaled(&account);
    let identity = predecessor
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1))
        .unwrap();
    let occurrence = test_completion_occurrence_v1(1);
    predecessor.mark_published(identity, occurrence).unwrap();
    predecessor.complete_epoch(identity, occurrence).unwrap();
    predecessor.recycle_epoch(identity, occurrence).unwrap();
    let generation = predecessor.returned_generation().unwrap();
    let mut replacement = DispatchGenerationOwnerV1::after_recycled_with_capacity(
        generation,
        FixedDispatchCapacityProfileV1::Qualification1024,
        Some(&account),
    )
    .unwrap();
    assert_eq!(
        account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        2 * payload()
    );
    assert_eq!(replacement.next_generation, 2);
    assert_ne!(replacement.recipe_occurrence, predecessor.recipe_occurrence);
    assert_eq!(replacement.slots.len(), 1024);
    let before = account.usage();
    assert!(
        DispatchGenerationOwnerV1::after_detached_with_capacity(
            generation,
            FixedDispatchCapacityProfileV1::Qualification1024,
            Some(&account)
        )
        .is_err()
    );
    assert_eq!(predecessor.returned_generation().unwrap(), 1);
    assert_eq!(replacement.next_generation, 2);
    assert_eq!(account.usage(), before);
    drop(predecessor);
    for generation in 2..=1025 {
        replacement
            .reserve(
                test_dispatch_queue_v1(),
                test_completion_roster_v1(generation),
            )
            .unwrap();
    }
    assert!(
        replacement
            .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1026))
            .is_err()
    );
    drop(replacement);
    let resumed = DispatchGenerationOwnerV1::after_detached_with_capacity(
        1,
        FixedDispatchCapacityProfileV1::Qualification1024,
        Some(&account),
    )
    .unwrap();
    assert_eq!(resumed.next_generation, 2);
    assert_eq!(resumed.slots.len(), 1024);
    drop(resumed);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn recipe_occurrence_exhaustion_refunds_the_allocated_scaled_table() {
    let account = account(payload());
    let before = account.usage();
    assert!(matches!(
        DispatchGenerationOwnerV1::with_capacity_and_occurrence(
            1,
            FixedDispatchCapacityProfileV1::Qualification1024,
            Some(&account),
            || {
                assert_eq!(
                    account
                        .usage()
                        .used
                        .get(ResourceKindV1::ControlResidentBytes),
                    payload()
                );
                Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
            }
        ),
        Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
    ));
    assert_eq!(account.usage(), before);
}
