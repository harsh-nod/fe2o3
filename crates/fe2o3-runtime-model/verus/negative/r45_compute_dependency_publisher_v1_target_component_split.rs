// Expected-negative R45 mutation: a split target event remains exact.
use vstd::prelude::*;
verus! {
pub open spec fn batch_target_v1() -> nat { 41 }
pub open spec fn event_target_v1() -> nat { 42 }
pub proof fn mutated_target_component_split_is_exact_v1()
    ensures batch_target_v1() == event_target_v1(),
{}
}
