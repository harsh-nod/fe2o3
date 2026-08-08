use fe2o3_contracts::{AllocationSpecV1, ByteRegionV1};
use fe2o3_device::{DisjointSlice, StaticIndex, StaticViewMut};

fn alias_parent<'parent>(
    parent: &'parent mut DisjointSlice<u32>,
    allocation: AllocationSpecV1,
    region: ByteRegionV1,
) {
    let view = StaticViewMut::<u32, 4>::from_disjoint_slice(parent, allocation, region, 0)
        .unwrap();
    let _ = unsafe { parent.get_mut_at(0) };
    let _ = view.at_const(StaticIndex::<4, 0>::CHECKED);
}

fn main() {}
