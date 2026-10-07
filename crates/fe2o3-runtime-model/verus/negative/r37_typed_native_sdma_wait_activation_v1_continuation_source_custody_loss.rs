// Expected-negative R37 mutation: a completed window enters Ready after losing
// one source-allocation custody owner.
use vstd::prelude::*;

verus! {
pub struct StateV1 { pub source_custody_count: nat }

pub open spec fn mutated_continuation_v1(initial_source_custody_count: nat) -> StateV1 {
    StateV1 { source_custody_count: (initial_source_custody_count - 1) as nat }
}

pub proof fn mutated_continuation_retains_source_custody_v1(initial_source_custody_count: nat)
    requires initial_source_custody_count > 0,
    ensures
        mutated_continuation_v1(initial_source_custody_count).source_custody_count
            == initial_source_custody_count,
{}
}
