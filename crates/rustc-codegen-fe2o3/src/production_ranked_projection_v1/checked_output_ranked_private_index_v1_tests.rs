use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn row(value: u32) -> Definition {
    Definition {
        value,
        block: value.wrapping_add(8),
        operation: value.wrapping_add(19),
    }
}

fn local(value: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(fe2o3_pliron::ProductionRankedValueIdV1::new(value))
}

#[test]
fn temporary_numeric_order_has_literal_exact_and_one_under_work() {
    // Two numeric rows only, no source or owner admission. Scratch is already
    // allocated here: this is the sorting component, not construction/storage.
    // Copy2 + 4*(histogram256 + count12 + prefix768 + scatter14 + swap1)
    // + adjacent comparison2 = 4208. The final two-unit batch is atomic.
    for limit in [4208, 4207] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let mut rows = vec![row(u32::MAX), row(0)];
        let mut scratch = Vec::with_capacity(2);
        let result = order(&mut rows, &mut scratch, &mut budget);
        if limit == 4208 {
            result.unwrap();
            assert_eq!(
                rows.iter().map(|row| row.value).collect::<Vec<_>>(),
                [0, u32::MAX]
            );
            assert_eq!(budget.work(), 4208);
        } else {
            assert!(matches!(
                result,
                Err(ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(
                    Resource::Work(error)
                ))) if error.actual() == 4208 && error.limit() == 4207
            ));
            assert_eq!(budget.work(), 4206);
        }
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn temporary_numeric_lookup_has_literal_bounds_and_repeated_history() {
    // One local classification; each iteration has a guard then a five-unit
    // middle/lookup/compare batch, with one extra increment for a larger key.
    for (rows, key, cost, found) in [
        (vec![], 9, 2, false),
        (vec![row(9)], 9, 7, true),
        (vec![row(9)], 8, 8, false),
        (vec![row(9)], 10, 9, false),
    ] {
        let mut work = Work::new(cost);
        let mut budget = Budget::new(&mut work, 0);
        let result = find(&rows, local(key), &mut budget);
        if found {
            assert_eq!(result.unwrap(), (17, 28));
        } else {
            assert!(matches!(
                result,
                Err(ProjectionError::Incomplete(
                    "checked output private ranked definition is absent"
                ))
            ));
        }
        assert_eq!(budget.work(), cost);
    }
    let mut work = Work::new(13);
    let mut budget = Budget::new(&mut work, 0);
    assert_eq!(find(&[row(9)], local(9), &mut budget).unwrap(), (17, 28));
    assert!(matches!(
        find(&[row(9)], local(9), &mut budget),
        Err(ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(
            Resource::Work(error)
        ))) if error.actual() == 14 && error.limit() == 13
    ));
    assert_eq!(budget.work(), 9);
}
