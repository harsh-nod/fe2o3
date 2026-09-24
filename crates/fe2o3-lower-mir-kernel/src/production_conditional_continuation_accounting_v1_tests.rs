// These guard/Drop tests deliberately have no final graph or proof authority.
// They exercise the same reservation guard used by the consuming transitions.
fn accounting_root<'source, 'ledger>(
    source: &'source ProductionPreRankedKirOwnerV1,
    ledger: &'ledger mut dyn ConditionalOriginalLedgerV1,
) -> ProductionConditionalFinalRootV1<'source, 'ledger> {
    let ranked_ir = String::with_capacity(64);
    let reserved =
        ranked_ir.capacity() + std::mem::size_of::<ProductionConditionalFinalRootV1<'_, '_>>();
    let floor = ledger.storage();
    let account = ledger.with_budget(|budget| {
        budget.reserve_storage(reserved).unwrap();
        budget.work_ledger_identity_v1()
    });
    ProductionConditionalFinalRootV1 {
        source,
        graph: None,
        arguments: vec![],
        semantic_root: 0,
        launch_rank: 1,
        access_sources: vec![],
        executable_effect_sources: vec![],
        ranked_ir,
        ledger,
        reserved,
        account,
        floor,
        poisoned: false,
    }
}

fn visit_accounting_root<R>(
    root: &mut ProductionConditionalFinalRootV1<'_, '_>,
    callback: impl FnOnce(
        &mut AssertOriginBudgetV1<'_>,
    ) -> Result<R, ProductionConditionalContinuationErrorV1>,
) -> Result<R, ProductionConditionalContinuationErrorV1> {
    let protected = root.protected_storage_v1()?;
    with_conditional_original_budget_v1(
        root.ledger,
        root.account,
        protected,
        &mut root.poisoned,
        callback,
    )
}

#[test]
fn conditional_input_retained_storage_counts_arena_and_spare_capacities() {
    let source = materialize(Fixture::default());
    let mut input = continuation_input(&source, 0);
    let arena = input.pending.retained_analysis_storage_v1();
    assert_eq!(input.retained_storage_v1().unwrap(), arena);
    input.access_sources.reserve_exact(17);
    input.executable_effect_sources.reserve_exact(23);
    input.ranked_ir.reserve_exact(127);
    assert!(input.access_sources.is_empty() && input.executable_effect_sources.is_empty());
    assert!(input.ranked_ir.is_empty());
    let expected = arena
        + input.access_sources.capacity() * std::mem::size_of::<ProductionRankedAccessSourceV1>()
        + input.executable_effect_sources.capacity()
            * std::mem::size_of::<ProductionRankedExecutableEffectSourceV1>()
        + input.ranked_ir.capacity();
    assert_eq!(input.retained_storage_v1().unwrap(), expected);
}

#[test]
fn conditional_root_drop_preserves_callback_reservations_on_success_and_nested_error() {
    let source = materialize(Fixture::default());
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    for nested_error in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(7).unwrap();
        let account = budget.work_ledger_identity_v1();
        let mut root = accounting_root(&source, &mut budget);
        let result = visit_accounting_root(&mut root, |budget| {
            budget.reserve_storage(11)?;
            budget.charge_work(13)?;
            Ok(if nested_error {
                Err("consumer refusal")
            } else {
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(result.is_err(), nested_error);
        assert!(!root.poisoned);
        drop(root);
        assert_eq!(budget.storage(), floor + 11);
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.work(), 20);
    }
}

#[test]
fn conditional_root_drop_never_debits_substituted_account_or_damaged_floor() {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let source = materialize(Fixture::default());
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    for substitute in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let mut root = accounting_root(&source, &mut budget);
        let protected = root.protected_storage_v1().unwrap();
        let result = visit_accounting_root(&mut root, |budget| {
            if substitute {
                *budget = AssertOriginBudgetV1::new(
                    Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(WORK))),
                    STORAGE,
                );
                // Sufficient storage makes an accidental Drop debit observable.
                budget.reserve_storage(protected + 11)?;
            } else {
                budget.release_storage(1)?;
            }
            Ok(())
        });
        assert!(matches!(
            result,
            Err(ProductionConditionalContinuationErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert!(root.poisoned);
        let before = root.ledger.storage();
        assert!(
            visit_accounting_root(&mut root, |_| -> Result<(), _> {
                panic!("a corrupted root cannot be retried")
            })
            .is_err()
        );
        drop(root);
        assert_eq!(
            budget.storage(),
            before,
            "Drop must not further damage either account"
        );
    }
}

#[test]
fn conditional_root_drop_checks_the_original_account_even_without_postflight() {
    let source = materialize(Fixture::default());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let root = accounting_root(&source, &mut budget);
    let protected = root.protected_storage_v1().unwrap();
    root.ledger.with_budget(|budget| {
        *budget = AssertOriginBudgetV1::new(
            Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(WORK))),
            STORAGE,
        );
        budget.reserve_storage(protected + 11).unwrap();
    });
    drop(root);
    assert_eq!(budget.storage(), protected + 11);
}

#[test]
fn conditional_root_guard_handles_unwind_without_releasing_foreign_or_callback_charges() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let source = materialize(Fixture::default());
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    for mutation in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let mut root = accounting_root(&source, &mut budget);
        let protected = root.protected_storage_v1().unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = visit_accounting_root(&mut root, |budget| -> Result<(), _> {
                    match mutation {
                        0 => budget.reserve_storage(11).unwrap(),
                        1 => budget.release_storage(1).unwrap(),
                        _ => {
                            *budget = AssertOriginBudgetV1::new(
                                Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(WORK))),
                                STORAGE,
                            );
                            budget.reserve_storage(protected + 11).unwrap();
                        }
                    }
                    panic!("consumer panic");
                });
            }))
            .is_err()
        );
        assert_eq!(root.poisoned, mutation != 0);
        let before = root.ledger.storage();
        drop(root);
        assert_eq!(
            budget.storage(),
            if mutation == 0 { floor + 11 } else { before }
        );
    }
}

#[test]
fn conditional_root_owned_ledger_remembers_transient_callback_substitution() {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let source = materialize(Fixture::default());
    let mut ledger = Owned::new(CanonicalKernelIrWorkBudgetV1::new(WORK), STORAGE);
    ledger
        .with_budget(|budget| budget.reserve_storage(FLOOR))
        .unwrap();
    let mut root = accounting_root(&source, &mut ledger);
    let original = root.ledger.storage();
    assert!(
        visit_accounting_root(&mut root, |budget| {
            *budget = AssertOriginBudgetV1::new(
                Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(WORK))),
                STORAGE,
            );
            budget.reserve_storage(original + 11)?;
            Ok(())
        })
        .is_err()
    );
    assert!(root.poisoned);
    // Owned::with_budget has ended, restoring its view of the original meter.
    // Poison must survive that temporary foreign view.
    assert!(
        visit_accounting_root(&mut root, |_| -> Result<(), _> {
            panic!("transient substitution must remain a refusal")
        })
        .is_err()
    );
    drop(root);
    assert_eq!(ledger.storage(), original);
}

#[test]
fn conditional_root_transfer_keeps_exact_input_and_callback_storage_charged() {
    let source = materialize(Fixture::default());
    let mut input = continuation_input(&source, 0);
    input.access_sources.reserve_exact(17);
    input.ranked_ir.reserve_exact(127);
    let retained = input.retained_storage_v1().unwrap();
    let graph = input.pending.kernel().unwrap() as *const _;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    budget.reserve_storage(floor).unwrap();
    let mut root = accounting_root(&source, &mut budget);
    root.ledger
        .with_budget(|budget| budget.reserve_storage(retained))
        .unwrap();
    root.reserved += retained;
    let protected = root.protected_storage_v1().unwrap();
    visit_accounting_root(&mut root, |budget| {
        budget.reserve_storage(11)?;
        Ok(())
    })
    .unwrap();
    root.transfer_input_storage_v1(retained).unwrap();
    assert_eq!(root.protected_storage_v1().unwrap(), protected);
    drop(root);
    assert_eq!(budget.storage(), floor + retained + 11);
    assert_eq!(input.pending.kernel().unwrap() as *const _, graph);
    drop(input);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor + 11);
}

#[test]
fn conditional_root_failed_storage_transfer_preserves_its_reservation() {
    let source = materialize(Fixture::default());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let mut root = accounting_root(&source, &mut budget);
    let original = (root.floor, root.reserved);
    assert!(root.transfer_input_storage_v1(root.reserved + 1).is_err());
    assert_eq!((root.floor, root.reserved), original);
    drop(root);
    assert_eq!(budget.storage(), FLOOR);
}
