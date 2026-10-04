use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/defined_helper_semantic_fixture_v1.rs"
    ));
}

fn expected_bytes() -> usize {
    2 * size_of::<SemanticLogicalArgumentMapV1<'_>>() + 4 * size_of::<Option<SemanticLocalIdV1>>()
}

#[test]
fn both_argument_maps_stay_charged_through_success_error_and_unwind() {
    let semantic = fixture::source(vec![fixture::subtract()]);
    let root = semantic.roots()[0];
    for case in 0..3 {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 41 + expected_bytes());
        budget.reserve_storage(41).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_argument_maps_v1(&semantic, root, root, &mut budget, |root, body, budget| {
                assert_eq!(budget.storage(), 41 + expected_bytes());
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(root.source_arguments().len(), 2);
                assert_eq!(body.adjusted_arguments().len(), 2);
                let copied = body.adjusted_arguments().nth(1).unwrap().ordinal();
                match case {
                    0 => Ok(copied),
                    1 => Err(Error::OutputArgument),
                    _ => panic!("consumer unwind while maps remain prepaid"),
                }
            })
        }));
        match case {
            0 => assert_eq!(result.unwrap().unwrap(), 1),
            1 => assert!(matches!(result.unwrap(), Err(Error::OutputArgument))),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), 41);
        assert_eq!(budget.peak_storage(), 41 + expected_bytes());
        assert!(budget.work() > 4);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn work_and_storage_are_admitted_before_constructing_and_lending_maps() {
    let semantic = fixture::source(vec![fixture::subtract()]);
    let root = semantic.roots()[0];
    for (limit, storage) in [
        (4, 41 + expected_bytes()),
        (1_000_000, 40 + expected_bytes()),
    ] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, storage);
        budget.reserve_storage(41).unwrap();
        let result: Result<(), Error> =
            with_argument_maps_v1(&semantic, root, root, &mut budget, |_, _, _| {
                panic!("map consumer reached without prepaid work/storage")
            });
        assert!(matches!(result, Err(Error::Resource(_))));
        assert_eq!(budget.storage(), 41);
        assert_eq!(budget.peak_storage(), 41);
    }
}

#[test]
fn map_scope_cannot_refund_a_substituted_work_account() {
    let semantic = fixture::source(vec![fixture::subtract()]);
    let root = semantic.roots()[0];
    let mut work = Work::new(1_000_000);
    let mut foreign_work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 41 + expected_bytes());
    let mut foreign = Budget::new(&mut foreign_work, 7);
    budget.reserve_storage(41).unwrap();
    foreign.reserve_storage(7).unwrap();
    let result = with_argument_maps_v1(&semantic, root, root, &mut budget, |_, _, budget| {
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 7);
    assert_eq!(foreign.storage(), 41 + expected_bytes());
}
