// Expected-negative R74 mutation: a nonempty prefix is mistaken for completion.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_success_v1(completed: nat, count: nat) -> bool { completed > 0 }
pub proof fn mutated_early_success_v1()
    ensures !mutated_success_v1(1, 4096),
{}
}
