use super::*;

include!("acquire_declarations.rs");
include!("acquire_bodies.rs");

#[inline]
fn stable_read_output_vacant_exec_v1(output: &[Option<ContextReadLeaseReferenceV1>]) -> bool {
    stable_read_output_vacant_body!(reader_rust_expr, output, index, [])
}

#[inline]
fn stable_read_order_less_exec_v1(left: (u64, u64, u64), right: (u64, u64, u64)) -> bool {
    stable_read_order_less_body!(left, right)
}

#[inline]
fn stable_acquire_header_exec_v1(
    contents: &ContextReadLeasedJournalV1,
    consumer: ContextWriterKeyV1,
    count: usize,
    output: &[Option<ContextReadLeaseReferenceV1>],
) -> Result<(), ContextVersionJournalErrorV1> {
    stable_acquire_header_body!(contents, consumer, count, output)
}

#[inline]
#[allow(clippy::question_mark)]
fn stable_acquire_item_exec_v1(
    contents: &ContextReadLeasedJournalV1,
    request: ContextAllocationReadV1,
    index: usize,
    state: StableReadAcquireScanV1,
) -> Result<StableReadAcquireScanV1, ContextVersionJournalErrorV1> {
    stable_acquire_item_body!(contents, request, index, state)
}

#[inline]
#[allow(clippy::question_mark)]
pub(super) fn stable_acquire_preflight_exec_v1(
    contents: &ContextReadLeasedJournalV1,
    consumer: ContextWriterKeyV1,
    requests: &[ContextAllocationReadV1],
    output: &[Option<ContextReadLeaseReferenceV1>],
) -> Result<(), ContextVersionJournalErrorV1> {
    stable_acquire_preflight_body!(
        reader_rust_expr,
        contents,
        consumer,
        requests,
        output,
        index,
        state,
        []
    )
}

#[inline]
pub(super) fn stable_acquire_commit_exec_v1(
    contents: &mut ContextReadLeasedJournalV1,
    consumer: ContextWriterKeyV1,
    requests: &[ContextAllocationReadV1],
    output: &mut [Option<ContextReadLeaseReferenceV1>],
) {
    stable_acquire_commit_body!(
        reader_rust_expr,
        contents,
        consumer,
        requests,
        output,
        index,
        [],
        [],
        [],
        [],
        []
    )
}
