//! Borrowed admission checks for outer owners, using the same execution bodies.

use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn preflight_begin_write_v1(
        &self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.require_unread_writes(members)?;
        self.stable
            .preflight_begin_write(writer, members)
            .map(|_| ())
    }

    pub(crate) fn preflight_acquire_reads_v1(
        &self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextAllocationReadV1],
        output: &[Option<ContextReadLeaseReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        stable_wrappers::producer_stable_acquire_header_exec_v1(
            self,
            consumer,
            requests.len(),
            output,
        )?;
        stable_acquire_preflight_exec_v1(&self.stable, consumer, requests, output)
    }

    pub(crate) fn preflight_acquire_producer_reads_v1(
        &self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextProducerReadV1],
        output: &[Option<ContextProducerReadReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        acquire::producer_acquire_preflight_exec_v1(self, consumer, requests, output)
    }

    #[allow(clippy::question_mark)]
    pub(crate) fn preflight_acquire_mixed_reads_v1(
        &self,
        consumer: ContextWriterKeyV1,
        stable_requests: &[ContextAllocationReadV1],
        stable_output: &[Option<ContextReadLeaseReferenceV1>],
        producer_requests: &[ContextProducerReadV1],
        producer_output: &[Option<ContextProducerReadReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        mixed_acquire_preflight_body!(
            reader_rust_expr,
            self,
            self.context_generation(),
            consumer,
            stable_requests,
            stable_output,
            producer_requests,
            producer_output,
            stable_acquire_preflight_exec_v1,
            acquire::producer_acquire_preflight_exec_v1,
            _value,
            [],
            [],
            [],
            [Ok(())]
        )
    }
}
