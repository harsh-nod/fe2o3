// Expected-negative R44 mutation: final restore skips its full scan.
use vstd::prelude::*;
verus! {
pub open spec fn scans_before_restore_v1() -> nat { 1 }
pub open spec fn mutated_scans_after_restore_v1() -> nat { 1 }
pub proof fn mutated_restore_performs_final_scan_v1()
    ensures mutated_scans_after_restore_v1() == scans_before_restore_v1() + 1,
{}
}
