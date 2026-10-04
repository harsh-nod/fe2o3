use fe2o3_device::{LaneFragment, MaskedTile1D, WorkgroupCapability};

fn workgroup<'wg, A, B>(value: WorkgroupCapability<'wg, A>) -> WorkgroupCapability<'wg, B> {
    value
}

fn tile<'wg, A, B>(value: MaskedTile1D<'wg, u32, 1, 1, A>) -> MaskedTile1D<'wg, u32, 1, 1, B> {
    value
}

fn fragment<'wg, A, B>(value: LaneFragment<'wg, u32, 1, 1, A>) -> LaneFragment<'wg, u32, 1, 1, B> {
    value
}

fn main() {}
