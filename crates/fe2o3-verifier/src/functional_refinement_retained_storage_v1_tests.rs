use super::super::{
    InertFunctionalRefinementReceiptSignatureV2, RetainedImportedFunctionalRefinementReceiptV2,
    import_and_retain_functional_refinement_receipt_v2,
};
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportErrorV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementResultV2, SafeReferenceKindV2, UnsignedFunctionalRefinementReceiptV2,
    VerusToolchainIdentityV2,
};
use fe2o3_proof_contracts::DigestV1;
use std::mem::size_of;

fn digest(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

fn fixture() -> (
    FunctionalRefinementBindingV2,
    FunctionalRefinementImportPolicyV2,
    InertFunctionalRefinementReceiptSignatureV2,
) {
    // A real signature/import consistency fixture, not a Verus execution or
    // compiler-authenticated aggregate. Do not construct the execution wrapper.
    let signing = SigningKey::from_bytes(&[0x4a; 32]);
    let key = signing.verifying_key().to_bytes();
    let toolchain =
        VerusToolchainIdentityV2::new(digest(1), digest(2), digest(3), digest(4), digest(5))
            .unwrap();
    let binding = FunctionalRefinementBindingV2::new(
        SafeReferenceKindV2::Mir,
        digest(6),
        DigestV1::ZERO,
        digest(7),
        digest(8),
        digest(9),
        digest(10),
    )
    .unwrap();
    let boundary = FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir;
    let policy = FunctionalRefinementImportPolicyV2::new(key, toolchain, boundary).unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(11),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    (
        binding,
        policy,
        InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(wire, key),
    )
}

fn bounded(
    owner: &RetainedImportedFunctionalRefinementReceiptV2,
    byte_limit: usize,
    item_limit: usize,
) -> Result<(usize, usize), &'static str> {
    let mut bytes = size_of::<RetainedImportedFunctionalRefinementReceiptV2>();
    let mut items = 1usize;
    if bytes > byte_limit {
        return Err("bytes");
    }
    if items > item_limit {
        return Err("items");
    }
    owner.visit_retained_heap_storage_v1(|count, width| {
        let next_bytes = bytes
            .checked_add(count.checked_mul(width).ok_or("arithmetic")?)
            .ok_or("arithmetic")?;
        let next_items = items.checked_add(1).ok_or("arithmetic")?;
        if next_bytes > byte_limit {
            return Err("bytes");
        }
        if next_items > item_limit {
            return Err("items");
        }
        bytes = next_bytes;
        items = next_items;
        Ok(())
    })?;
    Ok((bytes, items))
}

#[test]
fn genuine_retained_import_has_no_heap_and_preserves_exact_transport() {
    let (binding, policy, signature) = fixture();
    let owner =
        import_and_retain_functional_refinement_receipt_v2(binding, &signature, &policy).unwrap();
    let identity = owner.proof().receipt_identity();
    let mut visits = Vec::new();
    owner
        .visit_retained_heap_storage_v1(|count, width| {
            visits.push((count, width));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(visits.is_empty());
    let (proof, after) = owner.into_parts();
    assert_eq!(proof.receipt_identity(), identity);
    assert_eq!(proof.binding(), binding);
    assert_eq!(after, signature);
}

#[test]
fn retained_header_is_counted_once_with_exact_and_one_short_bounds() {
    let (binding, policy, signature) = fixture();
    let owner =
        import_and_retain_functional_refinement_receipt_v2(binding, &signature, &policy).unwrap();
    let header = size_of::<RetainedImportedFunctionalRefinementReceiptV2>();
    assert_eq!(bounded(&owner, header, 1), Ok((header, 1)));
    assert_eq!(bounded(&owner, header - 1, 1), Err("bytes"));
    assert_eq!(bounded(&owner, header, 0), Err("items"));
}

#[test]
fn retained_zero_heap_delegation_invokes_no_callback() {
    let (binding, policy, signature) = fixture();
    let owner =
        import_and_retain_functional_refinement_receipt_v2(binding, &signature, &policy).unwrap();
    let mut calls = 0;
    let result = owner.visit_retained_heap_storage_v1(|_, _| {
        calls += 1;
        Err::<(), _>("stop")
    });
    assert_eq!(result, Ok(()));
    assert_eq!(calls, 0);
}

#[test]
fn retained_owner_still_requires_exact_signature_and_policy() {
    let (binding, policy, signature) = fixture();
    let mut damaged = *signature.wire();
    let last = damaged.len() - 1;
    damaged[last] ^= 1;
    let damaged = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        damaged,
        *signature.verifying_key(),
    );
    assert!(matches!(
        import_and_retain_functional_refinement_receipt_v2(binding, &damaged, &policy),
        Err(FunctionalRefinementImportErrorV2::SignatureRejected)
    ));
    let wrong_key = SigningKey::from_bytes(&[0x4b; 32])
        .verifying_key()
        .to_bytes();
    let wrong_policy =
        FunctionalRefinementImportPolicyV2::new(wrong_key, policy.toolchain(), policy.boundary())
            .unwrap();
    assert!(matches!(
        import_and_retain_functional_refinement_receipt_v2(binding, &signature, &wrong_policy),
        Err(FunctionalRefinementImportErrorV2::WrongSigner)
    ));
}
