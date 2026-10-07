// Expected-negative R44 mutation: admitted mutation leaves certificate revision stale.
use vstd::prelude::*;
verus! {
pub open spec fn foundation_revision_v1() -> nat { 44 }
pub open spec fn mutated_certificate_revision_v1() -> nat { 43 }
pub proof fn mutated_mutation_updates_certificate_revision_v1()
    ensures mutated_certificate_revision_v1() == foundation_revision_v1(),
{}
}
