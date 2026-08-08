use fe2o3_contracts::{AllocationSpecV1, ByteRegionV1};
use fe2o3_device::{DisjointSlice, Index1D, StaticIndex, StaticView, StaticViewMut};

fn shared<'parent>(
    parent: &'parent DisjointSlice<u32, Index1D>,
    allocation: AllocationSpecV1,
    region: ByteRegionV1,
) -> StaticView<'parent, u32, 4, Index1D> {
    let view = StaticView::from_disjoint_slice(parent, allocation, region, 2).unwrap();
    let _: &u32 = view.at_const(StaticIndex::<4, 0>::CHECKED);
    let _: &u32 = view.at_const(StaticIndex::<4, 3>::CHECKED);
    let _: Option<&u32> = view.get(1);
    let _: &[u32; 4] = view.as_array();
    let _ = view.contract();
    view
}

fn exclusive<'parent>(
    parent: &'parent mut DisjointSlice<u32, Index1D>,
    allocation: AllocationSpecV1,
    region: ByteRegionV1,
) -> StaticViewMut<'parent, u32, 4, Index1D> {
    let mut view = StaticViewMut::from_disjoint_slice(parent, allocation, region, 2).unwrap();
    let _: &u32 = view.at_const(StaticIndex::<4, 0>::CHECKED);
    let _: &mut u32 = view.at_const_mut(StaticIndex::<4, 3>::CHECKED);
    let _: Option<&u32> = view.get(1);
    let _: Option<&mut u32> = view.get_mut(1);
    let _: &[u32; 4] = view.as_array();
    let _: &mut [u32; 4] = view.as_mut_array();
    let _ = view.contract();
    view
}

fn main() {}
