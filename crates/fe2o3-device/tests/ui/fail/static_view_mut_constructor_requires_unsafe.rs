use fe2o3_device::StaticViewMut;

fn invocation_local_borrow_is_not_global_authority(parent: &mut [u32]) {
    let _ = StaticViewMut::<u32, 4>::from_globally_exclusive_slice(parent, 0);
}

fn main() {}
