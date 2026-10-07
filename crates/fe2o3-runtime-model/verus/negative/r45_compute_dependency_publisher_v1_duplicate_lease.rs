// Expected-negative R45 mutation: two reader records share one lease.
use vstd::prelude::*;
verus! {
pub open spec fn first_lease_v1() -> nat { 73 }
pub open spec fn second_lease_v1() -> nat { 73 }
pub proof fn mutated_reader_leases_are_distinct_v1()
    ensures first_lease_v1() != second_lease_v1(),
{}
}
