use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct Generic<T> {
    value: T,
}

fn main() {}
