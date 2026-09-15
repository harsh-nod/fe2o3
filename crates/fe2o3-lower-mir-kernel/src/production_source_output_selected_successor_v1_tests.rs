use super::*;

fn selected_source_owner(
    explicit_value: u128,
    condition: bool,
    duplicate_targets: bool,
    failed_assert: bool,
) -> ProductionPreRankedKirOwnerV1 {
    let seed = omitted_write_owner();
    let semantic = seed.semantic_ssa().source_semantic();
    let source = &semantic.functions()[0];
    let boolean = SemanticTypeIdV1::from_index(3);
    assert!(matches!(
        semantic.types()[boolean.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ));
    let condition = constant(boolean, u128::from(condition), 1);
    let entry = if failed_assert {
        SemanticTerminatorKindV1::Assert {
            condition,
            expected: true,
            message: SemanticAssertMessageV1::NullPointerDereference,
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
            unwind: SemanticUnwindActionV1::Unreachable,
        }
    } else {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: condition,
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    explicit_value,
                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                )],
                edge(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    if duplicate_targets { 1 } else { 2 },
                ),
            )
            .unwrap(),
        }
    };
    let first = source.blocks()[1].statements().to_vec();
    let mut second = first.clone();
    second[0] = assignment(
        2,
        ARRAY_SCALAR,
        SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 7, 4)),
    );
    let function = SemanticFunctionDeclV1::new(
        source.identity(),
        source.role(),
        source.item_definition_identity(),
        source.monomorphization_identity(),
        source.generic_type_arguments_identity(),
        source.const_generic_arguments_identity(),
        source.source(),
        source.abi().clone(),
        source.locals().to_vec(),
        source.entry(),
        vec![
            block(211, vec![], entry),
            block(212, first, SemanticTerminatorKindV1::Return),
            block(213, second, SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(source.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ARRAY_ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "array_relation",
            [202; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

#[test]
fn checked_boolean_selection_preserves_live_and_dead_source_writes() {
    for explicit in [0, 1] {
        for condition in [false, true] {
            with_output(
                selected_source_owner(explicit, condition, false, false),
                |view, budget| {
                    let stores = |owner: &VerifiedCanonicalKernelIrModuleV12| {
                        owner
                            .module()
                            .functions
                            .iter()
                            .filter_map(|function| function.body.as_ref())
                            .flat_map(|body| &body.blocks)
                            .flat_map(|block| &block.operations)
                            .filter(|operation| {
                                matches!(operation.kind, OperationKind::Store { .. })
                            })
                            .count()
                    };
                    assert_eq!(stores(view.source().executable()), 2);
                    assert_eq!(stores(view.output()), 1);
                    let selected = view
                        .selected_successor(
                            ARRAY_ROOT,
                            ARRAY_ROOT,
                            SemanticBlockIdV1::from_index(0),
                            budget,
                        )
                        .unwrap()
                        .unwrap();
                    let ordinal = u32::from(u128::from(condition) != explicit);
                    assert_eq!(selected.semantic_ordinal(), ordinal);
                    assert_eq!(selected.semantic_target().index(), 1 + ordinal);
                    assert_eq!(selected.input().successor, u32::from(!condition));
                    let source_row = view
                        .blocks
                        .iter()
                        .find(|row| row.source.semantic_block.index() == 0)
                        .unwrap();
                    assert_eq!(selected.input().source, source_row.original);
                    assert_eq!(
                        source_row.control.selected_successor,
                        Some(selected.input())
                    );
                    for source_block in [1, 2] {
                        assert!(
                            view.source()
                                .has_materialized_private_array_access(
                                    ARRAY_ROOT,
                                    ARRAY_ROOT,
                                    site(source_block, 1),
                                    Role::Destination,
                                    budget
                                )
                                .unwrap()
                        );
                        let access = view
                            .private_array_write(
                                ARRAY_ROOT,
                                ARRAY_ROOT,
                                site(source_block, 1),
                                Role::Destination,
                                budget,
                            )
                            .unwrap();
                        if source_block == selected.semantic_target().index() {
                            assert!(
                                matches!(access, ProductionSourceOutputPrivateArrayAccessV1::Retained { index, executable: true, .. } if index == if source_block == 1 { 0 } else { 7 })
                            );
                        } else {
                            assert_eq!(
                                access,
                                ProductionSourceOutputPrivateArrayAccessV1::OmittedUnreachable
                            );
                        }
                    }
                    assert!(!view.grants_authority());
                },
            );
        }
    }
}

#[test]
fn duplicate_target_blocks_do_not_collapse_source_edge_occurrences() {
    for explicit in [0, 1] {
        for condition in [false, true] {
            with_output(
                selected_source_owner(explicit, condition, true, false),
                |view, budget| {
                    let selected = view
                        .selected_successor(
                            ARRAY_ROOT,
                            ARRAY_ROOT,
                            SemanticBlockIdV1::from_index(0),
                            budget,
                        )
                        .unwrap()
                        .unwrap();
                    assert_eq!(selected.semantic_target().index(), 1);
                    assert_eq!(
                        selected.semantic_ordinal(),
                        u32::from(u128::from(condition) != explicit)
                    );
                    assert_eq!(selected.input().successor, u32::from(!condition));
                },
            );
        }
    }
}

#[test]
fn selected_successor_does_not_reinterpret_failed_assertions() {
    with_output(
        selected_source_owner(0, false, false, true),
        |view, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            assert!(matches!(
                view.assertion(ARRAY_ROOT, ARRAY_ROOT, block, budget)
                    .unwrap()
                    .outcome(),
                SemanticKirOptimizedAssertOutcomeV1::SelectedFailure { .. }
            ));
            assert!(matches!(
                view.selected_successor(ARRAY_ROOT, ARRAY_ROOT, block, budget),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "selected successor requires an ordinary Boolean source switch"
                ))
            ));
        },
    );
}

#[test]
fn selected_successor_checks_source_axes_and_live_floor_without_allocation() {
    with_output(
        selected_source_owner(0, false, false, false),
        |view, budget| {
            let floor = budget.storage();
            let zero = SemanticBlockIdV1::from_index(0);
            for (root, function, block) in [(1, 0, 0), (0, 1, 0), (0, 0, u32::MAX)] {
                assert!(
                    view.selected_successor(
                        SemanticFunctionIdV1::from_index(root),
                        SemanticFunctionIdV1::from_index(function),
                        SemanticBlockIdV1::from_index(block),
                        budget
                    )
                    .is_err()
                );
                assert_eq!(budget.storage(), floor);
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
            let mut missing = AssertOriginBudgetV1::new(&mut work, STORAGE);
            assert!(matches!(
                view.selected_successor(ARRAY_ROOT, ARRAY_ROOT, zero, &mut missing),
                Err(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Accounting
                ))
            ));
            assert_eq!(missing.work(), 4);
            assert_eq!(missing.storage(), 0);
        },
    );
    with_output(
        array_owner(ArrayCase::Write { sparse: false }),
        |view, budget| {
            assert_eq!(
                view.selected_successor(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    SemanticBlockIdV1::from_index(0),
                    budget
                )
                .unwrap(),
                None
            );
        },
    );
}

#[test]
fn exact_selected_edge_mapping_has_repeated_query_work_boundaries() {
    with_output(
        selected_source_owner(0, false, false, false),
        |view, live_budget| {
            let selected = view
                .selected_successor(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    SemanticBlockIdV1::from_index(0),
                    live_budget,
                )
                .unwrap()
                .unwrap();
            let semantic = view.source().semantic_ssa().source_semantic();
            let function = &semantic.functions()[0];
            let original = &view.source().executable().module().functions
                [selected.input().source.function.0 as usize]
                .body
                .as_ref()
                .unwrap()
                .blocks[selected.input().source.block as usize];
            // This component isolates only the fixed mapping helper, not source
            // origin/index lookup or the view-construction/session budget.
            for (limit, success) in [(39, true), (38, false)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, 23);
                budget.reserve_storage(23).unwrap();
                budget.charge_work(7).unwrap();
                let first = source_output_selected_semantic_edge_v1(
                    semantic.types(),
                    function,
                    SemanticBlockIdV1::from_index(0),
                    original,
                    selected.input(),
                    &mut budget,
                )
                .unwrap();
                assert_eq!(first, selected);
                assert_eq!(budget.work(), 23);
                let second = source_output_selected_semantic_edge_v1(
                    semantic.types(),
                    function,
                    SemanticBlockIdV1::from_index(0),
                    original,
                    selected.input(),
                    &mut budget,
                );
                if success {
                    assert_eq!(second.unwrap(), selected);
                    assert_eq!(budget.work(), 39);
                } else {
                    assert!(matches!(
                        second,
                        Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Work(error)
                    )) if error.actual() == 39 && error.limit() == 38
                    ));
                    assert_eq!(budget.work(), 37);
                }
                assert_eq!(budget.storage(), 23);
            }
            let mut changed = original.clone();
            if let Some(Terminator::ConditionalBranch { then_target, .. }) = &mut changed.terminator
            {
                *then_target = BlockId(0);
            } else {
                panic!("expected original conditional branch");
            }
            assert!(matches!(
                source_output_selected_semantic_edge_v1(
                    semantic.types(),
                    function,
                    SemanticBlockIdV1::from_index(0),
                    &changed,
                    selected.input(),
                    live_budget
                ),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "selected successor original target occurrences differ from source"
                ))
            ));
            let mut wrong_edge = selected.input();
            wrong_edge.successor = 2;
            assert!(
                source_output_selected_semantic_edge_v1(
                    semantic.types(),
                    function,
                    SemanticBlockIdV1::from_index(0),
                    original,
                    wrong_edge,
                    live_budget
                )
                .is_err()
            );
            changed.terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
            assert!(matches!(
                source_output_selected_semantic_edge_v1(
                    semantic.types(),
                    function,
                    SemanticBlockIdV1::from_index(0),
                    &changed,
                    selected.input(),
                    live_budget
                ),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "selected successor requires the exact N conditional branch"
                ))
            ));
        },
    );
}
