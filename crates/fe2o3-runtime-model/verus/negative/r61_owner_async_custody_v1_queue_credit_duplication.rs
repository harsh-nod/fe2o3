// Expected-negative R61 policy mutation: queue_credit_duplication.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_admits_v1(retained: nat, capacity: nat) -> bool { 0 < capacity && capacity <= 65536 && retained <= capacity }
pub proof fn mutated_capacity_rejects_overflow_v1()
    ensures !mutated_admits_v1(65536, 65536),
{}
}
