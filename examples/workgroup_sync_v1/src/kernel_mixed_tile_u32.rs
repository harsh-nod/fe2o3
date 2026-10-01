//! Mixed masked-tile loading and ordinary per-lane wrapping arithmetic.
use fe2o3_device::{DisjointSlice, KernelContext, MaskedTile1D, kernel, thread};

/// Produces one weighted sum per active logical lane in a 64-lane workgroup.
///
/// Blocked and striped diagnostic distributions have distinct per-lane input
/// mappings; they are not asserted to implement equivalent whole-kernel output.
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn mixed_tile_probe(
    mut ctx: KernelContext<'_>,
    input: &[u32],
    base: usize,
    mut output: DisjointSlice<u32>,
) {
    let ([x, y, z], [mx, my, mz]) = ctx.with_workgroup(move |workgroup| {
        let tile = MaskedTile1D::<u32, 64, 3, _>::load_masked(&workgroup, input, base);
        tile.into_fragment().into_parts()
    });
    let x = if mx {
        x.wrapping_mul(3).wrapping_add(11)
    } else {
        0
    };
    let y = if my {
        y.wrapping_mul(5).wrapping_add(13)
    } else {
        0
    };
    let z = if mz {
        z.wrapping_mul(7).wrapping_add(17)
    } else {
        0
    };
    if mx || my || mz {
        if let Some(slot) = output.get_mut(thread::index_1d()) {
            *slot = x.wrapping_add(y).wrapping_add(z);
        }
    }
}
