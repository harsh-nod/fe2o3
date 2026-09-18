use super::*;
use crate::{NativeRankedStagingCommitmentV1, NativeSourceReplayErrorV1 as E};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

#[test]
fn erased_staging_is_original_root_qualified_and_never_a_signature() {
    for calls in [&[1][..], &[2, 2][..]] {
        let source = produced_erased_owner(UnitCase::Initializer, calls);
        let floor = FLOOR + source.retained_storage_floor_v1();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        for ordinal in 0..calls.len() {
            let root = source
                .original_source()
                .semantic_ssa()
                .source_semantic()
                .roots()[ordinal];
            let before = budget.work();
            let (rows, receipt) = source
                .ranked_staging_commitments_v1(ordinal, root.index(), &mut budget)
                .unwrap();
            assert!(
                rows.is_empty(),
                "this fixture has no approved signed execution"
            );
            assert_eq!(budget.work() - before, 13);
            assert_eq!(
                receipt.retained_storage(),
                std::mem::size_of::<Vec<NativeRankedStagingCommitmentV1>>()
            );
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            drop(rows);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert!(matches!(
                source.ranked_staging_commitments_v1(ordinal, u32::MAX, &mut budget),
                Err(E::Mismatch("retained erased staging root"))
            ));
        }
        assert!(matches!(
            source.ranked_staging_commitments_v1(calls.len(), 0, &mut budget),
            Err(E::Mismatch("retained erased staging root"))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn erased_staging_exact_work_header_and_full_source_floor() {
    let source = produced_erased_owner(UnitCase::ScalarSlot, &[1]);
    let floor = source.retained_storage_floor_v1();
    let root = source
        .original_source()
        .semantic_ssa()
        .source_semantic()
        .roots()[0]
        .index();
    let header = std::mem::size_of::<Vec<NativeRankedStagingCommitmentV1>>();
    for (limit, cap) in [
        (13, floor + header),
        (12, floor + header),
        (13, floor + header - 1),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, cap);
        budget.reserve_storage(floor).unwrap();
        let result = source.ranked_staging_commitments_v1(0, root, &mut budget);
        if limit == 12 {
            assert!(
                matches!(result, Err(E::Resource(Resource::Work(e))) if e.limit()==12 && e.actual()==13)
            );
            assert_eq!(budget.work(), 10);
        } else if cap < floor + header {
            assert!(
                matches!(result, Err(E::Resource(Resource::Storage(e))) if e.limit()==cap && e.actual()==floor+header)
            );
            assert_eq!(budget.work(), 13);
        } else {
            let (rows, receipt) = result.unwrap();
            assert!(rows.is_empty());
            assert_eq!(receipt.retained_storage(), header);
            assert_eq!(budget.work(), 13);
            budget.reserve_storage(header).unwrap();
            drop(rows);
            budget.release_storage(header).unwrap();
        }
        assert_eq!(budget.storage(), floor);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
    let mut budget = ArgumentBudgetV1::new(&mut work, floor);
    budget.reserve_storage(floor - 1).unwrap();
    assert!(matches!(
        source.ranked_staging_commitments_v1(0, root, &mut budget),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.work(), 6);
    assert_eq!(budget.storage(), floor - 1);
}
