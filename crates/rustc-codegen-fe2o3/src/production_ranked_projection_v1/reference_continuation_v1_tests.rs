//! Roster transfer tests use ordinary component fixtures, not protected proofs.
use super::*;
use crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1 as References;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn reference_continuation_preserves_ordinary_root_and_original_account() {
    let program = neutral_ranked_program_v1();
    let expected = program.roots[0].kernel_binding;
    let source =
        RankedProjectionSourceV1::from_materialized_checked(&program.materialized).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget
        .reserve_storage(
            program
                .materialized
                .unit_local_source_storage_floor_v1()
                .unwrap()
                + 1024,
        )
        .unwrap();
    budget.charge_work(17).unwrap();
    let floor = budget.storage();
    let account = budget.work_ledger_identity_v1();
    let roots = ProjectedReferenceRootsV1 {
        roots: program.roots,
        references: vec![References::default()],
    }
    .finish(&source, &mut budget)
    .unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].kernel_binding, expected);
    assert!(roots[0].verification.ordinary().is_some());
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == account);
    assert!(budget.work() >= 17);
}

#[test]
fn reference_continuation_rejects_missing_roster_without_debiting_source_floor() {
    let program = neutral_ranked_program_v1();
    let source =
        RankedProjectionSourceV1::from_materialized_checked(&program.materialized).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget
        .reserve_storage(
            program
                .materialized
                .unit_local_source_storage_floor_v1()
                .unwrap()
                + 1024,
        )
        .unwrap();
    let floor = budget.storage();
    let result = ProjectedReferenceRootsV1 {
        roots: program.roots,
        references: Vec::new(),
    }
    .finish(&source, &mut budget);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "reference continuation lost its complete root roster"
        ))
    ));
    assert_eq!(budget.storage(), floor);
}
