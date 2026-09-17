// Component projection tests do not grant source, HB, or dispatch authority.
fn publication_control_rebuild_v1(
    function: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals,
        function.entry(),
        blocks,
    )
    .unwrap()
}

fn publication_control_fixture_v1(
    operation: SemanticBinaryOpV1,
    explicit: u128,
) -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
    Vec<Option<AllocationContractV1>>,
) {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_fixture_v1();
    let mut locals = function.locals().to_vec();
    locals[10] = local(250, ty(9), SemanticLocalRoleV1::Temporary);
    let mut blocks = function.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    statements.extend([
        typed_assignment(
            6,
            ty(8),
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: typed_operand(2, ty(4)),
            },
        ),
        typed_assignment(
            10,
            ty(9),
            SemanticRvalueKindV1::Binary {
                operation,
                left: typed_operand(4, ty(8)),
                right: typed_operand(6, ty(8)),
            },
        ),
    ]);
    blocks[0] = block(
        230,
        statements,
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: typed_operand(10, ty(9)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    explicit,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                )],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        },
    );
    (
        types,
        publication_control_rebuild_v1(&function, locals, blocks),
        callables,
        allocations,
    )
}

fn publication_control_project_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    allocations: &[Option<AllocationContractV1>],
) -> (
    ProjectedStaticPublicationV1,
    Vec<ProductionRankedOperationV1>,
) {
    let source = audit_static_publication_source_v1(types, callables, function, allocations)
        .unwrap()
        .unwrap();
    let indices = vec![None; function.locals().len()];
    let mut extents = vec![None; function.locals().len()];
    let mut next_argument = 1;
    let mut operations = Vec::new();
    let mut next_value = 0;
    let projected = StaticPublicationProjectorV1 {
        types,
        callables,
        function,
        index_values: &indices,
        extent_arguments: &mut extents,
        next_argument: &mut next_argument,
        entry_operations: &mut operations,
        next_value: &mut next_value,
    }
    .project(source)
    .unwrap();
    (projected, operations)
}

#[test]
fn publication_exact_control_preserves_all_unsigned_comparison_polarities() {
    use SemanticBinaryOpV1::{Equal, GreaterOrEqual, GreaterThan, LessOrEqual, LessThan, NotEqual};
    for (operation, kind, swap, inverted) in [
        (
            Equal,
            StaticPublicationComparisonKindV1::Equal,
            false,
            false,
        ),
        (
            NotEqual,
            StaticPublicationComparisonKindV1::Equal,
            false,
            true,
        ),
        (
            LessThan,
            StaticPublicationComparisonKindV1::LessThan,
            false,
            false,
        ),
        (
            GreaterOrEqual,
            StaticPublicationComparisonKindV1::LessThan,
            false,
            true,
        ),
        (
            GreaterThan,
            StaticPublicationComparisonKindV1::LessThan,
            true,
            false,
        ),
        (
            LessOrEqual,
            StaticPublicationComparisonKindV1::LessThan,
            true,
            true,
        ),
    ] {
        for explicit in [0, 1] {
            let (types, function, callables, allocations) =
                publication_control_fixture_v1(operation, explicit);
            let (projected, _) =
                publication_control_project_v1(&types, &function, &callables, &allocations);
            let producer = projected.blocks[1].as_ref().unwrap();
            assert_eq!(producer.index, projected.blocks[2].as_ref().unwrap().index);
            let flag_extent = ProductionRankedValueV1::Argument(2);
            let (lhs, rhs) = if swap {
                (flag_extent, producer.index)
            } else {
                (producer.index, flag_extent)
            };
            let true_block = if (explicit == 1) != inverted { 1 } else { 2 };
            assert_eq!(
                projected.controls[0],
                Some(ProjectedCfgTerminatorV1::PublicationComparison {
                    kind,
                    lhs,
                    rhs,
                    true_block,
                    false_block: 3 - true_block,
                })
            );
        }
    }
}

#[test]
fn publication_exact_control_retains_original_integer_length_switch_literal() {
    let ty = SemanticTypeIdV1::from_index;
    for literal in [127, 128] {
        let (types, function, callables, allocations) =
            publication_control_fixture_v1(SemanticBinaryOpV1::LessThan, 0);
        let mut blocks = function.blocks().to_vec();
        blocks[0] = block(
            230,
            blocks[0].statements()[..2].to_vec(),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: typed_operand(6, ty(8)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        literal,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        );
        let function = static_publication_reblock_v1(&function, blocks);
        let (projected, operations) =
            publication_control_project_v1(&types, &function, &callables, &allocations);
        let Some(ProjectedCfgTerminatorV1::ExactSwitch(switch)) = &projected.controls[0] else {
            panic!("exact source switch missing")
        };
        assert_eq!(switch.source_discriminant, typed_operand(6, ty(8)));
        assert_eq!(switch.discriminant, ProductionRankedValueV1::Argument(2));
        assert_eq!(switch.targets[0].0, literal);
        assert_eq!((switch.targets[0].2, switch.otherwise), (1, 2));
        assert!(operations.iter().any(|op| matches!(op, ProductionRankedOperationV1::IndexConstant { value, .. } if u128::from(*value) == literal)));
    }
}

#[test]
fn publication_exact_control_does_not_reuse_stale_or_unsupported_guard_values() {
    let ty = SemanticTypeIdV1::from_index;
    for mutant in 0..3 {
        let (types, function, callables, allocations) =
            publication_control_fixture_v1(SemanticBinaryOpV1::LessThan, 0);
        let mut blocks = function.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        match mutant {
            0 => statements.push(statements[1].clone()),
            1 => statements.swap(1, 2),
            _ => {
                statements[1] = typed_assignment(
                    6,
                    ty(8),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Subtract,
                        left: typed_constant(ty(8), 128, 8),
                        right: typed_operand(4, ty(8)),
                    },
                )
            }
        }
        blocks[0] = block(230, statements, blocks[0].terminator().kind().clone());
        let function = static_publication_reblock_v1(&function, blocks);
        let (projected, _) =
            publication_control_project_v1(&types, &function, &callables, &allocations);
        assert_eq!(
            projected.controls[0], None,
            "mutant {mutant} gained exact authority"
        );
    }
}

#[test]
fn publication_exact_control_binds_payload_snapshot_length_to_original_extent() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_metadata_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    blocks[4] = block(234, vec![], zero_switch(10, ty(8), 1, 2));
    let function = static_publication_reblock_v1(&function, blocks);
    let (projected, _) =
        publication_control_project_v1(&types, &function, &callables, &allocations);
    let Some(ProjectedCfgTerminatorV1::ExactSwitch(switch)) = &projected.controls[4] else {
        panic!("snapshot length lost")
    };
    assert_eq!(switch.discriminant, ProductionRankedValueV1::Argument(1));
    assert_eq!(
        projected.blocks[1].as_ref().unwrap().extent,
        switch.discriminant
    );
}

#[test]
fn publication_exact_control_cache_rechecks_each_use_and_metadata_root() {
    let ty = SemanticTypeIdV1::from_index;
    let (types, function, callables, allocations) = static_publication_metadata_fixture_v1();
    let source = audit_static_publication_source_v1(&types, &callables, &function, &allocations)
        .unwrap()
        .unwrap();
    let indices = vec![None; function.locals().len()];
    let mut operations = Vec::new();
    let mut next_value = 0;
    let mut exact = StaticPublicationControlProjectorV1 {
        types: &types,
        callables: &callables,
        function: &function,
        source: &source,
        index_values: &indices,
        extents: [
            ProductionRankedValueV1::Argument(1),
            ProductionRankedValueV1::Argument(2),
        ],
        operations: &mut operations,
        next_value: &mut next_value,
        proofs: SemanticAssertProofsV1::new(&types, &function).unwrap(),
        calls: vec![None; function.locals().len()],
        values: vec![None; function.locals().len()],
    };
    let after = ScalarAssignmentSiteV1 {
        block: 0,
        statement: 1,
    };
    let before = ScalarAssignmentSiteV1 {
        block: 0,
        statement: 0,
    };
    assert!(
        exact
            .operand(&typed_operand(4, ty(8)), after, 0)
            .unwrap()
            .is_some()
    );
    assert!(exact.values[4].is_some());
    assert_eq!(
        exact.operand(&typed_operand(4, ty(8)), before, 0).unwrap(),
        None
    );
    // The same nominal DisjointSlice type on another exclusive argument is not
    // the publication payload, even though it has its own valid allocation.
    assert_eq!(
        exact
            .metadata_root(&typed_place(11, ty(11)), after, 0)
            .unwrap(),
        None
    );
    assert_eq!(
        exact
            .metadata_root(&typed_place(1, ty(11)), after, 0)
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        exact
            .metadata_root(&typed_place(2, ty(4)), after, 0)
            .unwrap(),
        Some(1)
    );
}
