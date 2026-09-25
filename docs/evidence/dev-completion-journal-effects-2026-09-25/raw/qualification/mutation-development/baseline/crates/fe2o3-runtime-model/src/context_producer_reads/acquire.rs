use super::*;

include!("acquire_declarations.rs");
include!("acquire_bodies.rs");

#[allow(unused_macros)]
#[macro_use]
mod stable_templates {
    include!("../context_read_leases/acquire_bodies.rs");
}

#[inline]
fn producer_output_vacant_exec_v1(output: &[Option<ContextProducerReadReferenceV1>]) -> bool {
    stable_read_output_vacant_body!(reader_rust_expr, output, index, [])
}

#[inline]
fn producer_read_order_less_exec_v1(left: (u64, u64, u64), right: (u64, u64, u64)) -> bool {
    stable_read_order_less_body!(left, right)
}

#[inline]
fn producer_acquire_header_exec_v1(
    contents: &ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    count: usize,
    output: &[Option<ContextProducerReadReferenceV1>],
) -> Result<(), ContextVersionJournalErrorV1> {
    producer_acquire_header_body!(
        contents,
        contents.context_generation(),
        consumer,
        count,
        output
    )
}

#[inline]
#[allow(clippy::question_mark)]
fn producer_acquire_item_exec_v1(
    contents: &ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    request: ContextProducerReadV1,
    index: usize,
    state: ProducerReadAcquireScanV1,
) -> Result<ProducerReadAcquireScanV1, ContextVersionJournalErrorV1> {
    producer_acquire_item_body!(contents, consumer, request, index, state)
}

#[inline]
#[allow(clippy::question_mark)]
pub(super) fn producer_acquire_preflight_exec_v1(
    contents: &ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    requests: &[ContextProducerReadV1],
    output: &[Option<ContextProducerReadReferenceV1>],
) -> Result<(), ContextVersionJournalErrorV1> {
    producer_acquire_preflight_body!(
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
pub(super) fn producer_acquire_commit_exec_v1(
    contents: &mut ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    requests: &[ContextProducerReadV1],
    output: &mut [Option<ContextProducerReadReferenceV1>],
) {
    producer_acquire_commit_body!(
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
