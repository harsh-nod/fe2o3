use fe2o3_contracts::{AllocationSpecV1, ByteRegionV1};
use fe2o3_device::{DisjointSlice, StaticViewMut};

fn escape(
    parent: &mut DisjointSlice<u32>,
    allocation: AllocationSpecV1,
    region: ByteRegionV1,
) -> StaticViewMut<'static, u32, 4> {
    StaticViewMut::from_disjoint_slice(parent, allocation, region, 0).unwrap()
}

fn main() {}
