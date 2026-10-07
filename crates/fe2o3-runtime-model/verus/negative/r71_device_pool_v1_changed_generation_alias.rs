// Comparing the full incarnation instead of allocation ID hides stale aliases.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_distinct_v1(id: u64, generation: u64, incoming_id: u64, incoming_generation: u64) -> bool {
    id != incoming_id || generation != incoming_generation
}
pub proof fn mutated_changed_generation_alias_v1(id: u64, generation: u64, incoming_id: u64, incoming_generation: u64)
    requires id != 0, generation != 0, incoming_generation != 0,
        id == incoming_id, generation != incoming_generation,
    ensures !mutated_distinct_v1(id, generation, incoming_id, incoming_generation),
{}
}
