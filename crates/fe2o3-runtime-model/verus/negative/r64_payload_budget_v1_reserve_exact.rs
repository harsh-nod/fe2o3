use vstd::prelude::*;
verus! {
pub open spec fn reserve_v1(used: nat, charge: nat, limit: nat, max: nat) -> Option<nat> {
    if used + charge <= limit && used + charge <= max { Some(used + charge + 1) } else { None }
}
pub open spec fn release_v1(used: nat, charge: nat) -> Option<nat> {
    if charge <= used { Some((used - charge) as nat) } else { None }
}
pub proof fn mutated_reserve_exact_v1(u: nat, c: nat, l: nat, m: nat, n: nat)
    requires reserve_v1(u, c, l, m) == Some(n),
    ensures n == u + c,
{}
}
