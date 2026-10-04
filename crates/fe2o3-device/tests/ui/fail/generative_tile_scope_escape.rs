use fe2o3_device::{KernelContext, MaskedTile1D};

fn workgroup(context: &mut KernelContext<'_>) {
    let _ = context.with_workgroup(|workgroup| workgroup);
}

fn tile(context: &mut KernelContext<'_>, input: &[u32]) {
    let _ = context.with_workgroup(|workgroup| {
        MaskedTile1D::<u32, 1, 1, _>::load_masked(&workgroup, input, 0)
    });
}

fn fragment(context: &mut KernelContext<'_>, input: &[u32]) {
    let _ = context.with_workgroup(|workgroup| {
        MaskedTile1D::<u32, 1, 1, _>::load_masked(&workgroup, input, 0).into_fragment()
    });
}

fn main() {}
