use vstd::prelude::*;
verus! {
pub open spec fn reserve_v1(used: nat, charge: nat, limit: nat, max: nat) -> Option<nat> {
    if used + charge <= limit && used + charge <= max { Some(used + charge) } else { None }
}
pub open spec fn release_v1(used: nat, charge: nat) -> Option<nat> {
    if charge <= used { Some(used) } else { None }
}
pub proof fn mutated_release_exact_v1(u: nat, c: nat, n: nat)
    requires release_v1(u, c) == Some(n),
    ensures n + c == u,
{}
}
