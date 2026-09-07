// Expected-negative R45 mutation: final dispatch names another target.
use vstd::prelude::*;
verus! {
pub open spec fn bundled_target_v1() -> nat { 47 }
pub open spec fn dispatch_target_v1() -> nat { 48 }
pub proof fn mutated_final_dispatch_target_is_exact_v1()
    ensures bundled_target_v1() == dispatch_target_v1(),
{}
}
