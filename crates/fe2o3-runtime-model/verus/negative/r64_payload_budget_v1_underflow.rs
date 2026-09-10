use vstd::prelude::*;
verus! {
pub open spec fn reserve_v1(used: nat, charge: nat, limit: nat, max: nat) -> Option<nat> {
    if used + charge <= limit && used + charge <= max { Some(used + charge) } else { None }
}
pub open spec fn release_v1(used: nat, charge: nat) -> Option<nat> {
    if charge <= used { Some((used - charge) as nat) } else { Some(used) }
}
pub proof fn mutated_underflow_v1(u: nat, c: nat)
    requires u < c,
    ensures release_v1(u, c) == None,
{}
}
