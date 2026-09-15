// Included only after the genuine slice source fixture. These tests enter the
// actual MIR admission, materialization, B->O and mandatory ranked-root callback.
#[test]
fn actual_global_scalar_values_are_attached_at_the_source_write_site() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for shape in [
            GlobalWriteExpressionShapeV1::Parameter,
            GlobalWriteExpressionShapeV1::ParameterArithmetic,
            GlobalWriteExpressionShapeV1::LoadArithmetic,
        ] {
            let source = genuine_dynamic_global_source_v1(shape, 1);
            with_actual_dynamic_global_root_v1(&source, profile, 1, |root, _, _| {
                let mut writes = 0;
                for site in &root.access_sources {
                    let block = &root.lowering.kernel().blocks()[site.ranked_block() as usize];
                    let operation = site.ranked_operation() as usize;
                    let fe2o3_pliron::ProductionRankedOperationV1::ValueAccess {
                        kind: dialect_kernel::AccessKindAttr::Write,
                        value: fe2o3_pliron::ProductionRankedValueV1::Local(value),
                        ..
                    } = &block.operations()[operation]
                    else {
                        continue;
                    };
                    writes += 1;
                    assert_ne!(site.ranked_block(), 0);
                    let fe2o3_pliron::ProductionRankedOperationV1::SemanticExpression {
                        result,
                        expression,
                        numerical_contract,
                    } = &block.operations()[operation - 1]
                    else {
                        panic!("adjacent source-derived scalar expression")
                    };
                    assert_eq!(result, value);
                    assert_eq!(
                        *numerical_contract,
                        fe2o3_pliron::ProductionNumericalContractV2::exact_for_expression(
                            expression
                        )
                    );
                    if matches!(shape, GlobalWriteExpressionShapeV1::LoadArithmetic) {
                        let fe2o3_pliron::ProductionSemanticExpressionV2::Binary { lhs, .. } =
                            expression
                        else {
                            panic!("load arithmetic")
                        };
                        let fe2o3_pliron::ProductionSemanticExpressionV2::Load(load) = lhs.as_ref()
                        else {
                            panic!("exact captured read")
                        };
                        assert_ne!(load.block, 0);
                        if load.block == site.ranked_block() {
                            assert!((load.operation as usize) < operation - 1);
                        }
                        assert!(
                            root.access_sources
                                .iter()
                                .any(|read| read.ranked_block() == load.block
                                    && read.ranked_operation() == load.operation)
                        );
                    }
                }
                assert_eq!(writes, 1);
                Ok(())
            })
            .unwrap();
        }
    }
}

fn genuine_global_load_carrier_source_v1() -> ProductionPreRankedKirOwnerV1 {
    let original =
        genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::LoadArithmetic, 1);
    let semantic = original.semantic_ssa().source_semantic();
    let function = &semantic.functions()[0];
    let statements = function.blocks()[1].statements();
    assert_eq!(statements.len(), 3);
    let blocks = vec![
        function.blocks()[0].clone(),
        block(
            202,
            vec![statements[0].clone()],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        function.blocks()[2].clone(),
        block(
            206,
            statements[1..].to_vec(),
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let function = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(function.kernel_entry().unwrap().clone());
    let ssa = assertion_ssa_functions(global_ranked_slice_types_v1(), vec![function]);
    materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 1)]).unwrap()
}

#[test]
fn actual_global_value_keeps_a_dominating_cross_block_read_occurrence() {
    let source = genuine_global_load_carrier_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual_dynamic_global_root_v1(&source, profile, 1, |root, _, _| {
            let write = root
                .access_sources
                .iter()
                .find(|site| site.semantic_block() == 3)
                .expect("source carrier store");
            let expression = &root.lowering.kernel().blocks()[write.ranked_block() as usize]
                .operations()[write.ranked_operation() as usize - 1];
            let fe2o3_pliron::ProductionRankedOperationV1::SemanticExpression {
                expression: fe2o3_pliron::ProductionSemanticExpressionV2::Binary { lhs, .. },
                ..
            } = expression
            else {
                panic!("source carrier arithmetic")
            };
            let fe2o3_pliron::ProductionSemanticExpressionV2::Load(load) = lhs.as_ref() else {
                panic!("source carrier read")
            };
            assert_ne!(load.block, write.ranked_block());
            assert!(
                root.access_sources
                    .iter()
                    .any(|site| site.semantic_block() == 1
                        && site.ranked_block() == load.block
                        && site.ranked_operation() == load.operation)
            );
            Ok(())
        })
        .unwrap();
    }
}
