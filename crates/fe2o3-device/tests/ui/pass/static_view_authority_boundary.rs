use fe2o3_device::{StaticIndex, StaticView, StaticViewMut};

fn shared(parent: &[u32]) -> StaticView<'_, u32, 4> {
    let view = StaticView::from_shared_slice(parent, 2).unwrap();
    let _: &u32 = view.at_const(StaticIndex::<4, 0>::CHECKED);
    let _: &u32 = view.at_const(StaticIndex::<4, 3>::CHECKED);
    let _: Option<&u32> = view.get(1);
    let _: &[u32; 4] = view.as_array();
    view
}

unsafe fn globally_exclusive(parent: &mut [u32]) -> StaticViewMut<'_, u32, 4> {
    // SAFETY: this example's caller must establish the documented device-wide
    // exclusivity contract; an invocation-local borrow is insufficient.
    let mut view = unsafe { StaticViewMut::from_globally_exclusive_slice(parent, 2).unwrap() };
    let _: &u32 = view.at_const(StaticIndex::<4, 0>::CHECKED);
    let _: &mut u32 = view.at_const_mut(StaticIndex::<4, 3>::CHECKED);
    let _: Option<&u32> = view.get(1);
    let _: Option<&mut u32> = view.get_mut(1);
    let _: &[u32; 4] = view.as_array();
    let _: &mut [u32; 4] = view.as_mut_array();
    view
}

fn main() {}
