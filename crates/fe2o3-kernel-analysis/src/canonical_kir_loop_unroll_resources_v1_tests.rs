use super::tests::{S, TestRows, W, admit, module};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn bounded_unroll_checker_exact_scope_and_pair_headers_refuse_before_inventory() {
    let (a, sa) = admit(&module(7));
    let (b, sb) = admit(&module(7));
    let rows = TestRows::identity(&a);
    let floor = sa + sb + rows.bytes() + 17;
    let sibling = [0xa7u8; 17];
    let scope = size_of::<Meter<'_, '_>>();
    let pair = size_of::<CheckedCanonicalKirLoopUnrollPairV1<'_, '_, '_>>();
    for (limit, actual, work, peak) in [
        (floor + scope - 1, floor + scope, 0, floor),
        (
            floor + scope + pair - 1,
            floor + scope + pair,
            11,
            floor + scope,
        ),
    ] {
        let mut w = Work::new(W);
        let mut budget = Budget::new(&mut w, limit);
        budget.reserve_storage(floor).unwrap();
        let Err(Error::Resource(Resource::Storage(e))) = check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits::default(),
            &mut budget,
        ) else {
            panic!("strict first headers");
        };
        assert_eq!((e.actual(), e.limit()), (actual, limit));
        assert_eq!(
            (
                budget.storage(),
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (floor, work, peak, Some(actual))
        );
    }
    assert_eq!(sibling, [0xa7; 17]);
}
#[test]
fn bounded_unroll_checker_exact_work_storage_and_final_work_short() {
    let (a, sa) = admit(&module(7));
    let (b, sb) = admit(&module(7));
    let rows = TestRows::identity(&a);
    let floor = sa + sb + rows.bytes();
    let run = |wcap, scap| {
        let mut work = Work::new(wcap);
        let (result, w, p, f) = {
            let mut budget = Budget::new(&mut work, scap);
            budget.reserve_storage(floor).unwrap();
            let result = match check_canonical_kir_loop_unroll_pair_v1(
                &a,
                &b,
                rows.view(),
                Limits::default(),
                &mut budget,
            ) {
                Ok((pair, receipt)) => {
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert!(!pair.grants_authority());
                    Ok(receipt.retained_storage())
                }
                Err(e) => Err(e),
            };
            let result = result.map(|bytes| budget.release_storage(bytes).unwrap());
            assert_eq!(budget.storage(), floor);
            (
                result,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
            )
        };
        (result, w, p, work.failed_work(), f)
    };
    let full = run(W, S);
    assert!(full.0.is_ok());
    let exact = run(full.1, full.2);
    assert!(exact.0.is_ok());
    assert_eq!(
        (exact.1, exact.2, exact.3, exact.4),
        (full.1, full.2, None, None)
    );
    let short = run(full.1 - 1, full.2);
    let Err(Error::Resource(Resource::Work(e))) = short.0 else {
        panic!("final checker transfer");
    };
    assert_eq!(
        (e.actual(), e.limit(), short.1, short.2, short.3, short.4),
        (full.1, full.1 - 1, full.1 - 1, full.2, Some(full.1), None)
    );
}
#[test]
fn bounded_unroll_checker_invalid_limit_and_exact_origin_cap_are_typed() {
    let (a, sa) = admit(&module(7));
    let (b, sb) = admit(&module(7));
    let rows = TestRows::identity(&a);
    let floor = sa + sb + rows.bytes();
    let mut w = Work::new(W);
    let mut budget = Budget::new(&mut w, S);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits {
                max_iterations: 9,
                ..Limits::default()
            },
            &mut budget
        ),
        Err(Error::InvalidIterationLimit(9))
    ));
    assert!(matches!(
        check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits {
                max_origin_rows: 3,
                ..Limits::default()
            },
            &mut budget
        ),
        Err(Error::OutputLimit {
            kind: "origin rows",
            actual: 4,
            limit: 3
        })
    ));
    assert_eq!(budget.storage(), floor);
}
