// Abstract machine-width arithmetic; lease ownership and CAS refinement are separate.
use vstd::prelude::*;
verus! {
pub open spec fn reserve_v1(used: nat, charge: nat, limit: nat, max: nat) -> Option<nat> {
    if used + charge <= limit && used + charge <= max { Some(used + charge) } else { None }
}
pub open spec fn release_v1(used: nat, charge: nat) -> Option<nat> {
    if charge <= used { Some((used - charge) as nat) } else { None }
}
pub proof fn reserve_exact_v1(u: nat, c: nat, l: nat, m: nat, n: nat)
    requires reserve_v1(u, c, l, m) == Some(n),
    ensures n == u + c,
{}
pub proof fn capacity_v1(u: nat, c: nat, l: nat, m: nat)
    requires l < u + c,
    ensures reserve_v1(u, c, l, m) == None,
{}
pub proof fn overflow_v1(u: nat, c: nat, l: nat, m: nat)
    requires m < u + c,
    ensures reserve_v1(u, c, l, m) == None,
{}
pub proof fn release_exact_v1(u: nat, c: nat, n: nat)
    requires release_v1(u, c) == Some(n),
    ensures n + c == u,
{}
pub proof fn underflow_v1(u: nat, c: nat)
    requires u < c,
    ensures release_v1(u, c) == None,
{}
pub proof fn roundtrip_v1(u: nat, c: nat, l: nat, m: nat, n: nat)
    requires reserve_v1(u, c, l, m) == Some(n),
    ensures release_v1(n, c) == Some(u),
{}
pub proof fn boundedness_v1(u: nat, c: nat, l: nat, m: nat, n: nat)
    requires reserve_v1(u, c, l, m) == Some(n),
    ensures n <= l && n <= m,
{}
pub proof fn zero_charge_v1(u: nat, l: nat, m: nat)
    requires u <= l && u <= m,
    ensures reserve_v1(u, 0, l, m) == Some(u) && release_v1(u, 0) == Some(u),
{}
}
