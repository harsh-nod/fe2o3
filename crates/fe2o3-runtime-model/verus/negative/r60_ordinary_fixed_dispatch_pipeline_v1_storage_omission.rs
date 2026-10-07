// Expected-negative R60 mutation: exact launch matching omits storage identity.
use vstd::prelude::*;
verus! {
pub struct FingerprintV1 { pub recipe: nat, pub storage: nat }
pub open spec fn mutated_matches_v1(a: FingerprintV1, b: FingerprintV1) -> bool {
    a.recipe == b.recipe
}
pub proof fn mutated_storage_substitution_is_rejected_v1(a: FingerprintV1, b: FingerprintV1)
    requires a.recipe == b.recipe, a.storage != b.storage,
    ensures !mutated_matches_v1(a, b),
{}
}
