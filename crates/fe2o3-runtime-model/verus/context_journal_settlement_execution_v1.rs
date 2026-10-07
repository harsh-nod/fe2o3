// Actual typed settlement bodies; physical storage, derived equality and public wrappers remain separate.
include!("context_settlement_shared_storage_v1.rs");
include!("../src/context_version_journal/retained_bodies.rs");
include!("../src/context_version_journal/settlement_scratch_bodies.rs");
include!("../src/context_version_journal/settlement_commit_body.rs");

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
    use vstd::prelude::verus as settlement_storage_declarations_v1;
    include!("../src/context_version_journal/settlement_storage_declarations.rs");
    include!("context_journal_settlement_decisions_v1.rs");
    include!("context_journal_settlement_bodies_v1.rs");
    include!("context_journal_settlement_historical_v1.rs");
}
