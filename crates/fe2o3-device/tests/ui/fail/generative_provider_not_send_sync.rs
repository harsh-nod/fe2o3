use fe2o3_device::{KernelContext, LaneFragment, MaskedTile1D, WorkgroupCapability};

fn send<T: Send>() {}
fn sync<T: Sync>() {}

fn main() {
    send::<KernelContext<'static>>();
    sync::<KernelContext<'static>>();
    send::<WorkgroupCapability<'static, ()>>();
    sync::<WorkgroupCapability<'static, ()>>();
    send::<MaskedTile1D<'static, u32, 1, 1, ()>>();
    sync::<MaskedTile1D<'static, u32, 1, 1, ()>>();
    send::<LaneFragment<'static, u32, 1, 1, ()>>();
    sync::<LaneFragment<'static, u32, 1, 1, ()>>();
}
