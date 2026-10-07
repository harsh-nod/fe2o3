include!("context_version_journal_begin_custody_v1.rs");

mod production {
    include!("context_reader_guards_execution_v1.rs");
    include!("context_journal_value_views_v1.rs");
    include!("context_journal_error_views_v1.rs");
    include!("context_journal_content_views_v1.rs");
    include!("context_journal_begin_value_views_v1.rs");
    include!("context_journal_begin_historical_bodies_v1.rs");
    include!("context_reader_owner_views_v1.rs");
    include!("context_reader_guards_correspondence_v1.rs");
    include!("context_reader_guards_witness_v1.rs");
}
