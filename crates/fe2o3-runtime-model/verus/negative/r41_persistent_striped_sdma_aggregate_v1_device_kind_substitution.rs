// Expected-negative R41 mutation: a coherent host allocation substitutes for device-local storage.
use vstd::prelude::*;
verus! {
pub enum StorageKindV1 { DeviceLocal, HostCoherent }
pub open spec fn mutated_device_storage_kind_v1() -> StorageKindV1 {
    StorageKindV1::HostCoherent
}
pub proof fn mutated_persistent_device_storage_is_local_v1()
    ensures mutated_device_storage_kind_v1() == StorageKindV1::DeviceLocal,
{}
}
