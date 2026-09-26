use super::*;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

fn payload() -> u64 {
    host_metadata_table_payload_bytes_v1::<DispatchEpochSlotV1>(1024).unwrap()
}

fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}

#[test]
fn scaled_preflight_transfers_the_same_table_without_a_second_charge() {
    let account = account(payload(), 1);
    let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
    let mut prepared =
        PreparedDispatchGenerationV1::preallocate::<1>(&capacity, DispatchGenerationSeedV1::Fresh)
            .unwrap();
    let pointer = prepared.as_ref().unwrap().0.slots.as_ptr();
    let usage = account.usage();
    assert_eq!(usage.retained_records, 1);
    assert_eq!(
        usage.used.get(ResourceKindV1::ControlResidentBytes),
        payload()
    );
    PreparedDispatchGenerationV1::ensure_preallocated::<1>(
        &mut prepared,
        &capacity,
        DispatchGenerationSeedV1::Fresh,
    )
    .unwrap();
    assert_eq!(account.usage(), usage);
    assert!(
        account
            .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1))
            .is_err()
    );
    let mut owner = PreparedDispatchGenerationV1::take_for(
        &mut prepared,
        &capacity,
        DispatchGenerationSeedV1::Fresh,
    )
    .unwrap();
    assert!(prepared.is_none());
    assert_eq!(owner.slots.as_ptr(), pointer);
    assert_eq!(account.usage(), usage);
    let epoch = owner
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1))
        .unwrap();
    assert_eq!(epoch.dispatch_generation, 1);
    owner.cancel_epoch(epoch).unwrap();
    assert!(
        PreparedDispatchGenerationV1::take_for(
            &mut prepared,
            &capacity,
            DispatchGenerationSeedV1::Fresh,
        )
        .is_err()
    );
    assert_eq!(account.usage(), usage);
    drop(owner);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(account.usage().retained_records, 0);
}

#[test]
fn scaled_preflight_rejects_wrong_ledger_profile_seed_or_state_without_consumption() {
    let account = account(payload(), 1);
    let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
    let foreign = Gfx942FixedDispatchCapacityV1::qualification_1024(self::account(payload(), 1));
    let mut prepared = PreparedDispatchGenerationV1::preallocate::<1>(
        &capacity,
        DispatchGenerationSeedV1::Recycled(7),
    )
    .unwrap();
    let pointer = prepared.as_ref().unwrap().0.slots.as_ptr();
    let usage = account.usage();
    for (target, seed) in [
        (&foreign, DispatchGenerationSeedV1::Recycled(7)),
        (
            &Gfx942FixedDispatchCapacityV1::default(),
            DispatchGenerationSeedV1::Recycled(7),
        ),
        (&capacity, DispatchGenerationSeedV1::Fresh),
        (&capacity, DispatchGenerationSeedV1::Pristine(7)),
    ] {
        assert!(PreparedDispatchGenerationV1::take_for(&mut prepared, target, seed).is_err());
        assert_eq!(prepared.as_ref().unwrap().0.slots.as_ptr(), pointer);
        assert_eq!(account.usage(), usage);
    }
    prepared.as_mut().unwrap().0.poisoned = true;
    assert!(
        PreparedDispatchGenerationV1::take_for(
            &mut prepared,
            &capacity,
            DispatchGenerationSeedV1::Recycled(7),
        )
        .is_err()
    );
    assert!(prepared.is_some());
    prepared.as_mut().unwrap().0.poisoned = false;
    let epoch = prepared
        .as_mut()
        .unwrap()
        .0
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(8))
        .unwrap();
    assert!(
        PreparedDispatchGenerationV1::take_for(
            &mut prepared,
            &capacity,
            DispatchGenerationSeedV1::Recycled(7),
        )
        .is_err()
    );
    prepared.as_mut().unwrap().0.cancel_epoch(epoch).unwrap();
    assert_eq!(account.usage(), usage);
}

#[test]
fn scaled_preflight_checks_all_seeds_and_refunds_unused_storage() {
    let account = account(payload(), 1);
    let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
    for (seed, next) in [
        (DispatchGenerationSeedV1::Fresh, 1),
        (DispatchGenerationSeedV1::Detached(0), 1),
        (DispatchGenerationSeedV1::Detached(7), 8),
        (DispatchGenerationSeedV1::Recycled(7), 8),
        (DispatchGenerationSeedV1::Pristine(7), 7),
    ] {
        let mut prepared = PreparedDispatchGenerationV1::preallocate::<1>(&capacity, seed).unwrap();
        assert_eq!(prepared.as_ref().unwrap().0.next_generation, next);
        let owner = PreparedDispatchGenerationV1::take_for(&mut prepared, &capacity, seed).unwrap();
        assert_eq!(owner.next_generation, next);
        drop(owner);
        assert_eq!(account.usage().retained_records, 0);
    }
    for seed in [
        DispatchGenerationSeedV1::Recycled(0),
        DispatchGenerationSeedV1::Recycled(u64::MAX),
        DispatchGenerationSeedV1::Pristine(0),
        DispatchGenerationSeedV1::Pristine(u64::MAX),
    ] {
        let before = account.usage();
        assert!(PreparedDispatchGenerationV1::preallocate::<1>(&capacity, seed).is_err());
        assert_eq!(account.usage(), before);
    }
    let prepared =
        PreparedDispatchGenerationV1::preallocate::<1>(&capacity, DispatchGenerationSeedV1::Fresh)
            .unwrap();
    assert_eq!(account.usage().retained_records, 1);
    drop(prepared);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn scaled_preflight_byte_record_and_packet_failures_leave_the_ledger_unchanged() {
    for record_exhaustion in [false, true] {
        let account = account(
            if record_exhaustion {
                payload() + 1
            } else {
                payload() - 1
            },
            1,
        );
        let competing = record_exhaustion.then(|| {
            account
                .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1))
                .unwrap()
        });
        let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
        let before = account.usage();
        assert!(matches!(
            PreparedDispatchGenerationV1::preallocate::<1>(
                &capacity,
                DispatchGenerationSeedV1::Fresh,
            ),
            Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity { .. })
        ));
        assert_eq!(account.usage(), before);
        drop(competing);
    }
    let account = account(payload(), 1);
    let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
    let before = account.usage();
    assert!(
        PreparedDispatchGenerationV1::preallocate::<2>(&capacity, DispatchGenerationSeedV1::Fresh,)
            .is_err()
    );
    assert_eq!(account.usage(), before);
}

#[test]
fn default_preflight_preserves_late_allocation_and_generation_failure() {
    let capacity = Gfx942FixedDispatchCapacityV1::default();
    let mut prepared = PreparedDispatchGenerationV1::preallocate::<3>(
        &capacity,
        DispatchGenerationSeedV1::Recycled(u64::MAX),
    )
    .unwrap();
    assert!(prepared.is_none());
    assert!(
        PreparedDispatchGenerationV1::take_for(
            &mut prepared,
            &capacity,
            DispatchGenerationSeedV1::Recycled(u64::MAX),
        )
        .is_err()
    );
    let owner = PreparedDispatchGenerationV1::take_for(
        &mut prepared,
        &capacity,
        DispatchGenerationSeedV1::Detached(0),
    )
    .unwrap();
    assert_eq!(
        owner.capacity_profile,
        FixedDispatchCapacityProfileV1::Default64
    );
    assert_eq!(owner.slots.len(), 64);
    assert!(owner.slots.account().is_none());
}
