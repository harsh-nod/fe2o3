use super::{
    AllocationEntryV1, ContextAllocationKeyV1 as AllocationKeyV1,
    ContextAllocationReferenceV1 as AllocationReferenceV1,
    ContextVersionJournalErrorV1 as ReadErrorV1, ContextVersionJournalV1 as JournalContentsV1,
    ContextWriterKeyV1 as WriterKeyV1, ContextWriterKindV1 as WriterKindV1,
    ContextWriterReferenceV1 as WriterReferenceV1, MemberEntryV1, WriterEntryV1,
};

include!("retained_bodies.rs");

macro_rules! retained_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[inline]
fn retained_indexed_access_v1(journal: &JournalContentsV1) {
    journal.count_indexed_access();
}

#[inline]
pub(super) fn shared_retained_writer_key_v1(left: WriterKeyV1, right: WriterKeyV1) -> bool {
    retained_writer_key_body!(left, right)
}

#[inline]
pub(super) fn shared_retained_allocation_less_v1(
    left: AllocationKeyV1,
    right: AllocationKeyV1,
) -> bool {
    retained_allocation_less_body!(left, right)
}

#[inline]
pub(super) fn shared_retained_allocation_v1(
    journal: &JournalContentsV1,
    reference: AllocationReferenceV1,
) -> Result<AllocationEntryV1, ReadErrorV1> {
    retained_allocation_body!(journal, reference)
}

pub(super) fn shared_retained_header_v1(
    journal: &JournalContentsV1,
    writer: WriterReferenceV1,
    allow_unknown: bool,
) -> Result<(Option<usize>, usize, bool), ReadErrorV1> {
    retained_header_body!(journal, writer, allow_unknown)
}

#[inline]
fn shared_retained_member_v1(
    journal: &JournalContentsV1,
    writer: WriterReferenceV1,
    head: Option<usize>,
    previous: Option<AllocationKeyV1>,
) -> Result<MemberEntryV1, ReadErrorV1> {
    retained_member_body!(journal, writer, head, previous)
}

pub(super) fn shared_retained_chain_v1(
    journal: &JournalContentsV1,
    writer: WriterReferenceV1,
    initial: Option<usize>,
    count: usize,
) -> Result<(), ReadErrorV1> {
    retained_chain_body!(
        retained_rust_expr,
        journal,
        writer,
        initial,
        count,
        head,
        previous,
        index,
        []
    )
}

// Keep the checked Result and its explicit early exit identical in both compilers.
#[allow(clippy::question_mark)]
pub(super) fn shared_retained_unknown_v1(
    journal: &mut JournalContentsV1,
    writer: WriterReferenceV1,
) -> Result<(), ReadErrorV1> {
    retained_unknown_body!(journal, writer)
}
