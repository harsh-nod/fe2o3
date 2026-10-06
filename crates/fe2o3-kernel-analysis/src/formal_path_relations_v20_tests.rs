use super::*;
use crate::{PresburgerQueryLimitsV2, with_presburger_queries_v2};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

fn equal(value: u64) -> Fact {
    Fact {
        expression: Affine {
            constant: -i128::from(value),
            coefficient: 1,
        },
        equality: true,
    }
}

fn observe(
    left: &[Fact],
    right: &[Fact],
    work_limit: usize,
    storage_limit: usize,
    count: usize,
) -> (QueryResult<bool>, usize, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(37).unwrap();
    let completed = Cell::new(0);
    let attempts = Cell::new(0);
    let settled = Cell::new(0);
    let outcome = with_presburger_queries_v2(Default::default(), &mut budget, |queries, budget| {
        let floor = budget.storage();
        let mut excluded = true;
        for _ in 0..count {
            attempts.set(attempts.get() + 1);
            let result = exclude(left, right, queries, budget);
            assert_eq!(budget.storage(), floor);
            settled.set(settled.get() + 1);
            excluded &= result?;
            completed.set(completed.get() + 1);
        }
        Ok(excluded)
    });
    assert_eq!(
        attempts.get(),
        settled.get(),
        "post-denial cleanup assertions must finish"
    );
    if outcome.is_ok() {
        assert_eq!(completed.get(), count);
    }
    assert_eq!(budget.storage(), 37);
    (
        outcome,
        budget.work(),
        budget.peak_storage(),
        completed.get(),
    )
}

#[test]
fn relation_inputs_have_independent_fifty_two_work_and_typed_capacity_bounds() {
    for work_limit in [52, 51] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, 100_000);
        let mut credit = InputCredit { bytes: 0 };
        let result = relation(&[equal(0)], &[equal(0)], [1, -1], &mut credit, &mut budget);
        if work_limit == 52 {
            let set = result.unwrap();
            assert_eq!(budget.work(), 52);
            assert_eq!(set.constraints().len(), 3);
            // Three affine coefficient pairs plus the two box-bound pairs.
            let expected = size_of::<Vec<Constraint>>()
                + 3 * size_of::<Constraint>()
                + 5 * (size_of::<Vec<i128>>() + 2 * size_of::<i128>());
            assert_eq!(credit.bytes, expected);
            assert_eq!(budget.storage(), expected);
            assert_eq!(set.domain().lower(), &[0, 0]);
            assert_eq!(set.domain().upper_exclusive(), &[1_i128 << 64; 2]);
            drop(set);
        } else {
            assert!(matches!(
                result,
                Err(PresburgerQueryErrorV2::Resource(Resource::Work(_)))
            ));
            assert!(budget.check_prior_denials_v1().is_err());
        }
        budget.release_storage(credit.bytes).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn full_coordinate_relations_check_both_orders_and_include_u64_max() {
    for value in [0, 1, 19, u64::MAX] {
        assert_eq!(
            observe(&[equal(value)], &[equal(value)], 2_000_000, 100_000, 1).0,
            Ok(true)
        );
    }
    for (left, right) in [(0, 1), (1, 0), (u64::MAX, 0), (0, u64::MAX)] {
        assert_eq!(
            observe(&[equal(left)], &[equal(right)], 2_000_000, 100_000, 1).0,
            Ok(false)
        );
    }
    assert_eq!(observe(&[], &[], 2_000_000, 100_000, 1).0, Ok(false));
}

#[test]
fn relation_inputs_repeat_on_one_ledger_and_restore_only_their_own_backing() {
    let one = observe(&[equal(0)], &[equal(0)], 2_000_000, 100_000, 1);
    let repeated = observe(&[equal(0)], &[equal(0)], 2_000_000, 100_000, 8);
    assert_eq!(one.0, Ok(true));
    assert_eq!(repeated.0, Ok(true));
    assert_eq!(repeated.2, one.2);
    assert!(repeated.1 > one.1);
    let exact = observe(&[equal(0)], &[equal(0)], repeated.1, repeated.2, 8);
    assert_eq!(exact.0, Ok(true));
    let short_work = observe(&[equal(0)], &[equal(0)], repeated.1 - 1, repeated.2, 8);
    match short_work.0 {
        Err(PresburgerQueryErrorV2::Resource(Resource::Work(error))) => {
            assert_eq!(error.limit(), repeated.1 - 1);
            assert!(error.actual() > error.limit());
        }
        Err(PresburgerQueryErrorV2::PriorDenial {
            work: Some(actual),
            storage: None,
        }) => assert!(actual > repeated.1 - 1),
        other => panic!("wrong shared work denial: {other:?}"),
    }
    assert!(short_work.3 < 8);
    let short_storage = observe(&[equal(0)], &[equal(0)], repeated.1, repeated.2 - 1, 8);
    match short_storage.0 {
        Err(PresburgerQueryErrorV2::Resource(Resource::Storage(error))) => {
            assert_eq!(error.limit(), repeated.2 - 1);
            assert!(error.actual() > error.limit());
        }
        Err(PresburgerQueryErrorV2::PriorDenial {
            work: None,
            storage: Some(actual),
        }) => assert!(actual > repeated.2 - 1),
        other => panic!("wrong shared storage denial: {other:?}"),
    }
    assert_eq!(short_storage.3, 0);
}

#[test]
fn relation_input_limit_is_not_an_empty_set_and_foreign_session_refuses_before_allocation() {
    let facts = vec![equal(0); crate::MAX_PRESBURGER_CONSTRAINTS_V1];
    assert_eq!(observe(&facts, &[], 2_000_000, 100_000, 1).0, Ok(false));
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let completed = Cell::new(false);
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |queries, _| {
            let mut foreign_work = Work::new(2_000_000);
            let mut foreign = Budget::new(&mut foreign_work, 100_000);
            foreign.reserve_storage(61).unwrap();
            let error = exclude(&[equal(0)], &[equal(0)], queries, &mut foreign).unwrap_err();
            assert_eq!(
                error,
                PresburgerQueryErrorV2::Resource(Resource::Accounting)
            );
            assert_eq!(foreign.storage(), 61);
            assert_eq!(foreign.work(), 0);
            completed.set(true);
            Ok(())
        },
    );
    assert!(completed.get());
    assert_eq!(
        result,
        Err(PresburgerQueryErrorV2::Resource(Resource::Accounting))
    );
    assert_eq!(budget.storage(), 0);
}
