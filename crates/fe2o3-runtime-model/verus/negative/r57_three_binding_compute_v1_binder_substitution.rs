// Expected-negative R57 mutation: publication accepts a substituted binder.
use vstd::prelude::*;
verus! {
pub struct PlanV1 { pub binder: nat }
pub struct PublishedV1 { pub binder: nat }
pub open spec fn mutated_publish_v1(plan: PlanV1, supplied_binder: nat) -> PublishedV1 {
    PublishedV1 { binder: supplied_binder }
}
pub proof fn mutated_binder_substitution_is_rejected_v1(
    plan: PlanV1, supplied_binder: nat,
)
    requires supplied_binder != plan.binder,
    ensures mutated_publish_v1(plan, supplied_binder).binder == plan.binder,
{}
}
fn main() {}
