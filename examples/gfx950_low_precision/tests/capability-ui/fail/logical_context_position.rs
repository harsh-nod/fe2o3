#![forbid(unsafe_code)]

use fe2o3_device::{KernelContext, kernel};

#[kernel(typed)]
pub fn misplaced(value: u32, context: KernelContext<'_>) {
    let _ = (value, context);
}
