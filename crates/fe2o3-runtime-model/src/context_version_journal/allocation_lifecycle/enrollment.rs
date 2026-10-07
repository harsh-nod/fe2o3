use super::ordering::{
    contains_key, contains_slot, enrollment_less, sort_enrollment_slots as adaptive_sort_slots,
};
use super::{
    AllocationEntryV1, ContextAllocationEnrollmentV1 as EnrollmentV1,
    ContextAllocationReferenceV1 as AllocationReferenceV1,
    ContextVersionJournalErrorV1 as EnrollmentErrorV1,
    ContextVersionJournalV1 as JournalContentsV1,
};

include!("../enrollment_bodies.rs");

macro_rules! enrollment_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[inline]
fn enrollment_indexed_access_v1(journal: &JournalContentsV1) {
    journal.count_indexed_access();
}

#[inline]
fn enrollment_entry_error_exec_v1(context: u64, entry: EnrollmentV1) -> Option<EnrollmentErrorV1> {
    enrollment_entry_error_body!(context, entry)
}

#[inline]
fn enrollment_header_exec_v1(
    context: u64,
    capacity: usize,
    free_len: usize,
    entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>],
) -> Result<(), EnrollmentErrorV1> {
    enrollment_header_body!(
        enrollment_rust_expr,
        context,
        capacity,
        free_len,
        entries,
        output,
        index,
        previous,
        [],
        []
    )
}

#[inline]
fn enrollment_fill_plan_exec_v1(
    journal: &JournalContentsV1,
    entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>],
) {
    enrollment_fill_plan_body!(
        enrollment_rust_expr,
        journal,
        entries,
        output,
        index,
        [],
        []
    )
}

#[inline]
fn enrollment_clear_output_exec_v1(output: &mut [Option<AllocationReferenceV1>]) {
    enrollment_clear_output_body!(enrollment_rust_expr, output, index, [], [])
}

#[inline]
fn enrollment_duplicate_slots_exec_v1(values: &[Option<AllocationReferenceV1>]) -> bool {
    enrollment_duplicate_slots_body!(enrollment_rust_expr, values, index, [], [])
}

#[inline]
fn enrollment_replay_exec_v1(journal: &JournalContentsV1, entries: &[EnrollmentV1]) -> bool {
    enrollment_replay_body!(enrollment_rust_expr, journal, entries, index, [])
}

#[inline]
fn enrollment_selected_vacant_exec_v1(journal: &JournalContentsV1, remaining: usize) -> bool {
    enrollment_selected_vacant_body!(enrollment_rust_expr, journal, remaining, index, [])
}

#[inline]
fn enrollment_retained_clear_exec_v1(
    journal: &JournalContentsV1,
    values: &[Option<AllocationReferenceV1>],
    remaining: usize,
) -> bool {
    enrollment_retained_clear_body!(enrollment_rust_expr, journal, values, remaining, index, [])
}

#[inline]
fn enrollment_commit_journal_exec_v1(
    contents: &mut JournalContentsV1,
    entries: &[EnrollmentV1],
    output: &[Option<AllocationReferenceV1>],
    remaining: usize,
) {
    enrollment_commit_journal_body!(
        enrollment_rust_expr,
        contents,
        entries,
        output,
        remaining,
        index,
        []
    )
}

#[allow(clippy::question_mark)] // Keep the explicit return shared with Verus.
pub(super) fn enrollment_journal_exec_v1(
    contents: &mut JournalContentsV1,
    entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>],
) -> Result<(), EnrollmentErrorV1> {
    enrollment_journal_body!(
        enrollment_rust_expr,
        contents,
        entries,
        output,
        remaining,
        [],
        [],
        [],
        [],
        [],
        []
    )
}
