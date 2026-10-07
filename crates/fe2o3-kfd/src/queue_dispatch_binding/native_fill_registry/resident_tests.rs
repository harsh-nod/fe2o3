//! Exact metadata admission; no native receipt or device is fabricated.
use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};

fn bytes<const N: usize>() -> u64 {
    ((N + 1) * 64 * core::mem::size_of::<DispatchEpochSlotV1>()
        + N * core::mem::size_of::<RegistryRecipeV1>()) as u64
}
fn new_account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}
fn check<const N: usize>() {
    let account = new_account(bytes::<N>(), N + 2);
    let original = account.usage();
    let mut storage =
        Gfx942NativeFillResidentRegistryStorageV1::<N>::preallocate(account.clone()).unwrap();
    assert_eq!(storage.recipes.len(), N);
    assert_eq!(account.usage().retained_records, N + 2);
    assert_eq!(
        account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        bytes::<N>()
    );
    storage.require_profile(false).unwrap();
    assert!(storage.require_profile(true).is_err());
    assert!(storage.settled());
    storage.recipes[N - 1].count = N - 1;
    assert!(storage.require_profile(false).is_err());
    assert!(!storage.settled());
    storage.recipes[N - 1].count = N;
    drop(storage);
    assert_eq!(account.usage().used, original.used);
    assert_eq!(account.usage().retained_records, 0);
    for (bytes, records) in [(bytes::<N>() - 1, N + 2), (bytes::<N>(), N + 1)] {
        let account = new_account(bytes, records);
        let before = account.usage();
        assert!(
            Gfx942NativeFillResidentRegistryStorageV1::<N>::preallocate(account.clone()).is_err()
        );
        assert_eq!(account.usage().used, before.used);
        assert_eq!(account.usage().retained_records, 0);
        assert_eq!(account.usage().reserved_records, 0);
    }
}
#[test]
fn resident_registry_metadata_counts_and_one_short_refusals_are_exact() {
    check::<2>();
    check::<3>();
    check::<4>();
    check::<5>();
    check::<6>();
    check::<7>();
    check::<8>();
    check::<9>();
    check::<10>();
    check::<11>();
    check::<12>();
    check::<13>();
    check::<14>();
    check::<15>();
    check::<16>();
}
#[test]
fn resident_registry_outside_closed_count_refuses_before_any_charge() {
    let account = new_account(0, 1);
    let before = account.usage();
    assert!(matches!(
        Gfx942NativeFillResidentRegistryStorageV1::<0>::preallocate(account.clone()),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert!(matches!(
        Gfx942NativeFillResidentRegistryStorageV1::<1>::preallocate(account.clone()),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert!(matches!(
        Gfx942NativeFillResidentRegistryStorageV1::<17>::preallocate(account.clone()),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(account.usage(), before);
}
