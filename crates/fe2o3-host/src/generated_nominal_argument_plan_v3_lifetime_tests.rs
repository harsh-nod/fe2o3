use super::*;

#[test]
fn nominal_host_v3_released_sibling_baseline_is_refused() {
    with_plan(&[Shape::Usize], "gfx942:xnack-", false, |plan, budget| {
        let initial = budget.storage();
        let (first, first_storage) = plan.bind_usize(0, 3, budget).unwrap();
        reserve(budget, first_storage);
        let (second, second_storage) = plan.bind_usize(0, 7, budget).unwrap();
        reserve(budget, second_storage);
        let backing = NOMINAL_ARGUMENT_REF_STORAGE_V3 + size_of::<[u8; 8]>();
        assert!(first_storage.retained_storage() > backing);
        budget.reserve_storage(backing).unwrap();
        let inputs = [second.as_input()];
        let mut bytes = [0xa5; 8];
        {
            let (packed, storage) = plan.pack(&inputs, &mut bytes, budget).unwrap();
            reserve(budget, storage);
            assert_eq!(packed.as_bytes(), 7u64.to_le_bytes());
            drop(packed);
            budget.release_storage(storage.retained_storage()).unwrap();
        }
        // Deliberately violate the documented inherited-sibling contract.
        drop(first);
        budget
            .release_storage(first_storage.retained_storage())
            .unwrap();
        bytes.fill(0xa5);
        let floor = budget.storage();
        let work = budget.work();
        let peak = budget.peak_storage();
        let failed = budget.failed_storage();
        let ledger = budget.work_ledger_identity_v1();
        let result = plan.pack(&inputs, &mut bytes, budget);
        assert!(matches!(
            result,
            Err(GeneratedNominalPackErrorV3::Resource(Resource::Accounting))
        ));
        assert_eq!(bytes, [0xa5; 8]);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), failed);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(
            budget.work() - work,
            1 + MAX_ARGUMENTS_PER_KERNEL + MAX_PHYSICAL_COMPONENTS_PER_KERNEL + 4
        );
        assert_eq!(
            budget.peak_storage(),
            peak.max(
                floor
                    + size_of::<GeneratedNominalPackedArgumentsV3<'_, '_, '_, '_, '_>>()
                    + NOMINAL_PACK_SCRATCH_STORAGE_V3
            )
        );
        drop(inputs);
        drop(second);
        budget
            .release_storage(second_storage.retained_storage() + backing)
            .unwrap();
        assert_eq!(budget.storage(), initial);
    });
}
