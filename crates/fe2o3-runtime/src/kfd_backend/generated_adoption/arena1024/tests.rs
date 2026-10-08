//! Pre-effect original metadata/profile checks, not a native-positive adapter.

use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};

fn account(bytes: u64) -> ResourceCreditAccountV1 {
    account_with_records(bytes, 16)
}

fn account_with_records(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}

#[test]
fn independent2048_runtime_storage_keeps_exact_count_and_original_family() {
    for (profile, slots) in [
        (GeneratedProfileV1::NativeFillArena1024, 1024),
        (GeneratedProfileV1::IndependentFillArena1024, 1024),
        (GeneratedProfileV1::IndependentFillArena2048, 2048),
    ] {
        // N lower generations plus common/recipe/premise tables and two runtime tables.
        let records = slots + 5;
        let funded = account_with_records(128 << 20, records);
        let storage = ArenaPreallocationV1::new_profile(&funded, 4096, profile).unwrap();
        assert_eq!(funded.usage().retained_records, records);
        assert_eq!(storage.cells.len(), slots);
        assert!(
            storage
                .cells
                .iter()
                .all(|cell| !cell.copied && matches!(cell.receipt, ReceiptV1::Ready))
        );
        assert_eq!(storage.initialization.as_ref().unwrap().len(), 4096);
        let lower = storage.lower.as_ref().unwrap();
        assert!(lower.matches(profile));
        assert_eq!(
            lower.matches(GeneratedProfileV1::IndependentFillArena2048),
            slots == 2048
        );
        assert_eq!(
            lower.matches(GeneratedProfileV1::NativeFillArena1024),
            slots == 1024
        );
        assert!(
            funded
                .usage()
                .used
                .get(ResourceKindV1::ControlResidentBytes)
                > (slots * size_of::<Cell>()) as u64
        );
        let short = account_with_records(128 << 20, records - 1);
        assert!(ArenaPreallocationV1::new_profile(&short, 4096, profile).is_err());
        assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(short.usage().retained_records, 0);
        drop(storage);
        assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(funded.usage().retained_records, 0);
    }
}

#[test]
fn independent2048_runtime_precharge_refuses_without_replenishing_or_native_entry() {
    let short = account(0);
    assert!(
        ArenaPreallocationV1::new_profile(
            &short,
            4096,
            GeneratedProfileV1::IndependentFillArena2048
        )
        .is_err()
    );
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
    let funded = account(128 << 20);
    let before = funded.usage();
    for profile in [
        GeneratedProfileV1::Singleton,
        GeneratedProfileV1::IndependentArenaMember,
        GeneratedProfileV1::NativeFillCohort3,
        GeneratedProfileV1::NativeFillRegistry16,
    ] {
        assert!(ArenaPreallocationV1::new_profile(&funded, 4096, profile).is_err());
        assert_eq!(funded.usage(), before);
    }
    assert!(
        ArenaPreallocationV1::new_profile(&funded, 0, GeneratedProfileV1::IndependentFillArena2048)
            .is_err()
    );
    assert_eq!(funded.usage(), before);
}
