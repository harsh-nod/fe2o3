use fe2o3_device::KernelContext;

fn copy_after_move(context: &KernelContext<'_>) {
    let private = context.private_memory::<u32, 4>();
    let moved = private;
    let _ = private.load(0);
    drop(moved);
}

