use super::*;

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

fn masked_fixture(
    operation: SemanticBinaryOpV1,
    mask: Option<u128>,
    shared: bool,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    fixture_with_blocks_and_symbol(
        Fixture::Literal(true),
        shared,
        |ordinal, old| {
            if shared && ordinal != 0 {
                return old;
            }
            vec![
                block(
                    201,
                    vec![
                        assignment(1, U32, SemanticRvalueKindV1::Use(constant(U32, 123, 4))),
                        assignment(
                            2,
                            U32,
                            SemanticRvalueKindV1::Use(constant(U32, u32::MAX.into(), 4)),
                        ),
                        assignment(
                            3,
                            U32,
                            match mask {
                                Some(bits) => SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::BitAnd,
                                    left: value(2, U32),
                                    right: constant(U32, bits, 4),
                                },
                                None => SemanticRvalueKindV1::Use(value(2, U32)),
                            },
                        ),
                        assignment(
                            4,
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: value(3, U32),
                                right: constant(U32, 32, 4),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::Assert {
                        condition: SemanticOperandV1::Move(place(4, BOOL)),
                        expected: true,
                        message: SemanticAssertMessageV1::Overflow {
                            operation,
                            left: value(1, U32),
                            right: value(3, U32),
                        },
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    202,
                    vec![assignment(
                        5,
                        U32,
                        SemanticRvalueKindV1::Binary {
                            operation,
                            left: value(1, U32),
                            right: SemanticOperandV1::Move(place(3, U32)),
                        },
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ]
        },
        |ordinal| format!("masked_assert_{ordinal}"),
        &[U32, U32, U32, BOOL, U32],
    )
}

fn private_plan() -> LoweredFunctionPlanV1 {
    LoweredFunctionPlanV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        kernel_ir_function: FunctionId::new("masked_assert_0"),
        role: SemanticKirFunctionRoleV1::KernelEntry,
        parameter_declarations: Vec::new(),
        parameter_types: Vec::new(),
        parameter_values: Vec::new(),
        call_arguments: Vec::new(),
        parameter_local_bindings: Vec::new(),
        parameter_component_bindings: Vec::new(),
        ignored_parameter_bindings: Vec::new(),
        result_types: Vec::new(),
    }
}

#[test]
fn masked_assertion_one_decision_elides_exact_origin_and_failure_block() {
    for operation in [
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
    ] {
        let (ssa, launch) = masked_fixture(operation, Some(31), false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), FLOOR);
        let payload = retained(&owner);
        budget.reserve_storage(payload).unwrap();
        let binding = owner
            .assert_origins()
            .assert_condition(ROOT, ROOT, SemanticBlockIdV1::from_index(0), &mut budget)
            .unwrap();
        let SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } =
            binding.outcome()
        else {
            panic!("exact source proof must record its actual branch");
        };
        assert_eq!(binding.semantic_success().index(), 1);
        let graph = owner.executable().module();
        let body = graph.functions[success_edge.source.function.0 as usize]
            .body
            .as_ref()
            .unwrap();
        assert_eq!(
            body.blocks.len(),
            2,
            "sizing must not reserve a stale failure block"
        );
        assert!(matches!(
            assert_origin_block_v1(graph, success_edge.source)
                .unwrap()
                .terminator,
            Some(Terminator::Branch { .. })
        ));
        drop(owner);
        budget.release_storage(payload).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn masked_assertion_wrong_mask_and_raw_count_keep_runtime_origin() {
    for mask in [None, Some(63), Some(30)] {
        let (ssa, launch) = masked_fixture(SemanticBinaryOpV1::ShiftLeft, mask, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let payload = retained(&owner);
        budget.reserve_storage(payload).unwrap();
        let binding = owner
            .assert_origins()
            .assert_condition(ROOT, ROOT, SemanticBlockIdV1::from_index(0), &mut budget)
            .unwrap();
        assert!(matches!(
            binding.outcome(),
            SemanticKirAssertConditionOutcomeV1::Emitted { .. }
        ));
        drop(owner);
        budget.release_storage(payload).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn masked_assertion_shared_helper_uses_actual_plan_owner_without_root_dispatch() {
    let (ssa, launch) = masked_fixture(SemanticBinaryOpV1::ShiftRight, Some(31), true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    let origins = owner.assert_origins();
    let one = origins
        .assert_condition(
            SemanticFunctionIdV1::from_index(1),
            ROOT,
            SemanticBlockIdV1::from_index(0),
            &mut budget,
        )
        .unwrap();
    let other = origins
        .assert_condition(
            SemanticFunctionIdV1::from_index(3),
            ROOT,
            SemanticBlockIdV1::from_index(0),
            &mut budget,
        )
        .unwrap();
    assert_eq!(one, other);
    assert!(matches!(
        one.outcome(),
        SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
    ));
    drop(owner);
    budget.release_storage(payload).unwrap();
}

#[test]
fn masked_assertion_private_table_rejects_other_source_plan_and_part_slices() {
    let (owner, _) = masked_fixture(SemanticBinaryOpV1::ShiftLeft, Some(31), false);
    let (other, _) = masked_fixture(SemanticBinaryOpV1::ShiftLeft, Some(31), false);
    let plans = [private_plan()];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    with_masked_assertion_plans_v1(&owner, &plans, BTreeSet::new(), &mut budget, |tables, _| {
        let decision =
            masked_decisions_for_plan_v1(owner.source_semantic(), &plans[0], &tables[0])?;
        assert!(decision.contains(&0));
        assert!(!decision.contains(&1));
        assert!(matches!(
            decision.require_source(other.source_semantic(), &plans[0]),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        let mut wrong = plans[0].clone();
        assert!(matches!(
            decision.require_source(owner.source_semantic(), &wrong),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        wrong.correspondence_owner = SemanticFunctionIdV1::from_index(1);
        assert!(matches!(
            decision.require_source(owner.source_semantic(), &wrong),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        let source = owner.source_semantic();
        let cloned_types = source.types().to_vec();
        assert!(matches!(
            decision.require_parts(
                &cloned_types,
                source.callables(),
                &source.functions()[0],
                ROOT,
                ROOT,
            ),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(!InfallibleAssertDecisionsV1::Legacy(BTreeSet::new()).contains(&0));
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn plan_profile(
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
) {
    let (owner, _) = masked_fixture(SemanticBinaryOpV1::ShiftLeft, Some(31), false);
    let plans = [private_plan()];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_masked_assertion_plans_v1(
        &owner,
        &plans,
        BTreeSet::new(),
        &mut budget,
        |tables, _| {
            assert!(
                masked_decisions_for_plan_v1(owner.source_semantic(), &plans[0], &tables[0])?
                    .contains(&0)
            );
            Ok(())
        },
    );
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.storage(),
    )
}

#[test]
fn masked_assertion_plan_exact_and_one_short_budgets_keep_unrelated_floor() {
    let (result, work, peak, floor) = plan_profile(WORK, STORAGE);
    result.unwrap();
    assert_eq!(floor, FLOOR);
    let exact = plan_profile(work, peak);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, peak, FLOOR));
    let short_work = plan_profile(work - 1, peak);
    assert!(matches!(
        short_work.0,
        Err(ProductionSemanticKirErrorV1::MaskedAssertionQuery(
            crate::ProductionSemanticMaskedShiftQueryErrorV1::Resource(ArgumentResourceV1::Work(_))
        )) | Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert_eq!(short_work.3, FLOOR);
    let short_storage = plan_profile(work, peak - 1);
    assert!(matches!(
        short_storage.0,
        Err(ProductionSemanticKirErrorV1::MaskedAssertionQuery(
            crate::ProductionSemanticMaskedShiftQueryErrorV1::Resource(
                ArgumentResourceV1::Storage(_)
            )
        )) | Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
    assert_eq!(short_storage.3, FLOOR);
}

#[test]
fn masked_assertion_scope_drops_tables_without_releasing_caller_output() {
    let (owner, _) = masked_fixture(SemanticBinaryOpV1::ShiftLeft, Some(31), false);
    let plans = [private_plan()];
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_masked_assertion_plans_v1(
            &owner,
            &plans,
            BTreeSet::new(),
            &mut budget,
            |_, budget| {
                budget.reserve_storage(23)?;
                match mode {
                    0 => Ok(()),
                    1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                    _ => panic!("consumer callback panic"),
                }
            },
        );
        match mode {
            0 => result.unwrap(),
            1 => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            )),
            _ => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::MaskedAssertionQuery(
                    crate::ProductionSemanticMaskedShiftQueryErrorV1::Panicked
                ))
            )),
        }
        assert_eq!(budget.storage(), FLOOR + 23);
        budget.release_storage(23).unwrap();
    }
}

#[test]
fn masked_assertion_table_refuses_foreign_budget_slot_before_charge() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let mut accounting = MaskedPlanStorageV1::new(&budget);
    let rows = accounting.table::<u32>(3, &mut budget).unwrap();
    let accepted = accounting.accepted;
    let original_work = budget.work();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut foreign =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut foreign_work, STORAGE);
    foreign.reserve_storage(FLOOR + accepted).unwrap();
    assert!(matches!(
        accounting.table::<u32>(1, &mut foreign),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), FLOOR + accepted);
    assert_eq!(budget.work(), original_work);
    drop(rows);
    budget.release_storage(accepted).unwrap();
}

#[test]
fn masked_assertion_scope_rejects_ledger_swap_and_floor_undercut_without_cleanup() {
    let (owner, _) = masked_fixture(SemanticBinaryOpV1::ShiftLeft, Some(31), false);
    let plans = [private_plan()];
    for foreign_ledger in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut other_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let mut other =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut other_work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        other.reserve_storage(43).unwrap();
        let other_token = other.work_ledger_identity_v1();
        let dropped = std::cell::Cell::new(0);
        struct ResultOwner<'a>(&'a std::cell::Cell<usize>);
        impl Drop for ResultOwner<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let result = with_masked_assertion_plans_v1(
            &owner,
            &plans,
            BTreeSet::new(),
            &mut budget,
            |_, budget| {
                if foreign_ledger {
                    std::mem::swap(budget, &mut other);
                } else {
                    budget.release_storage(budget.storage() - FLOOR + 1)?;
                }
                Ok(ResultOwner(&dropped))
            },
        );
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(dropped.get(), 1);
        if foreign_ledger {
            assert!(budget.work_ledger_identity_v1() == other_token);
            assert_eq!((budget.work(), budget.storage()), (0, 43));
            assert!(other.storage() > FLOOR);
            // Deliberate ownership violation: recovery belongs to the caller.
            other.release_storage(other.storage() - FLOOR).unwrap();
        } else {
            assert_eq!(budget.storage(), FLOOR - 1);
            budget.reserve_storage(1).unwrap();
        }
    }
}

#[test]
fn masked_assertion_absent_pattern_keeps_legacy_bounds_without_query_tables() {
    let (owner, _) = fixture(Fixture::Literal(true), false);
    let plans = [private_plan()];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    with_masked_assertion_plans_v1(
        &owner,
        &plans,
        BTreeSet::from([1]),
        &mut budget,
        |tables, _| {
            assert!(tables[0].as_ref().unwrap().rows.is_empty());
            let decisions =
                masked_decisions_for_plan_v1(owner.source_semantic(), &plans[0], &tables[0])?;
            assert!(!decisions.contains(&0));
            assert!(decisions.contains(&1));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
