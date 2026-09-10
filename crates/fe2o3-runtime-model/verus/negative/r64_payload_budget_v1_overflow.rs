use vstd::prelude::*;
verus! {
pub open spec fn reserve_v1(used: nat, charge: nat, limit: nat, max: nat) -> Option<nat> {
    let sum = used + charge;
    let wrapped = if max < sum { (sum - max - 1) as nat } else { sum };
    if wrapped <= limit { Some(wrapped) } else { None }
}
pub open spec fn release_v1(used: nat, charge: nat) -> Option<nat> {
    if charge <= used { Some((used - charge) as nat) } else { None }
}
pub proof fn mutated_overflow_v1(u: nat, c: nat, l: nat, m: nat)
    requires u <= m, c <= m, l <= m, m < u + c,
    ensures reserve_v1(u, c, l, m) == None,
{}
}
