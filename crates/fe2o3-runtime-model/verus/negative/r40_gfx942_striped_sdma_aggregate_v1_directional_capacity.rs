// Expected-negative R40 mutation: combined per-engine load omits its reserved directional queue.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_combined_engine_load_v1(striped_per_engine: nat) -> nat { striped_per_engine }
pub proof fn mutated_combined_load_includes_directional_v1(striped_per_engine: nat)
    ensures mutated_combined_engine_load_v1(striped_per_engine) == striped_per_engine + 1,
{}
}
