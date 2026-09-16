#![allow(dead_code)]
use fe2o3_device::{KernelContext, LaneFragment, MaskedTile1D, WorkgroupCapability};

type Parts<const E: usize> =
    for<'wg> fn(&WorkgroupCapability<'wg, ()>, &[u32], usize) -> ([u32; E], [bool; E]);

fn load_parts<'wg, const L: usize, const E: usize>(
    workgroup: &WorkgroupCapability<'wg, ()>,
    input: &[u32],
    base: usize,
) -> ([u32; E], [bool; E]) {
    let tile = MaskedTile1D::<u32, L, E, ()>::load_masked(workgroup, input, base);
    let fragment: LaneFragment<'wg, u32, L, E, ()> = tile.into_fragment();
    fragment.into_parts()
}

fn borrowed_callback<'kernel>(
    context: &mut KernelContext<'kernel>,
    input: &[u32],
    output: &mut [u32],
) -> ([u32; 2], [bool; 2]) {
    context.with_workgroup(|workgroup| {
        let tile = MaskedTile1D::<u32, 3, 2, _>::load_masked(&workgroup, input, 0);
        let ([a, b], [active_a, active_b]) = tile.into_fragment().into_parts();
        if let Some(slot) = output.first_mut() {
            *slot = if active_a { a.wrapping_add(1) } else { 0 };
        }
        ([a, b], [active_a, active_b])
    })
}

fn main() {
    assert_eq!(core::mem::size_of::<KernelContext<'static>>(), 0);
    assert_eq!(core::mem::size_of::<WorkgroupCapability<'static, ()>>(), 16);
    assert_eq!(
        core::mem::size_of::<MaskedTile1D<'static, u32, 256, 125, ()>>(),
        628
    );
    assert_eq!(
        core::mem::size_of::<LaneFragment<'static, u32, 256, 125, ()>>(),
        628
    );
    let _ = std::hint::black_box(load_parts::<1, 1> as Parts<1>);
    let _ = std::hint::black_box(load_parts::<3, 2> as Parts<2>);
    let _ = std::hint::black_box(load_parts::<256, 125> as Parts<125>);
    let _ = std::hint::black_box(borrowed_callback);
}
