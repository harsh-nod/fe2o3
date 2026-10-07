// Conditional invariant preservation; raw forwarding has no issuance premise.
use super::*;

verus! {

pub proof fn owner_enrollment_preserves_producer_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1>)
    requires producer_invariant_v1(before), producer_storage_frame_v1(before, after),
        enrollment_execution_relation_v1(before.stable.journal, after.stable.journal, entries, original, output, result),
    ensures producer_invariant_v1(after),
        forall|request: ProducerReadV1| producer_status_v1(before.stable.journal, request).is_some()
            ==> #[trigger] producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request),
{
    if let Ok(value) = result {
        assert(value == ());
        enrollment_success_admission_v1(before.stable.journal, entries, original);
        let remaining = (before.stable.journal.allocation_free@.len() - entries.len()) as usize;
        assert(enrollment_final_relation_v1(before.stable, after.stable, entries, output, remaining));
        enrollment_pending_custody_preserves_v1(before.stable, after.stable, entries, output, remaining);
        enrollment_prefix_reader_frame_v1(before.stable, entries, output, remaining, entries.len());
        reader_allocation_frame_preserves_v1(before.stable, after.stable);
        assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some()
            implies producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
            let entry = before.reservations@[s].unwrap();
            assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
            reveal(producer_entry_valid_v1);
            producer_status_enrollment_frame_v1(before.stable.journal, after.stable.journal, entry.request);
        }
        assert forall|request: ProducerReadV1| producer_status_v1(before.stable.journal, request).is_some()
            implies #[trigger] producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request) by {
            producer_status_enrollment_frame_v1(before.stable.journal, after.stable.journal, request);
        }
    }
}

}
