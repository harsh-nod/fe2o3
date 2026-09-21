use super::*;

include!("../enrollment_ordering_bodies.rs");

macro_rules! enrollment_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[inline]
fn enrollment_less(left: ContextAllocationKeyV1, right: ContextAllocationKeyV1) -> bool {
    enrollment_less_body!(left, right)
}

// The verified heap candidate stays test-only until it passes the performance gate.
#[cfg(test)]
#[inline]
fn enrollment_swap(values: &mut [Option<ContextAllocationReferenceV1>], left: usize, right: usize) {
    enrollment_swap_body!(values, left, right)
}

#[cfg(test)]
fn enrollment_sorted(values: &[Option<ContextAllocationReferenceV1>]) -> bool {
    enrollment_sorted_body!(enrollment_rust_expr, values, index, [])
}

#[cfg(test)]
fn enrollment_sift(values: &mut [Option<ContextAllocationReferenceV1>], start: usize, end: usize) {
    enrollment_sift_body!(
        enrollment_rust_expr,
        values,
        start,
        end,
        root,
        left,
        child,
        [],
        [],
        [],
        [],
        [],
        []
    )
}

#[cfg(test)]
fn enrollment_heapsort(values: &mut [Option<ContextAllocationReferenceV1>]) {
    enrollment_heapsort_body!(
        enrollment_rust_expr,
        values,
        len,
        start,
        end,
        [],
        [],
        [],
        [],
        [],
        []
    )
}

#[cfg(test)]
fn sort_slots(values: &mut [Option<ContextAllocationReferenceV1>]) {
    enrollment_sort_body!(values)
}

#[inline]
pub(super) fn contains_key(
    entries: &[ContextAllocationEnrollmentV1],
    key: ContextAllocationKeyV1,
) -> bool {
    enrollment_contains_key_body!(
        enrollment_rust_expr,
        entries,
        key,
        lo,
        hi,
        mid,
        found,
        [],
        [],
        [],
        []
    )
}

#[inline]
pub(super) fn contains_slot(values: &[Option<ContextAllocationReferenceV1>], slot: usize) -> bool {
    enrollment_contains_slot_body!(
        enrollment_rust_expr,
        values,
        slot,
        lo,
        hi,
        mid,
        found,
        [],
        [],
        [],
        []
    )
}

#[cfg(test)]
mod tests;
