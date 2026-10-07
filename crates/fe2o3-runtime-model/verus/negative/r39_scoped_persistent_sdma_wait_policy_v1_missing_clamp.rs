// Expected-negative R39 mutation: the active-spin endpoint exceeds an earlier
// deadline rather than clamping to it.
use vstd::prelude::*;

verus! {
pub open spec fn mutated_spin_until_v1(started_ns: nat) -> nat { started_ns + 50_000 }

pub proof fn mutated_floor_is_clamped_v1(started_ns: nat, deadline_ns: nat)
    requires deadline_ns < started_ns + 50_000,
    ensures mutated_spin_until_v1(started_ns) == deadline_ns,
{}
}
