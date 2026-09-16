use fe2o3_device::{KernelContext, MaskedTile1D};

fn same_scope<'wg, K>(_: MaskedTile1D<'wg, u32, 1, 1, K>, _: MaskedTile1D<'wg, u32, 1, 1, K>) {}

fn mix<'kernel>(
    first: &mut KernelContext<'kernel>,
    second: &mut KernelContext<'kernel>,
    input: &[u32],
) {
    first.with_workgroup(|outer| {
        let tile = MaskedTile1D::<u32, 1, 1, _>::load_masked(&outer, input, 0);
        second.with_workgroup(|inner| {
            let other = MaskedTile1D::<u32, 1, 1, _>::load_masked(&inner, input, 0);
            same_scope(tile, other);
        });
    });
}

fn main() {}
