//! Strict rejection only: no locally signed stand-in for genuine execution.
use super::{fixtures::digest, *};
use ed25519_dalek::SigningKey;
use fe2o3_functional_proof::{
    FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2, FunctionalRefinementImportErrorV2,
    FunctionalRefinementSubjectsV2, SafeReferenceKindV2, VerusToolchainIdentityV2,
};

fn policy() -> FunctionalRefinementImportPolicyV2 {
    FunctionalRefinementImportPolicyV2::new(
        SigningKey::from_bytes(&[71; 32]).verifying_key().to_bytes(),
        VerusToolchainIdentityV2::new(digest(10), digest(11), digest(12), digest(13), digest(14))
            .unwrap(),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap()
}

fn binding() -> FunctionalRefinementBindingV2 {
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(4),
    )
    .unwrap();
    FunctionalRefinementBindingV2::from_subjects(
        subjects,
        obligation_identity_v2(std::array::from_fn(|i| digest(i as u8 + 1)), digest(7)),
    )
    .unwrap()
}

#[test]
fn v2_transport_key_is_checked_against_external_policy_not_selected_by_packet() {
    let accepted = policy();
    for key_tag in [71, 72] {
        let signature = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
            [0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
            SigningKey::from_bytes(&[key_tag; 32])
                .verifying_key()
                .to_bytes(),
        );
        let result =
            import_and_retain_functional_refinement_receipt_v2(binding(), &signature, &accepted);
        if key_tag == 71 {
            // Matching an accepted key is not acceptance of a receipt/execution.
            assert!(matches!(
                result,
                Err(FunctionalRefinementImportErrorV2::WrongMagic)
            ));
        } else {
            // This must fail before even decoding the invalid wire.
            assert!(matches!(
                result,
                Err(FunctionalRefinementImportErrorV2::WrongSigner)
            ));
        }
    }
}

#[test]
fn v2_strict_import_rejects_empty_truncated_trailing_and_unsigned_bytes() {
    let accepted = policy();
    let zeros = vec![0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 + 1];
    for len in [
        0,
        1,
        FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 - 1,
        FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2,
        FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 + 1,
    ] {
        assert!(
            retention::import_expected(binding(), &accepted, &zeros[..len]).is_err(),
            "length {len}"
        );
    }
    for fill in [1, 0x7f, 0xff] {
        let wire = [fill; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2];
        assert!(retention::import_expected(binding(), &accepted, &wire).is_err());
        let signature = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
            wire,
            SigningKey::from_bytes(&[71; 32]).verifying_key().to_bytes(),
        );
        assert!(
            import_and_retain_functional_refinement_receipt_v2(binding(), &signature, &accepted)
                .is_err()
        );
    }
}

#[test]
fn v2_weak_transport_key_is_rejected_before_wire_import() {
    let accepted = policy();
    let mut identity_point = [0; 32];
    identity_point[0] = 1;
    let signature = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        [0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
        identity_point,
    );
    assert!(matches!(
        import_and_retain_functional_refinement_receipt_v2(binding(), &signature, &accepted),
        Err(FunctionalRefinementImportErrorV2::WeakVerifyingKey)
    ));
}

#[test]
fn v2_external_policy_is_not_rewritten_from_signature_transport() {
    let accepted = policy();
    let before = accepted.clone();
    let signature = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        [0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
        SigningKey::from_bytes(&[72; 32]).verifying_key().to_bytes(),
    );
    assert!(matches!(
        import_and_retain_functional_refinement_receipt_v2(binding(), &signature, &accepted),
        Err(FunctionalRefinementImportErrorV2::WrongSigner)
    ));
    assert_eq!(accepted, before);
    assert!(retention::import_expected(binding(), &accepted, signature.wire()).is_err());
    assert_eq!(accepted, before);
}
