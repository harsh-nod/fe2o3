use super::*;
include!("../functional_refinement_process_identity_v2_tests.rs");
use dialect_kernel::SemanticBinaryKindAttr;
use fe2o3_functional_proof::SafeReferenceKindV2;
use fe2o3_pliron::{
    ProductionNumericalContractV2, ProductionNumericalRefinementContractV2,
    ProductionOverflowContractV2, ProductionRankedBlockV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionSemanticBinaryOpV2, ProductionSemanticExpressionV2,
    ProductionSemanticScalarTypeV2,
};

fn output(
    exit_code: i32,
    stdout: &[u8],
    stderr: &[u8],
) -> FunctionalRefinementRuntimeProcessOutputV1 {
    FunctionalRefinementRuntimeProcessOutputV1 {
            policy: crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2::LegacySingleSolverV1,
            exit_code: Some(exit_code),
            signal: None,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
}

fn digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}

pub(super) fn subjects() -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(4),
    )
    .unwrap()
}

#[test]
fn retained_effect_signature_survives_move_and_independent_reimport() {
    let signing = SigningKey::from_bytes(&[0x63; 32]);
    let verifying_key = signing.verifying_key().to_bytes();
    let toolchain =
        VerusToolchainIdentityV2::new(digest(10), digest(11), digest(12), digest(13), digest(14))
            .unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects(), digest(15)).unwrap();
    let policy = FunctionalRefinementImportPolicyV2::new(
        verifying_key,
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    // A CPU consistency test key, not evidence of actual Verus execution.
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(16),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let import = |wire: &[u8]| {
        FunctionalRefinementReceiptImporterV2::new(policy.clone(), 1)
            .unwrap()
            .import(FunctionalRefinementImportExpectationV2::new(binding), wire)
    };
    let retained = RetainedImportedFunctionalRefinementReceiptV2 {
        proof: import(&wire).unwrap(),
        verifying_key,
        wire,
    };
    let (proof, sidecar) = retained.into_parts();
    assert_eq!(*sidecar.wire(), wire);
    assert_eq!(*sidecar.verifying_key(), verifying_key);
    assert_eq!(
        proof.receipt_identity(),
        import(sidecar.wire()).unwrap().receipt_identity()
    );
    assert_eq!(
        sidecar,
        InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(wire, verifying_key)
    );
    let mut mutated = wire;
    let last = mutated.len() - 1;
    mutated[last] ^= 1;
    assert!(import(&mutated).is_err());
    let wrong_key = SigningKey::from_bytes(&[0x64; 32])
        .verifying_key()
        .to_bytes();
    let wrong_policy = FunctionalRefinementImportPolicyV2::new(
        wrong_key,
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    assert!(
        FunctionalRefinementReceiptImporterV2::new(wrong_policy, 1)
            .unwrap()
            .import(
                FunctionalRefinementImportExpectationV2::new(binding),
                sidecar.wire()
            )
            .is_err()
    );
}

pub(super) fn formula_kernel(expected_kind: SemanticBinaryKindAttr) -> ProductionRankedKernelV1 {
    let lhs = ProductionRankedValueIdV1::new(0);
    let rhs = ProductionRankedValueIdV1::new(1);
    let actual = ProductionRankedValueIdV1::new(2);
    let expected = ProductionRankedValueIdV1::new(3);
    let local = ProductionRankedValueV1::Local;
    ProductionRankedKernelV1::new(
        "typed_generator",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticSymbol {
                    result: lhs,
                    symbol: 0,
                },
                ProductionRankedOperationV1::SemanticSymbol {
                    result: rhs,
                    symbol: 1,
                },
                ProductionRankedOperationV1::SemanticBinary {
                    result: actual,
                    kind: SemanticBinaryKindAttr::Add,
                    lhs: local(lhs),
                    rhs: local(rhs),
                },
                ProductionRankedOperationV1::SemanticBinary {
                    result: expected,
                    kind: expected_kind,
                    lhs: local(rhs),
                    rhs: local(lhs),
                },
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                    actual: local(actual),
                    expected: local(expected),
                    subjects: subjects(),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

fn shared_formula_kernel(depth: usize) -> (ProductionRankedKernelV1, usize) {
    let local = ProductionRankedValueV1::Local;
    let mut operations = vec![ProductionRankedOperationV1::SemanticSymbol {
        result: ProductionRankedValueIdV1::new(0),
        symbol: 0,
    }];
    for identity in 1..=depth {
        let previous = ProductionRankedValueIdV1::new((identity - 1) as u32);
        operations.push(ProductionRankedOperationV1::SemanticBinary {
            result: ProductionRankedValueIdV1::new(identity as u32),
            kind: SemanticBinaryKindAttr::Add,
            lhs: local(previous),
            rhs: local(previous),
        });
    }
    let result = local(ProductionRankedValueIdV1::new(depth as u32));
    let request = operations.len();
    operations.push(
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual: result,
            expected: result,
            subjects: subjects(),
        },
    );
    (
        ProductionRankedKernelV1::new(
            "shared_formula",
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap(),
        request,
    )
}

fn typed_expression_kernel(
    expected_operation: ProductionSemanticBinaryOpV2,
) -> ProductionRankedKernelV1 {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let expression = |operation| ProductionSemanticExpressionV2::Binary {
        operation,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol: 7, scalar }),
        rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 9 }),
    };
    let actual = ProductionRankedValueIdV1::new(0);
    let expected = ProductionRankedValueIdV1::new(1);
    let local = ProductionRankedValueV1::Local;
    ProductionRankedKernelV1::new(
        "typed_expression_generator",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticExpression {
                    result: actual,
                    expression: expression(ProductionSemanticBinaryOpV2::Add),
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: expected,
                    expression: expression(expected_operation),
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                    actual: local(actual),
                    expected: local(expected),
                    subjects: subjects(),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

pub(super) fn wrapping_bitvector_kernel(expected_bits: u64) -> ProductionRankedKernelV1 {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 8,
    };
    let constant = |bits| ProductionSemanticExpressionV2::Constant { scalar, bits };
    let actual = ProductionRankedValueIdV1::new(0);
    let expected = ProductionRankedValueIdV1::new(1);
    let local = ProductionRankedValueV1::Local;
    ProductionRankedKernelV1::new(
        "wrapping_bitvector_semantics",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticExpression {
                    result: actual,
                    expression: ProductionSemanticExpressionV2::Binary {
                        operation: ProductionSemanticBinaryOpV2::Add,
                        scalar,
                        overflow: ProductionOverflowContractV2::Wrapping,
                        lhs: Box::new(constant(255)),
                        rhs: Box::new(constant(1)),
                    },
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: expected,
                    expression: constant(expected_bits),
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                    actual: local(actual),
                    expected: local(expected),
                    subjects: subjects(),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

fn conditional_formula_source(
    coordinate_symbol: u32,
    predicate: u64,
    changed_value: bool,
) -> String {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let value = ProductionSemanticExpressionV2::Symbol { scalar, symbol: 2 };
    let operations = vec![
        ProductionRankedOperationV1::SemanticSymbol {
            result: ProductionRankedValueIdV1::new(0),
            symbol: coordinate_symbol,
        },
        ProductionRankedOperationV1::SemanticExpression {
            result: ProductionRankedValueIdV1::new(1),
            expression: ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: predicate,
            },
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        },
        ProductionRankedOperationV1::SemanticExpression {
            result: ProductionRankedValueIdV1::new(2),
            expression: value.clone(),
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        },
        ProductionRankedOperationV1::SemanticExpression {
            result: ProductionRankedValueIdV1::new(3),
            expression: if changed_value {
                ProductionSemanticExpressionV2::Constant { scalar, bits: 7 }
            } else {
                value
            },
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        },
    ];
    let kernel = ProductionRankedKernelV1::new(
        "conditional_formula",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
    let pairs = [
        (local(0), local(0)),
        (local(1), local(1)),
        (local(1), local(1)),
        (local(2), local(3)),
    ];
    let program = SemanticFormulaProgramV2::build(&kernel, &pairs).unwrap();
    let lemma = program
        .render_conditional_lemma(&pairs, "conditional_fixture")
        .unwrap();
    format!("use vstd::prelude::*;\nverus! {{\n{BITVECTOR_SEMANTICS_V2}\n{lemma}\n}}\n")
}

#[test]
fn conditional_formula_exposes_actual_semantics_in_its_postcondition() {
    let source = conditional_formula_source(0, 1, false);
    let ensures = source
        .split("        ensures\n")
        .nth(1)
        .unwrap()
        .split("\n    {\n")
        .next()
        .unwrap();
    assert!(ensures.contains("v0 == s0"));
    assert!(ensures.contains("v1 == 1"));
    assert!(ensures.contains("v2 == v3"));
    assert!(ensures.contains("let v2: int = fe2o3_bv_norm_v2(s2, 32);"));
    assert!(source.contains("requires n <= g,"));
    for forbidden in ["assume(", "external_body", "uninterp", "fn main"] {
        assert!(!source.contains(forbidden));
    }
    // Equal but wrong coordinates/predicates survive generation and must be
    // rejected by the theorem, not accepted through equality alone.
    assert!(conditional_formula_source(1, 0, false).contains("let v0: int = s1;"));
}

#[test]
fn conditional_formula_rejects_unsupported_value_types_and_pair_rosters() {
    let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
    let pairs = [(local(0), local(1)); 4];
    for kernel in [wrapping_bitvector_kernel(0)] {
        let program = SemanticFormulaProgramV2::build(&kernel, &pairs).unwrap();
        assert!(
            program
                .render_conditional_lemma(&pairs, "unsupported")
                .is_err()
        );
        assert!(
            program
                .render_conditional_lemma(&pairs[..3], "missing_coordinate")
                .is_err()
        );
    }
}

#[test]
#[ignore = "requires the root-owned pinned production functional-refinement runtime"]
fn protected_runtime_conditional_coverage_proves_and_rejects_semantic_mutants() {
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        std::env::var_os("FE2O3_FUNCTIONAL_REFINEMENT_TEST_RUNTIME_ROOT")
            .expect("set the protected runtime root"),
    )
    .unwrap();
    let positive = conditional_formula_source(0, 1, false);
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects(), digest(57)).unwrap();
    let (receipt, _) = execute_and_import_generated_conditional_composition_locally_v1(
        &runtime,
        CanonicalGeneratedVerusProofInputV3::new(positive.clone().into_bytes()).unwrap(),
        binding,
        60,
    )
    .unwrap();
    assert!(receipt.proof().signature_and_policy_verified());
    assert_eq!(
        receipt.proof().boundary(),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePlironConditionalCoverage
    );
    for (name, source) in [
        (
            "underlaunch",
            positive.replace("requires n <= g,", "requires true,"),
        ),
        (
            "reversed launch condition",
            positive.replace("requires n <= g,", "requires g <= n,"),
        ),
        (
            "off-by-one guard",
            positive.replacen(
                "s0 < g as int && s0 < n as int",
                "s0 < g as int && s0 <= n as int",
                1,
            ),
        ),
        (
            "equal wrong coordinates",
            conditional_formula_source(1, 1, false),
        ),
        (
            "equal false predicates",
            conditional_formula_source(0, 0, false),
        ),
        ("changed value", conditional_formula_source(0, 1, true)),
    ] {
        let error = execute_and_import_generated_conditional_composition_locally_v1(
            &runtime,
            CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).unwrap(),
            binding,
            60,
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult,
            "{name}: {error}"
        );
        assert!(
            error.to_string().contains("postcondition not satisfied")
                || error.to_string().contains("assertion failed"),
            "{name}: {error}"
        );
    }
}

#[test]
fn only_exact_nonzero_verified_success_is_proved() {
    validate_proved_output(&output(
        0,
        b"verification results:: 12 verified, 0 errors\n",
        b"",
    ))
    .unwrap();
    for hostile in [
        output(1, b"verification results:: 12 verified, 0 errors\n", b""),
        output(0, b"verification results:: 0 verified, 0 errors\n", b""),
        output(0, b"verification results:: 12 verified, 1 errors\n", b""),
        output(0, b"proved\n", b""),
        output(
            0,
            b"verification results:: 12 verified, 0 errors\n",
            b"warning",
        ),
    ] {
        assert_eq!(
            validate_proved_output(&hostile).unwrap_err().kind(),
            FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
        );
    }
}

#[test]
fn unexpected_proof_diagnostics_are_bounded_and_escape_control_characters() {
    let mut stderr = b"error:\n\x1b[31m".to_vec();
    stderr.resize(4096, b'x');
    let error = validate_proved_output(&output(1, b"", &stderr)).unwrap_err();
    let message = error.to_string();
    assert_eq!(
        error.kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
    );
    assert!(message.contains("exit=Some(1), signal=None"));
    assert!(message.contains("error:\\n\\u{1b}[31m"));
    assert!(message.ends_with(" (truncated)"));
    assert!(!message.contains(['\n', '\u{1b}']));
    assert!(message.len() < 2400);
}

#[test]
fn conditional_formula_exports_equalities_without_assuming_them() {
    let kernel = formula_kernel(SemanticBinaryKindAttr::Add);
    let local = |index| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(index));
    let pairs = [(local(2), local(3))];
    let program = SemanticFormulaProgramV2::build(&kernel, &pairs).unwrap();
    let conditional = program.render_lemma(&pairs, "conditional", true).unwrap();
    assert!(conditional.contains("ensures {"));
    assert!(conditional.contains("true && v2 == v3"));
    assert!(!conditional.contains("requires"));
    assert_eq!(conditional.matches("let v2: int = v0 + v1;").count(), 2);
    assert!(conditional.contains("assert(v2 == v3);"));
    let ordinary = program.render_lemma(&pairs, "ordinary", false).unwrap();
    assert!(!ordinary.contains("ensures"));
    assert_eq!(ordinary.matches("let v2: int = v0 + v1;").count(), 1);
}

#[test]
fn typed_generator_derives_source_and_mutation_from_ranked_formula_dag() {
    let positive = formula_kernel(SemanticBinaryKindAttr::Add);
    let (positive_binding, positive_source) =
        generate_ranked_functional_refinement_proof_v2(&positive, 0, 4, subjects()).unwrap();
    let source = std::str::from_utf8(positive_source.source()).unwrap();
    assert!(source.contains("let v2: int = v0 + v1;"));
    assert!(source.contains("let v3: int = v1 + v0;"));
    assert!(source.contains("assert(v2 == v3);"));
    assert_ne!(
        positive_binding.normalized_obligation_effect_ir_hash(),
        digest(20)
    );

    let mutated = formula_kernel(SemanticBinaryKindAttr::Multiply);
    let (mutated_binding, mutated_source) =
        generate_ranked_functional_refinement_proof_v2(&mutated, 0, 4, subjects()).unwrap();
    assert!(
        std::str::from_utf8(mutated_source.source())
            .unwrap()
            .contains("let v3: int = v1 * v0;")
    );
    assert_ne!(positive_source.source(), mutated_source.source());
    assert_ne!(
        positive_binding.normalized_obligation_effect_ir_hash(),
        mutated_binding.normalized_obligation_effect_ir_hash(),
    );
}

#[test]
fn typed_expression_generator_traverses_transcripts_and_binds_mutations() {
    let positive = typed_expression_kernel(ProductionSemanticBinaryOpV2::Add);
    let summary = fe2o3_pliron::typed_semantic_obligation_summary_v2(&positive).unwrap();
    assert!(summary.is_non_vacuous());
    assert_eq!(summary.expression_roots, 2);
    assert_eq!(summary.checked_operations, 0);
    assert_eq!(summary.statically_discharged_domain_roots, 2);
    assert_eq!(summary.exact_bitvector_operator_congruence_roots, 2);
    assert!(!summary.grants_target_ieee_value_authority());
    let (positive_binding, positive_source) =
        generate_ranked_functional_refinement_proof_v2(&positive, 0, 2, subjects()).unwrap();
    let source = std::str::from_utf8(positive_source.source()).unwrap();
    assert!(source.contains("open spec fn fe2o3_bv_norm_v2"));
    assert!(source.contains(IEEE_CONGRUENCE_PARAMETER_V2));
    assert!(!source.contains("uninterp"));
    assert!(!source.contains("fe2o3_semantic_op_v2"));
    assert!(source.contains("s7: int"));
    assert!(source.contains("fe2o3_bv_norm_v2"));
    assert!(source.contains("assert(v0 == v1);"));

    let mutated = typed_expression_kernel(ProductionSemanticBinaryOpV2::Subtract);
    let (mutated_binding, mutated_source) =
        generate_ranked_functional_refinement_proof_v2(&mutated, 0, 2, subjects()).unwrap();
    assert_ne!(positive_source.source(), mutated_source.source());
    assert_ne!(
        positive_binding.normalized_obligation_effect_ir_hash(),
        mutated_binding.normalized_obligation_effect_ir_hash(),
    );
}

#[test]
fn numerical_generator_rejects_the_unsound_exact_fallback() {
    let float = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    let boolean = ProductionSemanticScalarTypeV2::Bool;
    let local = ProductionRankedValueV1::Local;
    let ids = std::array::from_fn::<_, 4, _>(|index| ProductionRankedValueIdV1::new(index as u32));
    let build = |absolute: f64| {
        let contract = ProductionNumericalRefinementContractV2::new(
            7,
            local(ids[0]),
            local(ids[1]),
            local(ids[2]),
            local(ids[3]),
            absolute.to_bits(),
            0.01_f64.to_bits(),
        )
        .unwrap();
        ProductionRankedKernelV1::new(
            "numerical_exact_fallback",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ids[0],
                        expression: ProductionSemanticExpressionV2::Symbol {
                            symbol: 9,
                            scalar: float,
                        },
                        numerical_contract: ProductionNumericalContractV2::exact_for(float),
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ids[1],
                        expression: ProductionSemanticExpressionV2::Symbol {
                            symbol: 9,
                            scalar: float,
                        },
                        numerical_contract: ProductionNumericalContractV2::exact_for(float),
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ids[2],
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: boolean,
                            bits: 1,
                        },
                        numerical_contract: ProductionNumericalContractV2::exact_for(boolean),
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ids[3],
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: boolean,
                            bits: 1,
                        },
                        numerical_contract: ProductionNumericalContractV2::exact_for(boolean),
                    },
                    ProductionRankedOperationV1::RequestNumericalRefinement {
                        contract,
                        subjects: subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    };

    for absolute in [0.001, 0.002] {
        let error =
            generate_ranked_functional_refinement_proof_v2(&build(absolute), 0, 4, subjects())
                .unwrap_err();
        assert_eq!(
            error.kind(),
            FunctionalRefinementVerusExecutionErrorKindV2::ClaimSpecificNumericalProofRequired
        );
        assert!(error.to_string().contains("claim-specific receipt"));
    }
}

#[test]
fn bitvector_generator_interprets_wrapping_arithmetic_instead_of_tagging_it() {
    let positive = wrapping_bitvector_kernel(0);
    let (positive_binding, positive_source) =
        generate_ranked_functional_refinement_proof_v2(&positive, 0, 2, subjects()).unwrap();
    let source = std::str::from_utf8(positive_source.source()).unwrap();
    assert!(source.contains("fe2o3_bv_norm_v2"));
    assert!(
        source
            .contains("fe2o3_bv_norm_v2((fe2o3_bv_norm_v2(255, 8)) + (fe2o3_bv_norm_v2(1, 8)), 8)")
    );
    assert!(!source.contains("fe2o3_semantic_op_v2"));

    let hostile = wrapping_bitvector_kernel(1);
    let (hostile_binding, hostile_source) =
        generate_ranked_functional_refinement_proof_v2(&hostile, 0, 2, subjects()).unwrap();
    assert_ne!(positive_source.source(), hostile_source.source());
    assert_ne!(
        positive_binding.normalized_obligation_effect_ir_hash(),
        hostile_binding.normalized_obligation_effect_ir_hash(),
    );
}

#[test]
fn bitvector_renderer_covers_the_closed_integer_and_boolean_operator_set() {
    use fe2o3_pliron::{
        ProductionSemanticCastV2, ProductionSemanticComparisonV2, ProductionSemanticUnaryOpV2,
    };

    let u8_scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 8,
    };
    let i8_scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: true,
        bits: 8,
    };
    let constant = |scalar, bits| ProductionSemanticExpressionV2::Constant { scalar, bits };
    let binary = |operation, scalar, lhs, rhs| {
        let rhs_scalar = if matches!(
            operation,
            ProductionSemanticBinaryOpV2::ShiftLeft | ProductionSemanticBinaryOpV2::ShiftRight
        ) {
            u8_scalar
        } else {
            scalar
        };
        ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(constant(scalar, lhs)),
            rhs: Box::new(constant(rhs_scalar, rhs)),
        }
    };
    for (operation, scalar, lhs, rhs, marker) in [
        (ProductionSemanticBinaryOpV2::Add, u8_scalar, 7, 3, " + "),
        (
            ProductionSemanticBinaryOpV2::Subtract,
            u8_scalar,
            7,
            3,
            " - ",
        ),
        (
            ProductionSemanticBinaryOpV2::Multiply,
            u8_scalar,
            7,
            3,
            " * ",
        ),
        (
            ProductionSemanticBinaryOpV2::Divide,
            i8_scalar,
            249,
            3,
            "fe2o3_signed_div_v2",
        ),
        (
            ProductionSemanticBinaryOpV2::Remainder,
            i8_scalar,
            249,
            3,
            "fe2o3_signed_rem_v2",
        ),
        (
            ProductionSemanticBinaryOpV2::BitXor,
            u8_scalar,
            0xaa,
            0x0f,
            "fe2o3_bitwise_v2(0",
        ),
        (
            ProductionSemanticBinaryOpV2::BitAnd,
            u8_scalar,
            0xaa,
            0x0f,
            "fe2o3_bitwise_v2(1",
        ),
        (
            ProductionSemanticBinaryOpV2::BitOr,
            u8_scalar,
            0xaa,
            0x0f,
            "fe2o3_bitwise_v2(2",
        ),
        (
            ProductionSemanticBinaryOpV2::ShiftLeft,
            u8_scalar,
            3,
            2,
            "fe2o3_shift_left_v2",
        ),
        (
            ProductionSemanticBinaryOpV2::ShiftRight,
            i8_scalar,
            248,
            2,
            "fe2o3_shift_right_v2",
        ),
    ] {
        let expression = binary(operation, scalar, lhs, rhs);
        expression.validate().unwrap();
        expression.validate_static_domains().unwrap();
        let rendered = render_bitvector_expression_v2(&expression).unwrap();
        assert!(rendered.contains(marker), "{operation:?}: {rendered}");
        assert!(!rendered.contains("ieee_operator_congruence"));
    }

    let signed = constant(i8_scalar, 255);
    let unary = ProductionSemanticExpressionV2::Unary {
        operation: ProductionSemanticUnaryOpV2::Negate,
        scalar: i8_scalar,
        operand: Box::new(signed.clone()),
    };
    assert!(
        render_bitvector_expression_v2(&unary)
            .unwrap()
            .contains("fe2o3_bv_norm_v2(-")
    );
    let comparison = ProductionSemanticExpressionV2::Compare {
        operation: ProductionSemanticComparisonV2::LessThan,
        operand_scalar: i8_scalar,
        lhs: Box::new(signed.clone()),
        rhs: Box::new(constant(i8_scalar, 1)),
    };
    assert!(
        render_bitvector_expression_v2(&comparison)
            .unwrap()
            .contains("fe2o3_bv_signed_v2")
    );
    let cast = ProductionSemanticExpressionV2::Cast {
        kind: ProductionSemanticCastV2::Integer,
        source: i8_scalar,
        target: ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 32,
        },
        operand: Box::new(signed),
    };
    assert!(
        render_bitvector_expression_v2(&cast)
            .unwrap()
            .contains("fe2o3_bv_signed_v2")
    );
}

#[test]
fn dynamic_checked_overflow_fails_closed_at_ranked_admission() {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let expression = ProductionSemanticExpressionV2::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Checked,
        lhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol: 1, scalar }),
        rhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol: 2, scalar }),
    };
    let error = ProductionRankedKernelV1::new(
        "dynamic_checked_domain",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(0),
                    expression: expression.clone(),
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(1),
                    expression,
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                    actual: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                    expected: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
                    subjects: subjects(),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap_err();
    assert_eq!(
        error,
        fe2o3_pliron::ProductionRankedKernelErrorV1::InvalidSemanticExpression(
            fe2o3_pliron::ProductionSemanticExpressionErrorV2::IncompleteDomain,
        ),
    );
}

#[test]
fn shared_formula_dag_renders_once_per_node() {
    let (kernel, request) = shared_formula_kernel(128);
    let (_, source) =
        generate_ranked_functional_refinement_proof_v2(&kernel, 0, request, subjects()).unwrap();
    let source = std::str::from_utf8(source.source()).unwrap();
    assert_eq!(source.matches("        let v").count(), 129);
    assert!(source.len() < 16 * 1024);
}

#[test]
fn overdeep_formula_dag_fails_before_source_construction() {
    let (kernel, request) = shared_formula_kernel(MAX_FUNCTIONAL_REFINEMENT_FORMULA_DEPTH_V2);
    let error = generate_ranked_functional_refinement_proof_v2(&kernel, 0, request, subjects())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe
    );
    assert!(error.to_string().contains("depth bound"));
}

#[test]
fn oversized_semantic_inventory_fails_before_source_construction() {
    let local = ProductionRankedValueV1::Local;
    let mut operations = (0..=MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2)
        .map(|identity| ProductionRankedOperationV1::SemanticConstant {
            result: ProductionRankedValueIdV1::new(identity as u32),
            value: identity as u64,
        })
        .collect::<Vec<_>>();
    let request = operations.len();
    operations.push(
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual: local(ProductionRankedValueIdV1::new(0)),
            expected: local(ProductionRankedValueIdV1::new(0)),
            subjects: subjects(),
        },
    );
    let kernel = ProductionRankedKernelV1::new(
        "oversized_semantic_inventory",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let error = generate_ranked_functional_refinement_proof_v2(&kernel, 0, request, subjects())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe
    );
    assert!(error.to_string().contains("node"));
}
