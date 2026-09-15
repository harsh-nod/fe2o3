#[derive(Clone, Copy, Debug)]
enum BodyPredicateFixtureV1 {
    Exact,
    CopyAlias,
    Argument,
    LaneDependent,
    Unresolved,
    MultipleDefinitions,
    Escaped,
    WrongWidth,
    Signed,
    CyclicAlias,
}

fn induction_body_predicate_function_v1(
    fixture: BodyPredicateFixtureV1,
    polarity: u128,
    reversed: bool,
) -> SemanticFunctionDeclV1 {
    induction_body_predicate_typed_function_v1(fixture, polarity, reversed, SCALAR_TYPE, 4)
}

fn induction_body_predicate_typed_function_v1(
    fixture: BodyPredicateFixtureV1,
    polarity: u128,
    reversed: bool,
    index_type: SemanticTypeIdV1,
    index_bytes: u8,
) -> SemanticFunctionDeclV1 {
    let ty = if matches!(fixture, BodyPredicateFixtureV1::Signed) {
        I32_TYPE
    } else {
        index_type
    };
    let induction = typed_operand(1, index_type);
    let mut statements = Vec::new();
    let lhs = if matches!(
        fixture,
        BodyPredicateFixtureV1::CopyAlias | BodyPredicateFixtureV1::CyclicAlias
    ) {
        statements.push(typed_assignment(
            4,
            index_type,
            SemanticRvalueKindV1::Use(if matches!(fixture, BodyPredicateFixtureV1::CyclicAlias) {
                typed_operand(4, index_type)
            } else {
                induction.clone()
            }),
        ));
        typed_operand(4, index_type)
    } else {
        typed_operand(1, ty)
    };
    let rhs = match fixture {
        BodyPredicateFixtureV1::Argument
        | BodyPredicateFixtureV1::LaneDependent
        | BodyPredicateFixtureV1::Unresolved => typed_operand(5, index_type),
        BodyPredicateFixtureV1::WrongWidth => typed_constant(U64_TYPE, 768, 8),
        _ => typed_constant(ty, 768, index_bytes),
    };
    let (left, right) = if reversed { (rhs, lhs) } else { (lhs, rhs) };
    let comparison = typed_assignment(
        3,
        BOOL_TYPE,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        },
    );
    statements.push(comparison.clone());
    if matches!(fixture, BodyPredicateFixtureV1::MultipleDefinitions) {
        statements.push(comparison);
    }
    if matches!(fixture, BodyPredicateFixtureV1::Escaped) {
        statements.push(typed_assignment(
            6,
            POINTER_TYPE,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], index_type)
                    .unwrap(),
            },
        ));
    }
    let switch = |condition, explicit_value, explicit_target, otherwise| {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: typed_operand(condition, BOOL_TYPE),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    explicit_value,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, explicit_target),
                )],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
            )
            .unwrap(),
        }
    };
    let mut blocks = vec![
        block(
            150,
            vec![typed_assignment(
                1,
                index_type,
                SemanticRvalueKindV1::Use(typed_constant(index_type, 0, index_bytes)),
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            151,
            vec![typed_assignment(
                2,
                BOOL_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: induction.clone(),
                    right: typed_constant(index_type, 2, index_bytes),
                },
            )],
            switch(
                2,
                0,
                6,
                if matches!(fixture, BodyPredicateFixtureV1::LaneDependent) {
                    7
                } else {
                    2
                },
            ),
        ),
        block(
            152,
            statements,
            if polarity == 0 {
                switch(3, 0, 5, 3)
            } else {
                switch(3, 1, 3, 5)
            },
        ),
        block(
            153,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
        ),
        block(
            154,
            vec![typed_assignment(
                1,
                index_type,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: induction,
                    right: typed_constant(index_type, 1, index_bytes),
                },
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            155,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    None,
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        block(156, vec![], SemanticTerminatorKindV1::Return),
    ];
    if matches!(fixture, BodyPredicateFixtureV1::LaneDependent) {
        blocks.push(block(
            157,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(1),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        typed_place(5, index_type),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, 2),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ));
    }
    projection_function_with_locals(
        blocks,
        vec![
            local(150, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(151, index_type, SemanticLocalRoleV1::Temporary),
            local(152, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(153, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(154, index_type, SemanticLocalRoleV1::Temporary),
            local(
                155,
                index_type,
                if matches!(fixture, BodyPredicateFixtureV1::Argument) {
                    SemanticLocalRoleV1::Argument(0)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
            ),
            local(156, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}

fn induction_body_ranked_tensor_v1(
    function: &SemanticFunctionDeclV1,
    erase_predicates: bool,
) -> Result<ProductionRankedKernelV1, ProductionRankedProjectionErrorV1> {
    induction_body_ranked_tensor_at_v1(function, erase_predicates, 3)
}

fn induction_body_ranked_tensor_at_v1(
    function: &SemanticFunctionDeclV1,
    erase_predicates: bool,
    tensor_block: usize,
) -> Result<ProductionRankedKernelV1, ProductionRankedProjectionErrorV1> {
    let types = assertion_proof_types();
    let callables = vec![
        compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::Trap),
        compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::ThreadIndex(
            SemanticAxisV1::X,
        )),
    ];
    let (mut inductions, mut operations, argument_count) =
        project_test_inductions_with_types_and_callables(&types, &callables, function)?;
    if erase_predicates {
        for induction in &mut inductions {
            induction.body_predicates.clear();
        }
    }
    operations.insert(
        0,
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [64, 1, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        },
    );
    let mut projected = (0..function.blocks().len())
        .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
        .collect::<Vec<_>>();
    projected[tensor_block].items.push(ProjectedBlockItemV1::Effect {
        operation: ProductionRankedOperationV1::TensorLayout {
            contract: fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            convergence: TensorConvergenceAttr::UniformSubgroup, active_lanes: 64, binding: None,
        }, source: None,
    });
    let (blocks, _, _) = build_ranked_cfg(
        &types,
        function,
        &callables,
        &vec![None; function.locals().len()],
        &vec![None; function.blocks().len()],
        &inductions,
        operations,
        projected,
    )?;
    ProductionRankedKernelV1::new("induction_body_tensor", argument_count, blocks)
        .map_err(ProductionRankedProjectionErrorV1::Recipe)
}

#[test]
fn induction_body_predicates_preserve_both_polarities_and_reach_production_checks() {
    for fixture in [
        BodyPredicateFixtureV1::Exact,
        BodyPredicateFixtureV1::CopyAlias,
        BodyPredicateFixtureV1::Argument,
    ] {
        for (index_type, index_bytes) in [(SCALAR_TYPE, 4), (U64_TYPE, 8)] {
            for polarity in [0, 1] {
                for reversed in [false, true] {
                    let function = induction_body_predicate_typed_function_v1(
                        fixture,
                        polarity,
                        reversed,
                        index_type,
                        index_bytes,
                    );
                    let kernel = induction_body_ranked_tensor_v1(&function, false).unwrap();
                    assert!(
                        matches!(kernel.blocks()[3].terminator(), ProductionRankedTerminatorV1::IndexLessThanArgs {
                    true_arguments, false_arguments, true_block: 4, false_block: 6, ..
                } if true_arguments.len() == 1 && false_arguments.is_empty())
                    );
                    let input = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
                        ProductionConstructionV1::ranked_kernel("body_guard", kernel).unwrap(),
                        ProductionSessionLimitsV1::default(),
                    )
                    .unwrap();
                    assert!(input.all_mandatory_reports_are_clean());
                    assert!(input.tensor_layout_report().is_clean());
                    assert!(
                        !input
                            .tensor_layout_report()
                            .grants_artifact_or_launch_authority()
                    );
                }
            }
        }
    }
}

#[test]
fn removing_induction_body_predicate_reproduces_incomplete_tensor_control() {
    let function = induction_body_predicate_function_v1(BodyPredicateFixtureV1::Exact, 0, false);
    let kernel = induction_body_ranked_tensor_v1(&function, true).unwrap();
    let error = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("body_guard_missing", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("FE2O3-TENSOR-LAYOUT-002"));
}

#[test]
fn induction_body_predicates_do_not_admit_unresolved_or_malformed_sources() {
    for fixture in [
        BodyPredicateFixtureV1::Unresolved,
        BodyPredicateFixtureV1::LaneDependent,
        BodyPredicateFixtureV1::MultipleDefinitions,
        BodyPredicateFixtureV1::WrongWidth,
        BodyPredicateFixtureV1::Signed,
    ] {
        let function = induction_body_predicate_function_v1(fixture, 0, false);
        let kernel = induction_body_ranked_tensor_v1(&function, false).unwrap();
        assert!(
            matches!(kernel.blocks()[3].terminator(), ProductionRankedTerminatorV1::AnalysisSplitArgs {
            control_dependencies, ..
        } if control_dependencies.is_empty())
        );
        let error = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("body_guard_unknown", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("FE2O3-TENSOR-LAYOUT-002"));
    }
    for fixture in [
        BodyPredicateFixtureV1::Escaped,
        BodyPredicateFixtureV1::CyclicAlias,
    ] {
        let function = induction_body_predicate_function_v1(fixture, 0, false);
        match induction_body_ranked_tensor_v1(&function, false) {
            Err(ProductionRankedProjectionErrorV1::Incomplete(_)) => {}
            Err(other) => panic!("unexpected source rejection: {other}"),
            Ok(kernel) => {
                let error = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
                    ProductionConstructionV1::ranked_kernel("body_guard_invalid", kernel).unwrap(),
                    ProductionSessionLimitsV1::default(),
                )
                .unwrap_err();
                assert!(error.to_string().contains("FE2O3-TENSOR-LAYOUT-002"));
            }
        }
    }
}

#[test]
fn induction_body_predicate_materialization_uses_exact_nested_live_positions() {
    let function = induction_body_predicate_function_v1(BodyPredicateFixtureV1::Exact, 0, false);
    let callables = vec![compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::Trap,
    )];
    let (inductions, _, _) = project_test_inductions_with_types_and_callables(
        &assertion_proof_types(),
        &callables,
        &function,
    )
    .unwrap();
    let predicate = inductions[0].body_predicates[0].clone();
    let split = ProjectedCfgTerminatorV1::AnalysisSplit {
        first_block: 5,
        second_block: 3,
    };
    let bases = (0..6).map(Some).collect::<Vec<_>>();
    let mut live = vec![vec![]; 6];
    live[3] = vec![2, 0];
    let materialized = materialize_induction_body_predicate_v1(
        &function,
        &predicate,
        &split,
        42,
        &[2, 0],
        &bases,
        &live,
    )
    .unwrap();
    assert!(
        matches!(materialized, ProductionRankedTerminatorV1::IndexLessThanArgs {
        lhs: ProductionRankedValueV1::BlockArgument { block: 42, argument: 1 },
        true_arguments, false_arguments, ..
    } if true_arguments.len() == 2 && false_arguments.is_empty())
    );
    assert!(
        materialize_induction_body_predicate_v1(
            &function,
            &predicate,
            &split,
            42,
            &[2],
            &bases,
            &live
        )
        .is_err()
    );
    let wrong_split = ProjectedCfgTerminatorV1::AnalysisSplit {
        first_block: 4,
        second_block: 3,
    };
    assert!(
        materialize_induction_body_predicate_v1(
            &function,
            &predicate,
            &wrong_split,
            42,
            &[2, 0],
            &bases,
            &live
        )
        .is_err()
    );
    let mut stale = predicate.clone();
    stale.source_statement = 1;
    assert!(
        materialize_induction_body_predicate_v1(
            &function,
            &stale,
            &split,
            42,
            &[2, 0],
            &bases,
            &live
        )
        .is_err()
    );
    stale = predicate.clone();
    stale.source_assignment = typed_assignment(
        3,
        BOOL_TYPE,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: constant(0),
            right: constant(768),
        },
    )
    .kind()
    .clone();
    assert!(
        materialize_induction_body_predicate_v1(
            &function,
            &stale,
            &split,
            42,
            &[2, 0],
            &bases,
            &live
        )
        .is_err()
    );
    stale = predicate;
    std::mem::swap(&mut stale.true_block, &mut stale.false_block);
    assert!(
        materialize_induction_body_predicate_v1(
            &function,
            &stale,
            &split,
            42,
            &[2, 0],
            &bases,
            &live
        )
        .is_err()
    );
}

#[test]
fn induction_body_predicate_index_rejects_duplicate_and_outside_owners() {
    let function = induction_body_predicate_function_v1(BodyPredicateFixtureV1::Exact, 0, false);
    let callables = vec![compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::Trap,
    )];
    let (mut inductions, _, _) = project_test_inductions_with_types_and_callables(
        &assertion_proof_types(),
        &callables,
        &function,
    )
    .unwrap();
    let predicate = inductions[0].body_predicates[0].clone();
    for stale_operand in [
        ProjectedInductionPredicateOperandV1::Induction {
            ordinal: 1,
            source_local: SemanticLocalIdV1::from_index(1),
            source_type: SCALAR_TYPE,
        },
        ProjectedInductionPredicateOperandV1::Induction {
            ordinal: 0,
            source_local: SemanticLocalIdV1::from_index(4),
            source_type: SCALAR_TYPE,
        },
        ProjectedInductionPredicateOperandV1::Induction {
            ordinal: 0,
            source_local: SemanticLocalIdV1::from_index(1),
            source_type: U64_TYPE,
        },
    ] {
        inductions[0].body_predicates[0].lhs = stale_operand;
        assert!(indexed_induction_body_predicates_v1(&function, &inductions).is_err());
    }
    inductions[0].body_predicates[0] = predicate.clone();
    inductions[0].body_predicates.push(predicate.clone());
    assert!(indexed_induction_body_predicates_v1(&function, &inductions).is_err());
    inductions[0].body_predicates = vec![predicate];
    inductions[0].body_predicates[0].block = 6;
    assert!(indexed_induction_body_predicates_v1(&function, &inductions).is_err());
}

#[test]
fn induction_body_materialization_rejects_malformed_semantic_edges() {
    let function = induction_body_predicate_function_v1(BodyPredicateFixtureV1::Exact, 0, false);
    let callables = vec![compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::Trap,
    )];
    let (inductions, _, _) = project_test_inductions_with_types_and_callables(
        &assertion_proof_types(),
        &callables,
        &function,
    )
    .unwrap();
    let predicate = inductions[0].body_predicates[0].clone();
    let split = ProjectedCfgTerminatorV1::AnalysisSplit {
        first_block: 5,
        second_block: 3,
    };
    let bases = (0..7).map(Some).collect::<Vec<_>>();
    let mut live = vec![vec![]; 7];
    live[3] = vec![0];
    for (value, explicit_role, otherwise_role, otherwise_target, extra) in [
        (
            2,
            SemanticEdgeRoleV1::SwitchValue,
            SemanticEdgeRoleV1::SwitchOtherwise,
            3,
            false,
        ),
        (
            0,
            SemanticEdgeRoleV1::Goto,
            SemanticEdgeRoleV1::SwitchOtherwise,
            3,
            false,
        ),
        (
            0,
            SemanticEdgeRoleV1::SwitchValue,
            SemanticEdgeRoleV1::Goto,
            3,
            false,
        ),
        (
            0,
            SemanticEdgeRoleV1::SwitchValue,
            SemanticEdgeRoleV1::SwitchOtherwise,
            5,
            false,
        ),
        (
            0,
            SemanticEdgeRoleV1::SwitchValue,
            SemanticEdgeRoleV1::SwitchOtherwise,
            3,
            true,
        ),
    ] {
        let mut explicit = vec![SemanticSwitchTargetV1::new(
            value,
            cfg_edge(explicit_role, 5),
        )];
        if extra {
            explicit.push(SemanticSwitchTargetV1::new(
                1,
                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
            ));
        }
        let terminator = SemanticTerminatorKindV1::SwitchInt {
            discriminant: typed_operand(3, BOOL_TYPE),
            targets: SemanticSwitchTargetsV1::new(
                explicit,
                cfg_edge(otherwise_role, otherwise_target),
            )
            .unwrap(),
        };
        let mut blocks = function.blocks().to_vec();
        blocks[2] = block(152, blocks[2].statements().to_vec(), terminator.clone());
        let malformed = projection_function_with_locals(blocks, function.locals().to_vec());
        let mut stale = predicate.clone();
        stale.source_terminator = terminator;
        assert!(
            materialize_induction_body_predicate_v1(
                &malformed,
                &stale,
                &split,
                42,
                &[0],
                &bases,
                &live,
            )
            .is_err()
        );
    }
}

#[test]
fn induction_body_operand_rejects_header_latch_and_outside_uses() {
    let function = induction_body_predicate_function_v1(BodyPredicateFixtureV1::Exact, 0, false);
    let types = assertion_proof_types();
    let callables = vec![compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::Trap,
    )];
    let (inductions, _, _) =
        project_test_inductions_with_types_and_callables(&types, &callables, &function).unwrap();
    let constants = constant_locals(&function).unwrap();
    let definitions = local_definition_counts(&function);
    for block in [0, 1, 4, 5, 6] {
        let mut proofs = SemanticAssertProofsV1::new(&types, &function).unwrap();
        let result = induction_predicate_source_operand_v1(
            &types,
            &function,
            &typed_operand(1, SCALAR_TYPE),
            ScalarAssignmentSiteV1 {
                block,
                statement: function.blocks()[block].statements().len(),
            },
            &constants,
            &definitions,
            &mut proofs,
            &inductions,
            &mut 0,
        )
        .unwrap();
        assert!(result.is_none(), "out-of-body use in block {block}");
    }
    let mut proofs = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert!(matches!(
        induction_predicate_source_operand_v1(
            &types,
            &function,
            &typed_operand(1, SCALAR_TYPE),
            ScalarAssignmentSiteV1 {
                block: 2,
                statement: 0
            },
            &constants,
            &definitions,
            &mut proofs,
            &inductions,
            &mut usize::MAX,
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
    ));
}

fn nested_induction_body_predicate_function_v1(inner: bool) -> SemanticFunctionDeclV1 {
    let assign = |destination, value| {
        typed_assignment(
            destination,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, value, 8)),
        )
    };
    let compare = |destination, induction, bound| {
        typed_assignment(
            destination,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: typed_operand(induction, U64_TYPE),
                right: typed_constant(U64_TYPE, bound, 8),
            },
        )
    };
    let increment = |induction| {
        typed_assignment(
            induction,
            U64_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: typed_operand(induction, U64_TYPE),
                right: typed_constant(U64_TYPE, 1, 8),
            },
        )
    };
    let goto = |target| SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target));
    let switch = |condition, false_target, true_target| SemanticTerminatorKindV1::SwitchInt {
        discriminant: typed_operand(condition, BOOL_TYPE),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                cfg_edge(SemanticEdgeRoleV1::SwitchValue, false_target),
            )],
            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, true_target),
        )
        .unwrap(),
    };
    projection_function_with_locals(
        vec![
            block(160, vec![assign(1, 0)], goto(1)),
            block(161, vec![compare(2, 1, 2)], switch(2, 8, 2)),
            block(162, vec![assign(3, 0)], goto(3)),
            block(163, vec![compare(4, 3, 2)], switch(4, 7, 4)),
            block(
                164,
                vec![compare(5, if inner { 3 } else { 1 }, 768)],
                switch(5, 9, 5),
            ),
            block(165, vec![], goto(6)),
            block(166, vec![increment(3)], goto(3)),
            block(167, vec![increment(1)], goto(1)),
            block(168, vec![], SemanticTerminatorKindV1::Return),
            block(
                169,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![],
                        None,
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
        ],
        vec![
            local(160, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(161, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(162, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(163, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(164, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(165, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}

#[test]
fn nested_induction_body_predicates_use_inner_and_outer_live_arguments_in_production() {
    for inner in [false, true] {
        let function = nested_induction_body_predicate_function_v1(inner);
        let kernel = induction_body_ranked_tensor_at_v1(&function, false, 5).unwrap();
        assert!(
            matches!(kernel.blocks()[5].terminator(), ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs: ProductionRankedValueV1::BlockArgument { block: 5, argument },
            true_arguments, false_arguments, ..
        } if *argument == u32::from(inner) && true_arguments.len() == 2 && false_arguments.is_empty())
        );
        let input = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("nested_body_guard", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(input.all_mandatory_reports_are_clean());
        assert!(input.tensor_layout_report().is_clean());
    }
}

#[test]
fn induction_body_argument_exhaustion_preserves_allocator_state() {
    let function = induction_body_predicate_function_v1(BodyPredicateFixtureV1::Argument, 0, false);
    let types = assertion_proof_types();
    let callables = vec![compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::Trap,
    )];
    let (mut inductions, mut operations, _) =
        project_test_inductions_with_types_and_callables(&types, &callables, &function).unwrap();
    inductions[0].body_predicates.clear();
    let constants = constant_locals(&function).unwrap();
    let origins = local_stable_argument_origins(&types, &function).unwrap();
    let definitions = local_definition_counts(&function);
    for exhausted in [HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, usize::MAX] {
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = exhausted;
        assert!(
            project_induction_body_predicates_v1(
                &types,
                &function,
                &constants,
                &origins,
                &definitions,
                &mut inductions,
                &mut arguments,
                &mut next_argument,
                &mut operations,
                &mut 100,
            )
            .is_err()
        );
        assert_eq!(next_argument, exhausted);
        assert!(arguments.iter().all(Option::is_none));
        assert!(inductions[0].body_predicates.is_empty());
    }
}
