// Expected-negative R42 mutation: a Bound event becomes Unbound on a second bind.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum BindingV1 { Unbound, Bound }
pub open spec fn mutated_second_bind_v1() -> BindingV1 { BindingV1::Unbound }
pub proof fn mutated_second_bind_is_rejected_v1()
    ensures mutated_second_bind_v1() == BindingV1::Bound,
{}
}
