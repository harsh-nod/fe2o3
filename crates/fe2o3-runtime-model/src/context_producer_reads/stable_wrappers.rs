use super::*;

include!("stable_wrapper_bodies.rs");

#[inline]
fn producer_stable_output_vacant_exec_v1(output: &[Option<ContextReadLeaseReferenceV1>]) -> bool {
    stable_read_output_vacant_body!(reader_rust_expr, output, index, [])
}

#[inline]
pub(super) fn producer_stable_acquire_header_exec_v1(
    contents: &ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    count: usize,
    output: &[Option<ContextReadLeaseReferenceV1>],
) -> Result<(), ContextVersionJournalErrorV1> {
    producer_stable_acquire_header_body!(
        contents,
        contents.context_generation(),
        consumer,
        count,
        output
    )
}
