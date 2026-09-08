use fe2o3_device::{KernelContext, Wave64};

enum KernelA {}

fn duplicate<'kernel>(context: &KernelContext<'kernel, KernelA>) {
    let lane = context.lane::<Wave64>();
    let first = lane;
    let second = lane;
    let _ = (first, second);
}

fn main() {}
