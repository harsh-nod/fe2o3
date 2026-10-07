use super::*;
use fe2o3_runtime_model::{QueueGenerationV1, QueueInstanceIdV1};

#[test]
fn cancelled_only_admission_requires_exact_vacant_unpublished_history() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    assert!(owner.ensure_cancelled_only(queue).is_err());
    let epoch = owner.reserve(queue, test_completion_roster_v1(1)).unwrap();
    assert!(owner.ensure_cancelled_only(queue).is_err());
    let mut cancelled = owner.clone();
    cancelled.cancel_epoch(epoch).unwrap();
    cancelled.ensure_cancelled_only(queue).unwrap();
    assert!(cancelled.ensure_pristine().is_err());
    let completion = test_completion_occurrence_v1(1);
    owner.mark_published(epoch, completion).unwrap();
    assert!(owner.ensure_cancelled_only(queue).is_err());
    owner.complete_epoch(epoch, completion).unwrap();
    assert!(owner.ensure_cancelled_only(queue).is_err());
    owner.recycle_epoch(epoch, completion).unwrap();
    assert!(owner.ensure_cancelled_only(queue).is_err());
    let epoch = owner.reserve(queue, test_completion_roster_v1(2)).unwrap();
    owner.cancel_epoch(epoch).unwrap();
    assert!(
        owner.ensure_cancelled_only(queue).is_err(),
        "recycle history stays sticky"
    );

    for mutation in 0..7 {
        let mut invalid = cancelled.clone();
        match mutation {
            0 => invalid.recipe_queue = None,
            1 => invalid.recipe_queue.as_mut().unwrap().id = QueueInstanceIdV1(999),
            2 => invalid.slots[0].slot_generation = 0,
            3 => invalid.predecessor_detached_generation = Some(1),
            4 => invalid.next_generation = 0,
            5 => invalid.next_generation = 1,
            _ => invalid.poisoned = true,
        }
        let before = invalid.clone();
        assert!(
            invalid.ensure_cancelled_only(queue).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(invalid.next_generation, before.next_generation);
        assert_eq!(invalid.recipe_queue, before.recipe_queue);
        assert!(invalid.slots.iter().eq(before.slots.iter()));
    }
}

#[test]
fn cancelled_abort_returns_original_inputs_and_preserves_next_counter() {
    let queue = test_dispatch_queue_v1();
    for next in [1, 8, u64::MAX - 1] {
        let (mut memory, mut owner) = pristine_dispatch_fixture_v1(next);
        cancel_unpublished_fixture_epoch_v1(&mut owner, queue);
        let occurrence = owner.generation.recipe_occurrence;
        assert!(owner.prepare_pristine_abort_v1().is_err());
        let buffers = owner.prepare_cancelled_abort_v1(queue).unwrap();
        let mut abort = owner.begin_unpublished_abort_v1(buffers);
        let before = abort.custody_snapshot_for_test();
        abort.release_controls(&mut memory).unwrap();
        abort
            .custody_snapshot_for_test()
            .assert_preserved_inputs(&before, true);
        let (continuation, data, identities) = abort.into_detached();
        assert_eq!(continuation.next_generation, next + 1);
        assert_eq!(continuation.queue, Some(queue));
        assert!(continuation.matches_queue(queue));
        let mut foreign = queue;
        foreign.generation = QueueGenerationV1(999);
        assert!(!continuation.matches_queue(foreign));
        assert_eq!(data.len(), 5);
        assert_eq!(identities, before.identities);
        assert!(memory.data_is_retained());
        assert_eq!(memory.freed(), 3);
        if next == u64::MAX - 1 {
            let mut prepared = None;
            assert!(matches!(
                continuation.ensure_preallocated_resume::<1>(&mut prepared),
                Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
            ));
            assert!(continuation.preallocate_resume::<1>().is_err());
            assert!(prepared.is_none());
            assert_eq!(continuation.next_generation, u64::MAX);
        } else {
            let mut resumed = continuation.resume().unwrap();
            assert_eq!(resumed.next_generation, next + 1);
            assert_ne!(resumed.recipe_occurrence, occurrence);
            resumed.ensure_pristine().unwrap();
            assert!(resumed.returned_generation().is_err());
            resumed
                .reserve(queue, test_completion_roster_v1(next + 1))
                .unwrap();
        }
    }
}

#[test]
fn cancelled_scaled_continuation_preserves_account_and_preallocation_target() {
    use fe2o3_resource_accounting::{
        ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
    };
    let bytes = host_metadata_table_payload_bytes_v1::<DispatchEpochSlotV1>(1024).unwrap();
    let account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        1,
    )
    .unwrap();
    let capacity = Gfx942FixedDispatchCapacityV1 {
        profile: FixedDispatchCapacityProfileV1::Qualification1024,
        account: Some(account.clone()),
    };
    let mut owner =
        DispatchGenerationOwnerV1::with_capacity(7, capacity.profile, Some(&account)).unwrap();
    let queue = test_dispatch_queue_v1();
    for generation in 7..10 {
        let epoch = owner
            .reserve(queue, test_completion_roster_v1(generation))
            .unwrap();
        owner.cancel_epoch(epoch).unwrap();
    }
    owner.ensure_cancelled_only(queue).unwrap();
    let occurrence = owner.recipe_occurrence;
    let continuation = owner.into_unpublished_continuation();
    assert!(continuation.matches_capacity(&capacity));
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    let mut prepared = continuation.preallocate_resume::<1>().unwrap();
    let mut foreign = queue;
    foreign.id = QueueInstanceIdV1(999);
    prepared = prepared.map(|p| p.for_queue(foreign));
    assert!(
        continuation
            .ensure_preallocated_resume::<1>(&mut prepared)
            .is_err()
    );
    assert!(continuation.resume_preallocated(&mut prepared).is_err());
    assert!(prepared.is_some());
    prepared = prepared.map(|p| p.for_queue(queue));
    continuation
        .ensure_preallocated_resume::<1>(&mut prepared)
        .unwrap();
    let mut resumed = continuation.resume_preallocated(&mut prepared).unwrap();
    assert!(prepared.is_none());
    assert_ne!(resumed.recipe_occurrence, occurrence);
    assert_eq!(resumed.next_generation, 10);
    assert_eq!(resumed.slots.len(), 1024);
    resumed.ensure_pristine().unwrap();
    assert_eq!(
        account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        bytes
    );
    assert_eq!(account.usage().retained_records, 1);
    resumed
        .reserve(queue, test_completion_roster_v1(10))
        .unwrap();
    drop(resumed);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}
