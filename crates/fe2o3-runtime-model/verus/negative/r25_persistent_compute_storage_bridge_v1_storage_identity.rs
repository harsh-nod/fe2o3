use vstd::prelude::*;
verus! {
pub open spec fn source_storage_v1() -> nat { 41 }
pub open spec fn mutated_prepared_storage_v1() -> nat { 42 }
pub proof fn mutated_storage_identity_is_retained_v1()
    ensures mutated_prepared_storage_v1() == source_storage_v1(), {}
}
