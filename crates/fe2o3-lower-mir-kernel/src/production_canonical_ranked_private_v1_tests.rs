use super::*;
use ProductionCanonicalRankedPolicyErrorV1 as PolicyError;

fn run_private<T>(
    owner: &ProductionPreRankedKirOwnerV1,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalPrivateSourcePoliciesV1<'s, 'm, 'g>,
        &mut Budget<'_>,
    ) -> CrPolicyResultV1<T>,
) -> CrPolicyResultV1<T> {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = owner.with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
        Ok(view.with_private_policy_checks_v1(budget, callback))
    });
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result.unwrap()
}

fn counts(owner: &ProductionPreRankedKirOwnerV1) -> [usize; 4] {
    let mut counts = [0; 4];
    for operation in owner
        .executable()
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        match operation.kind {
            OperationKind::Alloca { .. } => counts[0] += 1,
            OperationKind::Store { .. } => counts[1] += 1,
            OperationKind::Load { .. } => counts[2] += 1,
            OperationKind::Call { .. } => counts[3] += 1,
            _ => {}
        }
    }
    counts
}

#[test]
fn genuine_original_unitlocal_memory_and_helper_call_keep_all_nine_actual_reports() {
    let erased_owner = erased();
    let owner = erased_owner.original_source();
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    assert_ne!(before, erased_owner.erased().canonical().canonical_bytes());
    assert_eq!(counts(owner), [1, 1, 1, 1]);
    run_private(owner, |view, budget| {
        assert!(std::ptr::eq(
            view.metadata(budget)?.inventory(budget)?.owner(),
            owner.executable()
        ));
        assert_eq!(view.census(budget)?, (1, 2, 1));
        let policies = view.policies(budget)?;
        assert!(std::ptr::eq(policies.owner(budget)?, owner.executable()));
        assert_eq!(
            policies.owner(budget)?.canonical().canonical_bytes(),
            before
        );
        assert_eq!(
            policies.owner(budget)?.module().kernels,
            owner.executable().module().kernels
        );
        assert_eq!(policies.function_count(budget)?, 2);
        for function in 0..2 {
            let report = policies.report(function, budget)?;
            assert_eq!(report.paired_stage_count(), 9);
            assert_eq!(report.reports().pass_order().len(), 9);
            assert!(report.reports().is_clean());
            assert_eq!(policies.history(function, budget)?.function(), function);
        }
        assert_eq!(policies.pending_obligations().iter().count(), 19);
        assert!(!view.ranked_verification_is_complete());
        assert!(!view.grants_artifact_or_launch_authority());
        Ok(())
    })
    .unwrap();
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
    assert_eq!(counts(owner), [1, 1, 1, 1]);
}

#[test]
fn ordinary_source_policy_still_refuses_this_original_memory() {
    let erased_owner = erased();
    let owner = erased_owner.original_source();
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    budget
        .reserve_storage(owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING)
        .unwrap();
    let result = owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            Ok(view.with_policy_checks_v1(budget, |_, _| Ok(())))
        })
        .unwrap();
    assert!(matches!(
        result,
        Err(PolicyError::Unsupported {
            requirement: ProductionCanonicalRankedSourceRequirementV1::LocalMemory,
            ..
        })
    ));
}

#[test]
fn actual_source_catalog_cannot_be_hidden_by_private_graph_reports() {
    let owner = cr_reachable_pipeline_owner();
    let result = run_private(&owner, |_, _| -> CrPolicyResultV1<()> {
        panic!("real pipeline subject remains outside restricted private language")
    });
    assert!(result.is_err());
}

#[test]
fn original_private_callback_error_panic_and_query_poison_are_not_success() {
    let erased_owner = erased();
    for mode in 0..3 {
        let error = run_private(
            erased_owner.original_source(),
            |view, budget| -> CrPolicyResultV1<()> {
                match mode {
                    0 => Err(cr_policy_unsupported_v1(
                        ProductionCanonicalRankedSourceRequirementV1::Callable,
                        19,
                    )),
                    1 => panic!("private source callback"),
                    _ => {
                        let policies = view.policies(budget)?;
                        assert!(policies.report(2, budget).is_err());
                        Ok(())
                    }
                }
            },
        )
        .unwrap_err();
        match mode {
            0 => assert!(matches!(
                error,
                PolicyError::Unsupported { ordinal: 19, .. }
            )),
            1 => assert!(matches!(
                error,
                PolicyError::Source(ProductionCanonicalRankedSourceErrorV1::Panicked)
            )),
            _ => assert!(matches!(
                error,
                PolicyError::Policy(_) | PolicyError::Query(_)
            )),
        }
    }
    run_private(erased_owner.original_source(), |view, budget| {
        assert_eq!(view.census(budget)?, (1, 2, 1));
        Ok(())
    })
    .unwrap();
}
