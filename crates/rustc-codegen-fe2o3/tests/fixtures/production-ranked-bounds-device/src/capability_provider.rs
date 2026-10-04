#![allow(dead_code, unused_imports)]
use fe2o3_device::{DisjointSlice, KernelContext, MaskedTile1D, kernel, thread};

type Context = KernelContext<'static>;
type ContextAlias = Context;
type Empty = [Context; 0];
type Workgroup = fe2o3_device::WorkgroupCapability<'static, ()>;
type Tile = MaskedTile1D<'static, u32, 64, 2, ()>;
type Fragment = fe2o3_device::LaneFragment<'static, u32, 64, 2, ()>;

#[cfg(feature = "provider_context_entry")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context_entry(_context: KernelContext<'_>, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_context_entry_result")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context_entry_result(
    _context: KernelContext<'_>,
    mut output: DisjointSlice<u32>,
) -> fe2o3_device::KernelResult {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
    Ok(())
}

#[cfg(feature = "provider_context_helper_issue")]
#[kernel(typed)]
pub fn provider_context_helper_issue(_context: KernelContext<'_>, _output: DisjointSlice<u32>) {
    let _: Context = KernelContext::__compiler_issue();
}

#[cfg(feature = "provider_context_unregistered_issue")]
#[kernel(typed)]
pub fn provider_context_unregistered_issue(_output: DisjointSlice<u32>) {
    let _: Context = KernelContext::__compiler_issue();
}

#[cfg(feature = "provider_phantom_reference")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_phantom_reference(
    _marker: core::marker::PhantomData<&'static u32>,
    mut output: DisjointSlice<u32>,
) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_phantom")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_phantom(
    _marker: core::marker::PhantomData<Context>,
    mut output: DisjointSlice<u32>,
) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[repr(C)]
pub struct Nested {
    scalar: u32,
    context: (Context,),
}

#[repr(C)]
pub struct Borrowed {
    context: &'static Context,
}

#[cfg(feature = "provider_context")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context(_authority: Context, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_context_alias")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context_alias(_authority: ContextAlias, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_context_nested")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context_nested(_authority: Nested, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_context_reference")]
#[inline(never)]
fn context_reference_result() -> Option<Borrowed> {
    None
}

#[cfg(feature = "provider_context_reference")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context_reference(mut output: DisjointSlice<u32>) {
    let _ = context_reference_result();
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_context_empty_array")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context_empty_array(_authority: Empty, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_workgroup")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_workgroup(_authority: Workgroup, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_tile")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_tile(_authority: Tile, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_fragment")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_fragment(_authority: Fragment, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_context_helper_result")]
#[inline(never)]
fn context_result() -> Option<Context> {
    None
}

#[cfg(feature = "provider_context_helper_result")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_context_helper_result(mut output: DisjointSlice<u32>) {
    let _ = context_result();
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = 7;
    }
}

#[cfg(feature = "provider_tile_chain")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn provider_tile_chain(
    mut context: Context,
    input: &[u32],
    base: u64,
    mut output: DisjointSlice<u32>,
) {
    context.with_workgroup(|workgroup| {
        let tile = MaskedTile1D::<u32, 64, 2, _>::load_masked(&workgroup, input, base as usize);
        let ([a, b], [active_a, active_b]) = tile.into_fragment().into_parts();
        let a = if active_a { a.wrapping_add(1) } else { 0 };
        let b = if active_b { b.wrapping_add(1) } else { 0 };
        if let Some(slot) = output.get_mut(thread::index_1d()) {
            *slot = a.wrapping_add(b);
        }
    });
}
