use super::*;

fn ranked_stage_roots(
    owner: &ProductionPreRankedKirOwnerV1,
) -> Vec<ProductionRankedSemanticProjectionRootV1> {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    owner
        .source_launch()
        .roots()
        .iter()
        .map(|root| {
            let source = &owner.semantic_ssa().source_semantic().functions()
                [root.selected_root().index() as usize];
            let name =
                std::str::from_utf8(source.kernel_entry().unwrap().export_symbol().as_bytes())
                    .unwrap();
            let layout = root.layout();
            let kernel = ProductionRankedKernelV1::new(
                name,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: layout.grid_identity(),
                        global_extents: layout.global_extents(),
                        workgroup_extents: layout.workgroup_extents(),
                        subgroup_size: layout.subgroup_size(),
                        full_physical_workgroups: layout.full_physical_workgroups(),
                    }],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel("unit_local_stage", kernel).unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            assert!(lowering.all_mandatory_reports_are_clean());
            ProductionRankedSemanticProjectionRootV1::new(
                root.selected_root(),
                root.source_rank(),
                lowering,
                "entry-only ranked projection; local effects retain source tokens\n".to_owned(),
                vec![],
                vec![],
            )
        })
        .collect()
}

fn stage_floor(owner: &ProductionPreRankedKirOwnerV1) -> usize {
    FLOOR + owner.unit_local_source_storage_floor_v1().unwrap()
}

fn replace_stage_ranked_body(
    root: &mut ProductionRankedSemanticProjectionRootV1,
    change_layout: bool,
    extra_fence: bool,
) {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let mut operations = root.lowering.kernel().blocks()[0].operations().to_vec();
    if change_layout {
        let ProductionRankedOperationV1::ExecutionLayout { grid_identity, .. } = &mut operations[0]
        else {
            panic!("ranked fixture layout");
        };
        *grid_identity ^= 1;
    }
    if extra_fence {
        operations.push(ProductionRankedOperationV1::Fence {
            memory_scope: dialect_gpu::MemoryScopeAttr::Device,
            address_space: dialect_gpu::AddressSpaceAttr::Global,
            order: dialect_gpu::MemoryOrderAttr::AcquireRelease,
        });
    }
    let kernel = ProductionRankedKernelV1::new(
        root.function_name(),
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    root.lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("unit_local_stage_mutation", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(root.lowering.all_mandatory_reports_are_clean());
}

fn stage_result(
    owner: &ProductionPreRankedKirOwnerV1,
    roots: &[ProductionRankedSemanticProjectionRootV1],
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = stage_floor(owner);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result = owner.with_checked_unit_local_ranked_stage_v1(roots, &mut budget, |_, _| Ok(()));
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn unit_ranked_stage_retains_shared_helpers_and_each_actual_call_occurrence() {
    for calls in [&[1][..], &[2][..], &[2, 2][..], &[0, 2][..]] {
        for case in [
            UnitCase::Initializer,
            UnitCase::ReadThenWrite,
            UnitCase::ScalarSlot,
        ] {
            let owner = unit_owner(case, calls);
            let roots = ranked_stage_roots(&owner);
            let identity = *owner.executable().canonical().identity();
            let floor = stage_floor(&owner);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            owner
                .with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |stage, budget| {
                    assert_eq!(stage.root_count(), calls.len());
                    assert_eq!(stage.local_call_count(), calls.iter().sum::<usize>());
                    assert!(!stage.grants_artifact_or_launch_authority());
                    let mut seen = [None; 4];
                    let mut seen_count = 0;
                    for (index, expected_calls) in calls.iter().enumerate() {
                        let root = stage.root(index, budget)?.unwrap();
                        assert_eq!(
                            root.selected_root(),
                            SemanticFunctionIdV1::from_index(index as u32)
                        );
                        assert_eq!(root.local_calls().len(), *expected_calls);
                        assert_eq!(root.entry_translation().memory_effects(), 0);
                        assert_eq!(root.entry_translation().value_expressions(), 0);
                        for call in root.local_calls() {
                            assert_eq!(call.root(), root.selected_root());
                            let key = Some((call.root(), call.native_call()));
                            assert!(!seen[..seen_count].contains(&key));
                            seen[seen_count] = key;
                            seen_count += 1;
                            assert!(matches!(call.operation().kind, OperationKind::Call { .. }));
                            assert!(owner.helper_memory.unit_source.calls.iter().any(|row| {
                                row.caller.root == call.root()
                                    && row.call == call.native_call()
                                    && row.source_block == call.source_block()
                            }));
                        }
                    }
                    assert!(stage.root(usize::MAX, budget)?.is_none());
                    Ok(())
                })
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(*owner.executable().canonical().identity(), identity);
            assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
            assert!(!owner.helper_memory.accesses.is_empty());
            assert!(!owner.helper_memory.unit_source.memory.is_empty());
        }
    }
}

#[test]
fn unit_ranked_stage_success_does_not_unlock_legacy_candidate_or_attachment() {
    let owner = unit_owner(UnitCase::Initializer, &[2, 2]);
    let roots = ranked_stage_roots(&owner);
    stage_result(&owner, &roots).unwrap();
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
    let owner = unit_owner(UnitCase::ReadThenWrite, &[1]);
    let roots = ranked_stage_roots(&owner);
    stage_result(&owner, &roots).unwrap();
    // Test-only packaging exercises the independent attachment fence without
    // bypassing source construction or pretending the public constructor passed.
    let receipt = ProductionMaterializedRankedModuleReceiptV1 {
        materialized: owner,
        roots: roots.into_boxed_slice(),
    };
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt),
        Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "ranked attachment"
            }
        )
    ));
}

#[test]
fn unit_ranked_stage_requires_the_complete_ordered_source_root_roster() {
    for mutation in 0..4 {
        let owner = unit_owner(UnitCase::Initializer, &[1, 1]);
        let mut roots = ranked_stage_roots(&owner);
        match mutation {
            0 => {
                roots.pop();
            }
            1 => roots.swap(0, 1),
            2 => roots[1].selected_root = roots[0].selected_root,
            3 => roots[0].launch_rank = 2,
            _ => unreachable!(),
        }
        assert!(matches!(
            stage_result(&owner, &roots),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
    }
}

#[test]
fn unit_ranked_stage_keeps_exact_layout_and_access_map_validation() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let mut roots = ranked_stage_roots(&owner);
    replace_stage_ranked_body(&mut roots[0], true, false);
    assert!(matches!(
        stage_result(&owner, &roots),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "ranked execution layout changed after executable materialization",
            ..
        })
    ));
    let mut roots = ranked_stage_roots(&owner);
    roots[0].access_sources =
        vec![ProductionRankedAccessSourceV1::new(0, Some(0), 0, 0, 0)].into_boxed_slice();
    assert!(matches!(
        stage_result(&owner, &roots),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "ranked projection receipt has invalid access correspondence",
            ..
        })
    ));
}

#[test]
fn unit_ranked_stage_checks_entry_translation_after_structural_roster() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let mut roots = ranked_stage_roots(&owner);
    replace_stage_ranked_body(&mut roots[0], false, true);
    validate_source_ranked_roster_v1(&owner.semantic_ssa, &owner.source_launch, &roots).unwrap();
    assert!(matches!(
        stage_result(&owner, &roots),
        Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
            ProductionMirPlironTranslationErrorV1::SynchronizationMismatch
        ))
    ));
}

#[test]
fn unit_ranked_stage_does_not_borrow_a_foreign_identical_inventory() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let other = unit_owner(UnitCase::Initializer, &[1]);
    let roots = ranked_stage_roots(&owner);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(stage_floor(&owner) + stage_floor(&other))
        .unwrap();
    let (inventory, storage) =
        CanonicalKirInventoryV1::derive(other.executable(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    let mut entered = false;
    assert!(matches!(
        with_unit_local_ranked_stage_inventory_v1(
            &owner,
            &roots,
            &inventory,
            &mut budget,
            |_, _| {
                entered = true;
                Ok(())
            }
        ),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(!entered);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
}

#[test]
fn unit_ranked_stage_rejects_missing_duplicate_or_misbound_sealed_call_rows() {
    for mutation in 0..5 {
        let mut owner = unit_owner(UnitCase::Initializer, &[2, 2]);
        let roots = ranked_stage_roots(&owner);
        let rows = &mut owner.helper_memory.unit_source.calls;
        match mutation {
            0 => {
                rows.pop();
            }
            1 => rows[1] = rows[0],
            2 => rows[0].call = rows[1].call,
            3 => rows[0].caller.root = SemanticFunctionIdV1::from_index(1),
            4 => rows[0].callee_association = rows[2].callee_association,
            _ => unreachable!(),
        }
        assert!(matches!(
            stage_result(&owner, &roots),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
}

#[test]
fn unit_ranked_stage_rejects_wrong_association_counts_even_when_total_calls_match() {
    let mut owner = unit_owner(UnitCase::Initializer, &[2, 2]);
    let roots = ranked_stage_roots(&owner);
    let rows = &mut owner.helper_memory.unit_source.associations;
    assert_eq!(rows.len(), 2);
    rows[0].call_count += 1;
    rows[1].call_count -= 1;
    assert!(matches!(
        stage_result(&owner, &roots),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn unit_ranked_stage_raw_empty_refusal_and_initial_work_are_allocation_free() {
    let owner = array_owner(ArrayCase::RetainedValueRead);
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::RawEmpty
    );
    for limit in [1, 6, 7] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let result = owner.with_checked_unit_local_ranked_stage_v1(&[], &mut budget, |_, _| Ok(()));
        if limit == 7 {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "ranked local-call stage requires retained Unit-local source evidence",
                    ..
                })
            ));
            assert_eq!(budget.work(), 7);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
            assert_eq!(budget.work(), if limit == 1 { 0 } else { 2 });
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (FLOOR, FLOOR));
    }
}

#[test]
fn unit_ranked_stage_requires_full_owner_floor_before_new_inventory() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let roots = ranked_stage_roots(&owner);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() - 1;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        owner.with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |_, _| Ok(())),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
    assert_eq!(budget.work(), 8);
}

#[test]
fn unit_ranked_stage_exact_and_one_short_work_and_actual_storage_restore_floor() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let roots = ranked_stage_roots(&owner);
    let floor = stage_floor(&owner);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    owner
        .with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |_, _| Ok(()))
        .unwrap();
    assert_eq!(budget.storage(), floor);
    let exact_work = budget.work();
    let exact_storage = budget.peak_storage();
    for (work_limit, storage_limit, expected) in [
        (exact_work, exact_storage, 0),
        (exact_work - 1, exact_storage, 1),
        (exact_work, exact_storage - 1, 2),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let mut entered = false;
        let result = owner.with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |_, _| {
            entered = true;
            Ok(())
        });
        match expected {
            0 => {
                result.unwrap();
                assert!(entered);
                assert_eq!(budget.work(), exact_work);
            }
            1 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            )),
            2 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            )),
            _ => unreachable!(),
        }
        if expected != 0 {
            assert!(!entered);
        }
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn unit_ranked_stage_root_query_is_paid_and_foreign_ledger_is_rejected() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let roots = ranked_stage_roots(&owner);
    let floor = stage_floor(&owner);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    owner
        .with_checked_unit_local_ranked_stage_v1(&roots, &mut budget, |stage, budget| {
            let before = budget.work();
            assert!(stage.root(0, budget)?.is_some());
            assert_eq!(budget.work() - before, 6);
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut other = ArgumentBudgetV1::new(&mut other_work, STORAGE);
            other.reserve_storage(budget.storage()).unwrap();
            assert!(matches!(
                stage.root(0, &mut other),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!(other.work(), 6);
            budget.release_storage(1)?;
            let below_floor = stage.root(0, budget);
            budget.reserve_storage(1)?;
            assert!(matches!(
                below_floor,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn unit_ranked_stage_callback_error_panic_and_storage_imbalance_keep_original_floor() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let roots = ranked_stage_roots(&owner);
    let floor = stage_floor(&owner);
    for mode in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.with_checked_unit_local_ranked_stage_v1(
                &roots,
                &mut budget,
                |_, budget| -> Result<(), ProductionSemanticKirErrorV1> {
                    match mode {
                        0 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                        1 => panic!("scoped stage callback"),
                        2 => {
                            budget.reserve_storage(1)?;
                            Ok(())
                        }
                        3 => {
                            budget.reserve_storage(1)?;
                            panic!("accounting takes precedence");
                        }
                        _ => unreachable!(),
                    }
                },
            )
        }));
        match mode {
            0 => assert!(matches!(
                result,
                Ok(Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch))
            )),
            1 => assert!(result.is_err()),
            2 | 3 => assert!(matches!(
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
