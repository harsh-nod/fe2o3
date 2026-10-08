//! Original epoch metadata controls, not native publication or copy evidence.

use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};

fn storage() -> (ResourceCreditAccountV1, Gfx942NativeFillRegistryStorageV1) {
    let bytes = (5 * 64 * core::mem::size_of::<DispatchEpochSlotV1>()
        + 4 * core::mem::size_of::<RegistryRecipeV1>()) as u64;
    let account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        6,
    )
    .unwrap();
    let storage = Gfx942NativeFillRegistryRepeat2StorageV1::preallocate(account.clone())
        .unwrap()
        .inner;
    (account, storage)
}

fn state(storage: &Gfx942NativeFillRegistryStorageV1) -> [(bool, u8, Option<u64>, u64); 4] {
    std::array::from_fn(|index| {
        let recipe = &storage.recipes[index];
        (
            recipe.accepted,
            recipe.accepted_cycles,
            recipe.copied_generation,
            recipe.generation.next_generation,
        )
    })
}

fn publish(
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
    recipe.accepted_cycles += 1;
    (identity, occurrence)
}

fn recycle(
    recipe: &mut RegistryRecipeV1,
    identity: DispatchEpochIdentityV1,
    occurrence: CompletionBatchOccurrenceV1,
) {
    recipe
        .generation
        .complete_epoch(identity, occurrence)
        .unwrap();
    recipe
        .generation
        .recycle_epoch(identity, occurrence)
        .unwrap();
}

#[test]
fn repeat2_rearm_requires_all_four_original_recycles_and_copy_generations() {
    let (account, mut storage) = storage();
    let charged = account.usage();
    let original = state(&storage);
    assert!(storage.rearm_second_cycle().is_err());
    assert_eq!(state(&storage), original);
    let publications = std::array::from_fn::<_, 4, _>(|i| publish(&mut storage.recipes[i]));
    for (i, publication) in publications.iter().enumerate() {
        let before = state(&storage);
        assert!(storage.rearm_second_cycle().is_err());
        assert_eq!(state(&storage), before);
        recycle(&mut storage.recipes[i], publication.0, publication.1);
        let before = state(&storage);
        assert!(storage.rearm_second_cycle().is_err());
        assert_eq!(state(&storage), before);
        // This is a metadata fixture. Production records this only after copy.
        storage.recipes[i].copied_generation = Some(publications[i].0.dispatch_generation);
    }
    let occurrences =
        std::array::from_fn::<_, 4, _>(|i| storage.recipes[i].generation.recipe_occurrence);
    storage.rearm_second_cycle().unwrap();
    assert_eq!(state(&storage), [(false, 1, None, 2); 4]);
    assert!(storage.settled());
    for (i, occurrence) in occurrences.into_iter().enumerate() {
        assert_eq!(storage.recipes[i].generation.recipe_occurrence, occurrence);
    }
    assert_eq!(account.usage(), charged);
    let rearmed = state(&storage);
    assert!(storage.rearm_second_cycle().is_err());
    assert_eq!(state(&storage), rearmed);
    for (i, old) in publications.into_iter().enumerate() {
        let new = publish(&mut storage.recipes[i]);
        assert_eq!(new.0.dispatch_generation, 2);
        assert!(
            storage.recipes[i]
                .generation
                .validate_published(old.0, old.1)
                .is_err()
        );
        recycle(&mut storage.recipes[i], new.0, new.1);
        storage.recipes[i].copied_generation = Some(2);
    }
    assert!(storage.settled());
    let completed = state(&storage);
    assert!(storage.rearm_second_cycle().is_err());
    assert_eq!(state(&storage), completed);
    assert_eq!(account.usage(), charged);
    drop(storage);
    assert_eq!(account.usage().retained_records, 0);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn repeat2_retry_burn_stale_copy_and_poison_do_not_partially_rearm() {
    let (_, mut storage) = storage();
    let recipe = &mut storage.recipes[3];
    let burned = recipe
        .generation
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1))
        .unwrap();
    recipe.cancel(burned).unwrap();
    for recipe in storage.recipes.iter_mut() {
        let actual = publish(recipe);
        recycle(recipe, actual.0, actual.1);
        recipe.copied_generation = Some(actual.0.dispatch_generation);
    }
    assert_eq!(
        storage.recipes[3].generation.returned_generation().unwrap(),
        2
    );
    storage.recipes[3].copied_generation = Some(1);
    let before = state(&storage);
    assert!(storage.rearm_second_cycle().is_err());
    assert_eq!(state(&storage), before);
    storage.recipes[3].copied_generation = Some(2);
    storage.recipes[2].poison();
    let before = state(&storage);
    assert!(storage.rearm_second_cycle().is_err());
    assert_eq!(state(&storage), before);
}

#[test]
fn repeat2_storage_cannot_satisfy_single_cycle_profile_or_substitute_one_recipe() {
    let (_, mut storage) = storage();
    assert!(storage.require_profile(false).is_err());
    storage.require_profile(true).unwrap();
    storage.recipes[2].repeat2 = false;
    assert!(storage.require_profile(true).is_err());
    assert!(storage.require_profile(false).is_err());
    let before = state(&storage);
    assert!(storage.rearm_second_cycle().is_err());
    assert_eq!(state(&storage), before);
}
