include!("context_version_journal_begin_custody_v1.rs");
#[path = "context_producer_query_historical_bodies_v1.rs"]
mod producer_query_historical_bodies;
use producer_query_historical_bodies::*;

#[path = "context_producer_acquire_historical_bodies_v1.rs"]
mod producer_acquire_historical_bodies;
use producer_acquire_historical_bodies::*;

#[path = "context_producer_release_historical_bodies_v1.rs"]
mod producer_release_historical_bodies;
use producer_release_historical_bodies::*;

#[path = "context_producer_stable_historical_bodies_v1.rs"]
mod producer_stable_historical_bodies;
use producer_stable_historical_bodies::*;

#[path = "context_mixed_acquire_model_v1.rs"]
mod mixed_acquire_model;
use mixed_acquire_model::*;

mod production {
    include!("context_producer_stable_execution_v1.rs");
    include!("context_journal_value_views_v1.rs");
    include!("context_journal_error_views_v1.rs");
    include!("context_journal_content_views_v1.rs");
    include!("context_journal_begin_value_views_v1.rs");
    include!("context_journal_begin_historical_bodies_v1.rs");
    include!("context_reader_owner_views_v1.rs");
    include!("context_reader_guards_correspondence_v1.rs");
    include!("context_reader_guards_witness_v1.rs");
    include!("context_stable_acquire_correspondence_v1.rs");
    include!("context_stable_acquire_witnesses_v1.rs");
    include!("context_stable_release_correspondence_v1.rs");
    include!("context_stable_release_witnesses_v1.rs");
    include!("context_producer_query_correspondence_v1.rs");
    include!("context_producer_query_witnesses_v1.rs");
    include!("context_producer_acquire_correspondence_v1.rs");
    include!("context_producer_acquire_witnesses_v1.rs");
    include!("context_producer_release_correspondence_v1.rs");
    include!("context_producer_release_witnesses_v1.rs");
    include!("context_producer_stable_correspondence_v1.rs");
    include!("context_producer_stable_witnesses_v1.rs");
    include!("context_mixed_acquire_execution_v1.rs");
    include!("context_mixed_acquire_correspondence_v1.rs");
    include!("context_mixed_acquire_witness_v1.rs");
}
