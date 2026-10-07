// Expected-negative R60 mutation: persistent N1 enters the ordinary pipeline.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum ClassV1 { Ordinary, PersistentN1 }
pub open spec fn mutated_admitted_v1(class: ClassV1) -> bool {
    class == ClassV1::Ordinary || class == ClassV1::PersistentN1
}
pub proof fn mutated_persistent_n1_is_excluded_v1()
    ensures !mutated_admitted_v1(ClassV1::PersistentN1),
{}
}
