// Expected-negative R60 mutation: slot identity ignores its generation.
use vstd::prelude::*;
verus! {
pub struct IdentityV1 { pub slot: nat, pub generation: nat }
pub open spec fn mutated_same_incarnation_v1(a: IdentityV1, b: IdentityV1) -> bool {
    a.slot == b.slot
}
pub proof fn mutated_fresh_generation_prevents_aba_v1(a: IdentityV1, b: IdentityV1)
    requires a.slot == b.slot, b.generation > a.generation,
    ensures !mutated_same_incarnation_v1(a, b),
{}
}
