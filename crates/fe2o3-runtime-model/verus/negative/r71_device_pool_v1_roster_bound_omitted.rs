// A caller-sized count cap cannot replace the closed 128-record domain.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_roster_size_valid_v1(count: usize, cap: usize) -> bool {
    count <= cap
}
pub proof fn mutated_roster_bound_omitted_v1(count: usize, cap: usize)
    requires 128 < count <= cap,
    ensures !mutated_roster_size_valid_v1(count, cap),
{}
}
