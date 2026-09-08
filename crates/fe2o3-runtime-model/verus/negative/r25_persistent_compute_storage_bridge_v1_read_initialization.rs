use vstd::prelude::*;
verus! {
// Mutation: read admission ignores the allocation's initialization coordinate.
pub open spec fn mutated_read_admitted_v1(_initialized: bool) -> bool { true }
pub proof fn mutated_reads_require_initialization_v1()
    ensures !mutated_read_admitted_v1(false), {}
}
