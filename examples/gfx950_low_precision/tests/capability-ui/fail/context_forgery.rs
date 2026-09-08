#![forbid(unsafe_code)]

use core::marker::PhantomData;
use fe2o3_device::kernel;

struct KernelContext<'kernel>(PhantomData<&'kernel mut &'kernel ()>);

#[kernel(typed)]
pub fn forged(context: KernelContext<'_>, value: u32) {
    let _ = (context, value);
}
