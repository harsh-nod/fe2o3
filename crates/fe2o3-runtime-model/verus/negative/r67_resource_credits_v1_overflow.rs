// Expected negative: wrapped arithmetic substitutes for checked admission.
use vstd::prelude::*;
verus! {
pub open spec fn wrapped_admission_v1(used: u64, charge: u64, capacity: u64) -> bool {
    (used as int + charge as int) % 18446744073709551616 <= capacity as int
}
pub proof fn mutated_overflow_v1(used: u64, charge: u64, capacity: u64)
    requires wrapped_admission_v1(used, charge, capacity),
    ensures used as int + charge as int <= capacity as int,
{}
}
