use super::*;

fn less_than(left: SemanticOperandV1, right: SemanticOperandV1) -> SemanticStatementV1 {
    assign(
        4,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        },
    )
}

fn subtraction() -> Vec<Block> {
    let mut blocks = fixture(SemanticUncheckedBinaryOpV1::Subtract);
    blocks[0].0 = vec![
        alias(5, copy(1, INTEGER)),
        alias(6, copy(2, INTEGER)),
        less_than(copy(5, INTEGER), copy(6, INTEGER)),
    ];
    blocks
}

#[test]
fn unsigned_less_than_proof_reaches_ranked_subtraction_without_rewriting_mir() {
    for bits in [8, 16, 32, 64] {
        let types = types(false, bits);
        let function = function(subtraction());
        let before = function.clone();
        assert!(
            semantic_unchecked_arithmetic_violation_v1(&function)
                .unwrap()
                .is_some()
        );
        assert!(
            semantic_unchecked_arithmetic_violation_with_types_v1(&types, &function)
                .unwrap()
                .is_none()
        );
        let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
        let expected = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Subtract,
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits,
            },
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(0).unwrap(),
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits,
                },
            }),
            rhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(1).unwrap(),
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits,
                },
            }),
        };
        for _ in 0..2 {
            assert_eq!(
                resolver.resolve_rvalue_v2(unchecked(&function)).unwrap(),
                expected
            );
        }
        assert_eq!(resolver.unchecked.proof.as_ref().unwrap().sites.len(), 1);
        assert_eq!(function, before);
    }
}

#[test]
fn unsigned_less_than_ranked_proof_rejects_signed_swapped_moved_and_reassigned_operands() {
    let signed = types(true, 32);
    let original = function(subtraction());
    assert_eq!(
        GpuSemanticExpressionResolverV2::new(&signed, &original)
            .resolve_rvalue_v2(unchecked(&original)),
        Err(UNPROVEN)
    );
    let mut swapped = subtraction();
    swapped[0].0[2] = less_than(copy(6, INTEGER), copy(5, INTEGER));
    let mut stale = subtraction();
    stale[2].0.insert(0, alias(6, constant(7, INTEGER, 4)));
    let mut consumed = subtraction();
    consumed[0].0[2] = less_than(moved(5, INTEGER), copy(6, INTEGER));
    let mut flag = subtraction();
    flag[0].0.push(alias(4, constant(0, BOOL, 1)));
    let mut width = subtraction();
    width[0].0[2] = less_than(copy(5, INTEGER), copy(6, OTHER_INTEGER));
    let mut join = subtraction();
    join[1].1 = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2));
    let mut backedge = subtraction();
    backedge[1].1 = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0));
    for blocks in [swapped, stale, consumed, flag, width, join, backedge] {
        rejects(&function(blocks));
    }
}

#[test]
fn unsigned_less_than_ranked_proof_remains_bound_to_exact_function_types_and_source_node() {
    let types = types(false, 32);
    let other_types = types.clone();
    let function = function(subtraction());
    let other_function = function.clone();
    let synthetic = unchecked(&function).clone();
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
    assert_eq!(resolver.resolve_rvalue_v2(&synthetic), Err(UNPROVEN));
    assert_eq!(
        resolver.resolve_rvalue_v2(unchecked(&other_function)),
        Err(UNPROVEN)
    );
    resolver.types = &other_types;
    assert_eq!(
        resolver.resolve_rvalue_v2(unchecked(&function)),
        Err(UNPROVEN)
    );
    resolver.types = &types;
    resolver.function = &other_function;
    assert_eq!(
        resolver.resolve_rvalue_v2(unchecked(&function)),
        Err(UNPROVEN)
    );
    resolver.function = &function;
    assert!(resolver.resolve_rvalue_v2(unchecked(&function)).is_ok());
}
