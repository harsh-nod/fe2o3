use vstd::prelude::*;
verus! {
pub open spec fn restored_storage_v1() -> nat { 41 }
pub open spec fn mutated_device_storage_v1() -> nat { 43 }
pub proof fn mutated_retirement_retains_storage_identity_v1()
    ensures mutated_device_storage_v1() == restored_storage_v1(), {}
}
