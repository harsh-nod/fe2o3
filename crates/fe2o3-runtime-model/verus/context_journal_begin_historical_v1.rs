// Verification-only pairing. Historical guards are not actual reader-wrapper refinement.
include!("context_version_journal_begin_custody_v1.rs");

mod production {
    include!("context_journal_begin_execution_v1.rs");
    include!("context_journal_value_views_v1.rs");
    include!("context_journal_error_views_v1.rs");
    include!("context_journal_content_views_v1.rs");
    include!("context_journal_begin_value_views_v1.rs");
    include!("context_journal_begin_historical_bodies_v1.rs");
    include!("context_journal_begin_guarded_historical_v1.rs");
    include!("context_journal_begin_historical_witnesses_v1.rs");
    include!("context_journal_begin_custody_witness_v1.rs");
}
