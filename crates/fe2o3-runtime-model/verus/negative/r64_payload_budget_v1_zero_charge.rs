use vstd::prelude::*;
verus! {
pub open spec fn reserve_v1(used: nat, charge: nat, limit: nat, max: nat) -> Option<nat> {
    if used + charge <= limit && used + charge <= max { Some(used + charge) } else { None }
}
pub open spec fn release_v1(used: nat, charge: nat) -> Option<nat> {
    if charge < used { Some((used - charge) as nat) } else { None }
}
pub proof fn mutated_zero_charge_v1(u: nat, l: nat, m: nat)
    requires u <= l && u <= m,
    ensures reserve_v1(u, 0, l, m) == Some(u) && release_v1(u, 0) == Some(u),
{}
}
