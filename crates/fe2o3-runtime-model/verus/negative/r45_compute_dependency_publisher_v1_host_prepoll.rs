// Expected-negative R45 mutation: publication host-loads a completion signal.
use vstd::prelude::*;
verus! {
pub open spec fn completion_loads_v1() -> nat { 1 }
pub proof fn mutated_publisher_performs_no_host_prepoll_v1()
    ensures completion_loads_v1() == 0,
{}
}
