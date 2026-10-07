// Expected-negative R45 mutation: a substituted arena mapping is exact.
use vstd::prelude::*;
verus! {
pub open spec fn source_mapping_v1() -> nat { 71 }
pub open spec fn routed_mapping_v1() -> nat { 72 }
pub proof fn mutated_substituted_owner_has_exact_route_v1()
    ensures source_mapping_v1() == routed_mapping_v1(),
{}
}
