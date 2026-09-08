// Expected-negative R45 mutation: equal epoch is not self-dependency.
use vstd::prelude::*;
verus! {
pub enum DependencyClassV1 { SelfDependency, CrossQueue }
pub open spec fn source_epoch_v1() -> nat { 53 }
pub open spec fn target_epoch_v1() -> nat { 53 }
pub open spec fn mutated_dependency_class_v1() -> DependencyClassV1 {
    DependencyClassV1::CrossQueue
}
pub proof fn mutated_equal_epoch_is_self_dependency_v1()
    ensures source_epoch_v1() == target_epoch_v1()
        ==> mutated_dependency_class_v1() == DependencyClassV1::SelfDependency,
{}
}
