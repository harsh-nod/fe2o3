#![no_std]

use fe2o3_device::{KernelContext, kernel};

struct Lookalike;

impl Lookalike {
    #[inline(never)]
    fn with_workgroup<Result>(&mut self, operation: impl FnOnce(u32) -> Result) -> Result {
        operation(0)
    }
}

#[kernel(typed)]
pub fn higher_order_lookalike(_context: KernelContext<'_>, _seed: u32) {
    let mut lookalike = Lookalike;
    lookalike.with_workgroup(|_| ());
}
