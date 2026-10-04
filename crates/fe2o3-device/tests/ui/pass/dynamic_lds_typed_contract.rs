use core::mem::MaybeUninit;
use fe2o3_device::{DynamicLds, LdsInitialized, WorkgroupLdsScope};

unsafe fn generated_kernel_contract<'workgroup>(
    scope: &'workgroup mut WorkgroupLdsScope<'workgroup>,
    base: *mut u8,
    bytes: usize,
) {
    let mut lds = unsafe { DynamicLds::<u32>::from_raw_parts(scope, base, bytes).unwrap() };
    let _ = (lds.len(), lds.byte_len(), lds.is_empty());
    let _: Option<&MaybeUninit<u32>> = lds.get_uninit(0);
    let _: Option<&mut MaybeUninit<u32>> = lds.get_uninit_mut(0);
    let _: &[MaybeUninit<u32>] = lds.as_uninit_slice();
    let _ = lds.write(0, 7);

    let (left, right) = lds.split_at(1).unwrap();
    let _ = (left.len(), right.len());
}

unsafe fn initialized_contract<'workgroup>(
    scope: &'workgroup mut WorkgroupLdsScope<'workgroup>,
    base: *mut u8,
    bytes: usize,
) {
    let lds = unsafe { DynamicLds::<u64>::from_raw_parts(scope, base, bytes).unwrap() };
    let mut lds: DynamicLds<'_, u64, LdsInitialized> = unsafe { lds.assume_init() };
    let _ = lds.get(0);
    let _ = lds.get_mut(0);
    let _: &[u64] = lds.as_slice();
    let _: &mut [u64] = lds.as_mut_slice();
}

fn supported_composites(_: DynamicLds<'_, [u32; 4]>) {}

fn main() {}
