use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportErrorV2, FunctionalRefinementImportExpectationV2,
    FunctionalRefinementImportPolicyV2, FunctionalRefinementReceiptImporterV2,
    FunctionalRefinementResultV2, SafeReferenceKindV2, UnsignedFunctionalRefinementReceiptV2,
    VerusToolchainIdentityV2,
};
use fe2o3_proof_contracts::DigestV1;

fn digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}

fn toolchain(base: u8) -> VerusToolchainIdentityV2 {
    VerusToolchainIdentityV2::new(
        digest(base),
        digest(base + 1),
        digest(base + 2),
        digest(base + 3),
        digest(base + 4),
    )
    .unwrap()
}

fn binding() -> FunctionalRefinementBindingV2 {
    FunctionalRefinementBindingV2::new(
        SafeReferenceKindV2::SourceAndMir,
        digest(1),
        digest(2),
        digest(3),
        digest(4),
        digest(5),
        digest(6),
    )
    .unwrap()
}

fn signer(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn policy(signing: &SigningKey) -> FunctionalRefinementImportPolicyV2 {
    FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain(10),
        FunctionalRefinementBoundaryV2::SafeReferenceSourceToKernelMir,
    )
    .unwrap()
}

fn signed(
    signing: &SigningKey,
    signer_identity: DigestV1,
    binding: FunctionalRefinementBindingV2,
    toolchain: VerusToolchainIdentityV2,
    result: FunctionalRefinementResultV2,
    boundary: FunctionalRefinementBoundaryV2,
) -> Vec<u8> {
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        signer_identity,
        binding,
        toolchain,
        digest(40),
        result,
        boundary,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    unsigned.attach_signature(signature).to_vec()
}

fn canonical(signing: &SigningKey) -> Vec<u8> {
    let policy = policy(signing);
    signed(
        signing,
        policy.signer_identity(),
        binding(),
        policy.toolchain(),
        FunctionalRefinementResultV2::Proved,
        policy.boundary(),
    )
}

#[test]
fn exact_signed_receipt_imports_once_with_narrow_authority() {
    let signing = signer(90);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy(&signing), 1).unwrap();
    let wire = canonical(&signing);
    let proof = importer
        .import(
            FunctionalRefinementImportExpectationV2::new(binding()),
            &wire,
        )
        .unwrap();
    assert!(proof.grants_functional_refinement_evidence());
    assert!(!proof.grants_source_to_isa_authority());
    assert!(!proof.grants_artifact_or_launch_authority());
    assert_eq!(proof.binding(), binding());
    assert_eq!(importer.imported_count(), 1);

    assert!(matches!(
        importer.import(
            FunctionalRefinementImportExpectationV2::new(binding()),
            &wire
        ),
        Err(FunctionalRefinementImportErrorV2::DuplicateReceipt(_))
    ));
}

#[test]
fn caller_forged_proved_and_wrong_signer_are_rejected() {
    let signing = signer(91);
    let pinned = policy(&signing);
    let forged = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        pinned.signer_identity(),
        binding(),
        pinned.toolchain(),
        digest(40),
        FunctionalRefinementResultV2::Proved,
        pinned.boundary(),
    )
    .unwrap()
    .attach_signature([0; 64]);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(pinned, 4).unwrap();
    assert_eq!(
        importer.import(
            FunctionalRefinementImportExpectationV2::new(binding()),
            &forged
        ),
        Err(FunctionalRefinementImportErrorV2::SignatureRejected)
    );
    assert_eq!(importer.imported_count(), 0);

    let wrong_signing = signer(92);
    let wrong_policy = policy(&wrong_signing);
    let wrong = signed(
        &wrong_signing,
        wrong_policy.signer_identity(),
        binding(),
        importer_policy_toolchain(),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceSourceToKernelMir,
    );
    assert_eq!(
        importer.import(
            FunctionalRefinementImportExpectationV2::new(binding()),
            &wrong
        ),
        Err(FunctionalRefinementImportErrorV2::WrongSigner)
    );
    assert_eq!(importer.imported_count(), 0);
}

fn importer_policy_toolchain() -> VerusToolchainIdentityV2 {
    toolchain(10)
}

#[test]
fn failed_wrong_toolchain_and_wrong_boundary_never_import() {
    let signing = signer(93);
    let pinned = policy(&signing);
    let signer_identity = pinned.signer_identity();
    let expected_toolchain = pinned.toolchain();
    let expected_boundary = pinned.boundary();
    let mut importer = FunctionalRefinementReceiptImporterV2::new(pinned, 8).unwrap();
    for (wire, expected) in [
        (
            signed(
                &signing,
                signer_identity,
                binding(),
                expected_toolchain,
                FunctionalRefinementResultV2::Failed,
                expected_boundary,
            ),
            FunctionalRefinementImportErrorV2::ResultNotProved(
                FunctionalRefinementResultV2::Failed,
            ),
        ),
        (
            signed(
                &signing,
                signer_identity,
                binding(),
                toolchain(20),
                FunctionalRefinementResultV2::Proved,
                expected_boundary,
            ),
            FunctionalRefinementImportErrorV2::WrongToolchain,
        ),
        (
            signed(
                &signing,
                signer_identity,
                binding(),
                expected_toolchain,
                FunctionalRefinementResultV2::Proved,
                FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
            ),
            FunctionalRefinementImportErrorV2::WrongBoundary,
        ),
    ] {
        assert_eq!(
            importer.import(
                FunctionalRefinementImportExpectationV2::new(binding()),
                &wire
            ),
            Err(expected)
        );
    }
    assert_eq!(importer.imported_count(), 0);
}

#[test]
fn every_current_compiler_hash_is_checked_for_staleness() {
    let signing = signer(94);
    let pinned = policy(&signing);
    let signer_identity = pinned.signer_identity();
    let expected_toolchain = pinned.toolchain();
    let expected_boundary = pinned.boundary();
    let mut importer = FunctionalRefinementReceiptImporterV2::new(pinned, 16).unwrap();
    let original = binding();
    let mutations = [
        (
            FunctionalRefinementBindingV2::new(
                SafeReferenceKindV2::SourceAndMir,
                digest(51),
                digest(2),
                digest(3),
                digest(4),
                digest(5),
                digest(6),
            )
            .unwrap(),
            FunctionalRefinementImportErrorV2::StaleSafeReferenceIdentity,
        ),
        (
            FunctionalRefinementBindingV2::new(
                SafeReferenceKindV2::SourceAndMir,
                digest(1),
                digest(52),
                digest(3),
                digest(4),
                digest(5),
                digest(6),
            )
            .unwrap(),
            FunctionalRefinementImportErrorV2::StaleSafeReferenceSource,
        ),
        (
            FunctionalRefinementBindingV2::new(
                SafeReferenceKindV2::SourceAndMir,
                digest(1),
                digest(2),
                digest(53),
                digest(4),
                digest(5),
                digest(6),
            )
            .unwrap(),
            FunctionalRefinementImportErrorV2::StaleSafeReferenceMir,
        ),
        (
            FunctionalRefinementBindingV2::new(
                SafeReferenceKindV2::SourceAndMir,
                digest(1),
                digest(2),
                digest(3),
                digest(54),
                digest(5),
                digest(6),
            )
            .unwrap(),
            FunctionalRefinementImportErrorV2::StaleKernelSubject,
        ),
        (
            FunctionalRefinementBindingV2::new(
                SafeReferenceKindV2::SourceAndMir,
                digest(1),
                digest(2),
                digest(3),
                digest(4),
                digest(55),
                digest(6),
            )
            .unwrap(),
            FunctionalRefinementImportErrorV2::StaleKernelMir,
        ),
        (
            FunctionalRefinementBindingV2::new(
                SafeReferenceKindV2::SourceAndMir,
                digest(1),
                digest(2),
                digest(3),
                digest(4),
                digest(5),
                digest(56),
            )
            .unwrap(),
            FunctionalRefinementImportErrorV2::StaleNormalizedObligationEffectIr,
        ),
    ];
    for (changed, expected) in mutations {
        let wire = signed(
            &signing,
            signer_identity,
            changed,
            expected_toolchain,
            FunctionalRefinementResultV2::Proved,
            expected_boundary,
        );
        assert_eq!(
            importer.import(
                FunctionalRefinementImportExpectationV2::new(original),
                &wire
            ),
            Err(expected)
        );
    }
    assert_eq!(importer.imported_count(), 0);
}

#[test]
fn truncation_corruption_and_mir_only_noncanonical_source_fail_closed() {
    let signing = signer(95);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy(&signing), 8).unwrap();
    let wire = canonical(&signing);
    assert!(matches!(
        importer.import(
            FunctionalRefinementImportExpectationV2::new(binding()),
            &wire[..wire.len() - 1]
        ),
        Err(FunctionalRefinementImportErrorV2::WrongWireLength { .. })
    ));
    let mut corrupted = wire;
    corrupted[100] ^= 1;
    assert_eq!(
        importer.import(
            FunctionalRefinementImportExpectationV2::new(binding()),
            &corrupted
        ),
        Err(FunctionalRefinementImportErrorV2::SignatureRejected)
    );
    assert_eq!(
        FunctionalRefinementBindingV2::new(
            SafeReferenceKindV2::Mir,
            digest(1),
            digest(2),
            digest(3),
            digest(4),
            digest(5),
            digest(6),
        ),
        Err(FunctionalRefinementImportErrorV2::NonCanonicalAbsentSourceHash)
    );
}
