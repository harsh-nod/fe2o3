use super::super::*;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionIeeeExceptionalValuePolicyV2, ProductionIeeeRoundingModeV2,
    ProductionNumericalContractV2, ProductionSemanticExpressionV2 as P,
    ProductionSemanticScalarTypeV2 as S,
};

#[path = "tests/parameter_fixture.rs"]
mod parameter_fixture;
#[path = "../functional_proof_phase_v1/tests/fixture.rs"]
mod ranked_fixture;

fn exact_float() -> ProductionNumericalContractV2 {
    ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
        rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
        exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
    }
}

#[test]
fn unproved_real_ranked_roster_cannot_construct_requests() {
    let (owner, roster) = ranked_fixture::source_fixture();
    let error = prepare_final_write_contract_requests_v1(
        &roster,
        &owner,
        &AuthenticatedReferenceEffectBindingsV1::default(),
        owner.canonical_kernel_ir_v13().unwrap(),
        7,
    )
    .unwrap_err();
    assert!(matches!(error, Failure::SourceProof(_)));
}

#[test]
fn actual_kernel_roster_requires_bijection_not_just_matching_names() {
    let (owner, roster) = ranked_fixture::source_fixture();
    require_kernel_bijection(&roster, owner.module()).unwrap();
    let mut module = owner.module().clone();
    module.kernels.pop();
    assert_eq!(
        require_kernel_bijection(&roster, &module),
        Err(Failure::RootBijection)
    );
    let mut module = owner.module().clone();
    module.kernels[1] = module.kernels[0].clone();
    assert_eq!(
        require_kernel_bijection(&roster, &module),
        Err(Failure::RootBijection)
    );
    let mut module = owner.module().clone();
    module.kernels[0].entry = module.kernels[1].entry.clone();
    assert_eq!(
        require_kernel_bijection(&roster, &module),
        Err(Failure::RootBijection)
    );
}

#[test]
fn actual_source_parameter_identity_survives_final_renumbering_and_ignored_argument() {
    let owner = parameter_fixture::owner();
    owner.verify_equivalence().unwrap();
    let [ignored] = owner.correspondence().ignored_parameter_bindings() else {
        panic!("the by-value unit argument must retain exactly one zero-component binding");
    };
    assert_eq!(
        ignored.semantic_function(),
        SemanticFunctionIdV1::from_index(0)
    );
    assert_eq!(ignored.semantic_local(), SemanticLocalIdV1::from_index(1));
    assert_eq!(ignored.semantic_type(), SemanticTypeIdV1::from_index(0));
    let source = &owner.module().functions[0];
    assert_eq!(source.signature.parameters, [Type::F32]);
    let source_parameter = source.body.as_ref().unwrap().parameters[0];
    assert_eq!(
        owner.source_argument_for_kernel_parameter(&source.id, source_parameter),
        Some(1)
    );
    let mut module = owner.module().clone();
    module.functions[0].body.as_mut().unwrap().parameters[0] = ValueId(700);
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let final_module = fe2o3_kernel_ir::decode_module_v13(canonical.canonical_bytes()).unwrap();
    let bindings = bind_parameters(&owner, &final_module.functions[0]).unwrap();
    assert_eq!(bindings.len(), 1);
    let binding = &bindings[0];
    assert_eq!(binding.source_argument(), 1);
    assert_eq!(binding.source_parameter(), source_parameter);
    assert_eq!(binding.final_parameter(), ValueId(700));
    assert_eq!(binding.final_ordinal(), 0);
    assert_eq!(binding.parameter_type(), &Type::F32);
    assert_eq!(
        owner.source_argument_for_kernel_parameter(&source.id, ValueId(700)),
        None
    );
    let expression = P::Symbol {
        symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(1).unwrap(),
        scalar: S::Float { bits: 32 },
    };
    let converted = typed_expression::reference_rhs(&expression, exact_float(), &bindings).unwrap();
    assert!(matches!(
        converted,
        SemanticTypedExpressionV1::Symbol { symbol: 0, .. }
    ));
}

#[test]
fn parameter_binding_rejects_foreign_function_type_and_missing_parameter() {
    let owner = parameter_fixture::owner();
    let source = &owner.module().functions[0];
    let mut changed = source.clone();
    changed.id = FunctionId::new("foreign");
    assert_eq!(
        bind_parameters(&owner, &changed).unwrap_err(),
        Failure::ParameterIdentity
    );
    let mut changed = source.clone();
    changed.signature.parameters[0] = Type::F64;
    assert_eq!(
        bind_parameters(&owner, &changed).unwrap_err(),
        Failure::ParameterIdentity
    );
    let mut changed = source.clone();
    changed.body.as_mut().unwrap().parameters.clear();
    assert_eq!(
        bind_parameters(&owner, &changed).unwrap_err(),
        Failure::ParameterIdentity
    );
}

#[test]
fn expression_conversion_rejects_load_namespace_unknown_and_mistyped_arguments() {
    let owner = parameter_fixture::owner();
    let bindings = bind_parameters(&owner, &owner.module().functions[0]).unwrap();
    for symbol in [
        0,
        crate::reference_effect_v1::kernel_scalar_symbol_v2(0).unwrap(),
        u32::MAX,
    ] {
        let expression = P::Symbol {
            symbol,
            scalar: S::Float { bits: 32 },
        };
        let error =
            typed_expression::reference_rhs(&expression, exact_float(), &bindings).unwrap_err();
        if symbol == u32::MAX {
            assert_eq!(error, Failure::UnsupportedLoad);
        } else {
            assert_eq!(error, Failure::UnboundScalarSymbol(symbol));
        }
    }
    let symbol = crate::reference_effect_v1::kernel_scalar_symbol_v2(1).unwrap();
    let expression = P::Symbol {
        symbol,
        scalar: S::Float { bits: 64 },
    };
    assert_eq!(
        typed_expression::reference_rhs(&expression, exact_float(), &bindings).unwrap_err(),
        Failure::UnboundScalarSymbol(symbol)
    );
    let load = P::Load(fe2o3_pliron::ProductionSemanticLoadV2 {
        block: 0,
        operation: 3,
        scalar: S::Float { bits: 32 },
        read_mode: fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedNonVolatile,
        allocation_origin: 1,
        view: fe2o3_pliron::ProductionRankedValueV1::Argument(0),
        indices: vec![fe2o3_pliron::ProductionRankedValueV1::Argument(1)].into_boxed_slice(),
    });
    assert_eq!(
        typed_expression::reference_rhs(&load, exact_float(), &bindings).unwrap_err(),
        Failure::UnsupportedLoad
    );
}

#[test]
fn independently_converted_cpu_bits_are_not_replaced_by_gpu_bits() {
    let cpu = P::Constant {
        scalar: S::Float { bits: 32 },
        bits: 0x8000_0000,
    };
    let gpu = P::Constant {
        scalar: S::Float { bits: 32 },
        bits: 0,
    };
    let cpu = typed_expression::reference_rhs(&cpu, exact_float(), &[]).unwrap();
    let gpu = typed_expression::reference_rhs(&gpu, exact_float(), &[]).unwrap();
    assert_ne!(cpu, gpu);
    assert!(matches!(
        cpu,
        SemanticTypedExpressionV1::Constant {
            bits: 0x8000_0000,
            ..
        }
    ));
    for bits in [0x7fc0_1234, 0x7f80_0000] {
        let expression = P::Constant {
            scalar: S::Float { bits: 32 },
            bits,
        };
        assert!(
            matches!(typed_expression::reference_rhs(&expression, exact_float(), &[]).unwrap(), SemanticTypedExpressionV1::Constant { bits: actual, .. } if actual == bits)
        );
    }
}

#[test]
fn typed_conversion_preserves_integer_division_overflow_and_cast() {
    let signed = S::Integer {
        signed: true,
        bits: 32,
    };
    let division = P::Binary {
        operation: fe2o3_pliron::ProductionSemanticBinaryOpV2::Divide,
        scalar: signed,
        overflow: fe2o3_pliron::ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(P::Constant {
            scalar: signed,
            bits: 0xffff_fff9,
        }),
        rhs: Box::new(P::Constant {
            scalar: signed,
            bits: 3,
        }),
    };
    let converted = typed_expression::reference_rhs(
        &division,
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        &[],
    )
    .unwrap();
    assert!(matches!(
        converted,
        SemanticTypedExpressionV1::Binary {
            operation: dialect_kernel::SemanticTypedBinaryKindAttr::Divide,
            overflow: dialect_kernel::SemanticOverflowAttr::Wrapping,
            ..
        }
    ));
    let checked_add = P::Binary {
        operation: fe2o3_pliron::ProductionSemanticBinaryOpV2::Add,
        scalar: signed,
        overflow: fe2o3_pliron::ProductionOverflowContractV2::Checked,
        lhs: Box::new(P::Constant {
            scalar: signed,
            bits: 7,
        }),
        rhs: Box::new(P::Constant {
            scalar: signed,
            bits: 3,
        }),
    };
    assert!(matches!(
        typed_expression::reference_rhs(
            &checked_add,
            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
            &[],
        )
        .unwrap(),
        SemanticTypedExpressionV1::Binary {
            operation: dialect_kernel::SemanticTypedBinaryKindAttr::Add,
            overflow: dialect_kernel::SemanticOverflowAttr::Checked,
            ..
        }
    ));
    for (operation, overflow, lhs_bits, rhs_bits) in [
        (
            fe2o3_pliron::ProductionSemanticBinaryOpV2::Divide,
            fe2o3_pliron::ProductionOverflowContractV2::Checked,
            7,
            3,
        ),
        (
            fe2o3_pliron::ProductionSemanticBinaryOpV2::Divide,
            fe2o3_pliron::ProductionOverflowContractV2::Wrapping,
            7,
            0,
        ),
        (
            fe2o3_pliron::ProductionSemanticBinaryOpV2::Divide,
            fe2o3_pliron::ProductionOverflowContractV2::Wrapping,
            0x8000_0000,
            0xffff_ffff,
        ),
        (
            fe2o3_pliron::ProductionSemanticBinaryOpV2::Add,
            fe2o3_pliron::ProductionOverflowContractV2::Checked,
            0x7fff_ffff,
            1,
        ),
    ] {
        let invalid = P::Binary {
            operation,
            scalar: signed,
            overflow,
            lhs: Box::new(P::Constant {
                scalar: signed,
                bits: lhs_bits,
            }),
            rhs: Box::new(P::Constant {
                scalar: signed,
                bits: rhs_bits,
            }),
        };
        assert_eq!(
            typed_expression::reference_rhs(
                &invalid,
                ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                &[],
            ),
            Err(Failure::InvalidTypedExpression)
        );
    }
    let cast = P::Cast {
        kind: fe2o3_pliron::ProductionSemanticCastV2::IntegerToFloat,
        source: signed,
        target: S::Float { bits: 32 },
        operand: Box::new(division),
    };
    assert!(matches!(
        typed_expression::reference_rhs(&cast, exact_float(), &[]).unwrap(),
        SemanticTypedExpressionV1::Cast {
            kind: dialect_kernel::SemanticTypedCastKindAttr::IntegerToFloat,
            ..
        }
    ));
}

#[test]
fn unsupported_numerical_policies_and_unbounded_trees_reject() {
    let constant = P::Constant {
        scalar: S::Float { bits: 32 },
        bits: 0,
    };
    for policy in [
        ProductionNumericalContractV2::Relaxed,
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding: ProductionIeeeRoundingModeV2::TowardZero,
            exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
        },
        ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
            exceptional_values: ProductionIeeeExceptionalValuePolicyV2::CanonicalNan,
        },
    ] {
        assert_eq!(
            typed_expression::reference_rhs(&constant, policy, &[]).unwrap_err(),
            Failure::UnsupportedNumericalPolicy
        );
    }
    let mut expression = constant;
    for _ in 0..128 {
        expression = P::Unary {
            operation: fe2o3_pliron::ProductionSemanticUnaryOpV2::Negate,
            scalar: S::Float { bits: 32 },
            operand: Box::new(expression),
        };
    }
    assert_eq!(
        typed_expression::reference_rhs(&expression, exact_float(), &[]).unwrap_err(),
        Failure::ExpressionLimit
    );
}

#[test]
fn dynamic_output_and_partial_frame_obligations_are_not_waived() {
    assert_eq!(
        require_total_static_write(Some(&[DYNAMIC_EXTENT]), None),
        Err(Failure::UnsupportedDynamicExtent)
    );
    assert_eq!(
        require_total_static_write(Some(&[DYNAMIC_EXTENT]), Some(ValueId(7))),
        Err(Failure::UnsupportedDynamicExtent)
    );
    assert_eq!(
        require_total_static_write(Some(&[8]), Some(ValueId(7))),
        Err(Failure::UnsupportedPartialOrFrame)
    );
    assert_eq!(
        require_total_static_write(None, None),
        Err(Failure::OutputBijection)
    );
    // A local shape check is not construction of an admitted final view.
    require_total_static_write(Some(&[8]), None).unwrap();
}

#[test]
fn output_join_requires_exactly_one_match() {
    assert_eq!(unique_index([false, true, false]), Some(1));
    assert_eq!(unique_index([]), None);
    assert_eq!(unique_index([false, false]), None);
    assert_eq!(unique_index([true, false, true]), None);
}

#[test]
fn output_bijection_uses_exact_sites_not_effect_order() {
    let a = ProductionReferenceOutputSiteV2::new(2, 3, 4);
    let b = ProductionReferenceOutputSiteV2::new(5, 6, 7);
    assert_eq!(
        bind_output_sites(&[5, 2], &[a, b], &[b, a]).unwrap(),
        vec![(1, 0), (0, 1)]
    );
}

#[test]
fn output_bijection_rejects_missing_duplicate_and_unconsumed_records() {
    let a = ProductionReferenceOutputSiteV2::new(2, 3, 4);
    let b = ProductionReferenceOutputSiteV2::new(5, 6, 7);
    for (arguments, references, source) in [
        (vec![], vec![], vec![]),
        (vec![2], vec![a, b], vec![a, b]),
        (vec![2, 2], vec![a, b], vec![a, b]),
        (vec![2, 5], vec![a, a], vec![a, b]),
        (vec![2, 5], vec![a, b], vec![a, a]),
    ] {
        assert_eq!(
            bind_output_sites(&arguments, &references, &source),
            Err(Failure::OutputBijection)
        );
    }
}

#[test]
fn output_bijection_rejects_argument_block_and_statement_substitution() {
    let original = ProductionReferenceOutputSiteV2::new(2, 3, 4);
    for changed in [
        ProductionReferenceOutputSiteV2::new(9, 3, 4),
        ProductionReferenceOutputSiteV2::new(2, 9, 4),
        ProductionReferenceOutputSiteV2::new(2, 3, 9),
    ] {
        assert_eq!(
            bind_output_sites(&[2], &[original], &[changed]),
            Err(Failure::OutputBijection)
        );
    }
}
