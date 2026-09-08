use fe2o3_device::prelude::*;

const PRIVATE_READ_WRITE: UnsafeRawMemoryObligationV1 =
    UnsafeRawMemoryObligationV1::for_view::<PrivateAddressSpace, ExclusiveReadWrite>();
const WORKGROUP_READ_ONLY: UnsafeRawMemoryObligationV1 =
    UnsafeRawMemoryObligationV1::for_view::<WorkgroupAddressSpace, ReadOnly>();

const _: () = assert!(
    PRIVATE_READ_WRITE.contract_version() == UNSAFE_RAW_MEMORY_OBLIGATION_CONTRACT_VERSION_V1
);
const _: () = assert!(matches!(
    PRIVATE_READ_WRITE.address_space(),
    MemoryAddressSpaceV1::Private
));
const _: () = assert!(matches!(
    PRIVATE_READ_WRITE.access(),
    MemoryAccessV1::ReadWrite
));
const _: () = assert!(matches!(
    PRIVATE_READ_WRITE.aliasing(),
    MemoryAliasV1::Exclusive
));
const _: () = assert!(
    PRIVATE_READ_WRITE
        .required()
        .contains(UnsafeRawMemoryObligationSetV1::INITIALIZED_READS)
);
const _: () = assert!(matches!(
    WORKGROUP_READ_ONLY.address_space(),
    MemoryAddressSpaceV1::Workgroup
));

fn main() {}
