#[test]
fn constant_select_replay_rejects_stale_receipt_despite_identical_rendered_value() {
    use crate::functional_refinement_receipt_v2::{
        FunctionalRefinementVerusExecutionErrorKindV2, generate_ranked_effect_formula_replay_v2,
    };
    use ProductionSemanticExpressionV2 as Expr;
    use ProductionSemanticScalarTypeV2 as Scalar;

    let scalar = Scalar::Float { bits: 32 };
    let (base, _) = bound_scalar_effect_kernel(scalar);
    let rebuild = |operations| {
        ProductionRankedKernelV1::new(
            base.function_name(),
            base.argument_count(),
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    };
    let ProductionRankedOperationV1::RequireEffectRefinement { contract, .. } =
        base.blocks()[0].operations()[9].clone()
    else {
        unreachable!()
    };
    let request = ProductionRankedOperationV1::RequestEffectRefinement {
        contract,
        subjects: replay_subjects(),
    };

    for condition in [0, 1] {
        let expression = |discarded| {
            let selected = Expr::Constant { scalar, bits: 7 };
            let discarded = Expr::Constant {
                scalar,
                bits: discarded,
            };
            let (when_true, when_false) = if condition == 1 {
                (selected, discarded)
            } else {
                (discarded, selected)
            };
            ProductionRankedOperationV1::SemanticExpression {
                result: ProductionRankedValueIdV1::new(2),
                numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
                expression: Expr::Select {
                    scalar,
                    condition: Box::new(Expr::Constant {
                        scalar: Scalar::Bool,
                        bits: condition,
                    }),
                    when_true: Box::new(when_true),
                    when_false: Box::new(when_false),
                },
            }
        };
        let mut operations = base.blocks()[0].operations().to_vec();
        operations[3] = expression(8);
        operations[9] = request.clone();
        let skeleton = rebuild(operations);
        // Existing synthetic receipt fixtures test replay binding, not production admission.
        let (proof, _) = test_effect_request(&skeleton, 9, 111);
        let original_obligation = proof.binding().normalized_obligation_effect_ir_hash();
        let bound = skeleton
            .bind_functional_refinement_request_v2(0, 9, proof)
            .unwrap();
        let replay =
            generate_ranked_effect_formula_replay_v2(&bound, 0, 9, "selection_replay").unwrap();
        assert!(replay.uses_ieee_congruence());
        let definition = |id| {
            let prefix = format!("let v{id}: int = ");
            replay
                .lemma()
                .lines()
                .find_map(|line| line.trim().strip_prefix(&prefix))
                .unwrap()
        };
        assert_eq!(definition(2), definition(3));

        let mut changed = bound.blocks()[0].operations().to_vec();
        changed[3] = expression(9);
        let stale = rebuild(changed.clone());
        let error = generate_ranked_effect_formula_replay_v2(&stale, 0, 9, "selection_replay")
            .err()
            .unwrap();
        assert_eq!(
            error.kind(),
            FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe
        );

        changed[9] = request.clone();
        let skeleton = rebuild(changed);
        let (proof, _) = test_effect_request(&skeleton, 9, 111);
        assert_ne!(
            original_obligation,
            proof.binding().normalized_obligation_effect_ir_hash()
        );
        let rebound = skeleton
            .bind_functional_refinement_request_v2(0, 9, proof)
            .unwrap();
        let replay_changed =
            generate_ranked_effect_formula_replay_v2(&rebound, 0, 9, "selection_replay").unwrap();
        assert_eq!(replay.lemma(), replay_changed.lemma());
    }
}
