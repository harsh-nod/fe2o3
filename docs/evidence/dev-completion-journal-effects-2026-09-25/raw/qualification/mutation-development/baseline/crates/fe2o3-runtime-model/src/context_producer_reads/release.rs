use super::*;
use crate::context_version_journal::ContextWriterKindV1 as WriterKindV1;

include!("release_declarations.rs");
include!("release_bodies.rs");

#[inline]
fn producer_release_consumer_same_exec_v1(
    left: ContextWriterKeyV1,
    right: ContextWriterKeyV1,
) -> bool {
    retained_writer_key_body!(left, right)
}

#[inline]
fn producer_release_order_less_exec_v1(
    left: (u64, u64, u64, u64),
    right: (u64, u64, u64, u64),
) -> bool {
    producer_release_order_less_body!(left, right)
}

#[inline]
fn producer_release_header_exec_v1(
    contents: &ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    evidence: ContextWriterKeyV1,
    count: usize,
) -> Result<(), ContextVersionJournalErrorV1> {
    producer_release_header_body!(
        contents,
        consumer,
        evidence,
        count,
        contents.free.capacity()
    )
}

#[inline]
#[allow(clippy::question_mark)]
fn producer_release_item_exec_v1(
    contents: &ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    reference: ContextProducerReadReferenceV1,
    state: ProducerReadReleaseScanV1,
) -> Result<ProducerReadReleaseScanV1, ContextVersionJournalErrorV1> {
    producer_release_item_body!(contents, consumer, reference, state)
}

#[inline]
#[allow(clippy::question_mark)]
pub(super) fn producer_release_preflight_exec_v1(
    contents: &ContextProducerReadJournalV1,
    consumer: ContextWriterKeyV1,
    references: &[ContextProducerReadReferenceV1],
    evidence: ContextWriterKeyV1,
) -> Result<(), ContextVersionJournalErrorV1> {
    producer_release_preflight_body!(
        reader_rust_expr,
        contents,
        consumer,
        references,
        evidence,
        [],
        index,
        state,
        []
    )
}

#[inline]
pub(super) fn producer_release_commit_exec_v1(
    contents: &mut ContextProducerReadJournalV1,
    references: &[ContextProducerReadReferenceV1],
) {
    producer_release_commit_body!(
        reader_rust_expr,
        contents,
        references,
        index,
        reference,
        entry,
        allocation,
        [],
        [],
        [],
        [],
        []
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    fn observed_header(
        contents: &ContextProducerReadJournalV1,
        consumer: ContextWriterKeyV1,
        evidence: ContextWriterKeyV1,
        count: usize,
        observations: &Cell<usize>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_release_header_body!(contents, consumer, evidence, count, {
            observations.set(observations.get() + 1);
            contents.free.capacity()
        })
    }

    #[test]
    fn release_observes_capacity_only_after_earlier_header_checks() {
        let consumer = ContextWriterKeyV1 {
            context_generation: 7,
            local: 20,
            kind: ContextWriterKindV1::Synchronous,
        };
        for fault in 0..6 {
            let mut owner = ContextProducerReadJournalV1::new(7, 1, 1, 2).unwrap();
            let mut evidence = consumer;
            let count = match fault {
                0 => {
                    evidence.local += 1;
                    0
                }
                1 => 0,
                2 => usize::MAX,
                3 => 1,
                _ => {
                    owner.free.clear();
                    1
                }
            };
            if fault == 4 {
                owner.free = Vec::new();
            }
            let observations = Cell::new(0);
            let result = observed_header(&owner, consumer, evidence, count, &observations);
            assert_eq!(observations.get(), usize::from(fault >= 4));
            assert_eq!(
                result,
                match fault {
                    0 => Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch),
                    1 => Err(ContextVersionJournalErrorV1::RosterCapacity),
                    2..=4 => Err(ContextVersionJournalErrorV1::InvalidState),
                    _ => Ok(()),
                }
            );
        }
    }
}
