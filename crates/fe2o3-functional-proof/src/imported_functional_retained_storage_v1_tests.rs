use crate::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportErrorV2, FunctionalRefinementImportExpectationV2,
    FunctionalRefinementImportPolicyV2, FunctionalRefinementReceiptImporterV2,
    FunctionalRefinementResultV2, ImportedFunctionalRefinementProofV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2, VerusToolchainIdentityV2,
};
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_proof_contracts::DigestV1;
use std::mem::size_of;

fn digest(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

fn fixture() -> (
    FunctionalRefinementReceiptImporterV2,
    FunctionalRefinementImportExpectationV2,
    [u8; crate::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
) {
    // Actual Ed25519/import-policy checks, with an explicitly local CPU test
    // signer. This is not evidence of Verus execution or compiler custody.
    let signing = SigningKey::from_bytes(&[0x49; 32]);
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
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
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
        FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap(),
        FunctionalRefinementImportExpectationV2::new(binding),
        wire,
    )
}

fn imported() -> ImportedFunctionalRefinementProofV2 {
    let (mut importer, expectation, wire) = fixture();
    importer.import(expectation, &wire).unwrap()
}

// An independent caller example: one actual inline header/root and no fields
// added a second time. No shared production Counter implementation is changed.
fn bounded(
    owner: &ImportedFunctionalRefinementProofV2,
    byte_limit: usize,
    item_limit: usize,
) -> Result<(usize, usize), &'static str> {
    let mut bytes = size_of::<ImportedFunctionalRefinementProofV2>();
    let mut items = 1usize;
    if bytes > byte_limit {
        return Err("bytes");
    }
    if items > item_limit {
        return Err("items");
    }
    owner.visit_retained_heap_storage_v1(|count, width| {
        let added = count.checked_mul(width).ok_or("arithmetic")?;
        let next_bytes = bytes.checked_add(added).ok_or("arithmetic")?;
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
fn actual_imported_owner_has_no_heap_callbacks_and_keeps_identity() {
    let (mut importer, expectation, wire) = fixture();
    let owner = importer.import(expectation, &wire).unwrap();
    let before = (
        owner.receipt_identity(),
        owner.signer_identity(),
        owner.binding(),
        owner.toolchain(),
        owner.execution_identity(),
        owner.boundary(),
    );
    let mut visits = Vec::new();
    owner
        .visit_retained_heap_storage_v1(|count, width| {
            visits.push((count, width));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(visits.is_empty());
    assert_eq!(
        before,
        (
            owner.receipt_identity(),
            owner.signer_identity(),
            owner.binding(),
            owner.toolchain(),
            owner.execution_identity(),
            owner.boundary(),
        )
    );
    assert_eq!(importer.imported_count(), 1);
}

#[test]
fn caller_counts_one_header_and_root_with_exact_limits() {
    let owner = imported();
    let header = size_of::<ImportedFunctionalRefinementProofV2>();
    assert_eq!(bounded(&owner, header, 1), Ok((header, 1)));
}

#[test]
fn caller_refuses_one_short_header_or_root_without_a_partial_result() {
    let owner = imported();
    let header = size_of::<ImportedFunctionalRefinementProofV2>();
    assert_eq!(bounded(&owner, header - 1, 1), Err("bytes"));
    assert_eq!(bounded(&owner, header, 0), Err("items"));
}

#[test]
fn zero_heap_does_not_invoke_even_a_refusing_callback() {
    let owner = imported();
    let mut calls = 0;
    let result = owner.visit_retained_heap_storage_v1(|_, _| {
        calls += 1;
        Err::<(), _>("refused")
    });
    assert_eq!(result, Ok(()));
    assert_eq!(calls, 0);
}

#[test]
fn storage_observation_does_not_bypass_signature_or_duplicate_refusal() {
    let (mut importer, expectation, wire) = fixture();
    let mut damaged = wire;
    let last = damaged.len() - 1;
    damaged[last] ^= 1;
    assert!(matches!(
        importer.import(expectation, &damaged),
        Err(FunctionalRefinementImportErrorV2::SignatureRejected)
    ));
    assert_eq!(importer.imported_count(), 0);
    let owner = importer.import(expectation, &wire).unwrap();
    owner
        .visit_retained_heap_storage_v1(|_, _| Ok::<_, ()>(()))
        .unwrap();
    assert!(matches!(
        importer.import(expectation, &wire),
        Err(FunctionalRefinementImportErrorV2::DuplicateReceipt(_))
    ));
}
