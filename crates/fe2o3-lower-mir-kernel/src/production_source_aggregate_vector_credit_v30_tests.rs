#[test]
fn aggregate_vector_credit_retains_empty_constructor_envelopes() {
    fn check<T>() {
        let header =
            size_of::<Vec<T>>() + 2 * size_of::<Result<Vec<T>, ProductionSemanticKirErrorV1>>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + header);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let rows = source_reference_emission_vec_v29::<T>(0, &mut budget).unwrap();
        assert_eq!(rows.capacity(), 0);
        let credit = aggregate_vector_credit_v30(&rows).unwrap();
        assert_eq!(credit, header);
        assert!(credit > 0);
        assert_eq!(budget.storage(), MODULE_FLOOR + credit);
        drop(rows);
        budget.release_storage(credit).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
    check::<u8>();
    check::<AggregateOperationV30>();
    check::<ProductionMixedRuntimeOccurrenceV26>();
}

#[test]
fn aggregate_vector_credit_covers_actual_capacity_and_exact_storage_limit() {
    for count in [1, 7, 128] {
        let header = size_of::<Vec<[u64; 3]>>()
            + 2 * size_of::<Result<Vec<[u64; 3]>, ProductionSemanticKirErrorV1>>();
        let limit = MODULE_FLOOR + header + count * size_of::<[u64; 3]>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let mut rows = source_reference_emission_vec_v29::<[u64; 3]>(count, &mut budget).unwrap();
        rows.resize(count, [13, 17, 19]);
        let credit = aggregate_vector_credit_v30(&rows).unwrap();
        assert_eq!(credit, header + rows.capacity() * size_of::<[u64; 3]>());
        assert_eq!(budget.storage(), limit);
        assert_eq!(budget.storage(), MODULE_FLOOR + credit);
        drop(rows);
        budget.release_storage(credit).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn aggregate_vector_credit_one_short_constructor_or_backing_still_refuses() {
    let header =
        size_of::<Vec<u8>>() + 2 * size_of::<Result<Vec<u8>, ProductionSemanticKirErrorV1>>();
    for count in [0, 128] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + header + count - 1);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let error = source_reference_emission_vec_v29::<u8>(count, &mut budget).unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        ));
        let paid = if count == 0 { 0 } else { header };
        assert_eq!(budget.storage(), MODULE_FLOOR + paid);
        budget.release_storage(paid).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn aggregate_vector_credit_replacement_preserves_the_live_successor() {
    let header =
        size_of::<Vec<u64>>() + 2 * size_of::<Result<Vec<u64>, ProductionSemanticKirErrorV1>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1 << 20);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut current = source_reference_emission_vec_v29::<u64>(0, &mut budget).unwrap();
    for count in [7, 32, 3, 0] {
        let mut next = source_reference_emission_vec_v29::<u64>(count, &mut budget).unwrap();
        next.resize(count, count as u64);
        let old_credit = aggregate_vector_credit_v30(&current).unwrap();
        let next_credit = header + next.capacity() * size_of::<u64>();
        assert_eq!(budget.storage(), MODULE_FLOOR + old_credit + next_credit);
        drop(current);
        budget.release_storage(old_credit).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR + next_credit);
        current = next;
    }
    let credit = aggregate_vector_credit_v30(&current).unwrap();
    drop(current);
    budget.release_storage(credit).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
