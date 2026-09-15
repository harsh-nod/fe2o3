use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1 as Inventory, CheckedCanonicalKirControlIndexV1 as Control,
    check_canonical_kir_transition_v1,
};
use fe2o3_pliron::KirPlironGraphV12;
use std::mem::size_of;

#[path = "production_retained_unreachable_component_v1_tests.rs"]
mod retained_unreachable_component_v1_tests;

fn with_optimized(
    owner: &ProductionPreRankedKirOwnerV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    inspect: impl FnOnce(
        &SemanticKirOptimizedAssertOriginOwnerV1,
        &Control<'_, '_, '_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let floor = budget.storage();
    let (input, input_storage) = Inventory::derive(owner.executable(), budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let (mut graph, graph_storage) = KirPlironGraphV12::import(owner.executable(), budget).unwrap();
    budget
        .reserve_storage(graph_storage.retained_storage())
        .unwrap();
    let observed = graph
        .execute_production_neutral_optimization_v1(budget)
        .unwrap()
        .extract()
        .unwrap();
    assert_eq!(observed.report().passes().len(), 7);
    let observed_storage = observed.storage().retained_storage();
    budget.reserve_storage(observed_storage).unwrap();
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    let (output, output_storage) = Inventory::derive(observed.owner(), budget).unwrap();
    budget
        .reserve_storage(output_storage.retained_storage())
        .unwrap();
    let checked_storage = {
        let (checked, storage) = check_canonical_kir_transition_v1(
            &input,
            &output,
            observed.occurrences().candidate(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (control, control_storage) = Control::derive(&checked, budget).unwrap();
        budget
            .reserve_storage(control_storage.retained_storage())
            .unwrap();
        let (result, result_storage) =
            transport_semantic_kir_assert_origins_v1(owner.assert_origins(), &control, budget)
                .unwrap();
        budget
            .reserve_storage(result_storage.retained_storage())
            .unwrap();
        assert_eq!(result.input_identity(), input.identity());
        assert_eq!(result.output_identity(), output.identity());
        assert!(!result.grants_authority());
        assert!(!control.grants_authority());
        inspect(&result, &control, budget);
        drop(result);
        budget
            .release_storage(result_storage.retained_storage())
            .unwrap();
        drop(control);
        budget
            .release_storage(control_storage.retained_storage())
            .unwrap();
        storage
    };
    budget
        .release_storage(checked_storage.retained_storage())
        .unwrap();
    drop(output);
    budget
        .release_storage(output_storage.retained_storage())
        .unwrap();
    drop(observed);
    budget.release_storage(observed_storage).unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

fn materialize_transformed(
    kind: Fixture,
    transform: impl FnMut(u32, Vec<SemanticBasicBlockV1>) -> Vec<SemanticBasicBlockV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks(kind, false, transform);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

fn assert_binding<'a>(
    result: &'a SemanticKirOptimizedAssertOriginOwnerV1,
    source_block: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> &'a SemanticKirOptimizedAssertBindingV1 {
    let root = SemanticFunctionIdV1::from_index(0);
    result
        .assert_condition(
            root,
            root,
            SemanticBlockIdV1::from_index(source_block),
            budget,
        )
        .unwrap()
}

#[test]
fn real_source_literal_selection_preserves_polarity_and_failure_traps() {
    for expected in [true, false] {
        for literal in [true, false] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let owner = materialize_transformed(
                Fixture::Literal(expected),
                |_, _| {
                    let mut assertion = literal_terminator(expected, 1);
                    let SemanticTerminatorKindV1::Assert { condition, .. } = &mut assertion else {
                        unreachable!()
                    };
                    *condition = constant(BOOL, u128::from(literal), 1);
                    vec![
                        block(31, vec![], assertion),
                        block(32, vec![], SemanticTerminatorKindV1::Return),
                    ]
                },
                &mut budget,
            );
            let payload = retained(&owner);
            budget.reserve_storage(payload).unwrap();
            with_optimized(&owner, &mut budget, |result, control, budget| {
                let binding = assert_binding(result, 0, budget);
                assert_eq!(binding.expected(), expected);
                assert_eq!(binding.semantic_success().index(), 1);
                assert!(result.arguments_for(binding, budget).unwrap().is_empty());
                let traps = control.output().operations().iter().filter(|operation| {
                    matches!(&operation.operation.kind, OperationKind::Call { callee, arguments }
                        if AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments) == Some(AmdGpuDiagnosticOperation::Trap))
                }).count();
                if literal == expected {
                    assert!(matches!(
                        binding.outcome(),
                        SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess {
                            success: CanonicalKirEdgePlacementV1::InternalConnector(_),
                            failure: CanonicalKirEdgePlacementV1::Omitted,
                        }
                    ));
                    assert_eq!(traps, 0);
                } else {
                    assert!(matches!(
                        binding.outcome(),
                        SemanticKirOptimizedAssertOutcomeV1::SelectedFailure {
                            success: CanonicalKirEdgePlacementV1::Omitted,
                            failure: CanonicalKirEdgePlacementV1::InternalConnector(_),
                        }
                    ));
                    assert_eq!(traps, 1);
                    assert!(
                        control
                            .output()
                            .blocks()
                            .iter()
                            .any(|block| matches!(block.terminator, Terminator::Unreachable))
                    );
                }
            });
            drop(owner);
            budget.release_storage(payload).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn real_source_dynamic_assertion_uses_actual_output_condition_and_both_edges() {
    for expected in [true, false] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let owner = materialize_transformed(
            Fixture::ElidedBounds,
            |_, _| {
                vec![
                    block(
                        31,
                        vec![
                            assignment(
                                2,
                                U64,
                                SemanticRvalueKindV1::Unary {
                                    operation: SemanticUnaryOpV1::PointerMetadata,
                                    operand: value(1, SLICE_REF),
                                },
                            ),
                            assignment(3, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
                            assignment(
                                4,
                                BOOL,
                                SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::LessThan,
                                    left: value(3, U64),
                                    right: value(2, U64),
                                },
                            ),
                        ],
                        SemanticTerminatorKindV1::Assert {
                            condition: value(4, BOOL),
                            expected,
                            message: SemanticAssertMessageV1::NullPointerDereference,
                            target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                    block(32, vec![], SemanticTerminatorKindV1::Return),
                ]
            },
            &mut budget,
        );
        let payload = retained(&owner);
        budget.reserve_storage(payload).unwrap();
        with_optimized(&owner, &mut budget, |result, control, budget| {
            let binding = assert_binding(result, 0, budget);
            let SemanticKirOptimizedAssertOutcomeV1::Conditional {
                condition,
                success,
                failure,
            } = binding.outcome()
            else {
                panic!("dynamic condition must remain");
            };
            assert_eq!(binding.expected(), expected);
            let CanonicalKirEdgePlacementV1::Retained(success) = success else {
                panic!("success occurrence");
            };
            let CanonicalKirEdgePlacementV1::Retained(failure) = failure else {
                panic!("failure occurrence");
            };
            assert_eq!(success.source, failure.source);
            assert_eq!(success.successor, u32::from(!expected));
            assert_eq!(failure.successor, u32::from(expected));
            let actual = control
                .output()
                .uses()
                .iter()
                .find(|row| row.coordinate == condition.coordinate)
                .unwrap();
            assert_eq!(
                control.output().definitions()[actual.definition].coordinate,
                condition.definition
            );
            assert_eq!(
                control.output().definitions()[actual.definition].ty,
                &Type::BOOL
            );
        });
        drop(owner);
        budget.release_storage(payload).unwrap();
    }
}

#[test]
fn source_assertion_made_unreachable_by_checked_folding_is_not_successful_discharge() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let owner = materialize_transformed(
        Fixture::Literal(true),
        |_, _| {
            vec![
                block(
                    31,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: constant(BOOL, 1, 1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                1,
                                edge(SemanticEdgeRoleV1::SwitchValue, 2),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                        )
                        .unwrap(),
                    },
                ),
                block(32, vec![], literal_terminator(true, 2)),
                block(33, vec![], SemanticTerminatorKindV1::Return),
            ]
        },
        &mut budget,
    );
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    assert_eq!(owner.assert_origins().binding_count(), 1);
    with_optimized(&owner, &mut budget, |result, _, budget| {
        assert!(matches!(
            assert_binding(result, 1, budget).outcome(),
            SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable {
                block: None,
                condition: None,
                success: CanonicalKirEdgePlacementV1::Omitted,
                failure: Some(CanonicalKirEdgePlacementV1::Omitted),
            }
        ));
    });
    drop(owner);
    budget.release_storage(payload).unwrap();
}

#[test]
fn source_assert_success_phi_arguments_bind_exact_actual_output_occurrences() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let owner = materialize_transformed(
        Fixture::ElidedBounds,
        |_, _| {
            vec![
                block(
                    31,
                    vec![
                        assignment(
                            2,
                            U64,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::PointerMetadata,
                                operand: value(1, SLICE_REF),
                            },
                        ),
                        assignment(3, U64, SemanticRvalueKindV1::Use(constant(U64, 8, 8))),
                        assignment(
                            4,
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: value(3, U64),
                                right: value(2, U64),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: value(4, BOOL),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                1,
                                edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    32,
                    vec![assignment(
                        3,
                        U64,
                        SemanticRvalueKindV1::Use(constant(U64, 7, 8)),
                    )],
                    SemanticTerminatorKindV1::Assert {
                        condition: value(4, BOOL),
                        expected: true,
                        message: SemanticAssertMessageV1::NullPointerDereference,
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    33,
                    vec![assignment(
                        4,
                        BOOL,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: value(3, U64),
                            right: value(2, U64),
                        },
                    )],
                    SemanticTerminatorKindV1::Assert {
                        condition: value(4, BOOL),
                        expected: true,
                        message: SemanticAssertMessageV1::NullPointerDereference,
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(34, vec![], SemanticTerminatorKindV1::Return),
            ]
        },
        &mut budget,
    );
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    with_optimized(&owner, &mut budget, |result, control, budget| {
        let binding = assert_binding(result, 1, budget);
        let SemanticKirOptimizedAssertOutcomeV1::Conditional { success, .. } = binding.outcome()
        else {
            panic!("first assertion remains conditional");
        };
        let CanonicalKirEdgePlacementV1::Retained(success) = success else {
            panic!("success edge");
        };
        let arguments = result.arguments_for(binding, budget).unwrap();
        assert!(
            !arguments.is_empty(),
            "success join must carry the differing SSA value"
        );
        let mut survivors = 0;
        for argument in arguments {
            if let Some(output) = argument.output {
                survivors += 1;
                assert_eq!(output.edge, success);
                assert!(
                    control
                        .output()
                        .edge_arguments()
                        .iter()
                        .any(|actual| actual.coordinate == output)
                );
                assert_eq!(
                    control.edge_argument(argument.input, budget).unwrap(),
                    Some(output)
                );
            }
        }
        assert!(survivors > 0);
    });
    drop(owner);
    budget.release_storage(payload).unwrap();
}

#[test]
fn shared_source_roots_still_alias_one_optimized_physical_assertion() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let owner = materialize(Fixture::Literal(false), true, &mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    with_optimized(&owner, &mut budget, |result, _, budget| {
        assert_eq!((result.source_site_count(), result.binding_count()), (2, 1));
        let helper = SemanticFunctionIdV1::from_index(0);
        let source = SemanticBlockIdV1::from_index(0);
        let one = result
            .assert_condition(SemanticFunctionIdV1::from_index(1), helper, source, budget)
            .unwrap();
        let other = result
            .assert_condition(SemanticFunctionIdV1::from_index(3), helper, source, budget)
            .unwrap();
        assert_eq!(one, other);
        assert!(matches!(
            one.outcome(),
            SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess { .. }
        ));
        assert!(matches!(
            result.assert_condition(helper, helper, source, budget),
            Err(SemanticKirOptimizedAssertOriginErrorV1::Source(
                SemanticKirAssertOriginErrorV1::MissingBinding { .. }
            ))
        ));
    });
    drop(owner);
    budget.release_storage(payload).unwrap();
}

#[test]
fn historical_source_elision_is_not_relabelled_as_optimizer_selection() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let owner = materialize(Fixture::ElidedBounds, false, &mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    with_optimized(&owner, &mut budget, |result, _, budget| {
        let binding = assert_binding(result, 1, budget);
        assert!(matches!(
            binding.import_binding().outcome(),
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
        ));
        assert!(matches!(
            binding.outcome(),
            SemanticKirOptimizedAssertOutcomeV1::SourceRuleElision { .. }
        ));
    });
    drop(owner);
    budget.release_storage(payload).unwrap();
}

#[test]
fn same_shaped_foreign_physical_binding_cannot_select_this_owners_arguments() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let owner = materialize(Fixture::Literal(true), false, &mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    with_optimized(&owner, &mut budget, |result, control, budget| {
        let (other, storage) =
            transport_semantic_kir_assert_origins_v1(owner.assert_origins(), control, budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let local = assert_binding(result, 0, budget);
        let foreign = assert_binding(&other, 0, budget);
        assert_eq!(local, foreign);
        assert!(!std::ptr::eq(local, foreign));
        assert_eq!(
            result.arguments_for(foreign, budget).unwrap_err(),
            SemanticKirOptimizedAssertOriginErrorV1::Invalid("foreign assertion binding")
        );
        assert!(result.arguments_for(local, budget).unwrap().is_empty());
        let copied = *local;
        assert!(result.arguments_for(&copied, budget).is_err());
        drop(other);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
    drop(owner);
    budget.release_storage(payload).unwrap();
}

#[test]
fn sealed_input_identity_and_independent_nonempty_transport_budget_boundaries() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize(Fixture::Literal(true), false, &mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    let other = materialize(Fixture::Literal(true), false, &mut budget);
    let other_payload = retained(&other);
    budget.reserve_storage(other_payload).unwrap();
    with_optimized(&owner, &mut budget, |_, control, budget| {
        let floor = budget.storage();
        assert_eq!(
            transport_semantic_kir_assert_origins_v1(other.assert_origins(), control, budget)
                .unwrap_err(),
            SemanticKirOptimizedAssertOriginErrorV1::InputOwner
        );
        assert_eq!(budget.storage(), floor);
        // One physical emitted assertion, two zero-argument edges, one source alias.
        // 1 entry + 9 census + 3 allocation + 17 copying + 15 queries + 1 alias.
        const EXPECTED_WORK: usize = 46;
        let expected_storage = size_of::<SemanticKirOptimizedAssertOriginOwnerV1>()
            + size_of::<SemanticKirOptimizedAssertBindingV1>()
            + size_of::<AssertOriginAliasV1>();
        for (work_limit, storage_limit, passes) in [
            (7 + EXPECTED_WORK, floor + expected_storage, true),
            (7 + EXPECTED_WORK - 1, floor + expected_storage, false),
            (7 + EXPECTED_WORK, floor + expected_storage - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut measured = AssertOriginBudgetV1::new(&mut work, storage_limit);
            measured.charge_work(7).unwrap();
            measured.reserve_storage(floor).unwrap();
            let attempt = transport_semantic_kir_assert_origins_v1(
                owner.assert_origins(),
                control,
                &mut measured,
            );
            assert_eq!(attempt.is_ok(), passes);
            assert_eq!(measured.storage(), floor);
            if let Ok((result, storage)) = attempt {
                assert_eq!(measured.work(), 7 + EXPECTED_WORK);
                assert_eq!(measured.peak_storage(), floor + expected_storage);
                assert_eq!(storage.retained_storage(), expected_storage);
                measured
                    .reserve_storage(storage.retained_storage())
                    .unwrap();
                drop(result);
                measured
                    .release_storage(storage.retained_storage())
                    .unwrap();
            }
        }
    });
    drop(other);
    budget.release_storage(other_payload).unwrap();
    drop(owner);
    budget.release_storage(payload).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
