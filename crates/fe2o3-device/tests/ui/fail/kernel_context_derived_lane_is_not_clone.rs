use fe2o3_device::{KernelContext, Wave64};

enum KernelA {}

fn duplicate<'kernel>(context: &KernelContext<'kernel, KernelA>) {
    let lane = context.lane::<Wave64>();
    let _ = lane.clone();
}

fn main() {}
