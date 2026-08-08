use fe2o3_device::{StaticIndex, StaticView, StaticViewMut};

#[unsafe(no_mangle)]
pub fn static_view_read_index_2(view: &StaticView<'_, u32, 4>) -> u32 {
    *view.at_const(StaticIndex::<4, 2>::CHECKED)
}

#[unsafe(no_mangle)]
pub fn static_view_write_index_1(view: &mut StaticViewMut<'_, u32, 4>, value: u32) {
    *view.at_const_mut(StaticIndex::<4, 1>::CHECKED) = value;
}
