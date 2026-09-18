use super::*;
use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementResultV2, FunctionalRefinementSubjectsV2,
    SafeReferenceKindV2, UnsignedFunctionalRefinementReceiptV2,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_pliron::{
    ProductionNumericalContractV2, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionReferenceProofV2, ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2,
    normalized_functional_refinement_formula_hash_for_kernel_v2,
};

const STORAGE: usize = 64 * 1024 * 1024;
const WORK: usize = 10_000_000;

fn digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}

struct Fixture {
    kernel: ProductionRankedKernelV1,
    signature: InertFunctionalRefinementReceiptSignatureV2,
    expected: NativeCompilerStagingCommitmentV1,
    toolchain: VerusToolchainIdentityV2,
}

// Explicit public TEST signing keys exercise only independent consistency.
// No production runtime, compiler origin, or source equivalence is asserted.
fn signed_fixture(seed: u8) -> Fixture {
    let local = |value| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(value));
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(10),
        DigestV1::ZERO,
        digest(11),
        digest(12),
        digest(13),
    )
    .unwrap();
    let expression = |id| ProductionRankedOperationV1::SemanticExpression {
        result: ProductionRankedValueIdV1::new(id),
        expression: ProductionSemanticExpressionV2::Constant {
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            },
            bits: 7,
        },
        numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
    };
    let kernel = ProductionRankedKernelV1::new(
        "native_ranked_replay_test",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                expression(0),
                expression(1),
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                    actual: local(0),
                    expected: local(1),
                    subjects,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let obligation = normalized_functional_refinement_formula_hash_for_kernel_v2(
        &kernel,
        0,
        3,
        local(0),
        local(1),
        subjects,
    )
    .unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation).unwrap();
    let signing = SigningKey::from_bytes(&[seed; 32]);
    let toolchain =
        VerusToolchainIdentityV2::new(digest(20), digest(21), digest(22), digest(23), digest(24))
            .unwrap();
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(seed),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let kernel = kernel
        .bind_functional_refinement_request_v2(
            0,
            3,
            ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
        )
        .unwrap();
    Fixture {
        kernel,
        signature: InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
            wire,
            signing.verifying_key().to_bytes(),
        ),
        expected: commitment(&imported),
        toolchain,
    }
}

fn candidate(kernel: &ProductionRankedKernelV1) -> NativeRankedSourceCandidateV1<'_> {
    NativeRankedSourceCandidateV1::from_untrusted_parts(0, 1, kernel, &[], &[], "diagnostic only")
}

fn run(
    kernel: &ProductionRankedKernelV1,
    receipts: &[InertFunctionalRefinementReceiptSignatureV2],
    commitments: &[NativeCompilerStagingCommitmentV1],
    toolchain: VerusToolchainIdentityV2,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    let floor = budget.storage();
    let result = recompile_root(
        NativeCompilerRankedRootV1 {
            candidate: candidate(kernel),
            effect_receipts: receipts,
        },
        commitments,
        toolchain,
        budget,
    )
    .map(drop);
    budget.release_storage(budget.storage() - floor).unwrap();
    result
}

#[test]
fn full_effect_signature_is_reimported_and_typed_ranked_pipeline_is_reexecuted() {
    let fixture = signed_fixture(41);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    run(
        &fixture.kernel,
        &[fixture.signature],
        &[fixture.expected],
        fixture.toolchain,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn missing_extra_and_foreign_effect_signatures_are_not_commitment_fallbacks() {
    let fixture = signed_fixture(41);
    let foreign = signed_fixture(42);
    for (receipts, commitments) in [
        (vec![], vec![fixture.expected]),
        (
            vec![fixture.signature, fixture.signature],
            vec![fixture.expected],
        ),
        (vec![foreign.signature], vec![fixture.expected]),
        (
            vec![fixture.signature, foreign.signature],
            vec![fixture.expected, foreign.expected],
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        assert!(
            run(
                &fixture.kernel,
                &receipts,
                &commitments,
                fixture.toolchain,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn original_binding_rejects_changed_signature_key_toolchain_and_staging_commitment() {
    let fixture = signed_fixture(41);
    let mut wire = *fixture.signature.wire();
    *wire.last_mut().unwrap() ^= 1;
    let bad_wire = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        wire,
        *fixture.signature.verifying_key(),
    );
    let bad_key = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        *fixture.signature.wire(),
        SigningKey::from_bytes(&[52; 32]).verifying_key().to_bytes(),
    );
    let mut changed = fixture.expected;
    changed.execution[0] ^= 1;
    let other_toolchain =
        VerusToolchainIdentityV2::new(digest(30), digest(21), digest(22), digest(23), digest(24))
            .unwrap();
    for (signature, expected, toolchain) in [
        (bad_wire, fixture.expected, fixture.toolchain),
        (bad_key, fixture.expected, fixture.toolchain),
        (fixture.signature, changed, fixture.toolchain),
        (fixture.signature, fixture.expected, other_toolchain),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        assert!(
            run(
                &fixture.kernel,
                &[signature],
                &[expected],
                toolchain,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn original_receipt_does_not_authorize_changed_typed_operands() {
    let fixture = signed_fixture(41);
    let mut operations = fixture.kernel.blocks()[0].operations().to_vec();
    let ProductionRankedOperationV1::SemanticExpression { expression, .. } = &mut operations[1]
    else {
        unreachable!()
    };
    let ProductionSemanticExpressionV2::Constant { bits, .. } = expression else {
        unreachable!()
    };
    *bits = 8;
    let changed = ProductionRankedKernelV1::new(
        fixture.kernel.function_name(),
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    assert!(matches!(
        run(
            &changed,
            &[fixture.signature],
            &[fixture.expected],
            fixture.toolchain,
            &mut budget
        ),
        Err(E::RankedCompile(_))
    ));
    assert_eq!(budget.storage(), 37);
}

#[test]
fn effect_import_has_literal_exact_and_one_short_cumulative_work_limits() {
    let fixture = signed_fixture(41);
    // Entry4, two vector reservations3 each, block1, operation4, wire+key+row,
    // and final complete-roster comparison1. PLIRON has its own bounded ledger.
    let exact = 337 + fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2;
    for limit in [exact, exact - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        let result = run(
            &fixture.kernel,
            &[fixture.signature],
            &[fixture.expected],
            fixture.toolchain,
            &mut budget,
        );
        if limit == exact {
            result.unwrap();
            assert_eq!(budget.work(), exact);
        } else {
            assert!(matches!(result,
                Err(E::Resource(Resource::Work(error))) if error.actual() == exact && error.limit() == limit));
        }
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn effect_import_reserves_both_real_vector_capacities_before_signature_work() {
    let fixture = signed_fixture(41);
    let exact = 37
        + std::mem::size_of::<ImportedFunctionalRefinementProofV2>()
        + std::mem::size_of::<DigestV1>();
    for limit in [exact, exact - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(37).unwrap();
        let result = run(
            &fixture.kernel,
            &[fixture.signature],
            &[fixture.expected],
            fixture.toolchain,
            &mut budget,
        );
        if limit == exact {
            result.unwrap();
            assert_eq!(budget.peak_storage(), exact);
        } else {
            assert!(matches!(result,
                Err(E::Resource(Resource::Storage(error))) if error.actual() == exact && error.limit() == limit));
        }
        assert_eq!(budget.storage(), 37);
    }
}

fn signed_two_request_fixture() -> (
    ProductionRankedKernelV1,
    [InertFunctionalRefinementReceiptSignatureV2; 2],
    [NativeCompilerStagingCommitmentV1; 2],
    VerusToolchainIdentityV2,
) {
    let base = signed_fixture(41);
    let ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent {
        actual,
        expected,
        proof,
    } = base.kernel.blocks()[0].operations()[3]
    else {
        unreachable!()
    };
    let subjects = proof.binding().subjects();
    let mut operations = base.kernel.blocks()[0].operations()[..3].to_vec();
    for _ in 0..2 {
        operations.push(
            ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                actual,
                expected,
                subjects,
            },
        );
    }
    let mut kernel = ProductionRankedKernelV1::new(
        base.kernel.function_name(),
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut signatures = Vec::new();
    let mut commitments = Vec::new();
    for (operation, seed) in [(3, 51), (4, 52)] {
        let obligation = normalized_functional_refinement_formula_hash_for_kernel_v2(
            &kernel, 0, operation, actual, expected, subjects,
        )
        .unwrap();
        let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation).unwrap();
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let key = signing.verifying_key().to_bytes();
        let policy = FunctionalRefinementImportPolicyV2::new(
            key,
            base.toolchain,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
            policy.signer_identity(),
            binding,
            base.toolchain,
            digest(seed),
            FunctionalRefinementResultV2::Proved,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
        let wire = unsigned.attach_signature(signature);
        let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
        let proof = importer
            .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
            .unwrap();
        commitments.push(commitment(&proof));
        signatures
            .push(InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(wire, key));
        kernel = kernel
            .bind_functional_refinement_request_v2(
                0,
                operation,
                ProductionReferenceProofV2::request_exact(proof.receipt_identity(), binding),
            )
            .unwrap();
    }
    (
        kernel,
        signatures.try_into().unwrap(),
        commitments.try_into().unwrap(),
        base.toolchain,
    )
}

#[test]
fn two_effect_receipts_are_ordered_and_cannot_be_permuted_with_their_commitments() {
    let (kernel, signatures, commitments, toolchain) = signed_two_request_fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    run(&kernel, &signatures, &commitments, toolchain, &mut budget).unwrap();
    for expected in [commitments, [commitments[1], commitments[0]]] {
        assert!(matches!(
            run(
                &kernel,
                &[signatures[1], signatures[0]],
                &expected,
                toolchain,
                &mut budget
            ),
            Err(E::EffectReceipt(_))
        ));
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn staged_compiler_independently_rejects_duplicate_effect_claims() {
    let (kernel, signatures, commitments, toolchain) = signed_two_request_fixture();
    let mut operations = kernel.blocks()[0].operations().to_vec();
    operations[4] = operations[3].clone();
    let duplicate = ProductionRankedKernelV1::new(
        kernel.function_name(),
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    let result = run(
        &duplicate,
        &[signatures[0], signatures[0]],
        &[commitments[0], commitments[0]],
        toolchain,
        &mut budget,
    );
    assert!(
        matches!(result, Err(E::RankedCompile(error)) if matches!(*error,
        fe2o3_pliron::ProductionRankedCompileErrorV2::Proof(
            fe2o3_pliron::ProductionFunctionalRefinementAdmissionErrorV2::DuplicateImportedReceipt(_))))
    );
    assert_eq!(budget.storage(), 37);
}
