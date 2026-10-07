// Expected-negative R45 mutation: a non-earlier source is acyclic.
use vstd::prelude::*;
verus! {
pub open spec fn source_epoch_v1() -> nat { 19 }
pub open spec fn target_epoch_v1() -> nat { 19 }
pub proof fn mutated_source_cycle_is_earlier_v1()
    ensures source_epoch_v1() < target_epoch_v1(),
{}
}
