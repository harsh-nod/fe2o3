//! Host-rustc code-shape fixture. This is not AMDGPU lowering evidence.

use fe2o3_device::{
    DisjointStaticTileMut, Index1D, StaticIndex, StaticView, StaticViewMut,
};

#[unsafe(no_mangle)]
pub fn host_static_view_read_index_0(view: &StaticView<'_, u32, 4>) -> u32 {
    *view.at_const(StaticIndex::<4, 0>::CHECKED)
}

#[unsafe(no_mangle)]
pub fn host_static_view_read_indices_1_3(view: &StaticView<'_, u32, 4>) -> u32 {
    view.at_const(StaticIndex::<4, 1>::CHECKED)
        .wrapping_add(*view.at_const(StaticIndex::<4, 3>::CHECKED))
}

#[unsafe(no_mangle)]
pub fn host_static_view_write_index_2(view: &mut StaticViewMut<'_, u32, 4>, value: u32) {
    *view.at_const_mut(StaticIndex::<4, 2>::CHECKED) = value;
}

#[unsafe(no_mangle)]
pub fn host_disjoint_tile_read_indices_1_3(
    tile: &DisjointStaticTileMut<'_, u32, Index1D, 4>,
) -> u32 {
    tile.at_const(StaticIndex::<4, 1>::CHECKED)
        .wrapping_add(*tile.at_const(StaticIndex::<4, 3>::CHECKED))
}

#[unsafe(no_mangle)]
pub fn host_disjoint_tile_write_index_2(
    tile: &mut DisjointStaticTileMut<'_, u32, Index1D, 4>,
    value: u32,
) {
    *tile.at_const_mut(StaticIndex::<4, 2>::CHECKED) = value;
}
