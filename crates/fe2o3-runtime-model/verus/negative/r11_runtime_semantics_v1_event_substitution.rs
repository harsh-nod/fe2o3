use vstd::prelude::*;

verus! {

pub proof fn mutated_event_query_retains_source_status_v1(source: int, substituted: int)
    requires source != substituted,
    ensures substituted == source,
{
}

} // verus!
