use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGfx942InlineInstructionV30 as Instruction, SemanticGfx942InlineU32V30,
    SemanticInlineAssemblySourceV30,
};

fn marker(instruction: Instruction, can_unwind: bool) -> SemanticCallableDeclV1 {
    let value = || {
        SemanticAbiValueV1::new(
            SCALAR_TYPE,
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        )
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(201)),
        SemanticLayoutIdentityV1::from_sha256(bytes(202)),
        SemanticCanonAbiV1::Rust,
        can_unwind,
        false,
        (0..instruction.input_count()).map(|_| value()).collect(),
        value(),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(203)),
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(204)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(205)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(206)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(207)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
            SemanticGfx942InlineU32V30::new(
                instruction,
                SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS,
            )
            .unwrap(),
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(208)),
    }
}

fn source(caller: u8) -> SemanticInlineAssemblySourceV30 {
    SemanticInlineAssemblySourceV30::new(
        bytes(210),
        SemanticFunctionIdentityV1::from_sha256(bytes(caller)),
        bytes(211),
        bytes(212),
    )
    .unwrap()
}

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR_TYPE).unwrap()
}

fn copy(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local))
}

fn call(arguments: Vec<SemanticOperandV1>, target: u32) -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        arguments,
        Some(SemanticCallDestinationV1::new(
            place(2),
            cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_inline_assembly_source_v30(source(11))
}

fn function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    projection_function_with_locals(
        blocks,
        vec![
            local(220, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(221, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(222, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(223, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}

fn fixture(
    call: SemanticDirectCallV1,
    before: Vec<SemanticStatementV1>,
    after: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    function(vec![
        block(224, before, SemanticTerminatorKindV1::Call(call)),
        block(225, after, SemanticTerminatorKindV1::Return),
    ])
}

fn output() -> SemanticStatementV1 {
    aggregate_output_v2(copy(2))
}

fn assignment(local: u32, value: u128) -> SemanticStatementV1 {
    aggregate_assignment_v2(
        local,
        SCALAR_TYPE,
        SemanticRvalueKindV1::Use(constant(value)),
    )
}

fn resolve(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    block: usize,
    statement: usize,
) -> Result<ProductionSemanticExpressionV2, &'static str> {
    let mut resolver = GpuSemanticExpressionResolverV2::new(types, function)
        .unwrap()
        .with_gfx942_inline_callables_v30(callables)
        .unwrap();
    let result = resolver.resolve_store_v2(
        function.blocks()[block].statements()[statement].kind(),
        ScalarAssignmentSiteV1 { block, statement },
    );
    assert!(resolver.use_site.is_none());
    assert!(resolver.visiting.is_empty());
    result
}

fn scalar(bits: u64) -> ProductionSemanticExpressionV2 {
    ProductionSemanticExpressionV2::Constant {
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        bits,
    }
}

#[test]
fn all_six_root_marker_values_use_exact_u32_wrapping_expressions() {
    let types = projection_types();
    for (instruction, operation) in [
        (Instruction::VMovB32, None),
        (
            Instruction::VAddU32,
            Some(ProductionSemanticBinaryOpV2::Add),
        ),
        (
            Instruction::VSubU32,
            Some(ProductionSemanticBinaryOpV2::Subtract),
        ),
        (
            Instruction::VAndB32,
            Some(ProductionSemanticBinaryOpV2::BitAnd),
        ),
        (
            Instruction::VOrB32,
            Some(ProductionSemanticBinaryOpV2::BitOr),
        ),
        (
            Instruction::VXorB32,
            Some(ProductionSemanticBinaryOpV2::BitXor),
        ),
    ] {
        let arguments = [constant(u128::from(u32::MAX)), constant(37)]
            .into_iter()
            .take(instruction.input_count())
            .collect();
        let function = fixture(call(arguments, 1), vec![], vec![output()]);
        let expected = operation.map_or_else(
            || scalar(u64::from(u32::MAX)),
            |operation| ProductionSemanticExpressionV2::Binary {
                operation,
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32,
                },
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(scalar(u64::from(u32::MAX))),
                rhs: Box::new(scalar(37)),
            },
        );
        assert_eq!(
            resolve(&types, &function, &[marker(instruction, false)], 1, 0),
            Ok(expected)
        );
    }
}

#[test]
fn source_profile_cannot_construct_extra_options_or_another_target() {
    for options in [0, 2, 3, 4, 5, 0xffff] {
        assert!(SemanticGfx942InlineU32V30::new(Instruction::VAddU32, options).is_err());
    }
    // The source enum has only this architecture. Actual KIR target and option
    // substitutions are covered independently by the relation-lane tests.
    let layout = fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1::gfx942(
        SemanticLayoutIdentityV1::from_sha256(bytes(213)),
    );
    assert_eq!(
        layout.architecture(),
        SemanticTargetArchitectureV1::AmdGpuGfx942
    );
}

#[test]
fn arguments_are_resolved_before_later_mutations() {
    let types = projection_types();
    let function = fixture(
        call(vec![copy(1), constant(2)], 1),
        vec![assignment(1, 5)],
        vec![assignment(1, 99), output()],
    );
    assert_eq!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VAddU32, false)],
            1,
            1
        ),
        Ok(ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32
            },
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(scalar(5)),
            rhs: Box::new(scalar(2)),
        })
    );
}

#[test]
fn later_assignment_has_precedence_and_earlier_call_value_is_not_reused() {
    let types = projection_types();
    let function = fixture(
        call(vec![constant(5)], 1),
        vec![],
        vec![assignment(2, 99), output()],
    );
    assert_eq!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VMovB32, false)],
            1,
            1
        ),
        Ok(scalar(99))
    );
    let function = fixture(
        call(vec![constant(5)], 1),
        vec![assignment(2, 99)],
        vec![output()],
    );
    assert!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VMovB32, false)],
            1,
            0
        )
        .is_err()
    );
}

#[test]
fn duplicate_call_definitions_and_deinitialization_reject() {
    let types = projection_types();
    let duplicate = function(vec![
        block(
            224,
            vec![],
            SemanticTerminatorKindV1::Call(call(vec![constant(1)], 1)),
        ),
        block(
            225,
            vec![],
            SemanticTerminatorKindV1::Call(call(vec![constant(2)], 2)),
        ),
        block(226, vec![output()], SemanticTerminatorKindV1::Return),
    ]);
    assert!(
        resolve(
            &types,
            &duplicate,
            &[marker(Instruction::VMovB32, false)],
            2,
            0
        )
        .is_err()
    );
    let deinitialized = fixture(
        call(vec![constant(5)], 1),
        vec![],
        vec![
            statement(SemanticStatementKindV1::Deinitialize(place(2))),
            output(),
        ],
    );
    assert!(
        resolve(
            &types,
            &deinitialized,
            &[marker(Instruction::VMovB32, false)],
            1,
            1
        )
        .is_err()
    );
}

#[test]
fn absent_wrong_caller_helper_and_borrowed_result_reject() {
    let types = projection_types();
    let good = call(vec![constant(5)], 1);
    let absent = SemanticDirectCallV1::new_callable(
        good.callee(),
        good.arguments().to_vec(),
        good.destination().cloned(),
        good.unwind(),
    )
    .unwrap();
    for candidate in [
        absent,
        good.clone().with_inline_assembly_source_v30(source(12)),
    ] {
        let function = fixture(candidate, vec![], vec![output()]);
        assert!(
            resolve(
                &types,
                &function,
                &[marker(Instruction::VMovB32, false)],
                1,
                0
            )
            .is_err()
        );
    }
    let helper = fixture(good.clone(), vec![], vec![output()])
        .with_role(SemanticFunctionRoleV1::InternalHelper);
    assert!(
        resolve(
            &types,
            &helper,
            &[marker(Instruction::VMovB32, false)],
            1,
            0
        )
        .is_err()
    );
    let borrowed = fixture(
        good,
        vec![],
        vec![
            aggregate_assignment_v2(
                3,
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(2),
                },
            ),
            output(),
        ],
    );
    assert!(
        resolve(
            &types,
            &borrowed,
            &[marker(Instruction::VMovB32, false)],
            1,
            1
        )
        .is_err()
    );
}

#[test]
fn unknown_callee_wrong_arity_type_and_unwind_reject() {
    let types = projection_types();
    let good = call(vec![constant(5)], 1);
    let function = fixture(good.clone(), vec![], vec![output()]);
    assert!(
        resolve(
            &types,
            &function,
            &[SemanticCallableDeclV1::defined(
                SemanticFunctionIdV1::from_index(0)
            )],
            1,
            0
        )
        .is_err()
    );
    assert!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VAddU32, false)],
            1,
            0
        )
        .is_err()
    );
    assert!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VMovB32, true)],
            1,
            0
        )
        .is_err()
    );
    let mut signed = types.clone();
    signed[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(bytes(231)),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        }),
    );
    assert!(
        resolve(
            &signed,
            &function,
            &[marker(Instruction::VMovB32, false)],
            1,
            0
        )
        .is_err()
    );
    for unwind in [
        SemanticUnwindActionV1::Terminate,
        SemanticUnwindActionV1::Cleanup(cfg_edge(SemanticEdgeRoleV1::CallUnwind, 1)),
    ] {
        let changed = SemanticDirectCallV1::new_callable(
            good.callee(),
            good.arguments().to_vec(),
            good.destination().cloned(),
            unwind,
        )
        .unwrap()
        .with_inline_assembly_source_v30(source(11));
        let function = fixture(changed, vec![], vec![output()]);
        assert!(
            resolve(
                &types,
                &function,
                &[marker(Instruction::VMovB32, false)],
                1,
                0
            )
            .is_err()
        );
    }
}

#[test]
fn call_return_edge_not_just_target_block_must_dominate_use() {
    let types = projection_types();
    let function = function(vec![
        block(
            224,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: constant(0),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            225,
            vec![],
            SemanticTerminatorKindV1::Call(call(vec![constant(5)], 2)),
        ),
        block(226, vec![output()], SemanticTerminatorKindV1::Return),
    ]);
    assert_eq!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VMovB32, false)],
            2,
            0
        )
        .unwrap_err(),
        "GPU typed ISA normal return edge does not dominate its use"
    );
}

#[test]
fn use_before_call_unreachable_use_and_loop_first_use_reject() {
    let types = projection_types();
    let before = fixture(call(vec![constant(5)], 1), vec![output()], vec![]);
    assert!(
        resolve(
            &types,
            &before,
            &[marker(Instruction::VMovB32, false)],
            0,
            0
        )
        .is_err()
    );
    let unreachable = function(vec![
        block(224, vec![], SemanticTerminatorKindV1::Return),
        block(
            225,
            vec![],
            SemanticTerminatorKindV1::Call(call(vec![constant(5)], 2)),
        ),
        block(226, vec![output()], SemanticTerminatorKindV1::Return),
    ]);
    assert!(
        resolve(
            &types,
            &unreachable,
            &[marker(Instruction::VMovB32, false)],
            2,
            0
        )
        .is_err()
    );
    let loop_first = function(vec![
        block(
            224,
            vec![output()],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            225,
            vec![],
            SemanticTerminatorKindV1::Call(call(vec![constant(5)], 0)),
        ),
    ]);
    assert!(
        resolve(
            &types,
            &loop_first,
            &[marker(Instruction::VMovB32, false)],
            0,
            0
        )
        .is_err()
    );
}

#[test]
fn malformed_nested_input_restores_use_site_and_visiting_state() {
    let types = projection_types();
    let function = fixture(
        call(vec![copy(1)], 1),
        vec![],
        vec![output(), aggregate_output_v2(constant(17))],
    );
    let callables = [marker(Instruction::VMovB32, false)];
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
        .unwrap()
        .with_gfx942_inline_callables_v30(&callables)
        .unwrap();
    assert!(
        resolver
            .resolve_store_v2(
                function.blocks()[1].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 0
                }
            )
            .is_err()
    );
    assert!(resolver.use_site.is_none());
    assert!(resolver.visiting.is_empty());
    assert_eq!(
        resolver.resolve_store_v2(
            function.blocks()[1].statements()[1].kind(),
            ScalarAssignmentSiteV1 {
                block: 1,
                statement: 1
            }
        ),
        Ok(scalar(17))
    );
}

#[test]
fn call_index_and_recursive_expression_share_existing_budgets() {
    let types = projection_types();
    let function = fixture(call(vec![constant(5)], 1), vec![], vec![output()]);
    let callables = [marker(Instruction::VMovB32, false)];
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
    resolver.definitions.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(
        resolver
            .with_gfx942_inline_callables_v30(&callables)
            .is_err()
    );
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
        .unwrap()
        .with_gfx942_inline_callables_v30(&callables)
        .unwrap();
    resolver.work = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 - 2;
    assert!(
        resolver
            .resolve_store_v2(
                function.blocks()[1].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 0
                }
            )
            .is_err()
    );
    assert!(resolver.use_site.is_none());
    assert!(resolver.visiting.is_empty());
    let ordinary = GpuSemanticExpressionResolverV2::new(&types, &function)
        .unwrap()
        .with_gfx942_inline_callables_v30(&[])
        .unwrap();
    assert!(ordinary.inline_calls_v30.is_none());
}

#[test]
fn inline_result_storage_and_move_kills_preserve_errors_and_resolver_state() {
    let types = projection_types();
    let callables = [marker(Instruction::VMovB32, false)];
    let kills = [
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(2),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(2),
        )),
        aggregate_assignment_v2(
            1,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2))),
        ),
    ];
    for kill in kills {
        let function = fixture(
            call(vec![constant(5)], 1),
            vec![],
            vec![kill, output(), aggregate_output_v2(constant(17))],
        );
        let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
            .unwrap()
            .with_gfx942_inline_callables_v30(&callables)
            .unwrap()
            .with_scalar_callables_v1(&callables)
            .unwrap();
        assert_eq!(
            resolver.resolve_store_v2(
                function.blocks()[1].statements()[1].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 1
                },
            ),
            Err("GPU scalar intrinsic result was killed before its use"),
        );
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
        assert_eq!(
            resolver.resolve_store_v2(
                function.blocks()[1].statements()[2].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 2
                },
            ),
            Ok(scalar(17)),
        );
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
    }
}

#[test]
fn inline_result_lifetime_boundaries_outside_definition_to_use_are_valid() {
    let types = projection_types();
    let function = fixture(
        call(vec![constant(5)], 1),
        vec![statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(2),
        ))],
        vec![
            output(),
            aggregate_assignment_v2(
                1,
                SCALAR_TYPE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2))),
            ),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )),
        ],
    );
    assert_eq!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VMovB32, false)],
            1,
            0
        ),
        Ok(scalar(5)),
    );
}

#[test]
fn inline_result_final_move_is_valid_but_a_subsequent_read_is_not() {
    let types = projection_types();
    let function = fixture(
        call(vec![constant(5)], 1),
        vec![],
        vec![
            aggregate_output_v2(SemanticOperandV1::Move(place(2))),
            output(),
        ],
    );
    let callables = [marker(Instruction::VMovB32, false)];
    assert_eq!(resolve(&types, &function, &callables, 1, 0), Ok(scalar(5)));
    assert_eq!(
        resolve(&types, &function, &callables, 1, 1),
        Err("GPU scalar intrinsic result was killed before its use"),
    );
}

#[test]
fn inline_ordered_operands_allow_final_moves_but_never_read_after_a_move() {
    let types = projection_types();
    for second in [copy(1), SemanticOperandV1::Move(place(1))] {
        let function = fixture(
            call(vec![SemanticOperandV1::Move(place(1)), second], 1),
            vec![assignment(1, 5)],
            vec![output(), aggregate_output_v2(constant(17))],
        );
        let callables = [marker(Instruction::VAddU32, false)];
        let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
            .unwrap()
            .with_gfx942_inline_callables_v30(&callables)
            .unwrap();
        assert_eq!(
            resolver.resolve_store_v2(
                function.blocks()[1].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 0
                },
            ),
            Err("GPU typed ISA argument reads a moved source local"),
        );
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
        assert_eq!(
            resolver.resolve_store_v2(
                function.blocks()[1].statements()[1].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 1
                },
            ),
            Ok(scalar(17)),
        );
    }
    for (arguments, rhs) in [
        (vec![copy(1), SemanticOperandV1::Move(place(1))], 5),
        (vec![SemanticOperandV1::Move(place(1)), constant(2)], 2),
    ] {
        let function = fixture(call(arguments, 1), vec![assignment(1, 5)], vec![output()]);
        assert_eq!(
            resolve(
                &types,
                &function,
                &[marker(Instruction::VAddU32, false)],
                1,
                0
            ),
            Ok(ProductionSemanticExpressionV2::Binary {
                operation: ProductionSemanticBinaryOpV2::Add,
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32
                },
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(scalar(5)),
                rhs: Box::new(scalar(rhs)),
            }),
        );
    }
    let function = fixture(
        call(vec![SemanticOperandV1::Move(place(1))], 1),
        vec![assignment(1, 5)],
        vec![output()],
    );
    assert_eq!(
        resolve(
            &types,
            &function,
            &[marker(Instruction::VMovB32, false)],
            1,
            0
        ),
        Ok(scalar(5)),
    );
}

fn nested_inline_operand_fixture(
    between: Vec<SemanticStatementV1>,
    operand: SemanticOperandV1,
) -> SemanticFunctionDeclV1 {
    let first = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        vec![constant(5)],
        Some(SemanticCallDestinationV1::new(
            place(1),
            cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_inline_assembly_source_v30(
        SemanticInlineAssemblySourceV30::new(
            bytes(210),
            SemanticFunctionIdentityV1::from_sha256(bytes(11)),
            bytes(211),
            bytes(213),
        )
        .unwrap(),
    );
    function(vec![
        block(224, vec![], SemanticTerminatorKindV1::Call(first)),
        block(
            225,
            between,
            SemanticTerminatorKindV1::Call(call(vec![operand], 2)),
        ),
        block(
            226,
            vec![output(), aggregate_output_v2(constant(17))],
            SemanticTerminatorKindV1::Return,
        ),
    ])
}

#[test]
fn nested_inline_call_input_must_be_live_and_failure_restores_every_frame() {
    let types = projection_types();
    let callables = [marker(Instruction::VMovB32, false)];
    for kill in [
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        aggregate_assignment_v2(
            0,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1))),
        ),
    ] {
        let function = nested_inline_operand_fixture(vec![kill], copy(1));
        let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
            .unwrap()
            .with_gfx942_inline_callables_v30(&callables)
            .unwrap();
        assert_eq!(
            resolver.resolve_store_v2(
                function.blocks()[2].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 2,
                    statement: 0
                },
            ),
            Err("GPU scalar intrinsic result was killed before its use"),
        );
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
        assert_eq!(
            resolver.resolve_store_v2(
                function.blocks()[2].statements()[1].kind(),
                ScalarAssignmentSiteV1 {
                    block: 2,
                    statement: 1
                },
            ),
            Ok(scalar(17)),
        );
    }
    // Moving a still-live producer into its final consumer is admitted.
    let function = nested_inline_operand_fixture(vec![], SemanticOperandV1::Move(place(1)));
    assert_eq!(resolve(&types, &function, &callables, 2, 0), Ok(scalar(5)));
}

#[test]
fn inline_liveness_consumes_the_existing_exact_graph_work_budget() {
    let types = projection_types();
    let function = fixture(call(vec![constant(5)], 1), vec![], vec![output()]);
    let callables = [marker(Instruction::VMovB32, false)];
    let resolver = || {
        GpuSemanticExpressionResolverV2::new(&types, &function)
            .unwrap()
            .with_gfx942_inline_callables_v30(&callables)
            .unwrap()
    };
    let mut measured = resolver();
    let before = measured.definitions.work;
    assert_eq!(
        measured.resolve_store_v2(
            function.blocks()[1].statements()[0].kind(),
            ScalarAssignmentSiteV1 {
                block: 1,
                statement: 0
            },
        ),
        Ok(scalar(5)),
    );
    let work = measured.definitions.work - before;
    assert!(work > 0);
    for extra in 0..=1 {
        let mut resolver = resolver();
        resolver.definitions.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - work + extra;
        let result = resolver.resolve_store_v2(
            function.blocks()[1].statements()[0].kind(),
            ScalarAssignmentSiteV1 {
                block: 1,
                statement: 0,
            },
        );
        if extra == 0 {
            assert_eq!(result, Ok(scalar(5)));
        } else {
            assert_eq!(
                result,
                Err("GPU scalar call source analysis exceeds its work limit")
            );
        }
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
    }
}

fn dependent_chain(
    instructions: &[(Instruction, Option<u32>)],
) -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
    assert!(!instructions.is_empty());
    let all = [
        Instruction::VMovB32,
        Instruction::VAddU32,
        Instruction::VSubU32,
        Instruction::VXorB32,
        Instruction::VAndB32,
        Instruction::VOrB32,
    ];
    let callables = all
        .into_iter()
        .map(|instruction| marker(instruction, false))
        .collect();
    let mut locals = vec![
        local(220, SCALAR_TYPE, SemanticLocalRoleV1::Return),
        local(221, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
        local(222, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
        local(223, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
    ];
    let mut blocks = Vec::new();
    for (index, (instruction, rhs)) in instructions.iter().enumerate() {
        let result = u32::try_from(index + 4).unwrap();
        locals.push(local(
            u8::try_from(index + 1).unwrap(),
            SCALAR_TYPE,
            SemanticLocalRoleV1::Temporary,
        ));
        let mut arguments = vec![if index == 0 {
            constant(0xffff_fff0)
        } else {
            copy(result - 1)
        }];
        arguments.extend(rhs.map(|value| constant(u128::from(value))));
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(
                all.iter()
                    .position(|candidate| candidate == instruction)
                    .unwrap() as u32,
            ),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(result),
                cfg_edge(
                    SemanticEdgeRoleV1::CallReturn,
                    u32::try_from(index + 1).unwrap(),
                ),
            )),
            SemanticUnwindActionV1::Continue,
        )
        .unwrap()
        .with_inline_assembly_source_v30(
            SemanticInlineAssemblySourceV30::new(
                bytes(210),
                SemanticFunctionIdentityV1::from_sha256(bytes(11)),
                bytes(211),
                bytes(u8::try_from(index + 1).unwrap()),
            )
            .unwrap(),
        );
        blocks.push(block(
            u8::try_from(index + 1).unwrap(),
            vec![],
            SemanticTerminatorKindV1::Call(call),
        ));
    }
    blocks.push(block(
        250,
        vec![aggregate_output_v2(copy(
            u32::try_from(instructions.len() + 3).unwrap(),
        ))],
        SemanticTerminatorKindV1::Return,
    ));
    (projection_function_with_locals(blocks, locals), callables)
}

fn seven_instructions() -> [(Instruction, Option<u32>); 7] {
    [
        (Instruction::VMovB32, None),
        (Instruction::VMovB32, None),
        (Instruction::VAddU32, Some(0x25)),
        (Instruction::VSubU32, Some(0x25)),
        (Instruction::VXorB32, Some(0x25)),
        (Instruction::VAndB32, Some(255)),
        (Instruction::VOrB32, Some(256)),
    ]
}

#[test]
fn seven_dependent_calls_match_the_actual_fixture_value_tree() {
    let types = projection_types();
    let (function, callables) = dependent_chain(&seven_instructions());
    let mut expected = scalar(0xffff_fff0);
    for (operation, rhs) in [
        (ProductionSemanticBinaryOpV2::Add, 0x25),
        (ProductionSemanticBinaryOpV2::Subtract, 0x25),
        (ProductionSemanticBinaryOpV2::BitXor, 0x25),
        (ProductionSemanticBinaryOpV2::BitAnd, 255),
        (ProductionSemanticBinaryOpV2::BitOr, 256),
    ] {
        expected = ProductionSemanticExpressionV2::Binary {
            operation,
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            },
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(expected),
            rhs: Box::new(scalar(rhs)),
        };
    }
    assert_eq!(resolve(&types, &function, &callables, 7, 0), Ok(expected));
    // Seven distinct call results remain in the actual semantic function; the
    // two move aliases disappear only from this scalar-expression projection.
    assert_eq!(
        function
            .blocks()
            .iter()
            .filter(|block| matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(_)))
            .count(),
        7
    );
}

#[test]
fn dependent_chain_exact_node_budget_and_one_short_restore_state() {
    let types = projection_types();
    let (function, callables) = dependent_chain(&seven_instructions());
    let mut reference = GpuSemanticExpressionResolverV2::new(&types, &function)
        .unwrap()
        .with_gfx942_inline_callables_v30(&callables)
        .unwrap();
    let output = function.blocks()[7].statements()[0].kind();
    let site = ScalarAssignmentSiteV1 {
        block: 7,
        statement: 0,
    };
    let expected = reference.resolve_store_v2(output, site).unwrap();
    let work = reference.work;
    assert!(work > 7);
    for extra in [0, 1] {
        let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
            .unwrap()
            .with_gfx942_inline_callables_v30(&callables)
            .unwrap();
        resolver.work = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 - work + extra;
        let result = resolver.resolve_store_v2(output, site);
        if extra == 0 {
            assert_eq!(result, Ok(expected.clone()));
        } else {
            assert_eq!(
                result.unwrap_err(),
                "GPU semantic expression exceeds its bounded node budget"
            );
        }
        assert!(resolver.use_site.is_none());
        assert!(resolver.visiting.is_empty());
    }
}

#[test]
fn dependent_move_chain_depth_rejection_restores_all_recursive_frames() {
    let types = projection_types();
    let length = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1;
    let (function, callables) = dependent_chain(&vec![(Instruction::VMovB32, None); length]);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function)
        .unwrap()
        .with_gfx942_inline_callables_v30(&callables)
        .unwrap();
    assert_eq!(
        resolver
            .resolve_store_v2(
                function.blocks()[length].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: length,
                    statement: 0
                }
            )
            .unwrap_err(),
        "GPU semantic expression exceeds its bounded recursion depth"
    );
    assert!(resolver.use_site.is_none());
    assert!(resolver.visiting.is_empty());
    assert!(resolver.work < fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2);
}
