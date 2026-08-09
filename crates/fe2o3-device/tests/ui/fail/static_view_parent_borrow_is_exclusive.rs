use fe2o3_device::{StaticIndex, StaticViewMut};

unsafe fn alias_parent(parent: &mut [u32]) {
    let view = unsafe { StaticViewMut::<u32, 4>::from_globally_exclusive_slice(parent, 0) }
        .unwrap();
    parent[0] = 7;
    let _ = view.at_const(StaticIndex::<4, 0>::CHECKED);
}

fn main() {}
