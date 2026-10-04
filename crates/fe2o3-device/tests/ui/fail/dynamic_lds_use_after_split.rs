use fe2o3_device::DynamicLds;

fn duplicate(view: DynamicLds<'_, u32>) {
    let _parts = view.split_at(1);
    let _ = view.len();
}

fn main() {}
