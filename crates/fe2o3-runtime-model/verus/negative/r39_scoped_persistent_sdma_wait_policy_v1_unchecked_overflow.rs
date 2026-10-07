// Expected-negative R39 mutation: overflowing floor addition wraps rather than
// falling back to the caller's deadline.
use vstd::prelude::*;

verus! {
pub open spec fn mutated_overflow_spin_until_v1() -> nat { 0 }

pub proof fn mutated_overflow_clamps_to_deadline_v1(deadline_ns: nat)
    requires deadline_ns > 0,
    ensures mutated_overflow_spin_until_v1() == deadline_ns,
{}
}
