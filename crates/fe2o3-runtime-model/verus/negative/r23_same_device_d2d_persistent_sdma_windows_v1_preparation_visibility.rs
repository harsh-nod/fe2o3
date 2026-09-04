use vstd::prelude::*;
verus! {
pub open spec fn mutated_preparation_publications_v1() -> nat { 1 }
pub proof fn mutated_d2d_preparation_has_no_publication_v1()
    ensures mutated_preparation_publications_v1() == 0, {}
}
