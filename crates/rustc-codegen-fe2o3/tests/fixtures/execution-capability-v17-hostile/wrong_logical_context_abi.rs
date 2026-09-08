use fe2o3_device::{KernelContext, kernel};

#[kernel(typed)]
pub fn wrong_logical_context_abi(value: u32, context: KernelContext<'_>) {
    let _ = (value, context);
}

