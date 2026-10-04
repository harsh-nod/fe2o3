use fe2o3_device::{LaneFragment, MaskedTile1D, SynchronizationEpoch};

fn tile<'wg, K, A: SynchronizationEpoch, B: SynchronizationEpoch>(
    value: MaskedTile1D<'wg, u32, 1, 1, K, A>,
) -> MaskedTile1D<'wg, u32, 1, 1, K, B> {
    value
}

fn fragment<'wg, K, A: SynchronizationEpoch, B: SynchronizationEpoch>(
    value: LaneFragment<'wg, u32, 1, 1, K, A>,
) -> LaneFragment<'wg, u32, 1, 1, K, B> {
    value
}

fn main() {}
