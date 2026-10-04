use vstd::prelude::*;
verus! {
pub enum StorageV1 { Host, Device }
pub open spec fn mutated_supported_v1(a: StorageV1, b: StorageV1) -> bool {
    match (a, b) { (StorageV1::Host, StorageV1::Host) => true, _ => false }
}
pub proof fn mutated_h2h_storage_is_rejected_v1()
    ensures !mutated_supported_v1(StorageV1::Host, StorageV1::Host), {}
}
