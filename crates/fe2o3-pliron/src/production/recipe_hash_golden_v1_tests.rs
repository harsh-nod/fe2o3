//! Inert hashing fixtures, not admission or proof fixtures.
use super::*;
use crate::production::*;
use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
use fe2o3_proof_contracts::DigestV1;

type E = ProductionSemanticExpressionV2;
type V = ProductionRankedValueV1;
type O = ProductionRankedOperationV1;
type T = ProductionRankedTerminatorV1;
type N = ProductionNumericalContractV2;
const U32: ProductionSemanticScalarTypeV2 = ProductionSemanticScalarTypeV2::Integer {
    signed: false,
    bits: 32,
};
include!("recipe_hash_golden_v1_data.rs");

fn local(n: u32) -> V {
    V::Local(ProductionRankedValueIdV1::new(n))
}

fn subjects() -> FunctionalRefinementSubjectsV2 {
    let d = |v| DigestV1::from_untrusted_bytes([v; 32]);
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        d(1),
        DigestV1::ZERO,
        d(2),
        d(3),
        d(4),
    )
    .unwrap()
}

fn expression(origin: u64) -> E {
    let load = E::Load(ProductionSemanticLoadV2 {
        block: 2,
        operation: 7,
        scalar: U32,
        allocation_origin: origin,
        view: local(9),
        indices: vec![
            V::Argument(1),
            V::BlockArgument {
                block: 2,
                argument: 0,
            },
        ]
        .into(),
    });
    E::Select {
        scalar: U32,
        condition: Box::new(E::Unary {
            operation: ProductionSemanticUnaryOpV2::Not,
            scalar: ProductionSemanticScalarTypeV2::Bool,
            operand: Box::new(E::Compare {
                operation: ProductionSemanticComparisonV2::LessThan,
                operand_scalar: U32,
                lhs: Box::new(E::Symbol {
                    symbol: 3,
                    scalar: U32,
                }),
                rhs: Box::new(E::Constant {
                    scalar: U32,
                    bits: 64,
                }),
            }),
        }),
        when_true: Box::new(E::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar: U32,
            overflow: ProductionOverflowContractV2::Checked,
            lhs: Box::new(load),
            rhs: Box::new(E::Constant {
                scalar: U32,
                bits: 7,
            }),
        }),
        when_false: Box::new(E::Cast {
            kind: ProductionSemanticCastV2::Integer,
            source: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 8,
            },
            target: U32,
            operand: Box::new(E::Constant {
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 8,
                },
                bits: 3,
            }),
        }),
    }
}

fn effect(rank: usize, reverse: bool) -> ProductionEffectRefinementContractV2 {
    let mut indices = vec![
        V::Argument(1),
        V::BlockArgument {
            block: 2,
            argument: 0,
        },
    ];
    indices.truncate(rank);
    let mut gpu = vec![local(10), local(11)];
    let mut reference = vec![local(12), local(13)];
    gpu.truncate(rank);
    reference.truncate(rank);
    if reverse {
        gpu.reverse();
        reference.reverse();
    }
    ProductionEffectRefinementContractV2::new(
        0x0102_0304_0506_0708,
        ProductionGpuWriteSiteV2::new(2, 7),
        ProductionReferenceOutputSiteV2::new(3, 5, 11),
        local(9),
        indices,
        gpu,
        reference,
        local(14),
        local(15),
        local(16),
        local(17),
        local(18),
        local(19),
    )
    .unwrap()
}

fn numerical(swapped: bool) -> ProductionNumericalRefinementContractV2 {
    let (a, r) = if swapped {
        (0.01_f64, 0.001_f64)
    } else {
        (0.001_f64, 0.01_f64)
    };
    ProductionNumericalRefinementContractV2::new(
        41,
        local(0),
        local(1),
        local(2),
        local(3),
        a.to_bits(),
        r.to_bits(),
    )
    .unwrap()
}

fn check(name: &str, bytes: [u8; 32]) {
    let expected = GOLDEN
        .iter()
        .find(|(label, _)| *label == name)
        .expect("frozen fixture");
    assert_eq!(bytes, expected.1, "legacy transcript changed: {name}");
}

fn resources() -> crate::production_analysis::ProductionAnalysisResourceContractV1 {
    crate::production_analysis::ProductionAnalysisResourceContractV1::new(
        crate::production_analysis::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
}

fn check_operation(name: &str, operation: &O) {
    check(name, derive_exact_ranked_operation_identity_v1(operation));
    let actual = hash_work::metered(&mut resources(), |digest| {
        digest.update(EXACT_RANKED_OPERATION_IDENTITY_DOMAIN_V1)?;
        hash_ranked_operation(digest, operation)
    })
    .unwrap();
    check(name, actual);
}

fn check_graph(name: &str, kernel: &ProductionRankedKernelV1) {
    check(name, derive_exact_ranked_graph_identity_v1(kernel));
    check(
        name,
        derive_exact_ranked_graph_identity_with_resources_v1(kernel, &mut resources()).unwrap(),
    );
}

#[test]
fn exact_recipe_hash_golden_transcripts() {
    for (name, origin) in [("expression", 11), ("expression_origin", 12)] {
        let expression = expression(origin);
        check(&format!("{name}_bare"), expression.canonical_sha256());
        check(
            &format!("{name}_transcript"),
            expression.canonical_transcript_sha256(N::ExactBitVectorOperatorCongruence),
        );
        check(
            &format!("{name}_materialized"),
            expression.materialized_pliron_transcript_sha256(N::ExactBitVectorOperatorCongruence),
        );
        check_operation(
            &format!("{name}_operation"),
            &O::SemanticExpression {
                result: ProductionRankedValueIdV1::new(27),
                expression,
                numerical_contract: N::ExactBitVectorOperatorCongruence,
            },
        );
    }
    let float = E::Constant {
        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        bits: 0x3f80_0000,
    };
    for (name, policy) in [
        (
            "nearest",
            N::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
            },
        ),
        (
            "zero",
            N::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::TowardZero,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
            },
        ),
        (
            "nan",
            N::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::CanonicalNan,
            },
        ),
        (
            "bounded",
            N::ErrorBounded {
                absolute_error_f64_bits: 0.001_f64.to_bits(),
                relative_error_f64_bits: 0.01_f64.to_bits(),
            },
        ),
        ("relaxed", N::Relaxed),
    ] {
        check(name, float.canonical_transcript_sha256(policy));
    }
    for (name, rank, reverse) in [
        ("effect", 2, false),
        ("effect_rank_one", 1, false),
        ("effect_reversed", 2, true),
    ] {
        let contract = effect(rank, reverse);
        check(name, *contract.request_shape_hash().as_bytes());
        check_operation(
            &format!("{name}_operation"),
            &O::RequestEffectRefinement {
                contract,
                subjects: subjects(),
            },
        );
    }
    for (name, swapped) in [("numerical", false), ("numerical_swapped", true)] {
        let contract = numerical(swapped);
        check(name, *contract.request_shape_hash().as_bytes());
        check_operation(
            &format!("{name}_operation"),
            &O::RequestNumericalRefinement {
                contract,
                subjects: subjects(),
            },
        );
    }
    for (name, terminator) in [
        (
            "split",
            T::AnalysisSplitArgs {
                control_dependencies: vec![V::Argument(2)],
                first_arguments: vec![V::Argument(0)],
                second_arguments: vec![V::Argument(1)],
                first_block: 1,
                second_block: 2,
            },
        ),
        (
            "split_empty",
            T::AnalysisSplitArgs {
                control_dependencies: vec![V::Argument(2)],
                first_arguments: vec![],
                second_arguments: vec![V::Argument(0), V::Argument(1)],
                first_block: 1,
                second_block: 2,
            },
        ),
        (
            "add_at_zero",
            T::BranchArgsAddAt {
                arguments: vec![V::Argument(0), V::Argument(1)],
                add_argument: 0,
                step: V::Argument(2),
                target: 3,
            },
        ),
        (
            "add_at_one",
            T::BranchArgsAddAt {
                arguments: vec![V::Argument(0), V::Argument(1)],
                add_argument: 1,
                step: V::Argument(2),
                target: 3,
            },
        ),
        ("return", T::Return),
        ("trap", T::Trap),
    ] {
        check(
            name,
            derive_exact_ranked_terminator_identity_v1(&terminator),
        );
    }
    let kernel = ProductionRankedKernelV1::new(
        "hash_cfg",
        2,
        vec![
            ProductionRankedBlockV1::new(
                vec![O::IndexConstant {
                    result: ProductionRankedValueIdV1::new(0),
                    value: 17,
                }],
                T::BranchArgs {
                    arguments: vec![V::Argument(1)],
                    target: 1,
                },
            ),
            ProductionRankedBlockV1::with_index_arguments(1, vec![], T::Return),
        ],
    )
    .unwrap();
    check_graph("cfg_exact", &kernel);
    check(
        "cfg_functional",
        derive_functional_refinement_graph_identity_v2(&kernel),
    );

    let mut operations = (0..4)
        .map(|i| {
            let scalar = if i < 2 {
                ProductionSemanticScalarTypeV2::Float { bits: 32 }
            } else {
                ProductionSemanticScalarTypeV2::Bool
            };
            O::SemanticExpression {
                result: ProductionRankedValueIdV1::new(i),
                expression: if i < 2 {
                    E::Symbol { symbol: 1, scalar }
                } else {
                    E::Constant { scalar, bits: 1 }
                },
                numerical_contract: N::exact_for(scalar),
            }
        })
        .collect::<Vec<_>>();
    operations.push(O::RequestNumericalRefinement {
        contract: numerical(false),
        subjects: subjects(),
    });
    let kernel = ProductionRankedKernelV1::new(
        "hash_request",
        0,
        vec![ProductionRankedBlockV1::new(operations, T::Return)],
    )
    .unwrap();
    check_graph("request_exact", &kernel);
    check(
        "request_functional",
        derive_functional_refinement_graph_identity_v2(&kernel),
    );
}
