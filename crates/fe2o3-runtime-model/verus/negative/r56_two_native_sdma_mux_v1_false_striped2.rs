// Expected-negative R56 mutation: logical multiplexing is relabeled striped-2.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum LabelV1 { Mux, PhysicalStriped2 }
pub open spec fn mutated_label_admitted_v1(label: LabelV1) -> bool { true }
pub proof fn mutated_false_striped_two_is_rejected_v1()
    ensures !mutated_label_admitted_v1(LabelV1::PhysicalStriped2),
{}
}
fn main() {}
