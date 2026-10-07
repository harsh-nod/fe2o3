// Reserving only one free record is insufficient for every member owner.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_record_admission_v1(count: usize, free: usize) -> bool {
    0 < count <= 65536 && free > 0
}
pub proof fn mutated_record_shortage_v1(count: usize, free: usize)
    requires 0 < free < count <= 65536,
    ensures !mutated_record_admission_v1(count, free),
{}
}
