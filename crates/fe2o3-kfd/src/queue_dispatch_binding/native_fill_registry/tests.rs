//! Metadata and exact pre-copy gates only; no native receipt is fabricated.

use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};

fn payload() -> u64 {
    (5 * 64 * core::mem::size_of::<DispatchEpochSlotV1>()
        + 4 * core::mem::size_of::<RegistryRecipeV1>()) as u64
}

fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}

fn storage() -> Gfx942NativeFillRegistryStorageV1 {
    Gfx942NativeFillRegistryStorageV1::preallocate(account(payload(), 6)).unwrap()
}

fn publish_metadata(
    recipe: &mut RegistryRecipeV1,
) -> (DispatchEpochIdentityV1, CompletionBatchOccurrenceV1) {
    let generation = recipe.generation.next_generation;
    let identity = recipe
        .generation
        .reserve(
            test_dispatch_queue_v1(),
            test_completion_roster_v1(generation),
        )
        .unwrap();
    let occurrence = test_completion_occurrence_v1(generation);
    recipe
        .generation
        .mark_published(identity, occurrence)
        .unwrap();
    recipe.accepted = true;
    (identity, occurrence)
}

fn premises() -> Vec<RetainedDataPremiseV1> {
    [4, 148, 260, 516]
        .map(|bytes| RetainedDataPremiseV1 {
            layout: Gfx942FixedDispatchDataLayoutV1 {
                kind: Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                requested_bytes: bytes,
                alignment: 4096,
            },
            role_identity: [1; 32],
            valid_bytes: bytes,
            effect: Some(DeviceDataEffectV1::WriteOnly),
            initialized_content: None,
            fully_initialized: false,
            writable_ranges: Box::new([CompletedWritableRangeV1 {
                offset: 0,
                byte_len: bytes,
            }]),
            completed_snapshots: Box::new([]),
        })
        .into()
}

#[test]
fn registry_four_metadata_is_prepaid_once_and_refunds_only_after_disposal() {
    let account = account(payload(), 6);
    let before = account.usage();
    let storage = Gfx942NativeFillRegistryStorageV1::preallocate(account.clone()).unwrap();
    let charged = account.usage();
    assert_eq!(charged.retained_records, 6);
    assert_eq!(
        charged.used.get(ResourceKindV1::ControlResidentBytes),
        payload()
    );
    assert!(storage.settled());
    for (index, recipe) in storage.recipes.iter().enumerate() {
        assert_eq!(recipe.index, index);
        assert_eq!(recipe.generation.slots.len(), 64);
        assert!(
            storage.recipes[..index]
                .iter()
                .all(|earlier| earlier.generation.recipe_occurrence
                    != recipe.generation.recipe_occurrence)
        );
    }
    assert_eq!(account.usage(), charged);
    drop(storage);
    assert_eq!(account.usage().used, before.used);
    assert_eq!(account.usage().retained_records, 0);
}

#[test]
fn registry_four_metadata_partial_budget_or_record_refusal_restores_all_credits() {
    for (bytes, records) in [(payload() - 1, 6), (payload(), 5), (0, 6), (payload(), 1)] {
        let account = account(bytes, records);
        let before = account.usage();
        assert!(Gfx942NativeFillRegistryStorageV1::preallocate(account.clone()).is_err());
        assert_eq!(account.usage().used, before.used);
        assert_eq!(account.usage().reserved_records, 0);
        assert_eq!(account.usage().retained_records, 0);
    }
}

#[test]
fn registry_four_recipe_three_read_gate_is_independent_of_other_publications() {
    let mut storage = storage();
    let occurrences =
        std::array::from_fn::<_, 4, _>(|index| publish_metadata(&mut storage.recipes[index]));
    let premises = premises();
    assert!(!storage.settled());
    for recipe in storage.recipes.iter() {
        assert!(
            recipe
                .read_request(&premises, premises[recipe.index].valid_bytes as usize)
                .is_err()
        );
    }
    let recipe = &mut storage.recipes[3];
    recipe
        .generation
        .complete_epoch(occurrences[3].0, occurrences[3].1)
        .unwrap();
    assert!(recipe.read_request(&premises, 516).is_err());
    recipe
        .generation
        .recycle_epoch(occurrences[3].0, occurrences[3].1)
        .unwrap();
    let request = recipe.read_request(&premises, 516).unwrap();
    assert_eq!(
        (
            request.data_index,
            request.dispatch_generation,
            request.offset,
            request.byte_len
        ),
        (3, 1, 0, 516)
    );
    assert!(recipe.read_request(&premises, 515).is_err());
    assert!(!storage.settled());
    for index in 0..3 {
        storage.recipes[index]
            .generation
            .validate_published(occurrences[index].0, occurrences[index].1)
            .unwrap();
        assert!(
            storage.recipes[index]
                .read_request(&premises, premises[index].valid_bytes as usize)
                .is_err()
        );
        storage.recipes[index]
            .generation
            .complete_epoch(occurrences[index].0, occurrences[index].1)
            .unwrap();
        storage.recipes[index]
            .generation
            .recycle_epoch(occurrences[index].0, occurrences[index].1)
            .unwrap();
    }
    assert!(storage.settled());
}

#[test]
fn registry_four_cross_recipe_and_generation_refusals_leave_other_cells_unchanged() {
    let mut storage = storage();
    let occurrences =
        std::array::from_fn::<_, 4, _>(|index| publish_metadata(&mut storage.recipes[index]));
    for left in 0..4 {
        for right in 0..4 {
            if left == right {
                continue;
            }
            assert!(
                storage.recipes[left]
                    .generation
                    .validate_published(occurrences[right].0, occurrences[right].1)
                    .is_err()
            );
            assert!(
                storage.recipes[left]
                    .generation
                    .complete_epoch(occurrences[right].0, occurrences[right].1)
                    .is_err()
            );
            storage.recipes[left]
                .generation
                .validate_published(occurrences[left].0, occurrences[left].1)
                .unwrap();
        }
    }
    assert!(!storage.settled());
    storage.recipes[1].poison();
    assert!(!storage.settled());
    for index in [0, 2, 3] {
        storage.recipes[index]
            .generation
            .validate_published(occurrences[index].0, occurrences[index].1)
            .unwrap();
    }
}

#[test]
fn registry_four_cancel_before_publication_burns_only_original_recipe_generation() {
    let mut storage = storage();
    let recipe = &mut storage.recipes[2];
    let original = recipe
        .generation
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1))
        .unwrap();
    assert!(!recipe.accepted);
    recipe.cancel(original).unwrap();
    assert!(recipe.cancel(original).is_err());
    assert_eq!(recipe.generation.next_generation, 2);
    assert!(storage.settled());
    for index in [0, 1, 3] {
        assert_eq!(storage.recipes[index].generation.next_generation, 1);
    }
    let (identity, occurrence) = publish_metadata(&mut storage.recipes[2]);
    assert_eq!(identity.dispatch_generation, 2);
    storage.recipes[2]
        .generation
        .validate_published(identity, occurrence)
        .unwrap();
    assert!(!storage.settled());
}
