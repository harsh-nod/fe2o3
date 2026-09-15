use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, plan_semantic_function_ssa_v1};

// Actual shared Graph/SSA cost fixture, not a source-authenticated capability.
fn body(unreachable_tail: bool) -> SemanticFunctionDeclV1 {
    let source =
        super::super::super::resource_tests::noop_semantic_owner(&["matrix_selection_cost"]);
    let original = &source.semantic().functions()[0];
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let blocks = (0..64u8)
        .map(|index| {
            let terminator = if index == 63 || (unreachable_tail && index == 62) {
                SemanticTerminatorKindV1::Return
            } else {
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(u32::from(index) + 1),
                ))
            };
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([index + 1; 32]),
                provenance,
                vec![SemanticStatementV1::new(
                    provenance,
                    SemanticStatementKindV1::Nop,
                )],
                SemanticTerminatorV1::new(provenance, terminator),
            )
            .unwrap()
        })
        .collect();
    SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn candidate() -> SemanticCompilerIntrinsicOperationV1 {
    SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
        lane: SemanticTypeIdV1::from_index(0),
        fragment: SemanticTypeIdV1::from_index(0),
        contract: SemanticMfmaAccumulatorContractV1 {
            profile: SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
            distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
            wave_width: 64,
        },
    }
}

#[test]
fn unrelated_blocks_do_not_spend_per_destination_reachability_work() {
    let body = body(false);
    let plan = plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        &body,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let unrelated = SemanticCompilerIntrinsicOperationV1::KernelContextIssue {
        context: SemanticTypeIdV1::from_index(0),
    };
    let mut filtered = Graph::new(&body, plan.plan(), 512).unwrap();
    for block in 0..64 {
        filtered.charge(1).unwrap();
        assert!(
            reachable_source_operand(&mut filtered, block, &unrelated)
                .unwrap()
                .is_none()
        );
    }
    // The previous ordering queried each target before recognizing that no
    // Matrix consumer existed. Same graph implementation and same allowance.
    let mut eager = Graph::new(&body, plan.plan(), 512).unwrap();
    let error = (0..64)
        .try_for_each(|block| {
            eager.charge(1)?;
            assert!(eager.reaches(0, block)?);
            Ok::<_, ProductionSemanticKirErrorV1>(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::ResourceLimit { limit: 512, .. }
    ));
    assert!(!ProductionScopedMatrixUseRelationV1::has_potential_source_consumers(&body, &[]));
}

#[test]
fn selected_operations_still_require_reachability_and_shared_budget() {
    for unreachable in [false, true] {
        let body = body(unreachable);
        let plan = plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &body,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut graph = Graph::new(&body, plan.plan(), 512).unwrap();
        assert_eq!(
            reachable_source_operand(&mut graph, 63, &candidate())
                .unwrap()
                .is_some(),
            !unreachable
        );
        let mut small = Graph::new(&body, plan.plan(), 128).unwrap();
        assert!(matches!(
            reachable_source_operand(&mut small, 63, &candidate()),
            Err(ProductionSemanticKirErrorV1::ResourceLimit { limit: 128, .. })
        ));
    }
}

#[test]
fn matrix_precheck_does_not_classify_unrelated_terminal_families() {
    let ty = SemanticTypeIdV1::from_index(0);
    for operation in [
        SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: ty },
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
            fragment: ty,
            values: ty,
        },
    ] {
        assert!(source_operand(&operation).is_none());
    }
    assert_eq!(source_operand(&candidate()), Some((0, Role::Lane, ty)));
}
