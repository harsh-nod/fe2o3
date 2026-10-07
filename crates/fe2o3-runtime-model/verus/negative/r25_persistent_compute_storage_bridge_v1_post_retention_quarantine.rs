use vstd::prelude::*;
verus! {
pub enum StorageDispositionV1 { Quarantined, Released }
pub open spec fn required_post_retention_disposition_v1() -> StorageDispositionV1 {
    StorageDispositionV1::Quarantined
}
pub open spec fn mutated_post_retention_disposition_v1() -> StorageDispositionV1 {
    StorageDispositionV1::Released
}
pub proof fn mutated_post_retention_fault_quarantines_v1()
    ensures mutated_post_retention_disposition_v1()
        == required_post_retention_disposition_v1(), {}
}
