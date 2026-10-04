use vstd::prelude::*;
verus! {
pub open spec fn mutated_publication_authority_count_v1() -> nat { 2 }
pub proof fn mutated_publication_preserves_unique_native_location_v1()
    ensures mutated_publication_authority_count_v1() == 1,
{}
}
