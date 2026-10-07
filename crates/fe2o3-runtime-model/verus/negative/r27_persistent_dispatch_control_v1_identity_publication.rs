use vstd::prelude::*;
verus! {
pub open spec fn retained_identity_v1() -> nat { 41 }
pub open spec fn mutated_publication_identity_v1() -> nat { 42 }
pub proof fn mutated_incompatible_identity_cannot_publish_v1()
    ensures mutated_publication_identity_v1() == retained_identity_v1(), {}
}
