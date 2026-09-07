// Expected-negative R41 mutation: preparation recovery reverses two request identities.
use vstd::prelude::*;
verus! {
pub open spec fn original_v1() -> Seq<nat> { seq![1nat, 2nat] }
pub open spec fn mutated_recovered_v1() -> Seq<nat> { seq![2nat, 1nat] }
pub proof fn mutated_preparation_recovery_is_ordered_v1()
    ensures mutated_recovered_v1() == original_v1(),
{}
}
