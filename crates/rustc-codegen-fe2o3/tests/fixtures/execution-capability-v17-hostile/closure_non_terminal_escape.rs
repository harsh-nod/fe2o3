#![no_std]

use fe2o3_device::{KernelContext, kernel};

#[inline(never)]
fn caller_defined_consumer(operation: impl FnOnce()) {
    operation();
}

#[kernel(typed)]
pub fn closure_non_terminal_escape(_context: KernelContext<'_>, captured: u32) {
    caller_defined_consumer(move || {
        let _ = captured;
    });
}
