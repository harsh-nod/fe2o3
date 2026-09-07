// Expected-negative R45 mutation: rejection drops one source custody record.
use vstd::prelude::*;
verus! {
pub open spec fn source_custody_before_v1() -> nat { 3 }
pub open spec fn source_custody_after_v1() -> nat { 2 }
pub proof fn mutated_rejection_preserves_exact_custody_v1()
    ensures source_custody_after_v1() == source_custody_before_v1(),
{}
}
