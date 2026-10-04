use fe2o3_device::{KernelContext, LaneFragment, MaskedTile1D, WorkgroupCapability};

fn context<'short, 'long: 'short>(value: KernelContext<'long>) -> KernelContext<'short> {
    value
}

fn workgroup<'short, 'long: 'short>(
    value: WorkgroupCapability<'long, ()>,
) -> WorkgroupCapability<'short, ()> {
    value
}

fn tile<'short, 'long: 'short>(
    value: MaskedTile1D<'long, u32, 1, 1, ()>,
) -> MaskedTile1D<'short, u32, 1, 1, ()> {
    value
}

fn fragment<'short, 'long: 'short>(
    value: LaneFragment<'long, u32, 1, 1, ()>,
) -> LaneFragment<'short, u32, 1, 1, ()> {
    value
}

fn main() {}
