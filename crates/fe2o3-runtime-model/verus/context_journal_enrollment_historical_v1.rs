// Paired verification harnesses only; production executes no historical journal.
// Existing value views also require Begin's AllocationWriteV1 declaration.
include!("context_version_journal_begin_v1.rs");

mod production {
    include!("context_journal_enrollment_execution_v1.rs");
    include!("context_journal_value_views_v1.rs");
    include!("context_journal_error_views_v1.rs");
    include!("context_journal_content_views_v1.rs");
    include!("context_journal_enrollment_value_views_v1.rs");
    include!("context_journal_enrollment_historical_bodies_v1.rs");
    include!("context_journal_enrollment_historical_witnesses_v1.rs");
    include!("context_journal_enrollment_custody_witness_v1.rs");
}
