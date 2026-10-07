use super::*;

include!("../../enrollment_adaptive_bodies.rs");

#[inline]
fn enrollment_ordered(values: &[Option<ContextAllocationReferenceV1>], reverse: bool) -> bool {
    enrollment_ordered_body!(enrollment_rust_expr, values, reverse, index, previous, [])
}

#[inline]
fn enrollment_reverse_halves(
    left: &mut [Option<ContextAllocationReferenceV1>],
    right: &mut [Option<ContextAllocationReferenceV1>],
) {
    enrollment_reverse_halves_body!(enrollment_rust_expr, left, right, index, len, [])
}

fn enrollment_reverse(values: &mut [Option<ContextAllocationReferenceV1>]) {
    enrollment_reverse_body!(
        enrollment_rust_expr,
        values,
        half,
        left,
        tail,
        _middle,
        right,
        [],
        []
    )
}

fn enrollment_insertion(values: &mut [Option<ContextAllocationReferenceV1>]) {
    enrollment_insertion_body!(
        enrollment_rust_expr,
        values,
        index,
        hole,
        held,
        [],
        [],
        [],
        [],
        [],
        [],
        []
    )
}

#[inline]
fn enrollment_median_of_three(first: usize, middle: usize, last: usize) -> usize {
    enrollment_median_body!(first, middle, last)
}

#[inline]
fn enrollment_pivot(values: &[Option<ContextAllocationReferenceV1>]) -> usize {
    enrollment_pivot_body!(values)
}

fn enrollment_partition(
    values: &mut [Option<ContextAllocationReferenceV1>],
    pivot: usize,
) -> (usize, usize) {
    enrollment_partition_body!(enrollment_rust_expr, values, pivot, less, scan, greater, [])
}

fn enrollment_depth(len: usize) -> u32 {
    enrollment_depth_body!(enrollment_rust_expr, len, remaining, depth, [])
}

fn enrollment_binary_partition(
    values: &mut [Option<ContextAllocationReferenceV1>],
    pivot: usize,
) -> (usize, usize) {
    enrollment_binary_partition_body!(
        enrollment_rust_expr,
        values,
        pivot,
        left,
        right,
        [],
        [],
        [],
        []
    )
}

fn enrollment_lomuto(
    values: &mut [Option<ContextAllocationReferenceV1>],
    pivot: usize,
) -> (usize, usize) {
    enrollment_lomuto_body!(
        enrollment_rust_expr,
        values,
        pivot,
        less,
        scan,
        gap,
        held,
        below,
        [],
        [],
        [],
        [],
        [],
        []
    )
}

#[inline]
fn enrollment_partition_adaptive(
    values: &mut [Option<ContextAllocationReferenceV1>],
    pivot: usize,
) -> (usize, usize) {
    enrollment_partition_adaptive_body!(values, pivot)
}

fn enrollment_introsort(values: &mut [Option<ContextAllocationReferenceV1>], depth: u32) {
    enrollment_introsort_body!(
        enrollment_rust_expr,
        values,
        depth,
        pivot,
        less,
        greater,
        left,
        tail,
        _middle,
        right,
        [],
        [],
        []
    )
}

pub(in super::super) fn adaptive_sort_slots(values: &mut [Option<ContextAllocationReferenceV1>]) {
    enrollment_adaptive_body!(enrollment_rust_expr, values, [])
}

#[cfg(test)]
mod tests;
