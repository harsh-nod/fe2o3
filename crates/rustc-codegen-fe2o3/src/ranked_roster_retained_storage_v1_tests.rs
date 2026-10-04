//! Arithmetic/inert transport controls plus type-only admitted-owner coverage.
//! These tests deliberately do not construct any authenticated roster.

use super::*;
use fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2;
use fe2o3_kernel_ir::LogicalStorageLimitsV1;
use std::mem::size_of;

fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
    LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    })
}

fn inert_signature() -> InertFunctionalRefinementReceiptSignatureV2 {
    // Untrusted inert bytes, not a successful signature import or execution.
    InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        [0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
        [0; 32],
    )
}

#[test]
fn all_roster_layers_are_type_checked_without_minting_authenticated_owners() {
    let _: fn(
        &AuthenticatedRankedVerificationRosterV1,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        AuthenticatedRankedVerificationRosterV1::charge_retained_heap_storage_v1;
    let _: fn(
        &AuthenticatedRankedVerificationRootV1,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        AuthenticatedRankedVerificationRootV1::charge_retained_heap_storage_v1;
    let _: fn(
        &AuthenticatedRankedVerificationV5,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        AuthenticatedRankedVerificationV5::charge_retained_heap_storage_v1;
    let _: fn(
        &AuthenticatedFunctionalVerificationV1,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        AuthenticatedFunctionalVerificationV1::charge_retained_heap_storage_v1;
}

#[test]
fn actual_inert_effect_receipt_vec_uses_capacity_and_preserves_transport() {
    let mut receipts = Vec::with_capacity(7);
    receipts.push(inert_signature());
    let first = receipts[0];
    let expected = receipts.capacity() * size_of::<InertFunctionalRefinementReceiptSignatureV2>();
    assert!(receipts.capacity() > receipts.len());
    let mut c = counter(Some(expected), 1);
    charge_effect_receipts(&mut c, &receipts).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 1));
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0], first);
}

#[test]
fn retained_empty_effect_vector_capacity_is_not_lost() {
    let receipts = Vec::with_capacity(5);
    let expected = receipts.capacity() * size_of::<InertFunctionalRefinementReceiptSignatureV2>();
    let mut c = counter(Some(expected), 1);
    charge_effect_receipts(&mut c, &receipts).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 1));
    let empty = Vec::new();
    let mut c = counter(Some(0), 1);
    charge_effect_receipts(&mut c, &empty).unwrap();
    assert_eq!((c.bytes(), c.items()), (0, 1));
}

#[test]
fn effect_vector_byte_and_item_refusals_do_not_commit_a_charge() {
    let receipts = vec![inert_signature()];
    let bytes = receipts.capacity() * size_of::<InertFunctionalRefinementReceiptSignatureV2>();
    let mut short_bytes = counter(Some(bytes - 1), 1);
    assert_eq!(
        charge_effect_receipts(&mut short_bytes, &receipts),
        Err(LogicalStorageErrorV1::ByteLimit)
    );
    assert_eq!((short_bytes.bytes(), short_bytes.items()), (0, 0));
    let mut short_items = counter(Some(bytes), 0);
    assert_eq!(
        charge_effect_receipts(&mut short_items, &receipts),
        Err(LogicalStorageErrorV1::ItemLimit)
    );
    assert_eq!((short_items.bytes(), short_items.items()), (0, 0));
}

#[test]
fn shared_extent_adapter_checks_multiplication_and_prefix_addition() {
    let mut c = counter(None, usize::MAX);
    c.charge(3, 1).unwrap();
    assert_eq!(
        charge_extent(&mut c, usize::MAX, 2),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (3, 1));
    assert_eq!(
        charge_extent(&mut c, usize::MAX, 1),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (3, 1));
}

#[test]
fn shared_extent_adapter_preserves_exact_byte_and_item_limits() {
    let mut c = counter(Some(6), 2);
    charge_extent(&mut c, 2, 3).unwrap();
    charge_extent(&mut c, 0, 1).unwrap();
    assert_eq!((c.bytes(), c.items()), (6, 2));
    assert_eq!(
        charge_extent(&mut c, 0, 1),
        Err(LogicalStorageErrorV1::ItemLimit)
    );
    assert_eq!((c.bytes(), c.items()), (6, 2));
    let mut c = counter(Some(5), 2);
    assert_eq!(
        charge_extent(&mut c, 2, 3),
        Err(LogicalStorageErrorV1::ByteLimit)
    );
    assert_eq!((c.bytes(), c.items()), (0, 0));
}

#[test]
fn shared_extent_adapter_item_overflow_is_atomic() {
    let mut c = counter(None, usize::MAX);
    c.charge(0, usize::MAX).unwrap();
    assert_eq!(
        charge_extent(&mut c, 0, 1),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (0, usize::MAX));
}
