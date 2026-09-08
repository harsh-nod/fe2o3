// Expected-negative R60 mutation: three-binding N3 enters the ordinary pipeline.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum ClassV1 { Ordinary, ThreeBindingN3 }
pub open spec fn mutated_admitted_v1(class: ClassV1) -> bool {
    class == ClassV1::Ordinary || class == ClassV1::ThreeBindingN3
}
pub proof fn mutated_three_binding_n3_is_excluded_v1()
    ensures !mutated_admitted_v1(ClassV1::ThreeBindingN3),
{}
}
