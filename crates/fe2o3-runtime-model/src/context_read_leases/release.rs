use super::*;
use crate::context_version_journal::ContextWriterKindV1 as WriterKindV1;

include!("release_declarations.rs");
include!("release_bodies.rs");

#[allow(unused_macros)]
#[macro_use]
mod equality_templates {
    include!("../context_version_journal/retained_bodies.rs");
}

#[inline]
fn stable_read_consumer_same_exec_v1(left: ContextWriterKeyV1, right: ContextWriterKeyV1) -> bool {
    retained_writer_key_body!(left, right)
}

#[inline]
pub(super) fn stable_read_reference_same_exec_v1(
    left: ContextReadLeaseReferenceV1,
    right: ContextReadLeaseReferenceV1,
) -> bool {
    stable_read_reference_same_body!(left, right)
}

#[inline]
fn stable_release_order_less_exec_v1(
    left: (u64, u64, u64, u64),
    right: (u64, u64, u64, u64),
) -> bool {
    stable_release_order_less_body!(left, right)
}

#[inline]
fn stable_release_header_exec_v1(
    contents: &ContextReadLeasedJournalV1,
    consumer: ContextWriterKeyV1,
    evidence: ContextWriterKeyV1,
    count: usize,
) -> Result<(), ContextVersionJournalErrorV1> {
    stable_release_header_body!(
        contents,
        consumer,
        evidence,
        count,
        contents.free_reads.capacity()
    )
}

#[inline]
#[allow(clippy::question_mark)]
fn stable_release_item_exec_v1(
    contents: &ContextReadLeasedJournalV1,
    consumer: ContextWriterKeyV1,
    reference: ContextReadLeaseReferenceV1,
    state: StableReadReleaseScanV1,
) -> Result<StableReadReleaseScanV1, ContextVersionJournalErrorV1> {
    stable_release_item_body!(contents, consumer, reference, state)
}

#[inline]
#[allow(clippy::question_mark)]
pub(super) fn stable_release_preflight_exec_v1(
    contents: &ContextReadLeasedJournalV1,
    consumer: ContextWriterKeyV1,
    references: &[ContextReadLeaseReferenceV1],
    evidence: ContextWriterKeyV1,
) -> Result<(), ContextVersionJournalErrorV1> {
    stable_release_preflight_body!(
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
pub(super) fn stable_release_commit_exec_v1(
    contents: &mut ContextReadLeasedJournalV1,
    references: &[ContextReadLeaseReferenceV1],
) {
    stable_release_commit_body!(
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
        contents: &ContextReadLeasedJournalV1,
        consumer: ContextWriterKeyV1,
        evidence: ContextWriterKeyV1,
        count: usize,
        observations: &Cell<usize>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        stable_release_header_body!(contents, consumer, evidence, count, {
            observations.set(observations.get() + 1);
            contents.free_reads.capacity()
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
            let mut owner = ContextReadLeasedJournalV1::new(7, 1, 1, 2).unwrap();
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
                    owner.free_reads.clear();
                    1
                }
            };
            if fault == 4 {
                owner.free_reads = Vec::new();
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
