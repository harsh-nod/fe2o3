// A correct byte coordinate alone omits the native allocation's owner record.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_charge_v1(cpu: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 2 { cpu } else { 0u64 })
}
pub proof fn mutated_allocation_record_omitted_v1(cpu: u64)
    requires 0 < cpu <= 2147483648, cpu % 4096 == 0,
    ensures mutated_charge_v1(cpu)[18] == 1,
{}
}
