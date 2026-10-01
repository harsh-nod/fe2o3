use super::*;
use fe2o3_kernel_ir::LogicalStorageLimitsV1;
use std::mem::size_of;

fn fixture() -> InertProductionMiddleEndEvidenceV5 {
    super::super::tests::retained_storage_test_fixture_v1()
}

fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
    LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    })
}

#[test]
fn codec_owner_counts_its_one_actual_box_without_recounting_ranked_ir() {
    let owner = fixture();
    let before = owner.canonical_bytes.to_vec();
    let expected = owner.canonical_bytes.len();
    assert!(owner.ranked_ir_range.end > owner.ranked_ir_range.start);
    let mut c = counter(Some(expected), 1);
    owner.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 1));
    assert_eq!(owner.canonical_bytes.as_ref(), before.as_slice());
    assert_eq!(
        InertProductionMiddleEndEvidenceV5::decode(&before).unwrap(),
        owner
    );
}

#[test]
fn caller_header_and_prior_owner_are_charged_exactly_once() {
    let owner = fixture();
    let header = size_of::<InertProductionMiddleEndEvidenceV5>();
    let expected = 13 + header + owner.canonical_bytes.len();
    let mut c = counter(Some(expected), 3);
    c.charge(13, 1).unwrap();
    c.charge(header, 1).unwrap();
    owner.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 3));
}

#[test]
fn byte_and_item_refusals_preserve_the_existing_counter_atomically() {
    let owner = fixture();
    for (limit, items, error) in [
        (
            Some(7 + owner.canonical_bytes.len() - 1),
            2,
            LogicalStorageErrorV1::ByteLimit,
        ),
        (None, 1, LogicalStorageErrorV1::ItemLimit),
    ] {
        let mut c = counter(limit, items);
        c.charge(7, 1).unwrap();
        assert_eq!(owner.charge_retained_heap_storage_v1(&mut c), Err(error));
        assert_eq!((c.bytes(), c.items()), (7, 1));
    }
}

#[test]
fn arithmetic_refusal_preserves_the_existing_counter_atomically() {
    let owner = fixture();
    assert!(!owner.canonical_bytes.is_empty());
    let mut c = counter(None, usize::MAX);
    c.charge(usize::MAX, 1).unwrap();
    assert_eq!(
        owner.charge_retained_heap_storage_v1(&mut c),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (usize::MAX, 1));
}

#[test]
fn wrapper_shape_delegates_and_distinct_equal_boxes_are_not_deduplicated() {
    // Private shape fixture only: this does not call try_new or establish
    // live-produced provenance. Both payloads come from the original codec.
    let inert = fixture();
    let wrapper = ProductionMiddleEndEvidenceV5 { inert: fixture() };
    assert_eq!(inert, wrapper.inert);
    assert_ne!(
        inert.canonical_bytes.as_ptr(),
        wrapper.inert.canonical_bytes.as_ptr()
    );
    let expected = 2 * inert.canonical_bytes.len();
    let mut c = counter(Some(expected), 2);
    inert.charge_retained_heap_storage_v1(&mut c).unwrap();
    wrapper.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 2));
}
