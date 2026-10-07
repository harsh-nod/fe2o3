// Expected negative: the endpoint capacity gate is widened by one.
use vstd::prelude::*;
verus! {
pub open spec fn widened_roster_bound_v1(compute: usize, copies: usize) -> bool {
    compute != 0 && compute <= 16 && copies <= 259
}
pub proof fn mutated_roster_bound_v1(compute: usize, copies: usize)
    requires widened_roster_bound_v1(compute, copies),
    ensures copies <= 258,
{}
}
