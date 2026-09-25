// Actual journal Unknown mutation; physical storage and unwind remain separate.
include!("context_shared_settlement_commit_v1.rs");

mod production {
    use vstd::prelude::*;
    use vstd::prelude::verus as context_journal_declarations_v1;
    include!("../src/context_version_journal/declarations.rs");
    include!("context_journal_value_views_v1.rs");
    include!("context_journal_error_views_v1.rs");
    include!("context_journal_content_views_v1.rs");
    include!("context_journal_view_anchors_v1.rs");
    include!("context_journal_retained_decisions_v1.rs");
    include!("context_journal_retained_bodies_v1.rs");
    include!("context_journal_unknown_decisions_v1.rs");
    include!("context_journal_unknown_body_v1.rs");
}
