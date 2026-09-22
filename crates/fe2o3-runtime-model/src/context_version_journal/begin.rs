use super::retained::{
    shared_retained_allocation_less_v1, shared_retained_allocation_v1,
    shared_retained_writer_key_v1,
};
use super::{
    BeginMemberPlanV1, ContextAllocationWriteV1 as AllocationWriteV1,
    ContextVersionJournalErrorV1 as ReadErrorV1, ContextVersionJournalV1 as JournalContentsV1,
    ContextWriterKeyV1 as WriterKeyV1, ContextWriterReferenceV1 as WriterReferenceV1,
    MemberEntryV1, WriterEntryV1,
};

include!("begin_bodies.rs");

macro_rules! begin_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[inline]
fn begin_indexed_access_v1(journal: &JournalContentsV1) {
    journal.count_indexed_access();
}

#[inline]
fn begin_reserved_exec_v1(
    journal: &JournalContentsV1,
    writer: WriterReferenceV1,
) -> Result<WriterKeyV1, ReadErrorV1> {
    begin_reserved_body!(journal, writer)
}

#[inline]
fn begin_canonical_exec_v1(roster: &[AllocationWriteV1]) -> Result<(), ReadErrorV1> {
    begin_canonical_body!(begin_rust_expr, roster, index, [])
}

#[inline]
#[allow(clippy::question_mark)]
fn begin_destinations_exec_v1(
    journal: &JournalContentsV1,
    roster: &[AllocationWriteV1],
) -> Result<(), ReadErrorV1> {
    begin_destinations_body!(begin_rust_expr, journal, roster, index, [])
}

#[inline(always)]
fn begin_slots_exec_v1(journal: &JournalContentsV1, count: usize) -> Result<(), ReadErrorV1> {
    begin_slots_body!(begin_rust_expr, journal, count, index, [])
}

#[inline]
#[allow(clippy::question_mark)]
pub(super) fn begin_preflight_exec_v1(
    journal: &JournalContentsV1,
    writer: WriterReferenceV1,
    roster: &[AllocationWriteV1],
) -> Result<usize, ReadErrorV1> {
    begin_preflight_body!(journal, writer, roster)
}

#[inline]
fn begin_stage_exec_v1(journal: &mut JournalContentsV1, roster: &[AllocationWriteV1]) {
    begin_stage_body!(begin_rust_expr, journal, roster, index, [], [])
}

#[inline]
fn begin_commit_exec_v1(
    journal: &mut JournalContentsV1,
    writer: WriterReferenceV1,
    roster: &[AllocationWriteV1],
    reserved_count: usize,
) {
    begin_commit_body!(
        begin_rust_expr,
        journal,
        writer,
        roster,
        reserved_count,
        index,
        count,
        head,
        [],
        []
    )
}

#[allow(clippy::question_mark)] // Keep the same explicit early exit in both compilers.
pub(super) fn begin_exec_v1(
    journal: &mut JournalContentsV1,
    writer: WriterReferenceV1,
    roster: &[AllocationWriteV1],
) -> Result<(), ReadErrorV1> {
    begin_execution_body!(
        begin_rust_expr,
        journal,
        writer,
        roster,
        reserved_count,
        [],
        []
    )
}
