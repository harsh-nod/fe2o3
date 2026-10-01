//! Inert-row ownership controls, not graph admission or compiler execution.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirTransitionCandidateV1 as Candidate, LogicalStorageLimitsV1 as Limits, Module,
    VerifiedCanonicalKernelIrV12,
};
fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}
fn receipt() -> Receipt {
    let canonical =
        VerifiedCanonicalKernelIrV12::from_module(Module::new("receipt-storage")).unwrap();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    Receipt::from_candidate_with_budget(
        canonical.identity(),
        canonical.identity(),
        Candidate {
            functions: &[],
            blocks: &[],
            segments: &[],
            operations: &[],
            definitions: &[],
            definition_outputs: &[],
            uses: &[],
            edges: &[],
            edge_arguments: &[],
        },
        &mut budget,
    )
    .unwrap()
    .0
}
fn expected(value: &Receipt) -> usize {
    // Independent actual-field formula, not the new payload/sum helpers.
    macro_rules! row_bytes {
        ($field:ident, $ty:ty) => {
            value.rows.$field.capacity() * size_of::<$ty>()
        };
    }
    value.bytes.capacity()
        + row_bytes!(functions, super::super::FunctionRow)
        + row_bytes!(blocks, super::super::BlockRow)
        + row_bytes!(segments, super::super::Segment)
        + row_bytes!(operations, super::super::OperationRow)
        + row_bytes!(definitions, super::super::DefinitionRow)
        + row_bytes!(definition_outputs, super::super::Descendant)
        + row_bytes!(uses, super::super::UseRow)
        + row_bytes!(edges, super::super::EdgeRow)
        + row_bytes!(edge_arguments, super::super::EdgeArgumentRow)
}

#[test]
fn all_ten_real_capacities_include_spare_slots_and_preserve_canonical_bytes() {
    let mut value = receipt();
    let before = value.canonical_bytes().to_vec();
    let digest = *value.digest();
    value.rows.functions.reserve_exact(1);
    value.rows.blocks.reserve_exact(2);
    value.rows.segments.reserve_exact(3);
    value.rows.operations.reserve_exact(4);
    value.rows.definitions.reserve_exact(5);
    value.rows.definition_outputs.reserve_exact(6);
    value.rows.uses.reserve_exact(7);
    value.rows.edges.reserve_exact(8);
    value.rows.edge_arguments.reserve_exact(9);
    value.bytes.reserve_exact(101);
    let mut counter = Counter::new(limits(None, 10));
    value.charge_retained_heap_storage_v1(&mut counter).unwrap();
    assert_eq!((counter.bytes(), counter.items()), (expected(&value), 10));
    assert!(counter.bytes() > before.len());
    assert_eq!(value.canonical_bytes(), before);
    assert_eq!(*value.digest(), digest);
    assert!(!value.grants_authority());
}

#[test]
fn exact_and_short_receipt_limits_preserve_nonzero_caller_state_on_failure() {
    let value = receipt();
    let heap = expected(&value);
    let mut exact = Counter::new(limits(Some(17 + heap), 13));
    exact.charge(17, 3).unwrap();
    value.charge_retained_heap_storage_v1(&mut exact).unwrap();
    assert_eq!((exact.bytes(), exact.items()), (17 + heap, 13));
    for (limit, error) in [
        (limits(Some(17 + heap - 1), 13), Error::ByteLimit),
        (limits(None, 12), Error::ItemLimit),
    ] {
        let mut counter = Counter::new(limit);
        counter.charge(17, 3).unwrap();
        assert_eq!(
            value.charge_retained_heap_storage_v1(&mut counter),
            Err(error)
        );
        assert_eq!((counter.bytes(), counter.items()), (17, 3));
    }
}

#[test]
fn capacity_products_sums_and_existing_counter_overflows_refuse() {
    assert_eq!(extent(usize::MAX, 2), Err(Error::Arithmetic));
    let mut payloads = [0; 10];
    payloads[0] = usize::MAX;
    payloads[9] = 1;
    assert_eq!(sum(payloads), Err(Error::Arithmetic));
    let value = receipt();
    for (bytes, items) in [(usize::MAX, 0), (0, usize::MAX - 9)] {
        let mut counter = Counter::new(limits(None, usize::MAX));
        counter.charge(bytes, items).unwrap();
        assert_eq!(
            value.charge_retained_heap_storage_v1(&mut counter),
            Err(Error::Arithmetic)
        );
        assert_eq!((counter.bytes(), counter.items()), (bytes, items));
    }
}

#[test]
fn equal_byte_receipts_are_distinct_allocations_and_their_headers_are_external() {
    let first = receipt();
    let second = receipt();
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    let owner = (first, second);
    let header = size_of::<(Receipt, Receipt)>();
    let mut counter = Counter::new(limits(None, 21));
    counter.charge(header, 1).unwrap();
    owner
        .0
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    owner
        .1
        .charge_retained_heap_storage_v1(&mut counter)
        .unwrap();
    assert_eq!(
        counter.bytes(),
        header + expected(&owner.0) + expected(&owner.1)
    );
    assert_eq!(counter.items(), 21);
}
