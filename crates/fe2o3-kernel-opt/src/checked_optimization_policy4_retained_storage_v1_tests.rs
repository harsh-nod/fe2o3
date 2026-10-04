//! Genuine constructors, followed by test-only spare-capacity perturbation.
use super::*;
use crate::{
    checked_load_forwarding_v1::tests::{fixture, with_owner},
    optimize_checked_canonical_kernel_ir_policy4_v1,
};
use fe2o3_kernel_ir::LogicalStorageLimitsV1 as Limits;
use std::mem::size_of;
fn observe(owner: &CheckedCanonicalKernelIrOwnerPolicy4V1) -> (usize, usize) {
    let mut counter = Counter::new(Limits {
        max_bytes: None,
        max_items: 100_000,
    });
    owner.charge_retained_heap_v11(&mut counter).unwrap();
    (counter.bytes(), counter.items())
}

#[test]
fn genuine_policy4_spare_rows_are_capacity_not_old_receipt_or_length() {
    with_owner(fixture(), |input, budget| {
        let mut owner = optimize_checked_canonical_kernel_ir_policy4_v1(input, budget).unwrap();
        let retained = owner.retained_storage();
        budget.reserve_storage(retained).unwrap();
        let bytes = owner.owner().canonical().canonical_bytes().to_vec();
        let original_rows = owner.rows.clone();
        let before_capacity = owner.rows.capacity();
        let before = observe(&owner);
        // Only this private control mutates capacity, never row contents.
        owner.rows.reserve_exact(before_capacity + 17);
        let after_capacity = owner.rows.capacity();
        assert!(after_capacity > before_capacity);
        let after = observe(&owner);
        assert_eq!(
            after.0 - before.0,
            (after_capacity - before_capacity) * size_of::<super::super::Row>()
        );
        assert_eq!(after.1, before.1);
        assert_eq!(owner.rows, original_rows);
        assert_eq!(owner.owner().canonical().canonical_bytes(), bytes);
        assert_eq!(owner.retained_storage(), retained);
        drop(owner);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn genuine_policy4_exact_composition_counts_each_distinct_output_once() {
    with_owner(fixture(), |input, budget| {
        let owner = optimize_checked_canonical_kernel_ir_policy4_v1(input, budget).unwrap();
        let retained = owner.retained_storage();
        budget.reserve_storage(retained).unwrap();
        let mut prefix = Counter::new(Limits {
            max_bytes: None,
            max_items: 100_000,
        });
        owner
            .intermediate
            .charge_retained_heap_v11(&mut prefix)
            .unwrap();
        let mut output = Counter::new(Limits {
            max_bytes: None,
            max_items: 100_000,
        });
        owner.output.charge_retained_heap_v11(&mut output).unwrap();
        let heap = prefix.bytes()
            + output.bytes()
            + owner.rows.capacity() * size_of::<super::super::Row>();
        let visits = prefix.items() + output.items() + 1;
        assert_eq!(observe(&owner), (heap, visits));
        let mut exact = Counter::new(Limits {
            max_bytes: Some(heap),
            max_items: visits,
        });
        owner.charge_retained_heap_v11(&mut exact).unwrap();
        let mut short = Counter::new(Limits {
            max_bytes: Some(heap - 1),
            max_items: visits,
        });
        assert_eq!(
            owner.charge_retained_heap_v11(&mut short),
            Err(Error::ByteLimit)
        );
        let mut short = Counter::new(Limits {
            max_bytes: Some(heap),
            max_items: visits - 1,
        });
        assert_eq!(
            owner.charge_retained_heap_v11(&mut short),
            Err(Error::ItemLimit)
        );
        drop(owner);
        budget.release_storage(retained).unwrap();
    });
}
