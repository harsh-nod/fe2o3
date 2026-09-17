//! Reviewed-host checks of compiler-generated ranked scalar/effect receipts.
//! These typed fixtures establish neither arbitrary Rust nor machine equivalence.

use super::tests::{subjects, wrapping_bitvector_kernel};
use super::*;
use dialect_kernel::{AccessKindAttr, OwnershipCoverageAttr, OwnershipPartitionAttr};
use fe2o3_pliron::{
    ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2, ProductionNumericalContractV2,
    ProductionOverflowContractV2, ProductionRankedBlockV1, ProductionRankedTerminatorV1,
    ProductionReferenceOutputSiteV2, ProductionSemanticBinaryOpV2, ProductionSemanticComparisonV2,
    ProductionSemanticExpressionV2 as Expression, ProductionSemanticScalarTypeV2 as Scalar,
    ProductionSemanticUnaryOpV2,
};

const PROTECTED_RUNTIME_ROOT: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
const COMPILER_TIMEOUT_SECONDS: u32 = 60;
const SCALAR_REQUEST: usize = 2;

fn protected_runtime() -> FunctionalRefinementVerusRuntimeLeaseV1 {
    FunctionalRefinementVerusRuntimeLeaseV1::open(PROTECTED_RUNTIME_ROOT)
        .expect("the installed root-owned pinned runtime is required; never skip this test")
}

fn nested_not_wrapping_kernel(expected_bits: u64) -> ProductionRankedKernelV1 {
    let kernel = wrapping_bitvector_kernel(expected_bits);
    let mut operations = kernel.blocks()[0].operations().to_vec();
    let ProductionRankedOperationV1::SemanticExpression { expression, .. } = &mut operations[0]
    else {
        unreachable!()
    };
    let scalar = expression.scalar();
    let Expression::Binary { rhs, .. } = expression else {
        unreachable!()
    };
    *rhs = Box::new(Expression::Constant { scalar, bits: 8 });
    for _ in 0..2 {
        *expression = Expression::Unary {
            operation: ProductionSemanticUnaryOpV2::Not,
            scalar,
            operand: Box::new(expression.clone()),
        };
    }
    ProductionRankedKernelV1::new(
        "nested_not_wrapping_generator",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

// This fixture models f32 operator congruence, not target IEEE arithmetic.
fn ieee_operator_kernel(
    expected_operation: ProductionSemanticBinaryOpV2,
) -> ProductionRankedKernelV1 {
    let scalar = Scalar::Float { bits: 32 };
    let mut operations = [
        (0, ProductionSemanticBinaryOpV2::Add),
        (1, expected_operation),
    ]
    .into_iter()
    .map(
        |(id, operation)| ProductionRankedOperationV1::SemanticExpression {
            result: ProductionRankedValueIdV1::new(id),
            expression: Expression::Binary {
                operation,
                scalar,
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(Expression::Symbol { symbol: 7, scalar }),
                rhs: Box::new(Expression::Symbol { symbol: 8, scalar }),
            },
            numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
        },
    )
    .collect::<Vec<_>>();
    operations.push(
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            expected: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
            subjects: subjects(),
        },
    );
    ProductionRankedKernelV1::new(
        "protected_ieee_operator_congruence",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

fn effect_kernel(reference_coordinate: u64) -> (ProductionRankedKernelV1, usize) {
    let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
    let typed = |id, expression| ProductionRankedOperationV1::SemanticExpression {
        result: ProductionRankedValueIdV1::new(id),
        expression,
        numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
    };
    let coordinate = |bits| Expression::Constant {
        scalar: Scalar::Integer {
            signed: false,
            bits: 64,
        },
        bits,
    };
    let mut operations = vec![ProductionRankedOperationV1::ExecutionLayout {
        grid_identity: 1,
        global_extents: [1, 1, 1],
        workgroup_extents: [1, 1, 1],
        subgroup_size: 1,
        full_physical_workgroups: true,
    }];
    operations.extend_from_slice(&wrapping_bitvector_kernel(0).blocks()[0].operations()[..2]);
    operations.extend([
        ProductionRankedOperationV1::View {
            result: ProductionRankedValueIdV1::new(2),
            element_width: 8,
            writable: true,
            shape: vec![1],
            dynamic_extents: vec![],
            allocation_origin: 1,
            noalias_class: 1,
        },
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(3),
            value: 0,
        },
        typed(4, coordinate(0)),
        typed(5, coordinate(reference_coordinate)),
        typed(
            6,
            Expression::Constant {
                scalar: Scalar::Bool,
                bits: 1,
            },
        ),
        typed(
            7,
            Expression::Unary {
                operation: ProductionSemanticUnaryOpV2::Not,
                scalar: Scalar::Bool,
                operand: Box::new(Expression::Constant {
                    scalar: Scalar::Bool,
                    bits: 0,
                }),
            },
        ),
        typed(
            8,
            Expression::Constant {
                scalar: Scalar::Bool,
                bits: 1,
            },
        ),
        typed(
            9,
            Expression::Compare {
                operation: ProductionSemanticComparisonV2::Equal,
                operand_scalar: Scalar::Integer {
                    signed: false,
                    bits: 64,
                },
                lhs: Box::new(coordinate(0)),
                rhs: Box::new(coordinate(0)),
            },
        ),
        ProductionRankedOperationV1::OwnershipContract {
            view: local(2),
            coverage: OwnershipCoverageAttr::TotalView,
            partition: OwnershipPartitionAttr::ExactSets,
        },
    ]);
    let write_site = ProductionGpuWriteSiteV2::new(0, operations.len() as u32);
    operations.push(ProductionRankedOperationV1::ValueAccess {
        kind: AccessKindAttr::Write,
        view: local(2),
        indices: vec![local(3)],
        value: local(0),
    });
    let request = operations.len();
    operations.push(ProductionRankedOperationV1::RequestEffectRefinement {
        contract: ProductionEffectRefinementContractV2::new(
            73,
            write_site,
            ProductionReferenceOutputSiteV2::new(0, 0, 0),
            local(2),
            vec![local(3)],
            vec![local(4)],
            vec![local(5)],
            local(6),
            local(7),
            local(8),
            local(9),
            local(0),
            local(1),
        )
        .unwrap(),
        subjects: subjects(),
    });
    (
        ProductionRankedKernelV1::new(
            "protected_generated_effect",
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

fn expected_binding(
    kernel: &ProductionRankedKernelV1,
    request: usize,
) -> FunctionalRefinementBindingV2 {
    let obligation = match &kernel.blocks()[0].operations()[request] {
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual,
            expected,
            subjects,
        } => normalized_functional_refinement_formula_hash_for_kernel_v2(
            kernel, 0, request, *actual, *expected, *subjects,
        ),
        ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects } => {
            normalized_effect_refinement_hash_for_kernel_v2(kernel, 0, request, contract, *subjects)
        }
        _ => panic!("expected an unproved ranked scalar/effect request"),
    }
    .unwrap();
    FunctionalRefinementBindingV2::from_subjects(subjects(), obligation).unwrap()
}

fn assert_imported_binding(
    proof: &ImportedFunctionalRefinementProofV2,
    binding: FunctionalRefinementBindingV2,
    toolchain: VerusToolchainIdentityV2,
) {
    // The strict importer only returns this owner after checking the Proved result.
    assert!(proof.signature_and_policy_verified());
    assert_eq!(proof.binding(), binding);
    assert_eq!(proof.binding().subjects(), subjects());
    assert_eq!(proof.toolchain(), toolchain);
    assert_eq!(
        proof.boundary(),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
    );
    assert!(!proof.execution_identity().is_zero());
    assert!(!proof.receipt_identity().digest().is_zero());
}

fn assert_generated_assertion_failure(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    kernel: &ProductionRankedKernelV1,
    request: usize,
) {
    let (binding, source) =
        generate_ranked_functional_refinement_proof_v2(kernel, 0, request, subjects()).unwrap();
    assert_eq!(binding, expected_binding(kernel, request));
    runtime.revalidate().unwrap();
    let output = runtime
        .execute_generated_rust_verify(
            &source,
            Instant::now() + Duration::from_secs(u64::from(COMPILER_TIMEOUT_SECONDS)),
            MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2,
        )
        .expect("execute the production-generated mismatch within the compiler deadline");
    runtime.revalidate().unwrap();
    assert_eq!((output.exit_code, output.signal), (Some(1), None));
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let verified = stdout
        .strip_prefix("verification results:: ")
        .and_then(|summary| summary.strip_suffix(" verified, 1 errors\n"))
        .expect("Verus must report one failed obligation, not a syntax or runtime error");
    verified.parse::<u32>().expect("canonical verified count");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("assertion failed"),
        "expected a semantic assertion failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        validate_proved_output(&output).unwrap_err().kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
    );
}

#[test]
fn generated_library_groups_nested_bitwise_not_operands() {
    let kernel = nested_not_wrapping_kernel(7);
    let (_, source) =
        generate_ranked_functional_refinement_proof_v2(&kernel, 0, SCALAR_REQUEST, subjects())
            .unwrap();
    let source = std::str::from_utf8(source.source()).unwrap();
    assert!(source.contains("(fe2o3_bv_modulus_v2(8) - 1) - ((fe2o3_bv_modulus_v2(8) - 1) - ("));
    assert!(!source.contains("fn main("));
    assert!(source.ends_with("}\n"));
}

#[test]
fn generated_effect_binds_coordinates_domains_preconditions_and_value() {
    let (kernel, request) = effect_kernel(0);
    let (binding, source) =
        generate_ranked_functional_refinement_proof_v2(&kernel, 0, request, subjects()).unwrap();
    assert_eq!(binding, expected_binding(&kernel, request));
    let source = std::str::from_utf8(source.source()).unwrap();
    assert_eq!(source.matches("assert(v4 == v5);").count(), 1);
    assert_eq!(source.matches("assert(v6 == v7);").count(), 1);
    assert_eq!(source.matches("assert(v8 == v9);").count(), 1);
    assert_eq!(source.matches("assert(v0 == v1);").count(), 1);
    let (mismatch, mismatch_request) = effect_kernel(1);
    assert_ne!(binding, expected_binding(&mismatch, mismatch_request));

    let mutated_hashes = [7, 9].map(|id| {
        let mut operations = kernel.blocks()[0].operations().to_vec();
        let expression = operations
            .iter_mut()
            .find_map(|operation| match operation {
                ProductionRankedOperationV1::SemanticExpression {
                    result, expression, ..
                } if *result == ProductionRankedValueIdV1::new(id) => Some(expression),
                _ => None,
            })
            .unwrap();
        *expression = Expression::Constant {
            scalar: Scalar::Bool,
            bits: 0,
        };
        let mutated = ProductionRankedKernelV1::new(
            "protected_generated_effect",
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let (mutated_binding, mutated_source) =
            generate_ranked_functional_refinement_proof_v2(&mutated, 0, request, subjects())
                .unwrap();
        assert_eq!(mutated_binding, expected_binding(&mutated, request));
        assert_ne!(mutated_source.source(), source.as_bytes());
        let hash = mutated_binding.normalized_obligation_effect_ir_hash();
        assert_ne!(
            hash,
            binding.normalized_obligation_effect_ir_hash(),
            "v{id}"
        );
        hash
    });
    assert_ne!(mutated_hashes[0], mutated_hashes[1]);
}

#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_ranked_scalar_prepares_bound_receipt() {
    let runtime = protected_runtime();
    let kernel = nested_not_wrapping_kernel(7); // !!(255_u8 + 8_u8) == 7_u8, wrapping.
    let binding = expected_binding(&kernel, SCALAR_REQUEST);
    let toolchain = functional_refinement_verus_toolchain_identity_v2(&runtime).unwrap();
    let signing = SigningKey::generate(&mut OsRng);
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    let prepared = prepare_ranked_functional_refinement_receipt_v2(
        &runtime,
        &kernel,
        0,
        SCALAR_REQUEST,
        subjects(),
        policy.signer_identity(),
        COMPILER_TIMEOUT_SECONDS,
    )
    .expect("prove the generated wrapping scalar formula through the protected runtime");
    assert_eq!(prepared.binding(), binding);
    let unsigned = prepared.into_unsigned();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let signer_identity = policy.signer_identity();
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
    let stale = expected_binding(&nested_not_wrapping_kernel(8), SCALAR_REQUEST);
    assert_eq!(
        importer
            .import(FunctionalRefinementImportExpectationV2::new(stale), &wire)
            .unwrap_err(),
        FunctionalRefinementImportErrorV2::StaleNormalizedObligationEffectIr
    );
    assert_eq!(importer.imported_count(), 0);
    let proof = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    assert_imported_binding(&proof, binding, toolchain);
    assert_eq!(proof.signer_identity(), signer_identity);
    assert_eq!(importer.imported_count(), 1);
    runtime.revalidate().unwrap();
}

#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_ranked_scalar_rejects_semantic_mismatch() {
    let runtime = protected_runtime();
    let kernel = nested_not_wrapping_kernel(8);
    assert_generated_assertion_failure(&runtime, &kernel, SCALAR_REQUEST);
    let error = prepare_ranked_functional_refinement_receipt_v2(
        &runtime,
        &kernel,
        0,
        SCALAR_REQUEST,
        subjects(),
        DigestV1::from_untrusted_bytes([71; 32]),
        COMPILER_TIMEOUT_SECONDS,
    )
    .err()
    .expect("a false generated scalar must not become a signable receipt");
    assert_eq!(
        error.kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
    );
    runtime.revalidate().unwrap();
}

#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_ranked_effect_executes_and_imports_bound_receipt() {
    let runtime = protected_runtime();
    let (kernel, request) = effect_kernel(0);
    let expected = expected_binding(&kernel, request);
    let toolchain = functional_refinement_verus_toolchain_identity_v2(&runtime).unwrap();
    let (binding, proof, policy) = execute_and_import_ranked_functional_refinement_locally_v2(
        &runtime,
        &kernel,
        0,
        request,
        subjects(),
        COMPILER_TIMEOUT_SECONDS,
    )
    .expect("prove and strictly import the generated effect formula");
    assert_eq!(binding, expected);
    assert_imported_binding(&proof, expected, toolchain);
    assert!(policy.accepts_signer(proof.signer_identity()));
    assert_eq!(policy.toolchain(), toolchain);
    runtime.revalidate().unwrap();
}

#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_ranked_effect_rejects_coordinate_mismatch() {
    let runtime = protected_runtime();
    let (kernel, request) = effect_kernel(1);
    assert_generated_assertion_failure(&runtime, &kernel, request);
    let error = execute_and_import_ranked_functional_refinement_locally_v2(
        &runtime,
        &kernel,
        0,
        request,
        subjects(),
        COMPILER_TIMEOUT_SECONDS,
    )
    .err()
    .expect("matching values cannot admit a mismatched effect coordinate");
    assert_eq!(
        error.kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
    );
    runtime.revalidate().unwrap();
}

#[test]
fn generated_ieee_operator_congruence_uses_quantified_function_parameter() {
    let positive = ieee_operator_kernel(ProductionSemanticBinaryOpV2::Add);
    let summary = fe2o3_pliron::typed_semantic_obligation_summary_v2(&positive).unwrap();
    assert!(summary.is_non_vacuous());
    assert_eq!(summary.exact_ieee_operator_congruence_roots, 2);
    assert!(!summary.grants_target_ieee_value_authority());
    let (binding, source) =
        generate_ranked_functional_refinement_proof_v2(&positive, 0, SCALAR_REQUEST, subjects())
            .unwrap();
    assert_eq!(binding, expected_binding(&positive, SCALAR_REQUEST));
    let mutated = ieee_operator_kernel(ProductionSemanticBinaryOpV2::Subtract);
    let (mutated_binding, mutated_source) =
        generate_ranked_functional_refinement_proof_v2(&mutated, 0, SCALAR_REQUEST, subjects())
            .unwrap();
    assert_eq!(mutated_binding, expected_binding(&mutated, SCALAR_REQUEST));
    assert_ne!(
        binding.normalized_obligation_effect_ir_hash(),
        mutated_binding.normalized_obligation_effect_ir_hash()
    );
    assert_ne!(source.source(), mutated_source.source());
    for source in [&source, &mutated_source] {
        let source = std::str::from_utf8(source.source()).unwrap();
        assert!(source.contains(
            "proof fn fe2o3_functional_refinement_v2(fe2o3_ieee_operator_congruence_v2: spec_fn(int, int, int, int) -> int, s7: int, s8: int)"
        ));
        assert!(source.contains("assert(v0 == v1);"));
        assert!(!source.contains("uninterp"));
        assert!(!source.contains("external"));
    }
}

#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_ieee_operator_congruence_imports_bound_receipt() {
    let runtime = protected_runtime();
    let kernel = ieee_operator_kernel(ProductionSemanticBinaryOpV2::Add);
    let expected = expected_binding(&kernel, SCALAR_REQUEST);
    let toolchain = functional_refinement_verus_toolchain_identity_v2(&runtime).unwrap();
    let (binding, proof, policy) = execute_and_import_ranked_functional_refinement_locally_v2(
        &runtime,
        &kernel,
        0,
        SCALAR_REQUEST,
        subjects(),
        COMPILER_TIMEOUT_SECONDS,
    )
    .expect("prove and strictly import identical f32 operator trees by congruence");
    assert_eq!(binding, expected);
    assert_imported_binding(&proof, expected, toolchain);
    assert!(policy.accepts_signer(proof.signer_identity()));
    assert_eq!(policy.toolchain(), toolchain);
    runtime.revalidate().unwrap();
}

#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_ieee_operator_mutation_rejects_bound_receipt() {
    let runtime = protected_runtime();
    let kernel = ieee_operator_kernel(ProductionSemanticBinaryOpV2::Subtract);
    assert_generated_assertion_failure(&runtime, &kernel, SCALAR_REQUEST);
    let error = execute_and_import_ranked_functional_refinement_locally_v2(
        &runtime,
        &kernel,
        0,
        SCALAR_REQUEST,
        subjects(),
        COMPILER_TIMEOUT_SECONDS,
    )
    .err()
    .expect("distinct f32 operator trees must not produce an imported congruence receipt");
    assert_eq!(
        error.kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
    );
    runtime.revalidate().unwrap();
}
