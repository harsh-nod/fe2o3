//! Exact source-memory contracts for lowerer component tests, not production authority.

use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2,
    FunctionalRefinementSubjectsV2, SafeReferenceKindV2, UnsignedFunctionalRefinementReceiptV2,
    VerusToolchainIdentityV2,
};
use fe2o3_pliron::*;
use fe2o3_proof_contracts::DigestV1;

fn local(index: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(index))
}

fn digest(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

pub(super) fn compile_component_input(
    source: ProductionRankedKernelV1,
) -> ProductionRankedKernelLoweringInputV1 {
    assert_eq!(source.blocks().len(), 1);
    assert_eq!(source.blocks()[0].operations().len(), 6);
    assert_eq!(
        source.blocks()[0].terminator(),
        &ProductionRankedTerminatorV1::Return
    );
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    // This specification is constructed independently of the source GPU DAG.
    // Its initial read refers to the actual retained source access (0, 3).
    let reference = ProductionSemanticExpressionV2::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(ProductionSemanticExpressionV2::Load(
            ProductionSemanticLoadV2 {
                block: 0,
                operation: 3,
                scalar,
                read_mode: ProductionSemanticReadModeV2::UnorderedNonVolatile,
                allocation_origin: 1,
                view: local(0),
                indices: vec![local(1)].into_boxed_slice(),
            },
        )),
        rhs: Box::new(ProductionSemanticExpressionV2::Constant {
            scalar,
            bits: u64::from(1.0_f32.to_bits()),
        }),
    };
    let numerical_contract = ProductionNumericalContractV2::exact_for_expression(&reference);
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(4),
    )
    .unwrap();
    let contract = ProductionEffectRefinementContractV2::new(
        101,
        ProductionGpuWriteSiteV2::new(0, 5),
        ProductionReferenceOutputSiteV2::new(0, 0, 0),
        local(0),
        vec![local(1)],
        vec![local(4)],
        vec![local(4)],
        local(5),
        local(5),
        local(5),
        local(5),
        local(2),
        local(3),
    )
    .unwrap();
    let mut operations = source.blocks()[0].operations().to_vec();
    operations.extend([
        ProductionRankedOperationV1::SemanticExpression {
            result: ProductionRankedValueIdV1::new(3),
            expression: reference,
            numerical_contract,
        },
        ProductionRankedOperationV1::SemanticSymbol {
            result: ProductionRankedValueIdV1::new(4),
            symbol: 0,
        },
        ProductionRankedOperationV1::SemanticConstant {
            result: ProductionRankedValueIdV1::new(5),
            value: 1,
        },
        ProductionRankedOperationV1::OwnershipContract {
            view: local(0),
            coverage: dialect_kernel::OwnershipCoverageAttr::TotalView,
            partition: dialect_kernel::OwnershipPartitionAttr::ExactSets,
        },
        ProductionRankedOperationV1::RequestEffectRefinement {
            contract: contract.clone(),
            subjects,
        },
    ]);
    let kernel = ProductionRankedKernelV1::new(
        source.function_name(),
        source.argument_count(),
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let hash = normalized_effect_refinement_hash_for_kernel_v2(&kernel, 0, 10, &contract, subjects)
        .unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, hash).unwrap();

    // Same inert receipt-staging pattern as PLIRON component tests. This key
    // and these synthetic subjects do NOT authenticate production execution,
    // a Rust input/output ABI, a final KIR graph, an artifact, or a launch.
    let signing = SigningKey::from_bytes(&[91; 32]);
    let toolchain =
        VerusToolchainIdentityV2::new(digest(10), digest(11), digest(12), digest(13), digest(14))
            .unwrap();
    let boundary = FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir;
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
    let staging =
        ProductionRefinementStagingPolicyV2::new([policy.signer_identity()], toolchain).unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(20),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let wire = unsigned
        .clone()
        .attach_signature(signing.sign(unsigned.signing_bytes()).to_bytes());
    let receipt = FunctionalRefinementReceiptImporterV2::new(policy, 1)
        .unwrap()
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let request = ProductionReferenceProofV2::request_exact(receipt.receipt_identity(), binding);
    let kernel = kernel
        .bind_functional_refinement_request_v2(0, 10, request)
        .unwrap();
    compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
        ProductionConstructionV1::ranked_kernel("value_translation_module", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
        vec![receipt],
        staging,
    )
    .expect("exact source read, independent CPU RHS, ownership and write contract must all pass")
}
