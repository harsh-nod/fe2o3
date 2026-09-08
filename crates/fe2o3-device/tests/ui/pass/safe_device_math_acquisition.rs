use fe2o3_device::{KernelContext, StrictIeee};

enum Kernel {}

fn device_only(context: KernelContext<'_, Kernel>, value: f32) -> f32 {
    let policy = context.numerical_policy::<StrictIeee>();
    context
        .math()
        .with_numerical_policy(&policy)
        .sqrt_f32(value)
}

fn main() {}
