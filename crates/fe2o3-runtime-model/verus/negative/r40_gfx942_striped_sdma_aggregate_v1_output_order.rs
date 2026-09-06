// Expected-negative R40 mutation: successful retirement reverses request order.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_output_v1() -> Seq<nat> { seq![1nat, 0nat] }
pub proof fn mutated_output_keeps_request_order_v1()
    ensures mutated_output_v1()[0] == 0 && mutated_output_v1()[1] == 1,
{}
}
