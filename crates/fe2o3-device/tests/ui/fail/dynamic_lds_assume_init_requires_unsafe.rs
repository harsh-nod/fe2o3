use fe2o3_device::DynamicLds;

fn initialize(view: DynamicLds<'_, u32>) {
    let _ = view.assume_init();
}

fn main() {}
