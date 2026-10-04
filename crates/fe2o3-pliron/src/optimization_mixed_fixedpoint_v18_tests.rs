use super::*;
use crate::neutral_optimization_v1::storage_v18::tests::{LIMITS, SPACE, WORK, fixture, input};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn mixed_fixedpoint_report_checker_rejects_terminal_order_epoch_work_and_growth_corruption() {
    let source = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(source.storage).unwrap();
    let value =
        crate::optimize_neutral_kernel_ir_mixed_fixedpoint_v18(&source.owner, LIMITS, &mut budget)
            .unwrap();
    budget
        .reserve_storage(value.storage().retained_storage())
        .unwrap();
    let report = value.report();
    assert!(report.passes.len() >= 10);
    assert!(crate::fixed_policy_v3::validate_fixedpoint_report(report).is_ok());
    for fault in 0..8 {
        let mut corrupt = report.clone();
        match fault {
            0 => corrupt.passes.truncate(corrupt.passes.len() - 5),
            1 => corrupt.passes.swap(0, 1),
            2 => {
                corrupt.passes[0].input_epoch =
                    corrupt.passes[0].input_epoch.checked_next().unwrap()
            }
            3 => corrupt.passes[0].output_graph_work = corrupt.passes[0].input_graph_work + 1,
            4 => corrupt.passes[0].work_units += 1,
            5 => {
                let terminal = corrupt.passes[corrupt.passes.len() - 5..].to_vec();
                corrupt.passes.extend_from_slice(&terminal);
            }
            6 => corrupt.passes.last_mut().unwrap().changed = true,
            _ => corrupt.passes.clear(),
        }
        assert!(
            matches!(
                crate::fixed_policy_v3::validate_fixedpoint_report(&corrupt),
                Err(Resource::Accounting)
            ),
            "fault {fault}"
        );
    }
    value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), source.storage);
}

#[test]
fn mixed_fixedpoint_local_presentation_is_exact_bounded_and_metered() {
    let mut session = PlironSession::new(crate::ShellLimits::default(), []).unwrap();
    let root = session.create_module("fixedpoint_presentation").unwrap();
    let pointer = session.with_operation(&root, |pointer, _| pointer).unwrap();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    let floor = 13;
    budget.reserve_storage(floor).unwrap();
    let mut ledger = crate::fixed_policy_v3::CseLedger::new(&mut budget);
    let before = session
        .fixedpoint_presentation_v18(pointer, 4096, &mut ledger)
        .unwrap();
    let after = session
        .fixedpoint_presentation_v18(pointer, 4096, &mut ledger)
        .unwrap();
    assert!(!before.is_empty());
    assert_eq!(before, after);
    let bytes = before.capacity() + after.capacity();
    drop(before);
    drop(after);
    ledger.release_fixedpoint_scratch(bytes).unwrap();
    assert!(ledger.finish().unwrap() > 0);
    assert!(!session.is_poisoned());
    assert!(matches!(
        session.fixedpoint_presentation_v18(pointer, 1, &mut ledger),
        Err(PlironOptimizationErrorV1::GraphAccountingMismatch)
    ));
    assert!(session.is_poisoned());
    drop(ledger);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn mixed_fixedpoint_incremental_round_denial_is_sticky_on_the_same_ledger() {
    for storage_denial in [false, true] {
        let mut work = Work::new(if storage_denial { 100 } else { 20 });
        let mut budget = Budget::new(&mut work, if storage_denial { 20 } else { 100 });
        budget.reserve_storage(3).unwrap();
        let mut ledger = crate::fixed_policy_v3::CseLedger::new(&mut budget);
        ledger.admit_fixedpoint_round(10, 10).unwrap();
        let first = ledger
            .admit_fixedpoint_round(
                if storage_denial { 1 } else { 11 },
                if storage_denial { 8 } else { 1 },
            )
            .unwrap_err();
        assert_eq!(ledger.admit_fixedpoint_round(0, 0), Err(first));
        assert_eq!(ledger.finish(), Err(first));
        assert!(matches!(first, Resource::Storage(_)) == storage_denial);
        assert!(matches!(first, Resource::Work(_)) != storage_denial);
        drop(ledger);
        assert_eq!(budget.storage(), 13);
        budget.release_storage(10).unwrap();
        assert_eq!(budget.storage(), 3);
    }
}
