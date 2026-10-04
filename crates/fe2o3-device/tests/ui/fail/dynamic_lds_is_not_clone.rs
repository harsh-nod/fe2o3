use fe2o3_device::DynamicLds;

fn require_clone<T: Clone>(_: &T) {}

fn reject(view: &DynamicLds<'_, u32>) {
    require_clone(view);
}

fn main() {}
