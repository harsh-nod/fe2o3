use fe2o3_device::{KernelContext, LaneFragment, MaskedTile1D, WorkgroupCapability};

fn copy<T: Copy>() {}
fn clone<T: Clone>() {}

fn main() {
    copy::<KernelContext<'static>>();
    clone::<KernelContext<'static>>();
    copy::<WorkgroupCapability<'static, ()>>();
    clone::<WorkgroupCapability<'static, ()>>();
    copy::<MaskedTile1D<'static, u32, 1, 1, ()>>();
    clone::<MaskedTile1D<'static, u32, 1, 1, ()>>();
    copy::<LaneFragment<'static, u32, 1, 1, ()>>();
    clone::<LaneFragment<'static, u32, 1, 1, ()>>();
}
