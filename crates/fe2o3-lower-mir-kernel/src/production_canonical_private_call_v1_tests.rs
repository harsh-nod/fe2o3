use super::*;
use ProductionCanonicalPrivateOperationKindV1 as PrivateKind;

fn cpc_prepare(
    source: ProductionPreRankedKirOwnerV1,
) -> ProductionCanonicalScalarFixedPointOwnerV1 {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (owner, receipt) =
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_private_calls_v1(
            source,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(owner.additional_storage(), receipt);
    assert_eq!(
        owner.retained_storage_floor_v1(),
        owner.input_storage_floor_v1() + receipt.retained_storage()
    );
    owner
}
fn cpc_run<T>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    run: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalPrivateCallPoliciesV1<'s, 'm, 'g>,
        &mut Budget<'_>,
    ) -> HistoryResult<T>,
) -> HistoryResult<T> {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = owner.with_private_call_policy_checks_v1(&mut budget, run);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}
fn cpc_reports(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    view: &ProductionCanonicalPrivateCallPoliciesV1<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> HistoryResult<()> {
    assert!(std::ptr::eq(
        view.original_metadata(budget)?.inventory(budget)?.owner(),
        owner.original_source().executable()
    ));
    let output = view.final_inventory(budget)?;
    assert!(std::ptr::eq(output.owner(), owner.output()));
    let policies = view.policies(budget)?;
    assert!(std::ptr::eq(policies.owner(budget)?, owner.output()));
    assert_eq!(
        policies.module_function_count(budget)?,
        output.functions().len()
    );
    let definitions = output
        .functions()
        .iter()
        .filter(|f| f.function.body.is_some())
        .count();
    assert!(definitions > 0);
    assert_eq!(policies.definition_count(budget)?, definitions);
    for ordinal in 0..definitions {
        let coordinate = policies.definition_coordinate(ordinal, budget)?;
        assert_eq!(
            policies.history(ordinal, budget)?.function(),
            coordinate.0 as usize
        );
        let report = policies.report(ordinal, budget)?;
        assert_eq!(report.paired_stage_count(), 9);
        assert_eq!(report.reports().pass_order().len(), 9);
        assert!(report.reports().is_clean());
    }
    assert_eq!(policies.pending_obligations().iter().count(), 19);
    assert!(!view.ranked_verification_is_complete());
    assert!(!view.grants_artifact_or_launch_authority());
    Ok(())
}
fn cpc_goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(target),
    ))
}

// Real source Store/Load declarations; no edited KIR, erased-owner input, or
// no-op substitute enters the new factory.
fn cpc_memory_source(
    split: bool,
    root_only: bool,
    rewrite: bool,
    bits: u128,
) -> ProductionPreRankedKirOwnerV1 {
    let erased_owner = erased();
    let seed = erased_owner.original_source();
    let semantic = seed.semantic_ssa().source_semantic();
    let helper = &semantic.functions()[1];
    let root = &semantic.functions()[0];
    let word = SemanticTypeIdV1::from_index(1);
    let place = |n| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(n), vec![], word).unwrap();
    let mut locals = helper.locals().to_vec();
    let mut first = Vec::new();
    let stored = if rewrite {
        assert!(
            root_only,
            "general arithmetic is not a UnitLocal helper recipe"
        );
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([220; 32]),
            word,
            SemanticLocalRoleV1::Temporary,
            root.source(),
        ));
        first.push(assign(
            3,
            word,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitOr,
                left: operand(word, bits, 8),
                right: operand(word, 0, 8),
            },
        ));
        local(3, word)
    } else {
        operand(word, bits, 8)
    };
    first.push(SemanticStatementV1::new(
        helper.source(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(1),
            stored,
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    let load = helper.blocks()[0].statements()[1].clone();
    let blocks = if split {
        vec![
            body(212, first, cpc_goto(1)),
            body(213, vec![load], SemanticTerminatorKindV1::Return),
        ]
    } else {
        first.push(load);
        vec![body(212, first, SemanticTerminatorKindV1::Return)]
    };
    let functions = if root_only {
        vec![copy_function(root, locals, blocks)]
    } else {
        vec![root.clone(), copy_function(helper, locals, blocks)]
    };
    rebuild(seed, semantic.types().to_vec(), functions)
}
fn cpc_shared_source() -> ProductionPreRankedKirOwnerV1 {
    let erased_owner = erased();
    let seed = erased_owner.original_source();
    let semantic = seed.semantic_ssa().source_semantic();
    let root = &semantic.functions()[0];
    let entry = root.kernel_entry().unwrap();
    let second = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([230; 32]),
        root.role(),
        SemanticItemDefinitionIdentityV1::from_sha256([231; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([232; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([233; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([234; 32]),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        root.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"private_history_second_root".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([235; 32]),
        entry.source_contract(),
    ));
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root.clone(), semantic.functions()[1].clone(), second],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(2),
        ],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    cr_owner_from_ssa(
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap(),
    )
}
fn cpc_counts(owner: &CsGraphV1) -> [usize; 5] {
    let mut result = [0; 5];
    for operation in owner
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
    {
        match operation.kind {
            OperationKind::Alloca { .. } => result[0] += 1,
            OperationKind::GetElementPointer { .. } => result[1] += 1,
            OperationKind::Store { .. } => result[2] += 1,
            OperationKind::Load { .. } => result[3] += 1,
            OperationKind::Call { .. } => result[4] += 1,
            _ => {}
        }
    }
    result
}

#[test]
fn genuine_private_helper_call_and_selected_assertion_reach_actual_final_nine() {
    let word = SemanticTypeIdV1::from_index(1);
    let source = unit_assertion(SemanticAssertMessageV1::BoundsCheck {
        length: operand(word, 8, 8),
        index: operand(word, 0, 8),
    });
    let original = source.executable().canonical().canonical_bytes().to_vec();
    assert_eq!(cpc_counts(source.executable()), [1, 0, 1, 1, 2]);
    let owner = cpc_prepare(source);
    assert_ne!(original, owner.output().canonical().canonical_bytes());
    assert_eq!(cpc_counts(owner.output()), [1, 0, 1, 1, 1]);
    let before = snapshots(&owner);
    cpc_run(&owner, |view, budget| {
        cpc_reports(&owner, view, budget)?;
        assert_eq!(view.memory_census(budget)?, [1, 2]);
        assert_eq!(view.calls(budget)?.len(), 1);
        assert_eq!(view.assertion_count(budget)?, 1);
        assert_eq!(
            view.assertion(0, budget)?.disposition(),
            Disposition::HistoryElided
        );
        assert!(view.assertion(0, budget)?.selection().is_some());
        let private = view
            .original_callables(budget)?
            .iter()
            .find(|r| r.kind() == ProductionCanonicalAssertionCallKindV1::PrivateFrame)
            .unwrap();
        assert_eq!(
            private.source_decision(),
            fe2o3_mir_model::SemanticCallableDecisionV1::Rejected
        );
        let removed = (0..view.operation_count(budget)?)
            .filter_map(|n| {
                let row = view.operation(n, budget).unwrap();
                row.removal().map(|step| (row.kind(), step))
            })
            .collect::<Vec<_>>();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].0, PrivateKind::Call);
        assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshots(&owner), before);
}

#[test]
fn real_cross_block_root_memory_and_scalar_operand_rewrite_keep_source_lifetimes() {
    let source = cpc_memory_source(true, true, true, 11);
    assert_eq!(cpc_counts(source.executable()), [1, 0, 1, 1, 0]);
    let body = source
        .executable()
        .module()
        .functions
        .iter()
        .find_map(|f| f.body.as_ref())
        .unwrap();
    let store = body
        .blocks
        .iter()
        .position(|b| {
            b.operations
                .iter()
                .any(|op| matches!(op.kind, OperationKind::Store { .. }))
        })
        .unwrap();
    let load = body
        .blocks
        .iter()
        .position(|b| {
            b.operations
                .iter()
                .any(|op| matches!(op.kind, OperationKind::Load { .. }))
        })
        .unwrap();
    assert_ne!(store, load);
    assert!(
        body.blocks
            .iter()
            .flat_map(|b| &b.operations)
            .any(|op| matches!(
                op.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    ..
                }
            ))
    );
    let original = source.executable().canonical().canonical_bytes().to_vec();
    let owner = cpc_prepare(source);
    assert_ne!(original, owner.output().canonical().canonical_bytes());
    assert_eq!(cpc_counts(owner.output()), [1, 0, 1, 1, 0]);
    cpc_run(&owner, |view, budget| {
        cpc_reports(&owner, view, budget)?;
        assert_eq!(view.memory_census(budget)?, [1, 2]);
        assert_eq!(view.calls(budget)?.len(), 0);
        assert_eq!(view.operation_count(budget)?, 3);
        for n in 0..3 {
            assert!(view.operation(n, budget)?.current().is_some());
            assert!(view.operation(n, budget)?.removal().is_none());
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn retained_unitlocal_cross_block_helper_uses_same_physical_and_source_engines() {
    let source = cpc_memory_source(true, false, false, 11);
    assert_eq!(cpc_counts(source.executable()), [1, 0, 1, 1, 1]);
    let owner = cpc_prepare(source);
    cpc_run(&owner, |view, budget| {
        cpc_reports(&owner, view, budget)?;
        assert_eq!(view.memory_census(budget)?, [1, 2]);
        assert_eq!(view.calls(budget)?.len(), 1);
        assert!(
            view.original_callables(budget)?
                .iter()
                .any(|r| r.kind() == ProductionCanonicalAssertionCallKindV1::PrivateFrame)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn shared_private_helper_preserves_all_root_qualified_aliases() {
    let source = cpc_shared_source();
    let roots = source.executable().module().kernels.clone();
    assert_eq!(roots.len(), 2);
    let owner = cpc_prepare(source);
    cpc_run(&owner, |view, budget| {
        cpc_reports(&owner, view, budget)?;
        assert_eq!(
            view.final_inventory(budget)?.owner().module().kernels,
            roots
        );
        assert_eq!(view.calls(budget)?.len(), 2);
        assert_ne!(view.calls(budget)?[0].root(), view.calls(budget)?[1].root());
        for n in 0..view.operation_count(budget)? {
            if view.operation(n, budget)?.kind() != PrivateKind::Call {
                assert_eq!(
                    view.source_aliases(budget)?
                        .iter()
                        .filter(|a| a.operation() == n)
                        .count(),
                    2
                );
            }
        }
        assert_eq!(
            view.original_callables(budget)?
                .iter()
                .filter(|r| r.kind() == ProductionCanonicalAssertionCallKindV1::PrivateFrame)
                .count(),
            2
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn genuine_private_noop_history_still_runs_full_final_nine_without_traps() {
    let source = cpc_memory_source(false, true, false, 11);
    let original = source.executable().canonical().canonical_bytes().to_vec();
    let owner = cpc_prepare(source);
    assert_eq!(original, owner.output().canonical().canonical_bytes());
    assert_eq!(owner.history().rounds().len(), 1);
    cpc_run(&owner, |view, budget| {
        cpc_reports(&owner, view, budget)?;
        assert_eq!(view.memory_census(budget)?, [1, 2]);
        assert_eq!(view.assertion_count(budget)?, 0);
        assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn original_scalar_and_assertion_profiles_remain_closed_on_real_private_memory() {
    use ProductionCanonicalRankedSourceRequirementV1 as Need;
    for (root_only, policy, need) in [
        (true, ProductionHelperSourcePolicyV1::RawEmpty, Need::Effect),
        (
            false,
            ProductionHelperSourcePolicyV1::UnitLocal,
            Need::LocalMemory,
        ),
    ] {
        let source = cpc_memory_source(false, root_only, false, 11);
        assert_eq!(source.helper_source_policy_v1(), policy);
        let owner = cpc_prepare(source);
        let mut work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let floor = owner.retained_storage_floor_v1() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let callbacks = std::cell::Cell::new(0);
        let first = owner.with_policy_checks_v1(&mut budget, |_, _| {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        });
        assert!(matches!(first, Err(HistoryError::SourcePolicy(
            ProductionCanonicalRankedPolicyErrorV1::Unsupported { requirement, ordinal: 0 }
        )) if requirement == need));
        assert_eq!(budget.storage(), floor);
        let second = owner.with_assertion_policy_checks_v1(&mut budget, |_, _| {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        });
        assert!(matches!(second, Err(HistoryError::SourcePolicy(
            ProductionCanonicalRankedPolicyErrorV1::Unsupported { requirement, ordinal: 0 }
        )) if requirement == need));
        assert_eq!(callbacks.get(), 0);
        assert_eq!(budget.storage(), floor);
    }
}

mod indexed {
    include!("production_canonical_private_call_index_v1_tests.rs");
}

mod whole_entry {
    include!("production_canonical_private_call_whole_entry_v1_tests.rs");
}
