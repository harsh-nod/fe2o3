use super::*;

include!("production_local_helper_erased_source_fixture_v1_tests.rs");

#[path = "production_checked_output_erased_policy4_v1_tests.rs"]
mod final_erased_policy4_tests;

#[path = "production_unit_local_source_replay_v1_tests.rs"]
mod original_source_replay_tests;

#[path = "production_source_output_erased_v1_tests.rs"]
mod erased_occurrence_tests;

#[path = "native_erased_source_staging_v1_tests.rs"]
mod native_staging_tests;

type ErasedOwner = ProductionUnitLocalErasedSourceOwnerV1;

fn erased_input_floor(
    original: &ProductionPreRankedKirOwnerV1,
    roots: &Vec<ProductionRankedSemanticProjectionRootV1>,
) -> usize {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    ErasedOwner::input_storage_floor_v1(original, roots, &mut budget).unwrap()
}

fn produced_erased_owner(case: UnitCase, calls: &[usize]) -> ErasedOwner {
    let original = unit_owner(case, calls);
    let roots = ranked_stage_roots(&original);
    let input = erased_input_floor(&original, &roots);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR + input).unwrap();
    let (owner, extra) = ErasedOwner::try_produce_v1(original, roots, &mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR + input);
    budget.reserve_storage(extra.retained_storage()).unwrap();
    owner.verify_equivalence(&mut budget).unwrap();
    assert_eq!(
        owner.retained_storage_floor_v1(),
        input + extra.retained_storage()
    );
    owner
}

#[test]
fn erased_owner_produces_real_e_and_retains_complete_source_n_and_ranked_roots() {
    for calls in [&[1][..], &[2][..], &[2, 2][..], &[0, 2][..]] {
        for case in [
            UnitCase::Initializer,
            UnitCase::ScalarSlot,
            UnitCase::ReadThenWrite,
            UnitCase::CastAssert { expected: true },
            UnitCase::CastAssert { expected: false },
        ] {
            let owner = produced_erased_owner(case, calls);
            assert_eq!(owner.deleted_call_count(), calls.iter().sum::<usize>());
            assert_eq!(owner.deleted_function_count(), 1);
            assert_eq!(owner.ranked_root_count(), calls.len());
            assert_eq!(
                owner.erased().module(),
                &deletion_module(owner.original_source())
            );
            assert_ne!(
                owner.erased().canonical().canonical_bytes(),
                owner
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes()
            );
            assert!(
                !owner
                    .original_source()
                    .helper_memory
                    .unit_source
                    .memory
                    .is_empty()
            );
            assert!(!owner.original_source().helper_memory.accesses.is_empty());
            assert!(!owner.grants_artifact_or_launch_authority());
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            let floor = FLOOR + owner.retained_storage_floor_v1();
            budget.reserve_storage(floor).unwrap();
            owner
                .with_checked_erasure_v1(&mut budget, |checked, budget| {
                    assert!(std::ptr::eq(checked.source(), owner.original_source()));
                    assert!(std::ptr::eq(checked.output(), owner.erased()));
                    assert_eq!(checked.deleted_call_count(), calls.iter().sum::<usize>());
                    for (ordinal, function) in owner.functions.iter().enumerate() {
                        assert_eq!(checked.function(ordinal as u32, budget)?, *function);
                    }
                    Ok(())
                })
                .unwrap();
            for ordinal in 0..calls.len() {
                let before = budget.work();
                let candidate = owner
                    .ranked_candidate_v1(ordinal, &mut budget)
                    .unwrap()
                    .unwrap();
                assert_eq!(budget.work() - before, 7);
                assert_eq!(candidate.semantic_root(), ordinal as u32);
                assert_eq!(candidate.kernel(), owner.roots[ordinal].lowering.kernel());
                assert_eq!(
                    candidate.access_sources(),
                    owner.roots[ordinal].access_sources.as_ref()
                );
                assert_eq!(
                    candidate.executable_effect_sources(),
                    owner.roots[ordinal].executable_effect_sources.as_ref()
                );
                assert_eq!(candidate.ranked_ir(), owner.roots[ordinal].ranked_ir);
            }
            assert!(
                owner
                    .ranked_candidate_v1(calls.len(), &mut budget)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn erased_owner_preserves_raw_empty_calls_and_external_declarations() {
    for with_raw_empty in [false, true] {
        let original = if with_raw_empty {
            deletion_mixed_owner()
        } else {
            unit_owner(UnitCase::CastAssert { expected: false }, &[1])
        };
        let roots = ranked_stage_roots(&original);
        let expected = deletion_module(&original);
        let floor = FLOOR + erased_input_floor(&original, &roots);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (owner, extra) = ErasedOwner::try_produce_v1(original, roots, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(extra.retained_storage()).unwrap();
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(owner.erased().module(), &expected);
        assert_eq!(
            owner
                .original_source()
                .empty_effect_helpers()
                .iter()
                .count(),
            usize::from(with_raw_empty)
        );
        if with_raw_empty {
            let calls = owner
                .erased()
                .module()
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks)
                .flat_map(|b| &b.operations)
                .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
                .count();
            assert_eq!(calls, 1);
        } else {
            assert!(
                owner
                    .erased()
                    .module()
                    .functions
                    .iter()
                    .any(|f| f.role == fe2o3_kernel_ir::FunctionRole::ExternalImport)
            );
        }
    }
}

#[test]
fn erased_owner_checks_supplied_candidate_without_producing_a_replacement() {
    for mutation in 0..6 {
        let original = unit_owner(UnitCase::Initializer, &[1, 1]);
        let roots = ranked_stage_roots(&original);
        let mut candidate = deletion_module(&original);
        match mutation {
            0 => {}
            1 => candidate = original.executable().module().clone(),
            2 => candidate.functions.swap(0, 1),
            3 => candidate.id = "wrong-erased-module".into(),
            4 => {
                let helper = original
                    .executable()
                    .module()
                    .functions
                    .iter()
                    .enumerate()
                    .find(|(i, _)| {
                        matches!(
                            original.helper_memory.functions[*i],
                            RetainedHelperKindV1::Local { .. }
                        )
                    })
                    .unwrap()
                    .1;
                candidate.functions.push(helper.clone());
            }
            5 => {
                let root = &mut candidate.functions[0];
                let body = root.body.as_mut().unwrap();
                body.blocks[0].operations.push(Operation::new(
                    vec![ValueDef::new(ValueId(500), Type::Scalar(ScalarType::U32))],
                    OperationKind::Constant(Constant::U32(19)),
                ));
            }
            _ => unreachable!(),
        }
        // The test owns this arbitrary candidate separately. A fresh admitted
        // copy gives its existing receipt, then the owner independently admits
        // its own E from that exact borrowed Module.
        let (candidate, candidate_storage) = deletion_verified(&candidate);
        let floor =
            FLOOR + erased_input_floor(&original, &roots) + candidate_storage.retained_storage();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let result =
            ErasedOwner::try_from_candidate_v1(original, roots, candidate.module(), &mut budget);
        assert_eq!(budget.storage(), floor);
        if mutation == 0 {
            let (owner, extra) = result.unwrap();
            budget.reserve_storage(extra.retained_storage()).unwrap();
            assert_eq!(
                owner.erased().canonical().canonical_bytes(),
                candidate.canonical().canonical_bytes()
            );
            owner.verify_equivalence(&mut budget).unwrap();
        } else {
            assert!(result.is_err(), "mutation {mutation}");
        }
    }
}

#[test]
fn erased_owner_rechecks_ranked_custody_and_cached_maps() {
    for mutation in 0..5 {
        let mut owner = produced_erased_owner(UnitCase::Initializer, &[1, 1]);
        match mutation {
            0 => owner.functions[0] = Some(u32::MAX),
            1 => owner.operations[0] = None,
            2 => owner.deleted_calls += 1,
            3 => owner.roots.swap(0, 1),
            4 => replace_stage_ranked_body(&mut owner.roots[0], true, false),
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        let floor = FLOOR + owner.retained_storage_floor_v1();
        budget.reserve_storage(floor).unwrap();
        let mut entered = false;
        assert!(
            owner
                .with_checked_erasure_v1(&mut budget, |_, _| {
                    entered = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!entered);
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn erased_owner_input_floor_counts_actual_ranked_vec_and_text_capacities() {
    let original = unit_owner(UnitCase::Initializer, &[1]);
    let mut roots = ranked_stage_roots(&original);
    let before = erased_input_floor(&original, &roots);
    let old_rows = roots.capacity();
    let old_text = roots[0].ranked_ir.capacity();
    roots.reserve_exact(17);
    roots[0].ranked_ir.reserve_exact(29);
    let after = erased_input_floor(&original, &roots);
    assert_eq!(
        after - before,
        (roots.capacity() - old_rows)
            * std::mem::size_of::<ProductionRankedSemanticProjectionRootV1>()
            + roots[0].ranked_ir.capacity()
            - old_text
    );
    let expected = original.unit_local_source_storage_floor_v1().unwrap()
        + std::mem::size_of::<Vec<ProductionRankedSemanticProjectionRootV1>>()
        + roots.capacity() * std::mem::size_of::<ProductionRankedSemanticProjectionRootV1>()
        + roots[0]
            .lowering
            .production_analysis_retained_storage_upper_bound_v1()
        + roots[0].ranked_ir.capacity()
        + roots[0].access_sources.len() * std::mem::size_of::<ProductionRankedAccessSourceV1>()
        + roots[0].executable_effect_sources.len()
            * std::mem::size_of::<ProductionRankedExecutableEffectSourceV1>();
    assert_eq!(after, expected);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(after - 1).unwrap();
    let result = ErasedOwner::try_produce_v1(original, roots, &mut budget);
    assert!(matches!(
        result,
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        ))
    ));
    assert_eq!(budget.storage(), after - 1);
}

fn erased_budget_run(
    work_limit: usize,
    storage_limit: usize,
    borrowed: bool,
) -> (bool, usize, usize, usize, usize, usize) {
    let original = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let mut roots = ranked_stage_roots(&original);
    roots.reserve_exact(9);
    roots[0].ranked_ir.reserve_exact(13);
    let input = erased_input_floor(&original, &roots);
    let mut preparation_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut preparation = ArgumentBudgetV1::new(&mut preparation_work, STORAGE);
    preparation.reserve_storage(input).unwrap();
    let (copy, copy_storage) = original
        .executable()
        .copy_module_for_transformation_v12(&mut preparation)
        .unwrap();
    drop(copy);
    let candidate = if borrowed {
        Some(deletion_verified(&deletion_module(&original)))
    } else {
        None
    };
    let floor = FLOOR
        + input
        + candidate
            .as_ref()
            .map_or(0, |(_, receipt)| receipt.retained_storage());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = match candidate.as_ref() {
        Some((candidate, _)) => {
            ErasedOwner::try_from_candidate_v1(original, roots, candidate.module(), &mut budget)
        }
        None => ErasedOwner::try_produce_v1(original, roots, &mut budget),
    };
    assert_eq!(budget.storage(), floor);
    let additional = result.as_ref().map_or(0, |(owner, receipt)| {
        assert_eq!(owner.input_storage_floor(), input);
        assert_eq!(
            owner.retained_storage_floor_v1(),
            input + receipt.retained_storage()
        );
        assert_eq!(owner.additional_storage(), *receipt);
        receipt.retained_storage()
    });
    (
        result.is_ok(),
        budget.work(),
        budget.peak_storage(),
        floor,
        additional,
        copy_storage.retained_storage(),
    )
}

#[test]
fn erased_owner_exact_and_one_short_budgets_include_candidate_e_and_maps() {
    for borrowed in [false, true] {
        let full = erased_budget_run(WORK, STORAGE, borrowed);
        assert!(full.0);
        let exact = erased_budget_run(full.1, full.2, borrowed);
        assert_eq!(exact, full);
        assert!(!erased_budget_run(full.1 - 1, full.2, borrowed).0);
        assert!(!erased_budget_run(full.1, full.2 - 1, borrowed).0);
        assert!(full.2 > full.3 + full.4);
        if !borrowed {
            assert!(full.2 >= full.3 + full.4 + full.5);
        }
    }
}

#[test]
fn erased_owner_full_floor_is_required_for_replay_and_recipe_export() {
    let owner = produced_erased_owner(UnitCase::Initializer, &[1]);
    for floor in [
        owner.additional_storage().retained_storage(),
        owner.retained_storage_floor_v1() - 1,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            owner.verify_equivalence(&mut budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert!(matches!(
            owner.ranked_candidate_v1(0, &mut budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn erased_owner_replay_callback_error_panic_and_accounting_restore_floor() {
    let owner = produced_erased_owner(UnitCase::Initializer, &[1]);
    for mode in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        let floor = FLOOR + owner.retained_storage_floor_v1();
        budget.reserve_storage(floor).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.with_checked_erasure_v1(
                &mut budget,
                |_, budget| -> Result<(), ProductionSemanticKirErrorV1> {
                    match mode {
                        0 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                        1 => panic!("erased owner callback"),
                        2 => {
                            budget.reserve_storage(1)?;
                            Ok(())
                        }
                        3 => {
                            budget.reserve_storage(1)?;
                            panic!("erased owner accounting precedence")
                        }
                        4 => {
                            budget.release_storage(1)?;
                            Ok(())
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
            _ => assert!(matches!(
                result,
                Ok(Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ))
            )),
        }
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn erased_owner_never_relabels_unitlocal_source_as_legacy_attachment() {
    let owner = produced_erased_owner(UnitCase::Initializer, &[1]);
    let ErasedOwner {
        original, roots, ..
    } = owner;
    assert!(matches!(
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            original, roots
        ),
        Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "materialized ranked receipt"
            }
        )
    ));
}
