// Expected-negative R74 mutation: failed closing currentness still grants success.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_success_v1(completed: nat, count: nat, close_ok: bool) -> bool { completed == count }
pub proof fn mutated_failed_close_v1()
    ensures !mutated_success_v1(65, 65, false),
{}
}
