// Expected-negative R45 mutation: equal epoch is not self-dependency.
use vstd::prelude::*;
verus! {
pub open spec fn source_epoch_v1() -> nat { 53 }
pub open spec fn target_epoch_v1() -> nat { 53 }
pub open spec fn classified_self_v1() -> bool { false }
pub proof fn mutated_equal_epoch_is_self_dependency_v1()
    ensures source_epoch_v1() == target_epoch_v1() ==> classified_self_v1(),
{}
}
