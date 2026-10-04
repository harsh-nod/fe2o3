//! Conversion components only, not source/proof admission.
use super::*;
use crate::conditional_reference_v1::fixtures::*;

#[test]
fn portable_join_conversion_preserves_scalar_cast_checked_and_read_semantics() {
    let fixture = Fixture::new();
    let kernel = kernel();
    let gpu = gpu_load();
    let load = reference_expression_inner_checked_v2(
        &fixture.ir,
        &cpu_load(),
        ReferenceScalarTypeV1::F32,
        Some(ReferenceGpuLoadsV2 {
            kernel: &kernel,
            expression: &gpu,
            arguments: None,
        }),
    )
    .unwrap();
    assert_eq!(load, gpu);
    let cast = ReferenceEffectExpressionV1::Cast {
        kind: ReferenceCastKindV1::FloatToIntegerSaturating,
        source: ReferenceScalarTypeV1::F32,
        target: ReferenceScalarTypeV1::I32,
        operand: Box::new(constant()),
    };
    let converted =
        reference_expression_inner_checked_v2(&fixture.ir, &cast, ReferenceScalarTypeV1::I32, None)
            .unwrap();
    assert!(matches!(
        converted,
        Expr::Cast {
            kind: ProductionSemanticCastV2::FloatToIntegerSaturating,
            ..
        }
    ));
    assert!(matches!(
        ProductionNumericalContractV2::exact_for_expression(&converted),
        ProductionNumericalContractV2::ExactIeee754OperatorCongruence { .. }
    ));
    let integer = || {
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::U32,
            bits: 7,
        })
    };
    for checked in [false, true] {
        let expression = ReferenceEffectExpressionV1::Binary {
            operation: ReferenceBinaryOpV1::Add,
            lhs: Box::new(integer()),
            rhs: Box::new(integer()),
            checked,
        };
        let converted = reference_expression_inner_checked_v2(
            &fixture.ir,
            &expression,
            ReferenceScalarTypeV1::U32,
            None,
        )
        .unwrap();
        assert!(matches!(converted, Expr::Binary { overflow, .. }
            if overflow == if checked { ProductionOverflowContractV2::Checked } else { ProductionOverflowContractV2::Wrapping }));
    }
}

#[test]
fn portable_join_conversion_refuses_coherent_read_origin_type_index_and_multiplicity_substitutions()
{
    let fixture = Fixture::new();
    let kernel = kernel();
    for mutation in 0..6 {
        let mut gpu = gpu_load();
        let Expr::Load(load) = &mut gpu else {
            unreachable!()
        };
        match mutation {
            0 => load.allocation_origin += 1,
            1 => {
                load.scalar = ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32,
                }
            }
            2 => load.indices[0] = Value::Argument(0),
            3 => load.indices = Box::default(),
            4 => {
                gpu = Expr::Compare {
                    operation: ProductionSemanticComparisonV2::Equal,
                    operand_scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                    lhs: Box::new(gpu.clone()),
                    rhs: Box::new(gpu.clone()),
                }
            }
            5 => {}
            _ => unreachable!(),
        }
        let result = reference_expression_inner_checked_v2(
            &fixture.ir,
            &cpu_load(),
            ReferenceScalarTypeV1::F32,
            Some(ReferenceGpuLoadsV2 {
                kernel: &kernel,
                expression: &gpu,
                arguments: if mutation == 5 { Some(&[]) } else { None },
            }),
        );
        assert!(
            matches!(result, Err(Error::UnsupportedReference(_))),
            "mutation {mutation}"
        );
    }
}

#[test]
fn portable_join_conversion_keeps_errors_and_depth_limits() {
    let fixture = Fixture::new();
    for (expression, expected, message) in [
        (
            constant(),
            ReferenceScalarTypeV1::U32,
            "reference output RHS type disagrees with its logical ABI",
        ),
        (
            cpu_load(),
            ReferenceScalarTypeV1::F32,
            "safe reference load requires an independently projected GPU expression",
        ),
        (
            ReferenceEffectExpressionV1::InputLength {
                reference_argument: 1,
            },
            ReferenceScalarTypeV1::Usize,
            "reference slice length cannot be used as an opaque semantic value",
        ),
    ] {
        assert!(
            matches!(reference_expression_inner_checked_v2(&fixture.ir, &expression, expected, None),
            Err(Error::UnsupportedReference(actual)) if actual == message)
        );
    }
    let mut deep = constant();
    for _ in 0..fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        deep = ReferenceEffectExpressionV1::Unary {
            operation: ReferenceUnaryOpV1::Negate,
            operand: Box::new(deep),
        };
    }
    assert!(matches!(
        reference_expression_inner_checked_v2(&fixture.ir, &deep, ReferenceScalarTypeV1::F32, None),
        Err(Error::UnsupportedReference(
            "reference RHS exceeds the typed semantic expression depth bound"
        ))
    ));
}
