use fe2o3_device::{DynamicLds, LdsInitialized};

fn alias(view: &mut DynamicLds<'_, u32, LdsInitialized>) {
    let first = view.get_mut(0).unwrap();
    let second = view.get_mut(1).unwrap();
    *first += *second;
}

fn main() {}
