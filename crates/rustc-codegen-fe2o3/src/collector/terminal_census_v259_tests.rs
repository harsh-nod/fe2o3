use super::*;
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::{SemanticMirLimitsV1, SemanticMirResourceV1};

fn leave_work(work: &mut SourceClosureWorkV1, remaining: u64) {
    let limit = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    work.charge(usize::try_from(limit - work.validation_work_for_test() - remaining).unwrap())
        .unwrap();
}

#[test]
fn only_the_three_tile_rules_change_collected_body_counts() {
    let mut census = CollectedTileTerminalCensusV259::empty();
    let mut work = SourceClosureWorkV1::default();
    for rule in [
        Rule::Expand(Expansion::ContextIssue),
        Rule::Expand(Expansion::WorkgroupDerive),
        Rule::Expand(Expansion::ThreadIndex1d),
        Rule::Reject(TrustedDeviceItem::KernelContextIssue),
    ] {
        census.record(rule, &mut work).unwrap();
        assert!(!census.has_tile_operations());
    }
    assert_eq!(work.validation_work_for_test(), 4);
    for (expansion, kind) in [
        (
            Expansion::MaskedTileLoadU32,
            CollectedTileTerminalKindV259::MaskedLoad,
        ),
        (
            Expansion::MaskedTileIntoFragmentU32,
            CollectedTileTerminalKindV259::IntoFragment,
        ),
        (
            Expansion::LaneFragmentIntoPartsU32,
            CollectedTileTerminalKindV259::IntoParts,
        ),
    ] {
        census.record(Rule::Expand(expansion), &mut work).unwrap();
        assert_eq!(census.call_occurrences(kind), 1);
    }
    census
        .record(Rule::Expand(Expansion::MaskedTileLoadU32), &mut work)
        .unwrap();
    assert_eq!(census.calls, [2, 1, 1]);
    assert!(census.has_tile_operations());
    assert_eq!(work.validation_work_for_test(), 12);
}

#[test]
fn terminal_census_work_is_cumulative_exact_and_one_short() {
    for (rule, cost, changed) in [
        (Rule::Expand(Expansion::ContextIssue), 1, false),
        (Rule::Expand(Expansion::MaskedTileLoadU32), 2, true),
    ] {
        for remaining in [cost - 1, cost] {
            let mut work = SourceClosureWorkV1::default();
            work.charge(17).unwrap();
            leave_work(&mut work, remaining);
            let before = work.validation_work_for_test();
            let mut census = CollectedTileTerminalCensusV259::empty();
            let result = census.record(rule, &mut work);
            assert_eq!(result.is_ok(), remaining == cost);
            assert_eq!(census.has_tile_operations(), changed && remaining == cost);
            assert_eq!(work.validation_work_for_test(), before + cost);
            if let Err(error) = result {
                assert!(error.to_string().contains("ValidationWork"));
                assert_eq!(census.calls, [0; 3]);
            }
        }
    }
}

#[test]
fn terminal_census_overflow_preserves_original_counts_and_paid_work() {
    let mut census = CollectedTileTerminalCensusV259 {
        calls: [u64::MAX, 7, 9],
    };
    let mut work = SourceClosureWorkV1::default();
    work.charge(17).unwrap();
    let error = census
        .record(Rule::Expand(Expansion::MaskedTileLoadU32), &mut work)
        .unwrap_err();
    assert!(error.to_string().contains("count overflowed"));
    assert_eq!(census.calls, [u64::MAX, 7, 9]);
    assert_eq!(work.validation_work_for_test(), 19);
}

#[test]
fn borrowed_census_query_preserves_identity_and_exact_account_headers() {
    type Census = CollectedTileTerminalCensusV259;
    let census = Census::empty();
    for (work_extra, storage_extra) in [(0, 0), (1, 0), (0, 1)] {
        let mut work = Work::new(17 + Census::QUERY_WORK - work_extra);
        let mut budget = Budget::new(&mut work, 23 + Census::QUERY_STORAGE - storage_extra);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(23).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&budget);
        let result = census.borrow_on_account(&mut budget);
        assert_eq!(result.is_ok(), work_extra == 0 && storage_extra == 0);
        if let Ok(view) = result {
            assert!(std::ptr::eq(view, &census));
            assert!(!view.has_tile_operations());
        }
        assert_eq!(std::ptr::from_ref(&budget), slot);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(
            budget.work(),
            if work_extra == 0 {
                17 + Census::QUERY_WORK
            } else {
                17
            }
        );
        assert_eq!(
            budget.storage(),
            if work_extra == 0 && storage_extra == 0 {
                23 + Census::QUERY_STORAGE
            } else {
                23
            }
        );
        if work_extra != 0 {
            assert_eq!(budget.failed_work(), Some(17 + Census::QUERY_WORK));
        }
        if storage_extra != 0 {
            assert_eq!(budget.failed_storage(), Some(23 + Census::QUERY_STORAGE));
        }
        if work_extra != 0 || storage_extra != 0 {
            let prefix = (
                budget.work(),
                budget.storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            assert!(census.borrow_on_account(&mut budget).is_err());
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                prefix,
            );
        }
    }
}
