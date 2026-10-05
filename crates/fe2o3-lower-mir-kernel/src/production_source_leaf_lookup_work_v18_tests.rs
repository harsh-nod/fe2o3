use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

fn expected_headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + size_of::<Result<T, ArgumentResourceV1>>()
            + size_of::<SourceOwnedResultV18<T>>()
    }
    h::<(&mut [SourceScalarLeafLookupV18], &mut ArgumentBudgetV1<'_>)>()
        + h::<(
            &[SourceScalarLeafLookupV18],
            [usize; 3],
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<(&mut SourceLeafLookupWorkV18, usize)>()
        + h::<SourceLeafLookupWorkV18>()
        + h::<&mut SourceLeafLookupWorkV18>()
        + h::<&SourceScalarLeafLookupV18>()
        + 2 * h::<[usize; 3]>()
        + h::<[usize; 8]>()
        + h::<Option<usize>>()
        + h::<u32>()
        + h::<bool>()
        + h::<&[usize]>()
        + h::<Result<(), fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>>()
        + h::<()>()
}

fn expected_search(length: usize) -> usize {
    1 + 10 * ((usize::BITS - length.leading_zeros()) as usize)
}

fn expected_sort(length: usize) -> usize {
    let levels = (usize::BITS - length.leading_zeros()) as usize;
    let exchanges = length.saturating_sub(1);
    1 + exchanges + 19 * levels * (length / 2 + exchanges)
}

fn rows(keys: &[[usize; 3]]) -> Vec<SourceScalarLeafLookupV18> {
    keys.iter()
        .enumerate()
        .map(|(row, &key)| SourceScalarLeafLookupV18 { key, row })
        .collect()
}

// These inert table keys isolate billing; they are not authenticated source
// occurrences. Genuine original/native/formal joins remain in consumer tests.
#[test]
fn source_leaf_address_lookup_preserves_exact_misses_without_address_dependent_debit() {
    struct Count(usize);
    impl PrivateArrayChargeV1 for Count {
        type Error = ProductionSourceOwnedViewErrorV18;
        fn charge_private_array_work(&mut self, amount: usize) -> SourceOwnedResultV18<()> {
            self.0 += amount;
            Ok(())
        }
    }
    let table = rows(&[[0, 0, 20], [1, 7, 0], [2, 0, 0]]);
    let mut below = Count(0);
    let mut above = Count(0);
    assert_eq!(
        private_array_partition_v1(&table, |row| row.key, [0, 0, 10], false, &mut below).unwrap(),
        0
    );
    assert_eq!(
        private_array_partition_v1(&table, |row| row.key, [0, 0, 30], false, &mut above).unwrap(),
        1
    );
    assert_eq!(
        above.0,
        below.0 + 1,
        "the old missing-address branch charged an extra update"
    );
    for (target, expected) in [([0, 0, 10], 0), ([0, 0, 20], 0), ([0, 0, 30], 1)] {
        let required = expected_search(table.len());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(required);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected_headers());
        budget.reserve_storage(expected_headers()).unwrap();
        assert_eq!(
            source_leaf_address_partition_v18(&table, target, &mut budget).unwrap(),
            expected
        );
        assert_eq!(
            (budget.work(), budget.storage()),
            (required, expected_headers())
        );
    }
}

#[test]
fn source_leaf_address_sort_and_search_have_permutation_invariant_exact_work() {
    for count in [0usize, 1, 2, 3, 7, 16, 31, 128] {
        for permutation in 0..4 {
            let mut table: Vec<_> = (0..count)
                .map(|row| SourceScalarLeafLookupV18 {
                    key: [row % 3, row % 2, row * 2],
                    row,
                })
                .collect();
            match permutation {
                1 => table.reverse(),
                2 if count > 0 => table.rotate_left(count / 2),
                3 => table.sort_unstable_by_key(|row| (row.row % 5, row.row)),
                _ => {}
            }
            let required = expected_sort(count);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(required);
            let mut budget = ArgumentBudgetV1::new(&mut work, expected_headers());
            budget.reserve_storage(expected_headers()).unwrap();
            source_leaf_lookup_sort_v18(&mut table, &mut budget).unwrap();
            assert_eq!(budget.work(), required);
            assert!(table.windows(2).all(|pair| pair[0].key < pair[1].key));
            for row in &table {
                assert_eq!(row.key, [row.row % 3, row.row % 2, row.row * 2]);
            }
            for target in [[0, 0, 0], [0, 1, 9], [0, 0, usize::MAX], [3, 0, 0]] {
                let required = expected_search(count);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(required);
                let mut budget = ArgumentBudgetV1::new(&mut work, expected_headers());
                budget.reserve_storage(expected_headers()).unwrap();
                let actual =
                    source_leaf_address_partition_v18(&table, target, &mut budget).unwrap();
                assert_eq!(actual, table.partition_point(|row| row.key < target));
                assert_eq!(
                    (budget.work(), budget.storage()),
                    (required, expected_headers())
                );
            }
        }
    }
}

#[test]
fn source_leaf_address_work_headers_and_one_short_denials_precede_mutation() {
    assert_eq!(
        source_leaf_lookup_work_headers_v18().unwrap(),
        expected_headers()
    );
    for count in [0usize, 1, 3, 32] {
        for sorting in [false, true] {
            let mut table: Vec<_> = (0..count)
                .rev()
                .map(|row| SourceScalarLeafLookupV18 {
                    key: [0, 0, row],
                    row,
                })
                .collect();
            let before: Vec<_> = table.iter().map(|row| (row.key, row.row)).collect();
            let required = if sorting {
                expected_sort(count)
            } else {
                expected_search(count)
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(required - 1);
            let mut budget = ArgumentBudgetV1::new(&mut work, expected_headers());
            budget.reserve_storage(expected_headers()).unwrap();
            let result = if sorting {
                source_leaf_lookup_sort_v18(&mut table, &mut budget)
            } else {
                source_leaf_address_partition_v18(&table, [0, 0, 1], &mut budget).map(|_| ())
            };
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Work(error))) if error.actual() == required && error.limit() == required - 1)
            );
            assert_eq!(budget.failed_work(), Some(required));
            assert_eq!((budget.work(), budget.storage()), (0, expected_headers()));
            assert_eq!(
                table
                    .iter()
                    .map(|row| (row.key, row.row))
                    .collect::<Vec<_>>(),
                before
            );
        }
    }
    assert!(matches!(
        source_leaf_sort_work_v18(usize::MAX),
        Err(ArgumentResourceV1::Arithmetic)
    ));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, expected_headers() - 1);
    assert!(
        matches!(budget.reserve_storage(expected_headers()), Err(ArgumentResourceV1::Storage(error))
        if error.actual() == expected_headers() && error.limit() == expected_headers() - 1)
    );
    assert_eq!((budget.work(), budget.storage()), (0, 0));
    let mut underpaid = SourceLeafLookupWorkV18 { remaining: 0 };
    assert!(matches!(
        underpaid.charge_private_array_work(1),
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
}
