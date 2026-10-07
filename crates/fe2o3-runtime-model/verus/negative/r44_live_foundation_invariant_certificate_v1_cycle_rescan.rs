// Expected-negative R44 mutation: a valid loan/reclaim cycle adds a global scan.
use vstd::prelude::*;
verus! {
pub open spec fn scans_before_cycle_v1() -> nat { 1 }
pub open spec fn mutated_scans_after_cycle_v1() -> nat { 2 }
pub proof fn mutated_cycles_add_zero_scans_v1()
    ensures mutated_scans_after_cycle_v1() == scans_before_cycle_v1(),
{}
}
