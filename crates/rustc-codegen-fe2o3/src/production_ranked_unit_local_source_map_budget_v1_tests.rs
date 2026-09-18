// Accounting component tests only; no authenticated ranked candidate is forged.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

const FLOOR: usize = 31;

fn maps(
    capacity: usize,
) -> (
    Vec<ProductionRankedAccessSourceV1>,
    Vec<ProductionRankedExecutableEffectSourceV1>,
) {
    let mut accesses = Vec::with_capacity(capacity);
    let mut effects = Vec::with_capacity(capacity + 3);
    accesses.push(ProductionRankedAccessSourceV1::new(2, Some(1), 0, 0, 8));
    effects.push(ProductionRankedExecutableEffectSourceV1::new(
        1,
        0,
        0,
        9,
        ProductionRankedExecutableEffectOriginV1::GeneratedFromSemanticTerminator,
        [3; 32],
    ));
    effects.push(ProductionRankedExecutableEffectSourceV1::new(
        1,
        1,
        0,
        10,
        ProductionRankedExecutableEffectOriginV1::GeneratedFromSemanticTerminator,
        [4; 32],
    ));
    (accesses, effects)
}

#[test]
fn source_map_conversion_reserves_real_capacities_and_box_overlap() {
    for capacity in [1, 7, 37] {
        let (accesses, effects) = maps(capacity);
        let temporary = std::mem::size_of_val(&accesses)
            + std::mem::size_of_val(&effects)
            + accesses.capacity() * std::mem::size_of::<ProductionRankedAccessSourceV1>()
            + effects.capacity() * std::mem::size_of::<ProductionRankedExecutableEffectSourceV1>();
        let boxed =
            std::mem::size_of_val(accesses.as_slice()) + std::mem::size_of_val(effects.as_slice());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(15);
        let mut budget = Budget::new(&mut work, FLOOR + temporary + boxed);
        budget.reserve_storage(FLOOR).unwrap();
        assert_eq!(
            prepare_source_map_conversion_v1(&accesses, &effects, &mut budget).unwrap(),
            (temporary, boxed)
        );
        assert_eq!(budget.work(), 15);
        assert_eq!(budget.storage(), FLOOR + temporary + boxed);
        let accesses = accesses.into_boxed_slice();
        let effects = effects.into_boxed_slice();
        budget.release_storage(temporary).unwrap();
        assert_eq!(budget.storage(), FLOOR + boxed);
        assert_eq!(
            std::mem::size_of_val(accesses.as_ref()) + std::mem::size_of_val(effects.as_ref()),
            boxed
        );
        // The later input_extra prepayment subtracts boxed once, never twice.
        let container = 101;
        let other_payload = 209;
        let input_extra = container + boxed + other_payload;
        assert_eq!(
            input_extra
                .checked_sub(container)
                .and_then(|n| n.checked_sub(boxed)),
            Some(other_payload)
        );
        drop((accesses, effects));
        budget.release_storage(boxed).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn source_map_conversion_denies_one_short_before_consuming_inputs() {
    let (accesses, effects) = maps(37);
    let temporary = std::mem::size_of_val(&accesses)
        + std::mem::size_of_val(&effects)
        + accesses.capacity() * std::mem::size_of::<ProductionRankedAccessSourceV1>()
        + effects.capacity() * std::mem::size_of::<ProductionRankedExecutableEffectSourceV1>();
    let boxed =
        std::mem::size_of_val(accesses.as_slice()) + std::mem::size_of_val(effects.as_slice());
    for (work_limit, storage_limit) in [
        (14, FLOOR + temporary + boxed),
        (15, FLOOR + temporary + boxed - 1),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(prepare_source_map_conversion_v1(&accesses, &effects, &mut budget).is_err());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!((accesses.len(), effects.len()), (1, 2));
    }
}

#[test]
fn empty_source_maps_charge_only_temporary_headers() {
    let accesses = Vec::new();
    let effects = Vec::new();
    let headers = std::mem::size_of::<Vec<ProductionRankedAccessSourceV1>>()
        + std::mem::size_of::<Vec<ProductionRankedExecutableEffectSourceV1>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(12);
    let mut budget = Budget::new(&mut work, FLOOR + headers);
    budget.reserve_storage(FLOOR).unwrap();
    assert_eq!(
        prepare_source_map_conversion_v1(&accesses, &effects, &mut budget).unwrap(),
        (headers, 0)
    );
    assert_eq!(budget.work(), 12);
    assert_eq!(budget.storage(), FLOOR + headers);
    drop((accesses, effects));
    budget.release_storage(headers).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
