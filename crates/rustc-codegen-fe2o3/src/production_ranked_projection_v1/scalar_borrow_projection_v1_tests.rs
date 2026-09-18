use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as B, CanonicalKernelIrWorkBudgetV1 as W,
};
use scalar_borrow_projection_v1::with_scalar_private_borrows_v1 as scope;

include!("scalar_borrow_fixture_v1_tests.rs");

#[test]
fn scalar_borrow_normal_projector_reaches_nonidentity_policy5_and_native_output() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let (source, inputs, _) = borrow_source_fixture();
        let original = *source.executable().canonical().identity();
        let input_storage = source.unit_local_source_storage_floor_v1().unwrap();
        let program = project_and_verify_ranked_materialized_semantic_mir_v1(
            source,
            &inputs,
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        )
        .unwrap();
        assert!(program.all_kernel_checks_are_clean());
        assert!(!program.roots[0].access_sources.is_empty());
        let verified = program.into_verified_roster_receipt().unwrap();
        let mut work = W::new(
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
        );
        let mut budget = B::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        budget.reserve_storage(19 + input_storage).unwrap();
        let (source, verification, source_storage) = verified
            .into_silent_unit_erased_source_v1(&mut budget)
            .unwrap();
        budget
            .reserve_storage(source_storage.retained_storage())
            .unwrap();
        let binding =
            dialect_amdgcn::bind_production_target_v1(source.erased().module(), profile).unwrap();
        let (bound, bound_storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(binding.module(), &mut budget).unwrap();
        budget
            .reserve_storage(bound_storage.retained_storage())
            .unwrap();
        drop(binding);
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
                .unwrap();
        assert_eq!(checked.load_forwarding_rows().len(), 1);
        assert!(checked.intermediate_policy4().forwarding_rows().is_empty());
        let checked_storage = checked.retained_storage();
        budget.reserve_storage(checked_storage).unwrap();
        let admitted = fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1::try_admit_v1(source, bound, checked, &mut budget).unwrap();
        admitted.verify_equivalence(&mut budget).unwrap();
        assert_eq!(
            *admitted
                .original_source()
                .executable()
                .canonical()
                .identity(),
            original
        );
        assert_ne!(
            admitted
                .checked_output()
                .intermediate_policy4()
                .owner()
                .canonical()
                .identity(),
            admitted.output().canonical().identity()
        );
        budget
            .reserve_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
            .unwrap();
        let text = match profile {
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(admitted.output()),
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(admitted.output()),
        }.unwrap();
        assert_eq!(
            text.lines()
                .filter(|line| line.contains(" = load i32, ptr addrspace(5) "))
                .count(),
            1
        );
        drop(text);
        budget
            .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
            .unwrap();
        drop(admitted);
        drop(verification);
        budget
            .release_storage(
                input_storage
                    + source_storage.retained_storage()
                    + bound_storage.retained_storage()
                    + checked_storage,
            )
            .unwrap();
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn scalar_borrow_genuine_source_materializes_exact_direct_private_pointer() {
    let (source, _, _) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let (result, _, _) = borrow_run(
        semantic.types(),
        &semantic.functions()[0],
        semantic.target(),
        usize::MAX,
        usize::MAX,
    );
    assert_eq!(result.unwrap(), [true, true]);
    let module = source.executable().module();
    let mut matched = false;
    for function in &module.functions {
        for block in function.body.iter().flat_map(|body| &body.blocks) {
            let pointers: Vec<_> = block
                .operations
                .iter()
                .filter_map(|op| match op.kind {
                    fe2o3_kernel_ir::OperationKind::Load { pointer, access }
                        if access.address_space == fe2o3_kernel_ir::AddressSpace::Private =>
                    {
                        Some(pointer)
                    }
                    _ => None,
                })
                .collect();
            matched |= pointers.windows(2).any(|pair| pair[0] == pair[1]);
        }
    }
    assert!(matched);
}

#[test]
fn scalar_borrow_census_keeps_shared_raw_and_overlapping_borrows_closed() {
    let (source, _, alias) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let alias_type = function.locals()[alias].ty();
    for value in [
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: typed_place(3, A_U32),
        },
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable,
            place: typed_place(3, A_U32),
        },
    ] {
        // Deliberate eligibility-only mutation; not an admitted source owner.
        let mut statements = function.blocks()[1].statements().to_vec();
        statements[1] = typed_assignment(alias as u32, alias_type, value);
        let changed = borrow_replace_statements(function, 1, statements);
        assert_eq!(
            borrow_run(
                semantic.types(),
                &changed,
                semantic.target(),
                usize::MAX,
                usize::MAX
            )
            .0
            .unwrap(),
            [false; 2]
        );
    }
    let mut statements = function.blocks()[1].statements().to_vec();
    statements[2] = statements[1].clone();
    let changed = borrow_replace_statements(function, 1, statements);
    assert_eq!(
        borrow_run(
            semantic.types(),
            &changed,
            semantic.target(),
            usize::MAX,
            usize::MAX
        )
        .0
        .unwrap(),
        [false; 2]
    );
}

#[test]
fn scalar_borrow_census_rejects_kills_reinitialization_and_alias_copy_escape() {
    let (source, _, alias) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let local = SemanticLocalIdV1::from_index(alias as u32);
    let alias_type = function.locals()[alias].ty();
    let failures = vec![
        statement(SemanticStatementKindV1::StorageDead(local)),
        statement(SemanticStatementKindV1::StorageLive(local)),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(3),
        )),
        statement(SemanticStatementKindV1::Deinitialize(typed_place(3, A_U32))),
        typed_assignment(
            3,
            A_U32,
            SemanticRvalueKindV1::Use(typed_constant(A_U32, 8, 4)),
        ),
        typed_assignment(
            alias as u32,
            alias_type,
            SemanticRvalueKindV1::Use(typed_operand(alias as u32, alias_type)),
        ),
    ];
    for failure in failures {
        let mut statements = function.blocks()[1].statements().to_vec();
        statements[2] = failure;
        let changed = borrow_replace_statements(function, 1, statements);
        assert_eq!(
            borrow_run(
                semantic.types(),
                &changed,
                semantic.target(),
                usize::MAX,
                usize::MAX
            )
            .0
            .unwrap(),
            [false; 2]
        );
    }
    for failure in [
        SemanticStatementKindV1::Nop,
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
    ] {
        let mut statements = function.blocks()[1].statements().to_vec();
        statements[0] = statement(failure);
        let changed = borrow_replace_statements(function, 1, statements);
        assert_eq!(
            borrow_run(
                semantic.types(),
                &changed,
                semantic.target(),
                usize::MAX,
                usize::MAX
            )
            .0
            .unwrap(),
            [false; 2]
        );
    }
}

#[test]
fn scalar_borrow_census_refuses_cross_block_value_use_but_allows_later_lifetime_end() {
    let (source, _, alias) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let changed = borrow_replace_statements(
        function,
        2,
        vec![function.blocks()[1].statements()[3].clone()],
    );
    assert_eq!(
        borrow_run(
            semantic.types(),
            &changed,
            semantic.target(),
            usize::MAX,
            usize::MAX
        )
        .0
        .unwrap(),
        [false; 2]
    );
    let ended = borrow_replace_statements(
        function,
        2,
        vec![statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(alias as u32),
        ))],
    );
    assert_eq!(
        borrow_run(
            semantic.types(),
            &ended,
            semantic.target(),
            usize::MAX,
            usize::MAX
        )
        .0
        .unwrap(),
        [true; 2]
    );
}

#[test]
fn scalar_borrow_query_requires_exact_source_occurrence_provenance_and_live_floor() {
    let (source, _, alias) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let foreign_function = function.clone();
    let foreign_types = semantic.types().to_vec();
    let place = borrow_read_place(function, 3);
    let foreign_place = place.clone();
    let provenance = local_provenance_v1(semantic.types(), function)
        .unwrap()
        .allocation_provenance[alias];
    let site = ProjectedSemanticAccessSiteV1 {
        block: 1,
        statement: Some(3),
    };
    let mut work = W::new(usize::MAX);
    let mut budget = B::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    scope(
        semantic.types(),
        function,
        semantic.target(),
        &mut BorrowMeter {
            budget: &mut budget,
        },
        |census, facts| {
            let census = census.unwrap();
            assert!(
                census
                    .resolve(
                        &foreign_function,
                        semantic.types(),
                        semantic.target(),
                        site,
                        place,
                        AccessKindAttr::Read,
                        None,
                        provenance,
                        facts
                    )
                    .is_err()
            );
            assert!(
                census
                    .resolve(
                        function,
                        &foreign_types,
                        semantic.target(),
                        site,
                        place,
                        AccessKindAttr::Read,
                        None,
                        provenance,
                        facts
                    )
                    .is_err()
            );
            assert!(
                census
                    .resolve(
                        function,
                        semantic.types(),
                        semantic.target(),
                        site,
                        &foreign_place,
                        AccessKindAttr::Read,
                        None,
                        provenance,
                        facts
                    )?
                    .is_none()
            );
            assert!(
                census
                    .resolve(
                        function,
                        semantic.types(),
                        semantic.target(),
                        site,
                        place,
                        AccessKindAttr::Read,
                        None,
                        None,
                        facts
                    )
                    .is_err()
            );
            let before = facts.budget.work();
            facts.budget.release_storage(1).unwrap();
            assert!(
                census
                    .resolve(
                        function,
                        semantic.types(),
                        semantic.target(),
                        site,
                        place,
                        AccessKindAttr::Read,
                        None,
                        provenance,
                        facts
                    )
                    .is_err()
            );
            assert_eq!(facts.budget.work(), before);
            facts.budget.reserve_storage(1).unwrap();
            assert!(
                census
                    .resolve(
                        function,
                        semantic.types(),
                        semantic.target(),
                        site,
                        place,
                        AccessKindAttr::Read,
                        None,
                        provenance,
                        facts
                    )?
                    .is_some()
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn scalar_borrow_query_refuses_same_slot_replacement_ledger_without_debit() {
    let (source, _, alias) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let provenance = local_provenance_v1(semantic.types(), function)
        .unwrap()
        .allocation_provenance[alias];
    let mut other_work = W::new(usize::MAX);
    let mut work = W::new(usize::MAX);
    let mut budget = B::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    scope(
        semantic.types(),
        function,
        semantic.target(),
        &mut BorrowMeter {
            budget: &mut budget,
        },
        |census, facts| {
            let mut foreign = B::new(&mut other_work, usize::MAX);
            foreign.reserve_storage(facts.budget.storage()).unwrap();
            let original = std::mem::replace(facts.budget, foreign);
            assert!(
                census
                    .unwrap()
                    .resolve(
                        function,
                        semantic.types(),
                        semantic.target(),
                        ProjectedSemanticAccessSiteV1 {
                            block: 1,
                            statement: Some(3)
                        },
                        borrow_read_place(function, 3),
                        AccessKindAttr::Read,
                        None,
                        provenance,
                        facts
                    )
                    .is_err()
            );
            assert_eq!(facts.budget.work(), 0);
            let _ = std::mem::replace(facts.budget, original);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn scalar_borrow_exact_and_one_short_budget_paths_restore_caller_floor() {
    let (source, _, _) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let (result, work, peak) = borrow_run(
        semantic.types(),
        function,
        semantic.target(),
        usize::MAX,
        usize::MAX,
    );
    assert_eq!(result.unwrap(), [true; 2]);
    assert!(
        peak > 19 + std::mem::size_of::<scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'_>>()
    );
    assert!(
        borrow_run(semantic.types(), function, semantic.target(), work, peak)
            .0
            .is_ok()
    );
    assert!(
        borrow_run(
            semantic.types(),
            function,
            semantic.target(),
            work - 1,
            peak
        )
        .0
        .is_err()
    );
    assert!(
        borrow_run(
            semantic.types(),
            function,
            semantic.target(),
            work,
            peak - 1
        )
        .0
        .is_err()
    );
}

#[test]
fn scalar_borrow_scope_preserves_original_panic_even_after_accounting_damage() {
    let (source, _, _) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    for damage in [false, true] {
        let mut work = W::new(usize::MAX);
        let mut budget = B::new(&mut work, usize::MAX);
        budget.reserve_storage(19).unwrap();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ProductionRankedProjectionErrorV1> = scope(
                semantic.types(),
                function,
                semantic.target(),
                &mut BorrowMeter {
                    budget: &mut budget,
                },
                |_, facts| {
                    if damage {
                        facts
                            .budget
                            .release_storage(facts.budget.storage())
                            .unwrap();
                    }
                    panic!("scalar-borrow original panic");
                },
            );
        }))
        .unwrap_err();
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"scalar-borrow original panic")
        );
        assert_eq!(budget.storage(), if damage { 0 } else { 19 });
    }
    let mut other_work = W::new(usize::MAX);
    let mut work = W::new(usize::MAX);
    let mut budget = B::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    let mut foreign = B::new(&mut other_work, usize::MAX);
    foreign.reserve_storage(23).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), ProductionRankedProjectionErrorV1> = scope(
            semantic.types(),
            function,
            semantic.target(),
            &mut BorrowMeter {
                budget: &mut budget,
            },
            |_, facts| {
                *facts.budget = foreign;
                panic!("scalar-borrow replaced-ledger panic");
            },
        );
    }))
    .unwrap_err();
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"scalar-borrow replaced-ledger panic")
    );
    assert_eq!((budget.storage(), budget.work()), (23, 0));
}

#[test]
fn scalar_borrow_scope_error_and_surplus_payload_cannot_leak_census_reservations() {
    let (source, _, _) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    for surplus in [false, true] {
        let mut work = W::new(usize::MAX);
        let mut budget = B::new(&mut work, usize::MAX);
        budget.reserve_storage(19).unwrap();
        let result: Result<(), ProductionRankedProjectionErrorV1> = scope(
            semantic.types(),
            function,
            semantic.target(),
            &mut BorrowMeter {
                budget: &mut budget,
            },
            |_, facts| {
                if surplus {
                    facts.budget.reserve_storage(7).unwrap();
                }
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "test callback refusal",
                ))
            },
        );
        assert!(result.is_err());
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn scalar_borrow_no_candidate_path_is_allocation_free_and_linearly_metered() {
    let (source, _) = erased_backend_materialized_fixture_v1(true, 1);
    let semantic = source.semantic_ssa().source_semantic();
    let mut statements = semantic.functions()[0].blocks()[1].statements().to_vec();
    statements.extend((0..1024).map(|_| statement(SemanticStatementKindV1::Nop)));
    let function = borrow_replace_statements(&semantic.functions()[0], 1, statements);
    let expected = 6
        + 3 * function.blocks().len()
        + 3 * function
            .blocks()
            .iter()
            .map(|b| b.statements().len())
            .sum::<usize>();
    for limit in [expected, expected - 1] {
        let mut work = W::new(limit);
        let mut budget = B::new(&mut work, 19);
        budget.reserve_storage(19).unwrap();
        let result = scope(
            semantic.types(),
            &function,
            semantic.target(),
            &mut BorrowMeter {
                budget: &mut budget,
            },
            |census, _| {
                assert!(census.is_none());
                Ok(())
            },
        );
        assert_eq!(result.is_ok(), limit == expected);
        assert_eq!((budget.storage(), budget.peak_storage()), (19, 19));
        if limit == expected {
            assert_eq!(budget.work(), expected);
        }
    }
}
