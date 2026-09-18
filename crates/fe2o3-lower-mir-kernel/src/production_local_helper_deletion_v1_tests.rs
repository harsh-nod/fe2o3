use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrReplayStorageV12, VerifiedCanonicalKernelIrModuleV12};

#[path = "production_local_helper_erased_owner_v1_tests.rs"]
mod erased_owner_tests;

// This producer is deliberately test-only. The checker consumes the separately
// verified actual graph, not a flag asserting that this edit was performed.
fn deletion_module(owner: &ProductionPreRankedKirOwnerV1) -> Module {
    let names = owner
        .executable()
        .module()
        .functions
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            matches!(
                owner.helper_memory.functions[*index],
                RetainedHelperKindV1::Local { .. }
            )
        })
        .map(|(_, function)| function.id.clone())
        .collect::<BTreeSet<_>>();
    let mut module = owner.executable().module().clone();
    for function in &mut module.functions {
        if let Some(body) = &mut function.body {
            for block in &mut body.blocks {
                block.operations.retain(|operation| !matches!(&operation.kind, OperationKind::Call { callee, .. } if names.contains(callee)));
            }
        }
    }
    module
        .functions
        .retain(|function| !names.contains(&function.id));
    module
}

fn deletion_verified(
    module: &Module,
) -> (
    VerifiedCanonicalKernelIrModuleV12,
    CanonicalKernelIrReplayStorageV12,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        module,
        &mut budget,
    )
    .unwrap()
}

fn deletion_result(
    owner: &ProductionPreRankedKirOwnerV1,
    output: &VerifiedCanonicalKernelIrModuleV12,
    storage: CanonicalKernelIrReplayStorageV12,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let roots = ranked_stage_roots(owner);
    let floor = stage_floor(owner) + storage.retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result =
        owner.with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |stage, budget| {
            stage.with_checked_silent_unit_call_deletion_v1(output, budget, |_, _| Ok(()))
        });
    assert_eq!(budget.storage(), floor);
    result
}

fn deletion_mixed_owner() -> ProductionPreRankedKirOwnerV1 {
    let (seed, _) = unit_source(UnitCase::Initializer, &[2]);
    let semantic = seed.source_semantic();
    let root = &semantic.functions()[0];
    let mut blocks = root.blocks().to_vec();
    blocks[0] = block(
        210,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(2),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    let replacement = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let source = SemanticSourceProvenanceV1::unavailable();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([201; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([201; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([201; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([201; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([201; 32]),
        source,
        local_abi(201, false),
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([248; 32]),
            UNIT,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        vec![block(249, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    let request = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![replacement, semantic.functions()[1].clone(), helper],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(request, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "unit_zero",
            [180; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

#[test]
fn silent_unit_deletion_keeps_uncertified_raw_empty_calls_and_helpers() {
    let owner = deletion_mixed_owner();
    assert_eq!(owner.empty_effect_helpers().iter().count(), 1);
    let module = deletion_module(&owner);
    let remaining_calls = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .count();
    assert_eq!(remaining_calls, 1);
    let (output, storage) = deletion_verified(&module);
    deletion_result(&owner, &output, storage).unwrap();
    let mut removed_call = module.clone();
    for function in &mut removed_call.functions {
        if let Some(body) = &mut function.body {
            for block in &mut body.blocks {
                block
                    .operations
                    .retain(|operation| !matches!(operation.kind, OperationKind::Call { .. }));
            }
        }
    }
    let (output, storage) = deletion_verified(&removed_call);
    assert!(matches!(
        deletion_result(&owner, &output, storage),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    removed_call
        .functions
        .retain(|function| function.role != fe2o3_kernel_ir::FunctionRole::InternalHelper);
    let (output, storage) = deletion_verified(&removed_call);
    assert!(matches!(
        deletion_result(&owner, &output, storage),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
}

#[test]
fn silent_unit_deletion_checks_actual_graph_and_every_survivor_coordinate() {
    for calls in [&[1][..], &[2][..], &[2, 2][..], &[0, 2][..]] {
        for case in [
            UnitCase::Initializer,
            UnitCase::ReadThenWrite,
            UnitCase::ScalarSlot,
            UnitCase::CastAssert { expected: true },
            UnitCase::CastAssert { expected: false },
        ] {
            let owner = unit_owner(case, calls);
            let roots = ranked_stage_roots(&owner);
            let original = owner.executable().canonical().canonical_bytes().to_vec();
            let module = deletion_module(&owner);
            let (output, storage) = deletion_verified(&module);
            assert_ne!(output.canonical().canonical_bytes(), original);
            let floor = stage_floor(&owner) + storage.retained_storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            owner
                .with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |stage, budget| {
                    stage.with_checked_silent_unit_call_deletion_v1(
                        &output,
                        budget,
                        |checked, budget| {
                            assert!(std::ptr::eq(checked.source(), &owner));
                            assert!(std::ptr::eq(checked.output(), &output));
                            assert_eq!(checked.deleted_call_count(), calls.iter().sum::<usize>());
                            assert_eq!(checked.deleted_function_count(), 1);
                            assert!(!checked.grants_artifact_or_launch_authority());
                            let mut deleted_calls = 0;
                            let mut deleted_body_ops = 0;
                            for row in stage.source.inventory.operations() {
                                match checked.operation(row.coordinate, budget)?.unwrap() {
                                    ProductionUnitLocalOperationDeletionV1::Retained(at) => {
                                        let operation = &output.module().functions
                                            [at.block.function.0 as usize]
                                            .body
                                            .as_ref()
                                            .unwrap()
                                            .blocks
                                            [at.block.block as usize]
                                            .operations
                                            [at.operation as usize];
                                        assert_eq!(operation, row.operation);
                                    }
                                    ProductionUnitLocalOperationDeletionV1::DeletedUnitCall => {
                                        assert!(
                                            stage.calls.iter().any(|token| token.native_call()
                                                == row.coordinate
                                                && std::ptr::eq(token.operation(), row.operation))
                                        );
                                        deleted_calls += 1;
                                    }
                                    ProductionUnitLocalOperationDeletionV1::DeletedLocalHelper => {
                                        assert!(matches!(
                                            owner.helper_memory.functions
                                                [row.coordinate.block.function.0 as usize],
                                            RetainedHelperKindV1::Local { .. }
                                        ));
                                        deleted_body_ops += 1;
                                    }
                                }
                            }
                            assert_eq!(deleted_calls, calls.iter().sum::<usize>());
                            assert!(deleted_body_ops > 0);
                            for (ordinal, function) in
                                owner.executable().module().functions.iter().enumerate()
                            {
                                match checked.function(ordinal as u32, budget)? {
                                    Some(index) => assert_eq!(
                                        output.module().functions[index as usize].id,
                                        function.id
                                    ),
                                    None => assert!(matches!(
                                        owner.helper_memory.functions[ordinal],
                                        RetainedHelperKindV1::Local { .. }
                                    )),
                                }
                            }
                            assert!(checked.function(u32::MAX, budget)?.is_none());
                            Ok(())
                        },
                    )
                })
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(owner.executable().canonical().canonical_bytes(), original);
            assert!(!owner.helper_memory.unit_source.calls.is_empty());
            assert!(!owner.helper_memory.accesses.is_empty());
            assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
        }
    }
}

#[test]
fn silent_unit_deletion_rejects_verified_nonexact_candidates() {
    for mutation in 0..8 {
        let owner = unit_owner(UnitCase::Initializer, &[2, 2]);
        let mut module = deletion_module(&owner);
        match mutation {
            0 => module = owner.executable().module().clone(),
            1 => {
                let original = owner.executable().module();
                let local = original
                    .functions
                    .iter()
                    .enumerate()
                    .find(|(index, _)| {
                        matches!(
                            owner.helper_memory.functions[*index],
                            RetainedHelperKindV1::Local { .. }
                        )
                    })
                    .unwrap()
                    .1;
                module.functions.push(local.clone());
            }
            2 => module.functions.swap(0, 1),
            3 => module.functions[0].body.as_mut().unwrap().blocks[0].id = BlockId(999),
            4 => {
                module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
                    Some(Terminator::Return { values: vec![] })
            }
            5 => module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::new(
                    vec![ValueDef::new(ValueId(30000), Type::Scalar(ScalarType::U32))],
                    OperationKind::Constant(Constant::U32(999)),
                )),
            6 => module.id = "changed_deletion_module".into(),
            7 => module.kernels[0].id = "changed_deletion_kernel".into(),
            _ => unreachable!(),
        }
        let (output, storage) = deletion_verified(&module);
        assert!(
            matches!(
                deletion_result(&owner, &output, storage),
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn silent_unit_deletion_preserves_inactive_trap_imports_and_their_metadata() {
    let owner = unit_owner(UnitCase::CastAssert { expected: true }, &[1]);
    let module = deletion_module(&owner);
    let externals = module
        .functions
        .iter()
        .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::ExternalImport)
        .count();
    assert!(externals > 0);
    let (output, storage) = deletion_verified(&module);
    deletion_result(&owner, &output, storage).unwrap();
    let mut changed = module;
    changed
        .functions
        .retain(|function| function.role != fe2o3_kernel_ir::FunctionRole::ExternalImport);
    let (output, storage) = deletion_verified(&changed);
    assert!(matches!(
        deletion_result(&owner, &output, storage),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
}

#[test]
fn silent_unit_deletion_retains_legacy_owner_fences_after_success() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let (output, storage) = deletion_verified(&deletion_module(&owner));
    deletion_result(&owner, &output, storage).unwrap();
    let roots = ranked_stage_roots(&owner);
    assert!(matches!(
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            owner, roots
        ),
        Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "materialized ranked receipt"
            }
        )
    ));
}

#[test]
fn silent_unit_deletion_does_not_admit_source_use_after_kill() {
    for kind in [
        ScalarKill::Move,
        ScalarKill::Storage,
        ScalarKill::Deinitialize,
    ] {
        let (ssa, launch) = unit_source(UnitCase::Killed(kind), &[1]);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                ssa,
                launch,
                ProductionSemanticKirLimitsV1::default(),
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn silent_unit_deletion_measured_exact_and_one_short_limits_restore_all_live_inputs() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let roots = ranked_stage_roots(&owner);
    let (output, storage) = deletion_verified(&deletion_module(&owner));
    let floor = stage_floor(&owner) + storage.retained_storage();
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let mut entered = false;
        let result =
            owner.with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |stage, budget| {
                stage.with_checked_silent_unit_call_deletion_v1(&output, budget, |_, _| {
                    entered = true;
                    Ok(())
                })
            });
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage(), entered)
    };
    let (result, exact_work, exact_storage, entered) = run(WORK, STORAGE);
    result.unwrap();
    assert!(entered);
    let (result, work, peak, entered) = run(exact_work, exact_storage);
    result.unwrap();
    assert!(entered);
    assert_eq!((work, peak), (exact_work, exact_storage));
    for (work, storage, storage_case) in [
        (exact_work - 1, exact_storage, false),
        (exact_work, exact_storage - 1, true),
    ] {
        let (result, _, _, entered) = run(work, storage);
        assert!(!entered);
        if storage_case {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        }
    }
}

#[test]
fn silent_unit_deletion_queries_and_initial_foreign_ledger_denials_are_paid() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let roots = ranked_stage_roots(&owner);
    let (output, storage) = deletion_verified(&deletion_module(&owner));
    let floor = stage_floor(&owner) + storage.retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    owner
        .with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |stage, budget| {
            for limit in [1, 8, 9] {
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut other = ArgumentBudgetV1::new(&mut other_work, STORAGE);
                other.reserve_storage(budget.storage())?;
                let before = other.storage();
                let result =
                    stage.with_checked_silent_unit_call_deletion_v1(&output, &mut other, |_, _| {
                        Ok(())
                    });
                if limit == 9 {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!(other.work(), 9);
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                    assert_eq!(other.work(), if limit == 1 { 0 } else { 2 });
                }
                assert_eq!((other.storage(), other.peak_storage()), (before, before));
            }
            stage.with_checked_silent_unit_call_deletion_v1(&output, budget, |checked, budget| {
                let before = budget.work();
                checked.function(0, budget)?;
                assert_eq!(budget.work() - before, 5);
                let call = stage.calls[0].native_call();
                let before = budget.work();
                assert_eq!(
                    checked.operation(call, budget)?,
                    Some(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall)
                );
                assert_eq!(budget.work() - before, 12);
                budget.release_storage(1)?;
                let denied = checked.function(0, budget);
                budget.reserve_storage(1)?;
                assert!(matches!(
                    denied,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                Ok(())
            })
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn silent_unit_deletion_error_panic_and_accounting_unwind_drop_inner_maps() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let roots = ranked_stage_roots(&owner);
    let (output, storage) = deletion_verified(&deletion_module(&owner));
    let floor = stage_floor(&owner) + storage.retained_storage();
    for mode in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |stage, budget| {
                stage.with_checked_silent_unit_call_deletion_v1(
                    &output,
                    budget,
                    |_, budget| -> Result<(), ProductionSemanticKirErrorV1> {
                        match mode {
                            0 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                            1 => panic!("deletion callback"),
                            2 => {
                                budget.reserve_storage(1)?;
                                Ok(())
                            }
                            3 => {
                                budget.reserve_storage(1)?;
                                panic!("deletion accounting precedence");
                            }
                            4 => {
                                budget.release_storage(1)?;
                                Ok(())
                            }
                            _ => unreachable!(),
                        }
                    },
                )
            })
        }));
        match mode {
            0 => assert!(matches!(
                result,
                Ok(Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch))
            )),
            1 => assert!(result.is_err()),
            2..=4 => assert!(matches!(
                result,
                Ok(Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ))
            )),
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), floor);
    }
}
