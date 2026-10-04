use core::marker::PhantomData;
use fe2o3_device::{KernelContext, LaneFragment, MaskedTile1D, WorkgroupCapability};

fn context() -> KernelContext<'static> {
    KernelContext {
        _kernel: PhantomData,
        _identity: PhantomData,
        _target: PhantomData,
        _launch: PhantomData,
        _not_send_sync: PhantomData,
    }
}

fn workgroup<'wg>(value: WorkgroupCapability<'wg, ()>) -> WorkgroupCapability<'wg, ()> {
    WorkgroupCapability { _size: 1, ..value }
}

fn tile() -> MaskedTile1D<'static, u32, 1, 1, ()> {
    MaskedTile1D {
        values: [1],
        active: [true],
        _contract: PhantomData,
        _not_send_sync: PhantomData,
    }
}

fn fragment() -> LaneFragment<'static, u32, 1, 1, ()> {
    LaneFragment {
        values: [1],
        active: [true],
        _contract: PhantomData,
        _not_send_sync: PhantomData,
    }
}

fn main() {}
