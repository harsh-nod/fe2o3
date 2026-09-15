use super::*;

#[test]
fn mutable_unverified_block_operations_keep_storage_and_coordinates() {
    let mut block = ProductionRankedBlockV1::with_index_arguments(
        2,
        vec![
            ProductionRankedOperationV1::SemanticConstant {
                result: ProductionRankedValueIdV1::new(0),
                value: 7,
            },
            ProductionRankedOperationV1::SemanticConstant {
                result: ProductionRankedValueIdV1::new(1),
                value: 9,
            },
        ],
        ProductionRankedTerminatorV1::Return,
    );
    let address = block.operations().as_ptr();
    let untouched = block.operations()[1].clone();
    block.operations_mut()[0] = ProductionRankedOperationV1::SemanticExpression {
        result: ProductionRankedValueIdV1::new(0),
        expression: ProductionSemanticExpressionV2::Constant {
            scalar: super::super::ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            },
            bits: 7,
        },
        numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
    };
    assert_eq!(block.operations().as_ptr(), address);
    assert_eq!(block.operations().len(), 2);
    assert_eq!(block.operations()[1], untouched);
    assert_eq!(block.index_argument_count(), 2);
    assert_eq!(block.terminator(), &ProductionRankedTerminatorV1::Return);
}

#[test]
fn mutated_builder_is_revalidated_and_cannot_mutate_a_constructed_kernel() {
    let mut block = ProductionRankedBlockV1::new(
        vec![ProductionRankedOperationV1::SemanticConstant {
            result: ProductionRankedValueIdV1::new(0),
            value: 7,
        }],
        ProductionRankedTerminatorV1::Return,
    );
    let kernel = ProductionRankedKernelV1::new("immutable_kernel", 0, vec![block.clone()]).unwrap();
    block.operations_mut()[0] = ProductionRankedOperationV1::SemanticConstant {
        result: ProductionRankedValueIdV1::new(1),
        value: 8,
    };
    assert!(
        matches!(kernel.blocks()[0].operations()[0], ProductionRankedOperationV1::SemanticConstant { result, value: 7 } if result.get() == 0)
    );
    assert!(matches!(
        ProductionRankedKernelV1::new("invalid_builder", 0, vec![block]),
        Err(ProductionRankedKernelErrorV1::NonCanonicalValueId {
            expected: 0,
            actual: 1
        })
    ));
}
