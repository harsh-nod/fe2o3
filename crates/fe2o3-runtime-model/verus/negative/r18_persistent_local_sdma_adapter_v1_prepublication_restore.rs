use vstd::prelude::*;
verus! {
pub enum LocationV1 { PreparedRequest, PersistentOwner }
pub open spec fn mutated_restore_v1() -> LocationV1 { LocationV1::PreparedRequest }
pub proof fn mutated_recoverable_failure_restores_native_owner_v1()
    ensures mutated_restore_v1() == LocationV1::PersistentOwner,
{}
}
