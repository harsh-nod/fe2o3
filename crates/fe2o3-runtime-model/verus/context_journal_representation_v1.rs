// Actual journal declarations; sequence contents are not physical Vec storage.
include!("context_shared_settlement_commit_v1.rs");

mod production {
    use vstd::prelude::*;
    use vstd::prelude::verus as context_journal_declarations_v1;
    include!("../src/context_version_journal/declarations.rs");
    include!("context_journal_value_views_v1.rs");
    include!("context_journal_error_views_v1.rs");
    include!("context_journal_content_views_v1.rs");
    include!("context_journal_view_anchors_v1.rs");
    use self::ContextWriterKindV1 as WriterKindV1;

    verus! {
    fn production_writer_key_equal(left: ContextWriterKeyV1, right: ContextWriterKeyV1) -> (result: bool)
        ensures result == logical::same_key_v1(writer_key_view(left), writer_key_view(right)),
    { retained_writer_key_body!(left, right) }

    fn production_allocation_less(left: ContextAllocationKeyV1, right: ContextAllocationKeyV1) -> (result: bool)
        ensures result == logical::enrollment_key_less_v1(allocation_key_view(left), allocation_key_view(right)),
    { retained_allocation_less_body!(left, right) }
    }
}
